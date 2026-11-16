//! Entwurf (Zweig mac-helfer): die Leitung für ein eigenes Mac-Hilfsprogramm.
//!
//! Die Sperre mit dem Rechner ist eine Einstellung: Wer sie einschaltet,
//! will, dass sich das Dashboard sperrt, sobald der Rechner gesperrt wird,
//! und dass danach eine neue Anmeldung nötig ist. Von der Sperre erfährt der
//! Server nicht selbst: Ein kleines Mac-Programm meldet sie, getrennt vom Kern.
//!
//! Aufbau in zwei Teilen:
//!
//! - **Endpunkte** (`router`): gewöhnliche HTTP-Wege, `POST /sperre`,
//!   `POST /entsperrt`, `POST /lebenszeichen`, `POST /schluessel`,
//!   `GET /stand`. Kein eigenes Leitungsformat.
//! - **Leitung**: worüber die Endpunkte erreichbar sind. Heute ein lokaler
//!   Socket `helfer` im Schlüsselordner, im Klartext; das Betriebssystem
//!   nennt bei jeder Verbindung die Benutzernummer der Gegenseite, und nur
//!   die eigene kommt bis zu den Endpunkten. Später sind andere Leitungen
//!   möglich (etwa TLS 1.3 mit Kanalbindung), ohne die Endpunkte zu ändern.
//!
//! Die Endpunkte gibt es nur auf solchen Leitungen, nie am Web-Port: Jede
//! Leitung liefert einen belegten [`Aufrufer`], und ohne ihn läuft kein Weg.
//!
//! Der Helfer ruft immer an; die Antwort kommt auf derselben Verbindung
//! zurück. Darum genügt ein Socket.
//!
//! Schlüssel: Mit der Einstellung kommt der Schlüssel der Aufgaben-Links
//! vom Helfer statt aus der Datei. In dieser ersten Fassung schickt der
//! Helfer ihn im Klartext über den lokalen Socket; geschützt ist das nur so
//! weit wie der Socket selbst (eigenes Konto, eigener Rechner). Der Server
//! hält ihn nur im Arbeitsspeicher. Nach einem Neustart fehlt er; das sagt
//! die Antwort auf das Lebenszeichen, und der Helfer übergibt ihn erneut.
//!
//! Im Zweifel gesperrt: Meldet sich der Helfer nicht mehr, gilt der Rechner
//! als gesperrt. Der Helfer kann nur sperren und sein Dasein melden; eine
//! Anmeldung ersetzt er nie.

use std::time::{Duration, Instant};

/// Meldet sich der Helfer so lange nicht, gilt der Rechner als gesperrt.
pub const ZEICHEN_FRIST: Duration = Duration::from_secs(20);

/// Name des Sockets im Schlüsselordner.
pub const HELFER_DATEI: &str = "helfer";

/// Der Stand der Sperre mit dem Rechner. Reine Logik mit eingespeisten
/// Zeiten: `jetzt` ist die stetige Uhr (für das Lebenszeichen), `wand` die
/// Wanduhr in Sekunden (für den Vergleich mit dem Zeitpunkt einer Anmeldung).
#[derive(Debug, Default)]
pub struct SperrStand {
    eingeschaltet: bool,
    /// Der Rechner ist laut Helfer gerade gesperrt.
    gesperrt: bool,
    /// Jede Anmeldung bis zu diesem Zeitpunkt gilt nicht mehr.
    grenze: i64,
    letztes_zeichen: Option<Instant>,
}

impl SperrStand {
    /// Schaltet die Sperre mit dem Rechner ein. Alles, was bis jetzt
    /// angemeldet war, gilt nicht mehr; bis zum ersten Lebenszeichen ist gesperrt.
    pub fn einschalten(&mut self, wand: i64) {
        self.eingeschaltet = true;
        self.grenze = self.grenze.max(wand);
    }

    pub fn eingeschaltet(&self) -> bool {
        self.eingeschaltet
    }

    /// Der Helfer meldet: Der Rechner ist jetzt gesperrt.
    pub fn sperre(&mut self, jetzt: Instant, wand: i64) {
        self.gesperrt = true;
        self.grenze = self.grenze.max(wand);
        self.letztes_zeichen = Some(jetzt);
    }

