// dashboard-helfer: das Mac-Hilfsprogramm des Dashboards (Entwurf).
//
// Drei Aufgaben, alle über einen lokalen Socket zum Server, mit HTTP:
//   1. Sperre und Entsperren des Rechners melden.
//   2. Alle paar Sekunden ein Lebenszeichen schicken. Bleibt es aus, sperrt
//      sich das Dashboard.
//   3. Die Schlüssel übergeben, die dem Server fehlen und die dieser Helfer
//      öffnen kann, je unter dem Pfad ihrer Nutzlast.
//
// Der Helfer kann nur sperren, sein Dasein melden und Schlüssel übergeben.
// Eine Anmeldung ersetzt er nie.
import Foundation
import LocalAuthentication

let hilfe = """
dashboard-helfer --socket <pfad> --ordner <ordner> --schluessel <pfad>=<name>=<etikett> [...]
                 [--mit-bestaetigung] [--ohne-chip] [--einmal]

  --socket            der lokale Socket des Servers (Datei helfer im Schlüsselordner)
  --ordner            Ordner dieses Helfers für seine verschlossenen Schlüssel (wird 0700 angelegt)
  --schluessel        ein Schlüssel, den dieser Helfer öffnen kann: Pfad der Nutzlast, Name, Etikett
  --mit-bestaetigung  beim Start einmal nach Fingerabdruck oder Kennwort des Macs fragen
  --ohne-chip         nur zum Proben: Schlüssel im Klartext ablegen, ohne Sicherheitschip
  --einmal            ein Lebenszeichen, fehlende Schlüssel übergeben, dann enden
"""

func ende(_ satz: String, code: Int32 = 2) -> Never {
    FileHandle.standardError.write(Data((satz + "\n").utf8))
    exit(code)
}

func melden(_ satz: String) {
    print(satz)
    fflush(stdout)
}

// ----- Aufruf lesen -----

var socket: String?
var ordnerPfad: String?
var beschreibungen: [Beschreibung] = []
var mitBestaetigung = false
var ohneChip = false
var einmal = false

var args = CommandLine.arguments.dropFirst().makeIterator()
while let a = args.next() {
    switch a {
    case "--socket": socket = args.next()
    case "--ordner": ordnerPfad = args.next()
    case "--schluessel":
        let teile = (args.next() ?? "").split(separator: "=", maxSplits: 2, omittingEmptySubsequences: false).map(String.init)
        guard teile.count == 3, teile.allSatisfy({ !$0.isEmpty }) else { ende("Nach --schluessel fehlt <pfad>=<name>=<etikett>.\n\n\(hilfe)") }
        beschreibungen.append(Beschreibung(pfad: teile[0], name: teile[1], etikett: teile[2]))
    case "--mit-bestaetigung": mitBestaetigung = true
    case "--ohne-chip": ohneChip = true
    case "--einmal": einmal = true
    case "--hilfe", "-h", "--help": print(hilfe); exit(0)
    default: ende("Das Argument „\(a)“ kennt dashboard-helfer nicht.\n\n\(hilfe)")
    }
}
guard let socket, let ordnerPfad else { ende("Es fehlen --socket und --ordner.\n\n\(hilfe)") }
let ordner = URL(fileURLWithPath: ordnerPfad)

// ----- Ordner und Tresor -----

do {
    try FileManager.default.createDirectory(at: ordner, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
} catch {
    ende("Der Ordner \(ordner.path) lässt sich nicht anlegen: \(error.localizedDescription)", code: 9)
}

if mitBestaetigung {
    // Der Dialog von macOS selbst: Fingerabdruck, sonst das Kennwort des Macs.
    let bestaetigt = (try? await LAContext().evaluatePolicy(.deviceOwnerAuthentication, localizedReason: "Das Dashboard-Hilfsprogramm starten")) ?? false
    guard bestaetigt else { ende("Nicht bestätigt. Der Helfer startet nicht.", code: 9) }
}

let tresor: Tresor
if ohneChip {
    melden("ACHTUNG: --ohne-chip. Die Schlüssel liegen im Klartext im Ordner. Nur zum Proben.")
    tresor = OffenerTresor()
} else {
    do {
        let chip = try ChipTresor(ordner: ordner)
        melden("Sicherheitschip bereit, Fingerabdruck des öffentlichen Teils: \(chip.fingerabdruck)")
        tresor = chip
    } catch {
        ende("Der Sicherheitschip lässt sich nicht nutzen: \(error)", code: 9)
    }
}

let bestand = Bestand(ordner: ordner, tresor: tresor, beschreibungen: beschreibungen)
let leitung = Leitung(socket: socket)

// ----- Ein Takt: Lebenszeichen, dann fehlende Schlüssel übergeben -----

@MainActor @discardableResult
func takt() async -> Bool {
    do {
        let antwort = try await leitung.lebenszeichen()
        let kann = antwort.fehlend.filter(bestand.kann)
        if !kann.isEmpty {
            let ergebnis = try await leitung.uebergeben(try bestand.uebergabe(fuer: kann))
            // Nie Werte ausgeben, nur Pfad und Ausgang.
            for e in ergebnis.ergebnis { melden("Schlüssel für \(e.pfad): \(e.stand)") }
        }
        return true
    } catch {
        melden("Lebenszeichen gescheitert: \(error)")
        return false
    }
}

if einmal {
    let gut = await takt()
    exit(gut ? 0 : 9)
}

// ----- Sperre und Entsperren hören -----

let zentrale = DistributedNotificationCenter.default()
zentrale.addObserver(forName: Notification.Name("com.apple.screenIsLocked"), object: nil, queue: .main) { _ in
    melden("Der Rechner ist gesperrt.")
    Task { do { try await leitung.sperre() } catch { melden("Sperre nicht gemeldet: \(error)") } }
}
zentrale.addObserver(forName: Notification.Name("com.apple.screenIsUnlocked"), object: nil, queue: .main) { _ in
    melden("Der Rechner ist entsperrt.")
    Task { do { try await leitung.entsperrt() } catch { melden("Entsperren nicht gemeldet: \(error)") } }
}

// ----- Lebenszeichen alle fünf Sekunden -----
// Während des Wartens bedient das Programm seine Hauptschlange; darüber
// kommen die Meldungen zu Sperre und Entsperren an.

melden("dashboard-helfer läuft: Socket \(socket), \(beschreibungen.count) Schlüssel im Angebot.")
while true {
    await takt()
    try? await Task.sleep(for: .seconds(5))
}
