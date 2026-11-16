// Die Schlüssel, die dieser Helfer öffnen kann. Jeder steht unter dem Pfad
// der Nutzlast, die er öffnet, und trägt einen Namen und ein Etikett. Ein
// Helfer muss nicht jeden Schlüssel haben.
//
// Auf der Platte liegt je Pfad nur eine verschlossene Datei. Öffnen kann sie
// nur der Sicherheitschip dieses Macs (Secure Enclave), über das fertige
// Verfahren HPKE aus CryptoKit. Eine Kopie der Dateien nützt auf einem
// anderen Gerät nichts.
import CryptoKit
import Foundation

struct Beschreibung {
    let pfad: String
    let name: String
    let etikett: String
}

/// Wer Schlüssel verschließt und öffnet.
protocol Tresor {
    /// Verschließt 32 Bytes für diesen Tresor.
    func verschliessen(_ klartext: Data, zweck: Data) throws -> Data
    /// Öffnet, was `verschliessen` geschrieben hat.
    func oeffnen(_ verschlossen: Data, zweck: Data) throws -> Data
}

enum TresorFehler: Error, CustomStringConvertible {
    case keinChip
    case format

    var description: String {
        switch self {
        case .keinChip: return "Dieser Mac hat keinen erreichbaren Sicherheitschip."
        case .format: return "Die verschlossene Datei hat nicht die erwartete Form."
        }
    }
}

/// Der Tresor mit dem Sicherheitschip. Der Chip-Schlüssel verlässt den Chip
/// nie; in `chip-schluessel.bin` liegt nur sein Verweis, der nur auf diesem
/// Gerät gilt. Keine Anwesenheit des Nutzers verlangt: Der Chip fragt nie nach.
struct ChipTresor: Tresor {
    private let chip: SecureEnclave.P256.KeyAgreement.PrivateKey
    private static let verfahren = HPKE.Ciphersuite.P256_SHA256_AES_GCM_256
    /// Länge des gekapselten Schlüssels bei P-256 (unkomprimierter Punkt).
    private static let kapselLaenge = 65

    init(ordner: URL) throws {
        guard SecureEnclave.isAvailable else { throw TresorFehler.keinChip }
        let datei = ordner.appendingPathComponent("chip-schluessel.bin")
        if let verweis = try? Data(contentsOf: datei) {
            chip = try SecureEnclave.P256.KeyAgreement.PrivateKey(dataRepresentation: verweis)
        } else {
            var fehler: Unmanaged<CFError>?
            guard let zugriff = SecAccessControlCreateWithFlags(nil, kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly, [.privateKeyUsage], &fehler) else {
                throw TresorFehler.keinChip
            }
            chip = try SecureEnclave.P256.KeyAgreement.PrivateKey(accessControl: zugriff)
            try chip.dataRepresentation.write(to: datei, options: [.atomic])
            try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: datei.path)
        }
    }

    /// Fingerabdruck des öffentlichen Teils, zum Wiedererkennen; nie geheim.
    var fingerabdruck: String {
        SHA256.hash(data: chip.publicKey.x963Representation).prefix(8).map { String(format: "%02x", $0) }.joined()
    }

    func verschliessen(_ klartext: Data, zweck: Data) throws -> Data {
        var sender = try HPKE.Sender(recipientKey: chip.publicKey, ciphersuite: Self.verfahren, info: zweck)
        let geheim = try sender.seal(klartext, authenticating: zweck)
        return sender.encapsulatedKey + geheim
    }

    func oeffnen(_ verschlossen: Data, zweck: Data) throws -> Data {
        guard verschlossen.count > Self.kapselLaenge else { throw TresorFehler.format }
        let kapsel = verschlossen.prefix(Self.kapselLaenge)
        let geheim = verschlossen.dropFirst(Self.kapselLaenge)
        var empfaenger = try HPKE.Recipient(privateKey: chip, ciphersuite: Self.verfahren, info: zweck, encapsulatedKey: kapsel)
        return try empfaenger.open(geheim, authenticating: zweck)
    }
}

/// Nur zum Proben ohne Chip (`--ohne-chip`): Die Dateien enthalten den
/// Klartext. Schützt nichts und sagt das beim Start.
struct OffenerTresor: Tresor {
    func verschliessen(_ klartext: Data, zweck: Data) throws -> Data { klartext }
    func oeffnen(_ verschlossen: Data, zweck: Data) throws -> Data { verschlossen }
}

/// Der Bestand dieses Helfers: je Pfad eine verschlossene Datei im Ordner.
struct Bestand {
    let ordner: URL
    let tresor: Tresor
    let beschreibungen: [Beschreibung]

    /// Der Zweck bindet die verschlossene Datei an ihren Pfad: Eine Datei
    /// lässt sich nicht für einen anderen Pfad unterschieben.
    private func zweck(_ pfad: String) -> Data {
        Data("dashboard-helfer/schluessel/v1/\(pfad)".utf8)
    }

    /// Dateiname aus dem Pfad, ohne dessen Zeichen in den Namen zu legen.
    private func datei(_ pfad: String) -> URL {
        let name = SHA256.hash(data: Data(pfad.utf8)).map { String(format: "%02x", $0) }.joined()
        return ordner.appendingPathComponent("\(name).schluessel")
    }

    func kann(_ pfad: String) -> Bool {
        beschreibungen.contains { $0.pfad == pfad }
    }

    /// Öffnet den Schlüssel für `pfad`. Gibt es noch keinen, entsteht einer:
    /// 32 Bytes Zufall, sofort verschlossen abgelegt.
    func schluessel(_ pfad: String) throws -> Data {
        let ort = datei(pfad)
        if let verschlossen = try? Data(contentsOf: ort) {
            let klartext = try tresor.oeffnen(verschlossen, zweck: zweck(pfad))
            guard klartext.count == 32 else { throw TresorFehler.format }
            return klartext
        }
        let neu = SymmetricKey(size: .bits256).withUnsafeBytes { Data($0) }
        try tresor.verschliessen(neu, zweck: zweck(pfad)).write(to: ort, options: [.atomic])
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: ort.path)
        return neu
    }

    /// Was dieser Helfer zu den fehlenden Pfaden übergeben kann.
    func uebergabe(fuer fehlend: [String]) throws -> [EinSchluessel] {
        try beschreibungen.filter { fehlend.contains($0.pfad) }.map {
            EinSchluessel(pfad: $0.pfad, name: $0.name, etikett: $0.etikett, wert: try schluessel($0.pfad).base64url)
        }
    }
}
