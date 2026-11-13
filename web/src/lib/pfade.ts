/**
 * Lokale Pfade in der Oberfläche (Prüfbericht: Links-Kästen setzten
 * lokale Pfade als Links, die im Browser nicht aufgehen; /pc zeigte rohe
 * Adressen mit dem Benutzernamen).
 *
 * - Ein lokaler Pfad (`C:\…`, `C:/…`, `file:…`, `\\rechner\…`, unter macOS
 *   und Linux `/Users/…`, `/home/…`, `/Volumes/…` und die Ordner des
 *   Systems) ist nie ein Link. Öffnen geht nur über „Orte“
 *   (`daten/orte.toml`, Knopf „Öffnen“, Entscheidung lokal-oeffnen).
 * - Der Teil `C:\Users\<name>` (auch „Benutzer“, auch mit `/`) wird beim
 *   Zeigen zu `%USERPROFILE%`: der Name des Kontos steht nirgends im Bild.
 *   Ebenso `/Users/<name>` und `/home/<name>` (auch hinter
 *   `/System/Volumes/Data` und `/Volumes/<x>`), `/root` und `/var/root`.
 *   Die Formen sind dieselben wie im Server (`src/datenschutz.rs`); der
 *   Server ist die eine Stelle, hier steht die zweite Sicherung.
 */

/** Hinweis neben einem lokalen Pfad in einem Links-Kasten. */
export const SATZ_LOKALER_PFAD =
	'Lokaler Ort: geht im Browser nicht auf. Der Chat kann ihn in daten/orte.toml eintragen, dann steht hier „Öffnen“.';

/** Platzhalter für das Benutzerprofil. */
export const PROFIL_PLATZHALTER = '%USERPROFILE%';

/** Platzhalter für den Namen eines Datenträgers unter `/Volumes`. */
export const VOLUME_PLATZHALTER = '%VOLUME%';

/**
 * Ordner direkt unter der Wurzel, die unter macOS und Linux ein lokaler Ort
 * sind: Profile und System. Alles andere mit `/` vorn ist ein Weg im
 * Dashboard (`/projects/x`, `/pc`) und bleibt ein Link.
 */
const POSIX_WURZELN = [
	'Users',
	'home',
	'root',
	'Volumes',
	'System',
	'Library',
	'Applications',
	'private',
	'var',
	'tmp',
	'opt',
	'usr',
	'etc',
	'mnt',
	'media',
	'run',
	'srv'
];
const POSIX_LOKAL = new RegExp(`^/(?:${POSIX_WURZELN.join('|')})(?:/|$)`);

/**
 * `C:\…`, `C:/…`, `file:…`, ein Netzwerkpfad `\\…` oder ein Pfad in der Form
 * von macOS und Linux unter einer Profil- oder Systemwurzel (`/Users/…`,
 * `/Volumes/…`, `/usr/…`). Ein Weg im Dashboard mit `/` vorn ist nie lokal.
 */
export function istLokalerPfad(adresse: string): boolean {
	const a = adresse.trim();
	return (
		/^[a-z]:[\\/]/i.test(a) || /^file:/i.test(a) || a.startsWith('\\\\') || POSIX_LOKAL.test(a)
	);
}

/** Ein Name in einem Pfad: bis zum nächsten Trenner oder Zeichen, das keiner trägt. */
const NAME = '[^\\\\/"\'<>|:*?\\r\\n]+';
/**
 * Davor steht nichts, was zu einem Wort oder einer Adresse gehört: So bleiben
 * `https://…/users/x`, `ssh://host/Users/x` und `host:/Users/x`, wie sie sind.
 */
const GRENZE = '(^|[^\\w.:/\\\\%-])';
const POSIX_PROFIL = new RegExp(
	`${GRENZE}(?:file://)?(?:/System/Volumes/Data|/Volumes/${NAME})?/(?:users|home)/${NAME}`,
	'gi'
);
const POSIX_ROOT = new RegExp(
	`${GRENZE}(?:file://)?/(?:private/var/|var/)?root(?=$|[\\\\/\\s"'<>|:*?])`,
	'g'
);
const POSIX_VOLUME = new RegExp(`${GRENZE}/Volumes/${NAME}`, 'g');

/**
 * Ersetzt jedes Benutzerprofil (`<Laufwerk>:\Users\<name>` oder
 * `…\Benutzer\<name>`, auch mit `/` und als `file:///`; unter macOS und Linux
 * `/Users/<name>`, `/home/<name>`, `/root`) durch `%USERPROFILE%` und den
 * Namen eines Datenträgers unter `/Volumes` durch `%VOLUME%`.
 * Der Rest des Pfads bleibt, damit man den Ort noch erkennt.
 */
export function profilVerbergen(text: string): string {
	return text
		.replace(
			/(file:\/{2,3})?[a-z]:[\\/]+(?:users|benutzer)[\\/]+[^\\/"'<>|:*?\r\n]+/gi,
			PROFIL_PLATZHALTER
		)
		.replace(POSIX_PROFIL, (_, davor: string) => `${davor}${PROFIL_PLATZHALTER}`)
		.replace(POSIX_ROOT, (_, davor: string) => `${davor}${PROFIL_PLATZHALTER}`)
		.replace(POSIX_VOLUME, (_, davor: string) => `${davor}/Volumes/${VOLUME_PLATZHALTER}`);
}