    /// Der Helfer meldet: Der Rechner ist wieder entsperrt. Anmeldungen bis
    /// zu diesem Zeitpunkt gelten nicht; danach ist eine neue nötig.
    pub fn entsperrt(&mut self, jetzt: Instant, wand: i64) {
        self.gesperrt = false;
        self.grenze = self.grenze.max(wand);
        self.letztes_zeichen = Some(jetzt);
    }

    /// Der Helfer meldet: Ich bin noch da.
    pub fn zeichen(&mut self, jetzt: Instant) {
        self.letztes_zeichen = Some(jetzt);
    }

    /// Ob der Helfer sich rechtzeitig gemeldet hat.
    fn helfer_lebt(&self, jetzt: Instant) -> bool {
        self.letztes_zeichen.is_some_and(|z| jetzt.saturating_duration_since(z) < ZEICHEN_FRIST)
    }

    /// Ist gerade gesperrt? Ausgeschaltet: nie. Sonst: wenn der Helfer die
    /// Sperre gemeldet hat oder sich nicht mehr meldet.
    pub fn ist_gesperrt(&self, jetzt: Instant) -> bool {
        self.eingeschaltet && (self.gesperrt || !self.helfer_lebt(jetzt))
    }

    /// Gilt eine Anmeldung vom Zeitpunkt `anmeldung` noch? Solange gesperrt
    /// ist, gilt keine, und die Grenze rückt mit: Auch eine Anmeldung, die
    /// währenddessen entsteht, gilt danach nicht.
    pub fn gilt(&mut self, anmeldung: i64, jetzt: Instant, wand: i64) -> bool {
        if !self.eingeschaltet {
            return true;
        }
        if self.ist_gesperrt(jetzt) {
            self.grenze = self.grenze.max(wand);
            return false;
        }
        anmeldung > self.grenze
    }
}

/// Was die Übergabe eines Schlüssels ergibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Uebernahme {
    /// Bisher lag keiner vor: Dieser gilt jetzt.
    Gesetzt,
    /// Derselbe wie schon vorhanden (etwa nach einem Neustart des Helfers).
    Gleich,
    /// Ein anderer als der vorhandene: abgewiesen. Ein stiller Tausch machte
    /// alles unlesbar, was mit dem bisherigen verschlossen wurde.
    Anders,
}

/// Übernimmt einen Schlüssel in die Ablage. Reine Logik über die Ablage:
/// der erste gilt, derselbe noch einmal ist in Ordnung, ein anderer wird abgewiesen.
pub fn uebernehmen(ablage: &mut Option<zeroize::Zeroizing<[u8; 32]>>, neu: zeroize::Zeroizing<[u8; 32]>) -> Uebernahme {
    match ablage {
        None => {
            *ablage = Some(neu);
            Uebernahme::Gesetzt
        }
        Some(alt) if **alt == *neu => Uebernahme::Gleich,
        Some(_) => Uebernahme::Anders,
    }
}

/// Liest einen übergebenen Schlüssel: Base64url, genau 32 Bytes.
pub fn schluessel_lesen(text: &str) -> Option<zeroize::Zeroizing<[u8; 32]>> {
    let roh = zeroize::Zeroizing::new(crate::passkey::b64_lesen(text)?);
    let feld: [u8; 32] = roh.as_slice().try_into().ok()?;
    Some(zeroize::Zeroizing::new(feld))
}

/// Wanduhr in Sekunden seit 1970.
pub fn wand_jetzt() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(unix)]
pub use leitung::{Aufrufer, Helfer, HelferZustand, Lebenszeichen, router, starten};

/// Die Endpunkte und die Leitung über den lokalen Socket.
#[cfg(unix)]
mod leitung {
    use std::future::Future;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    use axum::extract::{ConnectInfo, State};
    use axum::http::StatusCode;
    use axum::routing::{get, post};
    use axum::serve::IncomingStream;
    use axum::{Json, Router};
    use tokio::net::{UnixListener, UnixStream};
    use tracing::{info, warn};

    use super::SperrStand;

    /// Der belegte Aufrufer einer Leitung. Beim lokalen Socket: Das
    /// Betriebssystem hat seine Benutzernummer bezeugt, und sie ist die
    /// erwartete. Das Feld ist privat; den Wert erzeugt nur eine Leitung.
    #[derive(Debug, Clone)]
    pub struct Aufrufer {
        uid: u32,
    }

