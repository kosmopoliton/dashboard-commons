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
//! Schlüssel: Mit der Einstellung kommen Schlüssel vom Helfer statt aus
//! Dateien. Der Server führt einen **Schlüsselbund**: Jeder Schlüssel steht
//! unter dem Pfad der Nutzlast, die er öffnet (etwa
//! `speicher/aufgaben/links`), und trägt einen Namen und ein Etikett. Es
//! kann viele Nutzlasten geben, und nicht jeder Helfer liefert jeden
//! Schlüssel: Eine Übergabe nennt nur die Pfade, die dieser Helfer öffnen
//! kann, und bekommt je Pfad eine eigene Antwort. Der Server nimmt nur
//! Pfade an, die er selbst erwartet.
//!
//! In dieser ersten Fassung kommen die Schlüssel im Klartext über den
//! lokalen Socket; geschützt ist das nur so weit wie der Socket selbst
//! (eigenes Konto, eigener Rechner). Der Server hält sie nur im
//! Arbeitsspeicher. Nach einem Neustart fehlen sie; die Antwort auf das
//! Lebenszeichen nennt die fehlenden Pfade, und der Helfer übergibt erneut.
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

/// Ein Schlüssel von 32 Bytes in einem Puffer, der beim Freigeben überschrieben wird.
pub type Wert = zeroize::Zeroizing<[u8; 32]>;

/// Längster Name und längstes Etikett eines Schlüssels, in Zeichen.
pub const NAME_HOECHSTENS: usize = 60;
pub const ETIKETT_HOECHSTENS: usize = 120;

/// Was die Übergabe eines Schlüssels ergibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Uebernahme {
    /// Bisher lag unter diesem Pfad keiner vor: Dieser gilt jetzt.
    Gesetzt,
    /// Derselbe wie schon vorhanden (etwa nach einem Neustart des Helfers).
    Gleich,
    /// Ein anderer als der vorhandene: abgewiesen. Ein stiller Tausch machte
    /// alles unlesbar, was mit dem bisherigen verschlossen wurde.
    Anders,
    /// Der Server erwartet unter diesem Pfad keine Nutzlast.
    Unbekannt,
    /// Wert, Name oder Etikett sind nicht lesbar (kein Base64url, nicht 32
    /// Bytes, leer, zu lang oder mit Steuerzeichen).
    Unlesbar,
}

/// Was der Server über einen Pfad seines Schlüsselbunds sagt; nie der Wert.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Beschreibung {
    /// Der Pfad der Nutzlast, die der Schlüssel öffnet.
    pub pfad: String,
    /// Name und Etikett, wie der Helfer sie übergeben hat; leer, solange keiner vorliegt.
    pub name: String,
    pub etikett: String,
    pub vorhanden: bool,
}

struct Eintrag {
    name: String,
    etikett: String,
    wert: Wert,
}

/// Der Schlüsselbund des Servers: die Pfade der Nutzlasten, für die er
/// Schlüssel vom Helfer erwartet, und was davon schon übergeben ist. Reine Logik.
#[derive(Default)]
pub struct SchluesselBund {
    erwartet: std::collections::BTreeSet<String>,
    vorhanden: std::collections::BTreeMap<String, Eintrag>,
}

impl std::fmt::Debug for SchluesselBund {
    /// Nie die Werte zeigen.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchluesselBund").field("erwartet", &self.erwartet).field("vorhanden", &self.vorhanden.keys().collect::<Vec<_>>()).finish()
    }
}

fn beschriftung_gut(text: &str, hoechstens: usize) -> bool {
    !text.trim().is_empty() && text.chars().count() <= hoechstens && !text.chars().any(char::is_control)
}

impl SchluesselBund {
    /// Ein Bund, der genau diese Pfade erwartet.
    pub fn fuer(pfade: &[&str]) -> Self {
        Self { erwartet: pfade.iter().map(|p| (*p).to_string()).collect(), vorhanden: Default::default() }
    }

