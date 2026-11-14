/**
 * Lokale Pfade in der Oberfläche (Prüfbericht: Links-Kästen setzten
 * lokale Pfade als Links, die im Browser nicht aufgehen; /pc zeigte rohe
 * Adressen mit dem Benutzernamen).
 *
 * - Ein lokaler Pfad (`C:\…`, `C:/…`, `file:…`, `\\rechner\…`, unter macOS
 *   und Linux `/Users/<name>`, `/home/<name>`, `/Volumes/<x>` und die Ordner
 *   des Systems) ist nie ein Link. Öffnen geht nur über „Orte“
 *   (`daten/orte.toml`, Knopf „Öffnen“, Entscheidung lokal-oeffnen).
 * - Der Teil `C:\Users\<name>` (auch „Benutzer“, auch mit `/`) wird beim
 *   Zeigen zu `%USERPROFILE%`: der Name des Kontos steht nirgends im Bild.
 *   Ebenso `/Users/<name>` und `/home/<name>` (auch hinter
 *   `/System/Volumes/Data` und `/Volumes/<x>`), `/root` und `/var/root`, je
 *   auch hinter `file:`, in einer Liste mit Doppelpunkt (`PATH`) und mit
 *   `\/` wie in JSON.
 *   Der Server (`src/datenschutz.rs`) ist die eine Stelle, hier steht die
 *   zweite Sicherung. Sie kennt weniger Formen als der Server: nicht
 *   `/run/user/<n>`, nicht den Temp-Ordner je Konto unter `/var/folders` und
 *   keine URL-kodierten Pfade ohne Laufwerk.
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
 * von macOS und Linux unter einer Profil- oder Systemwurzel (`/Users/<name>`,
 * `/Volumes/<x>`, `/usr/bin`). Ein Weg im Dashboard mit `/` vorn ist nie lokal.
 */
export function istLokalerPfad(adresse: string): boolean {
	const a = adresse.trim();
	return (
		/^[a-z]:[\\/]/i.test(a) || /^file:/i.test(a) || a.startsWith('\\\\') || POSIX_LOKAL.test(a)
	);
}

/**
 * Ein Zeichen eines Namens in einem Pfad ohne Laufwerk: kein Trenner, kein
 * Leerraum und keins der Zeichen, an denen der Server einen Namen beendet.
 */
const ZEICHEN = '[^\\\\/\\s"\'`<>|:*?\\[\\];=,+()#&{}]';
/**
 * Ein Name: mit Leerzeichen nur, wenn danach ein Trenner kommt
 * (`/Users/Beispiel Konto/x`), sonst bis zum ersten Leerraum. So bleiben zwei
 * Pfade hintereinander zwei Pfade.
 */
const NAME = `(?:${ZEICHEN}+(?: +${ZEICHEN}+)*(?=\\\\?/)|${ZEICHEN}+)`;
/** Ein Schrägstrich, auch als `\/` wie in JSON. */
const SEP = '\\\\?/';
/** Ein Zeichen eines Worts ohne Leerraum und ohne `=`. */
const WORT = '[^\\s=]';
/**
 * Wo ein Pfad ohne Laufwerk beginnen darf, ohne ein Zeichen zu verbrauchen:
 * - Davor steht nichts, was zu einem Wort oder einer Adresse gehört. So
 *   bleiben `https://…/users/x`, `ssh://host/Users/x` und `host:/Users/x`,
 *   wie sie sind.
 * - Oder davor steht `:` als Trenner einer Liste (`PATH=/usr/bin:/home/x`):
 *   Das Wort vor dem Doppelpunkt enthält schon einen `/` und ist keine
 *   Adresse mit `://`.
 * - Oder davor steht `file://localhost`.
 */
const GRENZE =
	'(?:(?<![\\w.:/\\\\%-])' +
	`|(?<=(?:^|[\\s=])(?!${WORT}*://)${WORT}*/${WORT}*:)` +
	'|(?<=file://localhost))';
/** `file:` mit keinem, einem oder zwei Schrägstrichen vor dem des Pfads. */
const FILE = '(?:file:/{0,2})?';
const POSIX_PROFIL = new RegExp(
	`${GRENZE}${FILE}(?:${SEP}System${SEP}Volumes${SEP}Data|${SEP}Volumes${SEP}${NAME})?${SEP}(?:users|home)${SEP}${NAME}`,
	'gi'
);
const POSIX_ROOT = new RegExp(
	`${GRENZE}${FILE}${SEP}(?:private${SEP}var${SEP}|var${SEP})?root(?=$|[\\\\/\\s"'<>|:*?])`,
	'g'
);
const POSIX_VOLUME = new RegExp(`${GRENZE}(${FILE}${SEP}Volumes${SEP})${NAME}`, 'g');
/**
 * Wo ein Laufwerk beginnen darf: am Anfang, nach einem Zeichen, das weder
 * Buchstabe noch Ziffer ist, nach einem Escape wie `\n` oder `\t` und nach
 * einem kodierten Zeichen wie `%22`. In `host:/Users/x` ist `t:` keins.
 */
const LAUFWERK_PROFIL =
	/(^|[^a-z0-9]|\\[a-z]|%[0-9a-f]{2})(?:file:\/{2,3})?[a-z]:[\\/]+(?:users|benutzer)[\\/]+[^\\/"'<>|:*?\r\n]+/gi;

/**
 * Ersetzt jedes Benutzerprofil (`<Laufwerk>:\Users\<name>` oder
 * `…\Benutzer\<name>`, auch mit `/` und als `file:///`; unter macOS und Linux
 * `/Users/<name>`, `/home/<name>`, `/root`) durch `%USERPROFILE%` und den
 * Namen eines Datenträgers unter `/Volumes` durch `%VOLUME%`.
 * Der Rest des Pfads bleibt, damit man den Ort noch erkennt.
 */
export function profilVerbergen(text: string): string {
	// Zuerst die Formen ohne Laufwerk: `file:///Users/<name>` ist kein Laufwerk `e:`.
	return text
		.replace(POSIX_PROFIL, PROFIL_PLATZHALTER)
		.replace(POSIX_ROOT, PROFIL_PLATZHALTER)
		.replace(POSIX_VOLUME, (_, davor: string) => `${davor}${VOLUME_PLATZHALTER}`)
		.replace(LAUFWERK_PROFIL, (_, davor: string) => `${davor}${PROFIL_PLATZHALTER}`);
}