    impl Aufrufer {
        pub fn uid(&self) -> u32 {
            self.uid
        }
    }

    /// Was die Endpunkte brauchen.
    #[derive(Clone)]
    pub struct HelferZustand {
        pub sperre: Arc<Mutex<SperrStand>>,
        /// Die Ablage für den Schlüssel der Links; `None`, wenn er nicht vom Helfer kommt.
        pub schluessel: Option<crate::speicher::verschluesselung::Uebergabe>,
    }

    /// Antwort auf `POST /lebenszeichen`.
    #[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    pub struct Lebenszeichen {
        /// Der Server erwartet einen Schlüssel vom Helfer und hat keinen (etwa nach einem Neustart).
        pub schluessel_fehlt: bool,
    }

    /// Körper von `POST /schluessel`.
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SchluesselUebergabe {
        /// Der Schlüssel der Aufgaben-Links: 32 Bytes, Base64url.
        links: String,
    }

    /// Antwort auf `GET /stand`.
    #[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    pub struct Stand {
        pub eingeschaltet: bool,
        pub gesperrt: bool,
    }

    /// Die Endpunkte des Helfers. Jeder verlangt den belegten Aufrufer der
    /// Leitung; ohne ihn (etwa an einem Lauscher ohne Prüfung) antwortet axum mit 500.
    pub fn router(zustand: HelferZustand) -> Router {
        Router::new()
            .route("/sperre", post(sperre))
            .route("/entsperrt", post(entsperrt))
            .route("/lebenszeichen", post(lebenszeichen))
            .route("/schluessel", post(schluessel))
            .route("/stand", get(stand))
            .with_state(zustand)
    }

    fn stand_von(z: &HelferZustand) -> std::sync::MutexGuard<'_, SperrStand> {
        z.sperre.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    async fn sperre(State(z): State<HelferZustand>, ConnectInfo(_wer): ConnectInfo<Aufrufer>) -> StatusCode {
        stand_von(&z).sperre(Instant::now(), super::wand_jetzt());
        info!("Helfer: Der Rechner ist gesperrt.");
        StatusCode::NO_CONTENT
    }

    async fn entsperrt(State(z): State<HelferZustand>, ConnectInfo(_wer): ConnectInfo<Aufrufer>) -> StatusCode {
        stand_von(&z).entsperrt(Instant::now(), super::wand_jetzt());
        info!("Helfer: Der Rechner ist entsperrt.");
        StatusCode::NO_CONTENT
    }

    async fn lebenszeichen(State(z): State<HelferZustand>, ConnectInfo(_wer): ConnectInfo<Aufrufer>) -> Json<Lebenszeichen> {
        stand_von(&z).zeichen(Instant::now());
        let schluessel_fehlt = z.schluessel.as_ref().is_some_and(|u| u.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_none());
        Json(Lebenszeichen { schluessel_fehlt })
    }

    /// `POST /schluessel`: Der Helfer übergibt den Schlüssel der Links. Der
    /// Körper wird nie protokolliert. Der erste gilt; derselbe noch einmal
    /// ist in Ordnung; ein anderer wird mit 409 abgewiesen.
    async fn schluessel(State(z): State<HelferZustand>, ConnectInfo(_wer): ConnectInfo<Aufrufer>, koerper: axum::body::Bytes) -> (StatusCode, &'static str) {
        let Some(uebergabe) = &z.schluessel else {
            return (StatusCode::CONFLICT, "Der Server nimmt keinen Schlüssel vom Helfer; die Einstellung dafür ist aus.");
        };
        let neu = serde_json::from_slice::<SchluesselUebergabe>(&koerper).ok().and_then(|k| super::schluessel_lesen(&k.links));
        let Some(neu) = neu else {
            return (StatusCode::UNPROCESSABLE_ENTITY, "Erwartet wird {\"links\": \"<32 Bytes, Base64url>\"}.");
        };
        let ergebnis = super::uebernehmen(&mut uebergabe.lock().unwrap_or_else(std::sync::PoisonError::into_inner), neu);
        match ergebnis {
            super::Uebernahme::Gesetzt => {
                info!("Helfer: Schlüssel der Aufgaben-Links übergeben.");
                (StatusCode::NO_CONTENT, "")
            }
            super::Uebernahme::Gleich => (StatusCode::NO_CONTENT, ""),
            super::Uebernahme::Anders => {
                warn!("Helfer: Ein anderer Schlüssel als der vorhandene wurde abgewiesen.");
                (StatusCode::CONFLICT, "Der Server hat schon einen anderen Schlüssel; ein Tausch im laufenden Betrieb ist nicht möglich.")
            }
        }
    }

    async fn stand(State(z): State<HelferZustand>, ConnectInfo(_wer): ConnectInfo<Aufrufer>) -> Json<Stand> {
        let s = stand_von(&z);
        Json(Stand { eingeschaltet: s.eingeschaltet(), gesperrt: s.ist_gesperrt(Instant::now()) })
    }

    /// Die Leitung „lokaler Socket“: nimmt nur Verbindungen der erwarteten
    /// Benutzernummer an und gibt sie mit dem belegten Aufrufer weiter.
    struct GepruefterLauscher {
        innen: UnixListener,
        erwartete_uid: u32,
    }

    impl axum::serve::Listener for GepruefterLauscher {
        type Io = UnixStream;
        type Addr = Aufrufer;

        async fn accept(&mut self) -> (Self::Io, Self::Addr) {
            loop {
                match self.innen.accept().await {
                    Ok((verbindung, _)) => {
                        // Die Prüfung ist nie abschaltbar, wie am Kanal.
                        let bezeugt = verbindung.peer_cred().ok().map(|c| c.uid());
                        match crate::kanal::gegenseite_urteil(bezeugt, self.erwartete_uid) {
                            Some(konto) => return (verbindung, Aufrufer { uid: konto.uid() }),
                            None => warn!("Helfer-Leitung: Verbindung abgewiesen, die Gegenseite ist nicht das eigene Konto."),
                        }
                    }
                    Err(e) => {
                        warn!("Helfer-Leitung: Annehmen gescheitert ({e}).");
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            }
        }

        fn local_addr(&self) -> std::io::Result<Self::Addr> {
            Ok(Aufrufer { uid: self.erwartete_uid })
        }
    }

    impl axum::extract::connect_info::Connected<IncomingStream<'_, GepruefterLauscher>> for Aufrufer {
        fn connect_info(stream: IncomingStream<'_, GepruefterLauscher>) -> Self {
            stream.remote_addr().clone()
        }
    }

    /// Die laufende Leitung. Beim Freigeben endet sie, und der eigene Socket wird entfernt.
    #[derive(Debug)]
    pub struct Helfer {
        _socket: crate::kanal::Kanal,
    }

    /// Startet die Endpunkte auf einem lokalen Socket am Ort `ort`, mit
    /// denselben Prüfungen wie der Kanal. Ist `ende` fertig, nimmt die
    /// Leitung nichts mehr an. Muss in einer Tokio-Laufzeit aufgerufen werden.
    pub fn starten(ort: &Path, erwartete_uid: u32, ende: impl Future<Output = ()> + Send + 'static, zustand: HelferZustand) -> Result<Helfer, String> {
        let (innen, kennung) = crate::kanal::binden(ort)?;
        let lauscher = GepruefterLauscher { innen, erwartete_uid };
        let dienst = router(zustand).into_make_service_with_connect_info::<Aufrufer>();
        let aufgabe = tokio::spawn(async move {
            if let Err(e) = axum::serve(lauscher, dienst).with_graceful_shutdown(ende).await {
                warn!("Helfer-Leitung beendet: {e}");
            }
        });
        info!("Helfer-Leitung: {}", crate::datenschutz::verbergen(&ort.display().to_string()));
        Ok(Helfer { _socket: crate::kanal::Kanal::aus(ort, kennung, aufgabe) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nach(t: Instant, s: u64) -> Instant {
        t + Duration::from_secs(s)
    }

    #[test]
    fn ausgeschaltet_gilt_jede_anmeldung_und_nichts_ist_gesperrt() {
        let mut s = SperrStand::default();
        let t = Instant::now();
        assert!(!s.eingeschaltet() && !s.ist_gesperrt(t));
        assert!(s.gilt(1, t, 1000));
        // Auch Meldungen des Helfers ändern daran nichts.
        s.sperre(t, 1000);
        assert!(!s.ist_gesperrt(t));
        assert!(s.gilt(1, t, 1001));
    }

    #[test]
    fn eingeschaltet_ohne_helfer_ist_gesperrt() {
        let mut s = SperrStand::default();
        let t = Instant::now();
        s.einschalten(1000);
        assert!(s.ist_gesperrt(t));
        // Auch eine Anmeldung von eben gilt nicht, solange der Helfer fehlt.
        assert!(!s.gilt(1005, t, 1005));
    }

    #[test]
    fn mit_lebenszeichen_gilt_nur_eine_anmeldung_nach_dem_einschalten() {
        let mut s = SperrStand::default();
        let t = Instant::now();
        s.einschalten(1000);
        s.zeichen(t);
        assert!(!s.ist_gesperrt(t));
        assert!(!s.gilt(999, t, 1001), "Anmeldung vor dem Einschalten");
        assert!(!s.gilt(1000, t, 1001), "Anmeldung in derselben Sekunde");
        assert!(s.gilt(1001, t, 1001));
    }

    #[test]
    fn sperre_entwertet_jede_anmeldung_bis_dahin_und_entsperren_verlangt_eine_neue() {
        let mut s = SperrStand::default();
        let t = Instant::now();
        s.einschalten(1000);
        s.zeichen(t);
        assert!(s.gilt(1010, t, 1010));
        s.sperre(nach(t, 60), 1060);
        assert!(s.ist_gesperrt(nach(t, 60)));
        assert!(!s.gilt(1010, nach(t, 61), 1061));
        // Entsperrt: Die alte Anmeldung gilt nicht mehr, eine neue schon.
        s.entsperrt(nach(t, 120), 1120);
        assert!(!s.ist_gesperrt(nach(t, 121)));
        assert!(!s.gilt(1010, nach(t, 121), 1121));
        assert!(!s.gilt(1120, nach(t, 121), 1121), "Anmeldung in der Sekunde des Entsperrens");
        assert!(s.gilt(1121, nach(t, 121), 1121));
    }

    /// Eine Anmeldung, die während der Sperre entsteht, gilt danach nicht.
    #[test]
    fn anmeldung_waehrend_der_sperre_gilt_auch_danach_nicht() {
        let mut s = SperrStand::default();
        let t = Instant::now();
        s.einschalten(1000);
        s.sperre(t, 1050);
        assert!(!s.gilt(1055, nach(t, 5), 1055));
        // Der Rechner wird entsperrt, ohne dass der Helfer es meldet (er meldet nur noch sein Dasein).
        s.entsperrt(nach(t, 10), 1056);
        assert!(!s.gilt(1055, nach(t, 11), 1057));
        assert!(s.gilt(1057, nach(t, 11), 1057));
    }

    #[test]
    fn ohne_lebenszeichen_ist_nach_der_frist_gesperrt() {
        let mut s = SperrStand::default();
        let t = Instant::now();
        s.einschalten(1000);
        s.zeichen(t);
        assert!(s.gilt(1010, nach(t, 10), 1010));
        // Kurz vor der Frist noch offen, an der Frist gesperrt.
        assert!(!s.ist_gesperrt(t + ZEICHEN_FRIST - Duration::from_secs(1)));
        assert!(s.ist_gesperrt(t + ZEICHEN_FRIST));
        let spaet = t + ZEICHEN_FRIST + Duration::from_secs(5);
        assert!(!s.gilt(1010, spaet, 1030));
    }

    /// Kommt der Helfer nach einer Lücke wieder, ist eine neue Anmeldung nötig:
    /// Was in der Lücke geschah, weiß niemand.
    #[test]
    fn nach_einer_luecke_im_lebenszeichen_ist_eine_neue_anmeldung_noetig() {
        let mut s = SperrStand::default();
        let t = Instant::now();
        s.einschalten(1000);
        s.zeichen(t);
        assert!(s.gilt(1010, nach(t, 5), 1010));
        // Lücke: Eine Anfrage in dieser Zeit rückt die Grenze nach.
        let luecke = t + ZEICHEN_FRIST + Duration::from_secs(10);
        assert!(!s.gilt(1010, luecke, 1040));
        // Der Helfer ist wieder da.
        s.zeichen(luecke + Duration::from_secs(1));
        let danach = luecke + Duration::from_secs(2);
        assert!(!s.ist_gesperrt(danach));
        assert!(!s.gilt(1010, danach, 1042), "die alte Anmeldung lebt nicht wieder auf");
        assert!(s.gilt(1041, danach, 1042));
    }

    #[test]
    fn lebenszeichen_hebt_eine_gemeldete_sperre_nicht_auf() {
        let mut s = SperrStand::default();
        let t = Instant::now();
        s.einschalten(1000);
        s.sperre(t, 1010);
        s.zeichen(nach(t, 5));
        assert!(s.ist_gesperrt(nach(t, 6)));
        assert!(!s.gilt(1020, nach(t, 6), 1020));
    }

    #[test]
    fn schluessel_uebernahme_erster_gilt_gleicher_bleibt_anderer_wird_abgewiesen() {
        let k = |b: u8| zeroize::Zeroizing::new([b; 32]);
        let mut ablage = None;
        assert_eq!(uebernehmen(&mut ablage, k(1)), Uebernahme::Gesetzt);
        assert_eq!(uebernehmen(&mut ablage, k(1)), Uebernahme::Gleich);
        assert_eq!(uebernehmen(&mut ablage, k(2)), Uebernahme::Anders);
        assert_eq!(ablage.as_deref(), Some(&[1u8; 32]));
    }

    #[test]
    fn schluessel_lesen_nur_32_bytes_base64url() {
        assert_eq!(schluessel_lesen(&crate::passkey::b64(&[9u8; 32])).as_deref(), Some(&[9u8; 32]));
        assert!(schluessel_lesen(&crate::passkey::b64(&[9u8; 31])).is_none());
        assert!(schluessel_lesen(&crate::passkey::b64(&[9u8; 33])).is_none());
        assert!(schluessel_lesen("kein base64 !").is_none());
        assert!(schluessel_lesen("").is_none());
    }

    #[test]
    fn die_grenze_geht_nie_zurueck() {
        let mut s = SperrStand::default();
        let t = Instant::now();
        s.einschalten(2000);
        s.zeichen(t);
        // Eine Sperre mit früherer Wanduhr (Uhr zurückgestellt) senkt die Grenze nicht.
        s.sperre(t, 1500);
        s.entsperrt(t, 1600);
        assert!(!s.gilt(1900, t, 1601));
        assert!(s.gilt(2001, t, 1601));
    }
}

/// Die Endpunkte über den echten Socket, mit echtem HTTP.
#[cfg(all(test, unix))]
mod unix_tests {
    use std::os::unix::fs::DirBuilderExt;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    use super::*;

    struct Ordner(PathBuf);

    impl Drop for Ordner {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn ordner(name: &str) -> Ordner {
        let o = std::env::temp_dir().join(format!("dh-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&o);
        std::fs::DirBuilder::new().mode(0o700).create(&o).unwrap();
        Ordner(o)
    }

    /// Eine HTTP-Anfrage über den Socket; gibt Statuszeile und Körper zurück.
    async fn http(ort: &Path, methode: &str, pfad: &str) -> (String, String) {
        let mut v = UnixStream::connect(ort).await.unwrap();
        let anfrage = format!("{methode} {pfad} HTTP/1.1\r\nHost: helfer\r\nConnection: close\r\nContent-Length: 0\r\n\r\n");
        let _ = v.write_all(anfrage.as_bytes()).await;
        let mut roh = String::new();
        let _ = tokio::time::timeout(Duration::from_secs(10), v.read_to_string(&mut roh)).await.expect("die Verbindung endet");
        let status = roh.lines().next().unwrap_or_default().to_string();
        let koerper = roh.split_once("\r\n\r\n").map(|(_, k)| k.to_string()).unwrap_or_default();
        (status, koerper)
    }

    fn zustand() -> HelferZustand {
        HelferZustand { sperre: Arc::new(Mutex::new(SperrStand::default())), schluessel: None }
    }

    /// Wie [`http`], mit JSON-Körper.
    async fn http_mit(ort: &Path, pfad: &str, koerper: &str) -> (String, String) {
        let mut v = UnixStream::connect(ort).await.unwrap();
        let anfrage = format!(
            "POST {pfad} HTTP/1.1\r\nHost: helfer\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{koerper}",
            koerper.len()
        );
        let _ = v.write_all(anfrage.as_bytes()).await;
        let mut roh = String::new();
        let _ = tokio::time::timeout(Duration::from_secs(10), v.read_to_string(&mut roh)).await.expect("die Verbindung endet");
        let status = roh.lines().next().unwrap_or_default().to_string();
        let antwort = roh.split_once("\r\n\r\n").map(|(_, k)| k.to_string()).unwrap_or_default();
        (status, antwort)
    }

    /// Der Helfer übergibt den Schlüssel; der Speicher kann danach Links
    /// verschließen und öffnen, vorher nicht.
    #[tokio::test(flavor = "multi_thread")]
    async fn schluessel_uebergabe_macht_die_links_lesbar() {
        let o = ordner("schluessel");
        let ort = o.0.join(HELFER_DATEI);
        let uebergabe: crate::speicher::verschluesselung::Uebergabe = Default::default();
        let speicher = crate::speicher::Speicher::temporaer().mit_links_schluessel_vom_helfer(uebergabe.clone());
        let z = HelferZustand { sperre: Arc::new(Mutex::new(SperrStand::default())), schluessel: Some(uebergabe.clone()) };
        let _helfer = starten(&ort, crate::unix::eigene_uid(), std::future::pending(), z).unwrap();
        // Vorher: Der Schlüssel fehlt, mit festem Satz, und das Lebenszeichen sagt es.
        assert_eq!(speicher.link_verschliessen(b"https://beispiel.test/x").unwrap_err(), crate::speicher::verschluesselung::VOM_HELFER_FEHLT_SATZ);
        let (s, k) = http(&ort, "POST", "/lebenszeichen").await;
        assert_eq!(s, "HTTP/1.1 200 OK");
        assert_eq!(serde_json::from_str::<Lebenszeichen>(&k).unwrap(), Lebenszeichen { schluessel_fehlt: true });
        // Falscher Körper: abgewiesen, nichts gesetzt.
        for koerper in ["{}", "{\"links\":\"zu-kurz\"}", "kein json", "{\"links\":\"AAAA\",\"mehr\":1}"] {
            assert_eq!(http_mit(&ort, "/schluessel", koerper).await.0, "HTTP/1.1 422 Unprocessable Entity", "{koerper}");
        }
        assert!(uebergabe.lock().unwrap().is_none());
        // Übergabe.
        let schluessel = crate::passkey::b64(&[7u8; 32]);
        assert_eq!(http_mit(&ort, "/schluessel", &format!("{{\"links\":\"{schluessel}\"}}")).await.0, "HTTP/1.1 204 No Content");
        let (_, k) = http(&ort, "POST", "/lebenszeichen").await;
        assert_eq!(serde_json::from_str::<Lebenszeichen>(&k).unwrap(), Lebenszeichen { schluessel_fehlt: false });
        let geheim = speicher.link_verschliessen(b"https://beispiel.test/x").unwrap();
        assert_eq!(speicher.link_oeffnen(&geheim).unwrap().as_slice(), b"https://beispiel.test/x");
        // Derselbe noch einmal: in Ordnung. Ein anderer: abgewiesen, der alte gilt weiter.
        assert_eq!(http_mit(&ort, "/schluessel", &format!("{{\"links\":\"{schluessel}\"}}")).await.0, "HTTP/1.1 204 No Content");
        let anderer = crate::passkey::b64(&[8u8; 32]);
        assert_eq!(http_mit(&ort, "/schluessel", &format!("{{\"links\":\"{anderer}\"}}")).await.0, "HTTP/1.1 409 Conflict");
        assert_eq!(speicher.link_oeffnen(&geheim).unwrap().as_slice(), b"https://beispiel.test/x");
    }

    /// Ohne die Einstellung nimmt der Server keinen Schlüssel an.
    #[tokio::test(flavor = "multi_thread")]
    async fn ohne_die_einstellung_wird_kein_schluessel_angenommen() {
        let o = ordner("ohne");
        let ort = o.0.join(HELFER_DATEI);
        let _helfer = starten(&ort, crate::unix::eigene_uid(), std::future::pending(), zustand()).unwrap();
        let schluessel = crate::passkey::b64(&[7u8; 32]);
        assert_eq!(http_mit(&ort, "/schluessel", &format!("{{\"links\":\"{schluessel}\"}}")).await.0, "HTTP/1.1 409 Conflict");
        let (_, k) = http(&ort, "POST", "/lebenszeichen").await;
        assert_eq!(serde_json::from_str::<Lebenszeichen>(&k).unwrap(), Lebenszeichen { schluessel_fehlt: false });
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn endpunkte_ueber_den_socket_mit_http() {
        let o = ordner("wege");
        let ort = o.0.join(HELFER_DATEI);
        let z = zustand();
        let _helfer = starten(&ort, crate::unix::eigene_uid(), std::future::pending(), z.clone()).unwrap();
        let stand = |k: String| serde_json::from_str::<serde_json::Value>(&k).unwrap();
        // Ausgeschaltet: nie gesperrt.
        let (s, k) = http(&ort, "GET", "/stand").await;
        assert_eq!(s, "HTTP/1.1 200 OK");
        assert_eq!(stand(k), serde_json::json!({ "eingeschaltet": false, "gesperrt": false }));
        // Eingeschaltet, noch kein Lebenszeichen: gesperrt.
        z.sperre.lock().unwrap().einschalten(wand_jetzt());
        assert_eq!(stand(http(&ort, "GET", "/stand").await.1), serde_json::json!({ "eingeschaltet": true, "gesperrt": true }));
        assert_eq!(http(&ort, "POST", "/lebenszeichen").await.0, "HTTP/1.1 200 OK");
        assert_eq!(stand(http(&ort, "GET", "/stand").await.1), serde_json::json!({ "eingeschaltet": true, "gesperrt": false }));
        assert_eq!(http(&ort, "POST", "/sperre").await.0, "HTTP/1.1 204 No Content");
        assert_eq!(stand(http(&ort, "GET", "/stand").await.1), serde_json::json!({ "eingeschaltet": true, "gesperrt": true }));
        assert_eq!(http(&ort, "POST", "/entsperrt").await.0, "HTTP/1.1 204 No Content");
        assert_eq!(stand(http(&ort, "GET", "/stand").await.1), serde_json::json!({ "eingeschaltet": true, "gesperrt": false }));
        // Falsche Methode und unbekannter Weg: gewöhnliche HTTP-Antworten.
        assert_eq!(http(&ort, "GET", "/sperre").await.0, "HTTP/1.1 405 Method Not Allowed");
        assert_eq!(http(&ort, "POST", "/anderes").await.0, "HTTP/1.1 404 Not Found");
    }

    /// Die Leitung prüft die Gegenseite: Erwartet der Lauscher eine andere
    /// Benutzernummer, kommt keine Anfrage bis zu den Endpunkten.
    #[tokio::test(flavor = "multi_thread")]
    async fn fremde_gegenseite_kommt_nicht_bis_zu_den_endpunkten() {
        let o = ordner("fremd");
        let ort = o.0.join(HELFER_DATEI);
        let z = zustand();
        z.sperre.lock().unwrap().einschalten(wand_jetzt());
        let _helfer = starten(&ort, crate::unix::eigene_uid().wrapping_add(1), std::future::pending(), z.clone()).unwrap();
        assert_eq!(http(&ort, "POST", "/lebenszeichen").await, (String::new(), String::new()));
        // Nichts ist angekommen: weiter gesperrt, weil kein Lebenszeichen.
        assert!(z.sperre.lock().unwrap().ist_gesperrt(Instant::now()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn socket_hat_die_rechte_600_und_verschwindet_am_ende() {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        let o = ordner("rechte");
        let ort = o.0.join(HELFER_DATEI);
        let helfer = starten(&ort, crate::unix::eigene_uid(), std::future::pending(), zustand()).unwrap();
        let m = std::fs::symlink_metadata(&ort).unwrap();
        assert!(m.file_type().is_socket());
        assert_eq!(m.mode() & 0o777, 0o600);
        drop(helfer);
        assert!(std::fs::symlink_metadata(&ort).is_err());
    }

    /// Ohne den belegten Aufrufer der Leitung läuft kein Endpunkt: Am Router
    /// ohne Leitung (wie an einem Web-Port) gibt es keine Wirkung.
    #[tokio::test]
    async fn ohne_belegten_aufrufer_laeuft_kein_endpunkt() {
        use tower::ServiceExt;
        let z = zustand();
        z.sperre.lock().unwrap().einschalten(wand_jetzt());
        let req = axum::http::Request::post("/lebenszeichen").body(axum::body::Body::empty()).unwrap();
        let antwort = router(z.clone()).oneshot(req).await.unwrap();
        assert!(antwort.status().is_server_error(), "{}", antwort.status());
        assert!(z.sperre.lock().unwrap().ist_gesperrt(Instant::now()));
    }
}