    /// Übernimmt einen Schlüssel für `pfad`. Der erste gilt, derselbe noch
    /// einmal ist in Ordnung (Name und Etikett werden dabei aufgefrischt),
    /// ein anderer wird abgewiesen, ebenso ein Pfad, den der Server nicht erwartet.
    pub fn uebernehmen(&mut self, pfad: &str, name: &str, etikett: &str, wert: Option<Wert>) -> Uebernahme {
        if !self.erwartet.contains(pfad) {
            return Uebernahme::Unbekannt;
        }
        let Some(wert) = wert.filter(|_| beschriftung_gut(name, NAME_HOECHSTENS) && beschriftung_gut(etikett, ETIKETT_HOECHSTENS)) else {
            return Uebernahme::Unlesbar;
        };
        match self.vorhanden.get_mut(pfad) {
            None => {
                self.vorhanden.insert(pfad.to_string(), Eintrag { name: name.trim().to_string(), etikett: etikett.trim().to_string(), wert });
                Uebernahme::Gesetzt
            }
            Some(alt) if *alt.wert == *wert => {
                alt.name = name.trim().to_string();
                alt.etikett = etikett.trim().to_string();
                Uebernahme::Gleich
            }
            Some(_) => Uebernahme::Anders,
        }
    }

    /// Der Schlüssel für `pfad`, wenn er übergeben ist.
    pub fn holen(&self, pfad: &str) -> Option<Wert> {
        self.vorhanden.get(pfad).map(|e| e.wert.clone())
    }

    /// Die erwarteten Pfade ohne Schlüssel, sortiert.
    pub fn fehlend(&self) -> Vec<String> {
        self.erwartet.iter().filter(|p| !self.vorhanden.contains_key(*p)).cloned().collect()
    }

    /// Alle erwarteten Pfade mit Name, Etikett und ob ein Schlüssel vorliegt; nie die Werte.
    pub fn beschreiben(&self) -> Vec<Beschreibung> {
        self.erwartet
            .iter()
            .map(|pfad| match self.vorhanden.get(pfad) {
                Some(e) => Beschreibung { pfad: pfad.clone(), name: e.name.clone(), etikett: e.etikett.clone(), vorhanden: true },
                None => Beschreibung { pfad: pfad.clone(), name: String::new(), etikett: String::new(), vorhanden: false },
            })
            .collect()
    }
}

/// Der geteilte Schlüsselbund: zwischen den Endpunkten, die Schlüssel
/// annehmen, und den Stellen, die sie brauchen. Nie auf der Platte.
pub type Bund = std::sync::Arc<std::sync::Mutex<SchluesselBund>>;

/// Ein geteilter Bund für diese Pfade.
pub fn bund_fuer(pfade: &[&str]) -> Bund {
    std::sync::Arc::new(std::sync::Mutex::new(SchluesselBund::fuer(pfade)))
}

/// Liest einen übergebenen Wert: Base64url, genau 32 Bytes.
pub fn schluessel_lesen(text: &str) -> Option<Wert> {
    let roh = zeroize::Zeroizing::new(crate::passkey::b64_lesen(text)?);
    let feld: [u8; 32] = roh.as_slice().try_into().ok()?;
    Some(zeroize::Zeroizing::new(feld))
}

/// Wanduhr in Sekunden seit 1970.
pub fn wand_jetzt() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(unix)]
pub use leitung::{Aufrufer, Helfer, HelferZustand, Lebenszeichen, UebergabeAntwort, router, starten};

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
        /// Der Schlüsselbund; `None`, wenn der Server keine Schlüssel vom Helfer nimmt.
        pub schluessel: Option<super::Bund>,
    }

    /// Antwort auf `POST /lebenszeichen`.
    #[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    pub struct Lebenszeichen {
        /// Die Pfade der Nutzlasten, für die der Server einen Schlüssel
        /// erwartet und keinen hat (etwa nach einem Neustart). Ein Helfer
        /// übergibt davon, was er öffnen kann.
        pub fehlend: Vec<String>,
    }

    /// Körper von `POST /schluessel`: beliebig viele Schlüssel, je mit dem
    /// Pfad der Nutzlast, einem Namen und einem Etikett.
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SchluesselUebergabe {
        schluessel: Vec<EinSchluessel>,
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct EinSchluessel {
        pfad: String,
        name: String,
        etikett: String,
        /// 32 Bytes, Base64url.
        wert: String,
    }

    /// Antwort auf `POST /schluessel`: je übergebenem Pfad, was geschah.
    #[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    pub struct UebergabeAntwort {
        pub ergebnis: Vec<UebergabeErgebnis>,
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    pub struct UebergabeErgebnis {
        pub pfad: String,
        pub stand: super::Uebernahme,
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
            .route("/schluessel", post(schluessel).get(schluessel_liste))
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

    fn bund_von(b: &super::Bund) -> std::sync::MutexGuard<'_, super::SchluesselBund> {
        b.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    async fn lebenszeichen(State(z): State<HelferZustand>, ConnectInfo(_wer): ConnectInfo<Aufrufer>) -> Json<Lebenszeichen> {
        stand_von(&z).zeichen(Instant::now());
        let fehlend = z.schluessel.as_ref().map(|b| bund_von(b).fehlend()).unwrap_or_default();
        Json(Lebenszeichen { fehlend })
    }

    /// `POST /schluessel`: Der Helfer übergibt Schlüssel, je unter dem Pfad
    /// der Nutzlast. Der Körper wird nie protokolliert. Je Pfad eine eigene
    /// Antwort; ein Helfer muss nicht alle Pfade liefern.
    async fn schluessel(
        State(z): State<HelferZustand>,
        ConnectInfo(_wer): ConnectInfo<Aufrufer>,
        koerper: axum::body::Bytes,
    ) -> Result<Json<UebergabeAntwort>, (StatusCode, &'static str)> {
        let Some(bund) = &z.schluessel else {
            return Err((StatusCode::CONFLICT, "Der Server nimmt keine Schlüssel vom Helfer; die Einstellung dafür ist aus."));
        };
        let Ok(uebergabe) = serde_json::from_slice::<SchluesselUebergabe>(&koerper) else {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "Erwartet wird {\"schluessel\": [{\"pfad\": …, \"name\": …, \"etikett\": …, \"wert\": \"<32 Bytes, Base64url>\"}]}.",
            ));
        };
        let mut bund = bund_von(bund);
        let ergebnis: Vec<UebergabeErgebnis> = uebergabe
            .schluessel
            .iter()
            .map(|k| UebergabeErgebnis { pfad: k.pfad.clone(), stand: bund.uebernehmen(&k.pfad, &k.name, &k.etikett, super::schluessel_lesen(&k.wert)) })
            .collect();
        drop(bund);
        // Ins Protokoll nur, wie viele je Ausgang; Pfade, Namen und Werte nicht.
        let zahl = |s: super::Uebernahme| ergebnis.iter().filter(|e| e.stand == s).count();
        info!(
            "Helfer: Schlüssel übergeben: {} gesetzt, {} gleich, {} abgewiesen.",
            zahl(super::Uebernahme::Gesetzt),
            zahl(super::Uebernahme::Gleich),
            ergebnis.len() - zahl(super::Uebernahme::Gesetzt) - zahl(super::Uebernahme::Gleich)
        );
        Ok(Json(UebergabeAntwort { ergebnis }))
    }

    /// `GET /schluessel`: die Pfade, die der Server erwartet, mit Name,
    /// Etikett und ob ein Schlüssel vorliegt. Nie die Werte.
    async fn schluessel_liste(State(z): State<HelferZustand>, ConnectInfo(_wer): ConnectInfo<Aufrufer>) -> Json<Vec<super::Beschreibung>> {
        Json(z.schluessel.as_ref().map(|b| bund_von(b).beschreiben()).unwrap_or_default())
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

    fn wert(b: u8) -> Option<Wert> {
        Some(zeroize::Zeroizing::new([b; 32]))
    }

    #[test]
    fn bund_je_pfad_erster_gilt_gleicher_bleibt_anderer_wird_abgewiesen() {
        let mut bund = SchluesselBund::fuer(&["a/links", "b/notizen"]);
        assert_eq!(bund.uebernehmen("a/links", "links-1", "Links", wert(1)), Uebernahme::Gesetzt);
        assert_eq!(bund.uebernehmen("a/links", "links-1", "Links", wert(1)), Uebernahme::Gleich);
        assert_eq!(bund.uebernehmen("a/links", "links-2", "Links", wert(2)), Uebernahme::Anders);
        assert_eq!(bund.holen("a/links").as_deref(), Some(&[1u8; 32]));
        // Ein anderer Pfad ist davon unberührt und hat seinen eigenen Schlüssel.
        assert_eq!(bund.holen("b/notizen"), None);
        assert_eq!(bund.uebernehmen("b/notizen", "notizen-1", "Notizen", wert(2)), Uebernahme::Gesetzt);
        assert_eq!(bund.holen("b/notizen").as_deref(), Some(&[2u8; 32]));
        assert_eq!(bund.holen("a/links").as_deref(), Some(&[1u8; 32]));
    }

    /// Nicht jeder Helfer liefert jeden Schlüssel: Was fehlt, bleibt fehlend,
    /// und ein zweiter Helfer kann es nachliefern.
    #[test]
    fn bund_nennt_die_fehlenden_pfade_und_fuellt_sich_aus_mehreren_uebergaben() {
        let mut bund = SchluesselBund::fuer(&["c", "a", "b"]);
        assert_eq!(bund.fehlend(), ["a", "b", "c"]);
        // Der erste Helfer kann nur „b“ öffnen.
        assert_eq!(bund.uebernehmen("b", "n", "e", wert(1)), Uebernahme::Gesetzt);
        assert_eq!(bund.fehlend(), ["a", "c"]);
        // Der zweite liefert „a“ und „c“.
        assert_eq!(bund.uebernehmen("a", "n", "e", wert(2)), Uebernahme::Gesetzt);
        assert_eq!(bund.uebernehmen("c", "n", "e", wert(3)), Uebernahme::Gesetzt);
        assert!(bund.fehlend().is_empty());
    }

    #[test]
    fn bund_nimmt_nur_erwartete_pfade() {
        let mut bund = SchluesselBund::fuer(&["a/links"]);
        for pfad in ["", "a", "a/links/", "A/Links", "b/notizen", "../a/links"] {
            assert_eq!(bund.uebernehmen(pfad, "n", "e", wert(1)), Uebernahme::Unbekannt, "{pfad}");
        }
        assert_eq!(bund.fehlend(), ["a/links"]);
        // Ein leerer Bund erwartet nichts.
        assert_eq!(SchluesselBund::default().uebernehmen("a/links", "n", "e", wert(1)), Uebernahme::Unbekannt);
    }

    #[test]
    fn bund_weist_unlesbares_ab() {
        let mut bund = SchluesselBund::fuer(&["a"]);
        assert_eq!(bund.uebernehmen("a", "n", "e", None), Uebernahme::Unlesbar);
        assert_eq!(bund.uebernehmen("a", "", "e", wert(1)), Uebernahme::Unlesbar);
        assert_eq!(bund.uebernehmen("a", "  ", "e", wert(1)), Uebernahme::Unlesbar);
        assert_eq!(bund.uebernehmen("a", "n", "", wert(1)), Uebernahme::Unlesbar);
        assert_eq!(bund.uebernehmen("a", "n\u{7}", "e", wert(1)), Uebernahme::Unlesbar);
        assert_eq!(bund.uebernehmen("a", &"n".repeat(NAME_HOECHSTENS + 1), "e", wert(1)), Uebernahme::Unlesbar);
        assert_eq!(bund.uebernehmen("a", "n", &"e".repeat(ETIKETT_HOECHSTENS + 1), wert(1)), Uebernahme::Unlesbar);
        assert_eq!(bund.fehlend(), ["a"]);
        assert_eq!(bund.uebernehmen("a", &"n".repeat(NAME_HOECHSTENS), &"e".repeat(ETIKETT_HOECHSTENS), wert(1)), Uebernahme::Gesetzt);
    }

    #[test]
    fn bund_beschreibt_pfade_mit_name_und_etikett_nie_mit_wert() {
        let mut bund = SchluesselBund::fuer(&["a/links", "b/notizen"]);
        bund.uebernehmen("a/links", " links-2026 ", " Schlüssel der Aufgaben-Links ", wert(0x41));
        assert_eq!(
            bund.beschreiben(),
            [
                Beschreibung { pfad: "a/links".into(), name: "links-2026".into(), etikett: "Schlüssel der Aufgaben-Links".into(), vorhanden: true },
                Beschreibung { pfad: "b/notizen".into(), name: String::new(), etikett: String::new(), vorhanden: false },
            ]
        );
        // Derselbe Wert mit neuem Namen: Name und Etikett werden aufgefrischt.
        assert_eq!(bund.uebernehmen("a/links", "links-neu", "Neu", wert(0x41)), Uebernahme::Gleich);
        assert_eq!(bund.beschreiben()[0].name, "links-neu");
        // Weder die Beschreibung noch die Ausgabe zum Suchen zeigt den Wert.
        let text = format!("{:?} {:?} {}", bund, bund.beschreiben(), serde_json::to_string(&bund.beschreiben()).unwrap());
        assert!(!text.contains("AAAA") && !text.contains("65, 65"), "{text}");
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

    fn uebergabe(eintraege: &[(&str, u8)]) -> String {
        let liste: Vec<serde_json::Value> = eintraege
            .iter()
            .map(|(pfad, b)| serde_json::json!({ "pfad": pfad, "name": format!("name-{b}"), "etikett": format!("Etikett {b}"), "wert": crate::passkey::b64(&[*b; 32]) }))
            .collect();
        serde_json::json!({ "schluessel": liste }).to_string()
    }

    fn fehlend(koerper: &str) -> Vec<String> {
        serde_json::from_str::<Lebenszeichen>(koerper).unwrap().fehlend
    }

    fn staende(koerper: &str) -> Vec<(String, Uebernahme)> {
        serde_json::from_str::<UebergabeAntwort>(koerper).unwrap().ergebnis.into_iter().map(|e| (e.pfad, e.stand)).collect()
    }

    /// Der Helfer übergibt den Schlüssel der Links unter dessen Pfad; der
    /// Speicher kann danach Links verschließen und öffnen, vorher nicht.
    #[tokio::test(flavor = "multi_thread")]
    async fn schluessel_uebergabe_macht_die_links_lesbar() {
        use crate::speicher::verschluesselung::{LINKS_PFAD, VOM_HELFER_FEHLT_SATZ};
        let o = ordner("schluessel");
        let ort = o.0.join(HELFER_DATEI);
        let bund = bund_fuer(&[LINKS_PFAD]);
        let speicher = crate::speicher::Speicher::temporaer().mit_links_schluessel_vom_helfer(bund.clone());
        let z = HelferZustand { sperre: Arc::new(Mutex::new(SperrStand::default())), schluessel: Some(bund.clone()) };
        let _helfer = starten(&ort, crate::unix::eigene_uid(), std::future::pending(), z).unwrap();
        // Vorher: Der Schlüssel fehlt, mit festem Satz, und das Lebenszeichen nennt den Pfad.
        assert_eq!(speicher.link_verschliessen(b"https://beispiel.test/x").unwrap_err(), VOM_HELFER_FEHLT_SATZ);
        let (s, k) = http(&ort, "POST", "/lebenszeichen").await;
        assert_eq!(s, "HTTP/1.1 200 OK");
        assert_eq!(fehlend(&k), [LINKS_PFAD]);
        // Falscher Körper: abgewiesen, nichts gesetzt.
        for koerper in ["{}", "kein json", "{\"schluessel\":[{\"pfad\":\"x\"}]}", "{\"schluessel\":[],\"mehr\":1}"] {
            assert_eq!(http_mit(&ort, "/schluessel", koerper).await.0, "HTTP/1.1 422 Unprocessable Entity", "{koerper}");
        }
        assert_eq!(bund.lock().unwrap().fehlend(), [LINKS_PFAD]);
        // Übergabe.
        let (s, k) = http_mit(&ort, "/schluessel", &uebergabe(&[(LINKS_PFAD, 7)])).await;
        assert_eq!(s, "HTTP/1.1 200 OK");
        assert_eq!(staende(&k), [(LINKS_PFAD.to_string(), Uebernahme::Gesetzt)]);
        assert!(fehlend(&http(&ort, "POST", "/lebenszeichen").await.1).is_empty());
        let geheim = speicher.link_verschliessen(b"https://beispiel.test/x").unwrap();
        assert_eq!(speicher.link_oeffnen(&geheim).unwrap().as_slice(), b"https://beispiel.test/x");
        // Derselbe noch einmal: in Ordnung. Ein anderer: abgewiesen, der alte gilt weiter.
        assert_eq!(staende(&http_mit(&ort, "/schluessel", &uebergabe(&[(LINKS_PFAD, 7)])).await.1), [(LINKS_PFAD.to_string(), Uebernahme::Gleich)]);
        assert_eq!(staende(&http_mit(&ort, "/schluessel", &uebergabe(&[(LINKS_PFAD, 8)])).await.1), [(LINKS_PFAD.to_string(), Uebernahme::Anders)]);
        assert_eq!(speicher.link_oeffnen(&geheim).unwrap().as_slice(), b"https://beispiel.test/x");
    }

    /// Mehrere Nutzlasten, mehrere Helfer: Jeder übergibt nur, was er öffnen
    /// kann, und bekommt je Pfad eine eigene Antwort.
    #[tokio::test(flavor = "multi_thread")]
    async fn mehrere_nutzlasten_und_nicht_jeder_helfer_liefert_alles() {
        let o = ordner("mehrere");
        let ort = o.0.join(HELFER_DATEI);
        let bund = bund_fuer(&["speicher/a", "speicher/b", "speicher/c"]);
        let z = HelferZustand { sperre: Arc::new(Mutex::new(SperrStand::default())), schluessel: Some(bund.clone()) };
        let _helfer = starten(&ort, crate::unix::eigene_uid(), std::future::pending(), z).unwrap();
        assert_eq!(fehlend(&http(&ort, "POST", "/lebenszeichen").await.1), ["speicher/a", "speicher/b", "speicher/c"]);
        // Der erste Helfer öffnet „b“ und kennt dazu einen Pfad, den der Server nicht erwartet.
        let (_, k) = http_mit(&ort, "/schluessel", &uebergabe(&[("speicher/b", 2), ("speicher/fremd", 9)])).await;
        assert_eq!(staende(&k), [("speicher/b".to_string(), Uebernahme::Gesetzt), ("speicher/fremd".to_string(), Uebernahme::Unbekannt)]);
        assert_eq!(fehlend(&http(&ort, "POST", "/lebenszeichen").await.1), ["speicher/a", "speicher/c"]);
        // Der zweite liefert „a“ und „c“ und versucht „b“ mit einem anderen Wert.
        let (_, k) = http_mit(&ort, "/schluessel", &uebergabe(&[("speicher/a", 1), ("speicher/c", 3), ("speicher/b", 5)])).await;
        assert_eq!(
            staende(&k),
            [("speicher/a".to_string(), Uebernahme::Gesetzt), ("speicher/c".to_string(), Uebernahme::Gesetzt), ("speicher/b".to_string(), Uebernahme::Anders),]
        );
        assert!(fehlend(&http(&ort, "POST", "/lebenszeichen").await.1).is_empty());
        // Jeder Pfad hat seinen eigenen Schlüssel.
        {
            let b = bund.lock().unwrap();
            assert_eq!(
                (b.holen("speicher/a").as_deref(), b.holen("speicher/b").as_deref(), b.holen("speicher/c").as_deref()),
                (Some(&[1u8; 32]), Some(&[2u8; 32]), Some(&[3u8; 32]))
            );
        }
        // Die Liste nennt Pfade, Namen und Etiketten, nie die Werte.
        let (s, k) = http(&ort, "GET", "/schluessel").await;
        assert_eq!(s, "HTTP/1.1 200 OK");
        let liste: Vec<Beschreibung> = serde_json::from_str(&k).unwrap();
        assert_eq!(
            liste.iter().map(|e| (e.pfad.as_str(), e.name.as_str(), e.vorhanden)).collect::<Vec<_>>(),
            [("speicher/a", "name-1", true), ("speicher/b", "name-2", true), ("speicher/c", "name-3", true)]
        );
        assert!(!k.contains(&crate::passkey::b64(&[1u8; 32])), "{k}");
    }

    /// Ohne die Einstellung nimmt der Server keine Schlüssel an.
    #[tokio::test(flavor = "multi_thread")]
    async fn ohne_die_einstellung_wird_kein_schluessel_angenommen() {
        let o = ordner("ohne");
        let ort = o.0.join(HELFER_DATEI);
        let _helfer = starten(&ort, crate::unix::eigene_uid(), std::future::pending(), zustand()).unwrap();
        assert_eq!(http_mit(&ort, "/schluessel", &uebergabe(&[("speicher/aufgaben/links", 7)])).await.0, "HTTP/1.1 409 Conflict");
        assert!(fehlend(&http(&ort, "POST", "/lebenszeichen").await.1).is_empty());
        assert_eq!(http(&ort, "GET", "/schluessel").await.1, "[]");
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
