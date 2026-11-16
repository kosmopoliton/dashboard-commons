// Die Leitung zum Server: HTTP über den lokalen Socket `helfer` im
// Schlüsselordner. Der Helfer ruft immer an; die Antwort kommt auf derselben
// Verbindung zurück. Kein eigenes Leitungsformat.
import AsyncHTTPClient
import Foundation
import NIOCore
import NIOHTTP1

struct Lebenszeichen: Decodable {
    /// Pfade der Nutzlasten, zu denen dem Server ein Schlüssel fehlt.
    let fehlend: [String]
}

struct EinSchluessel: Encodable {
    let pfad: String
    let name: String
    let etikett: String
    /// 32 Bytes, Base64url.
    let wert: String
}

struct Uebergabe: Encodable {
    let schluessel: [EinSchluessel]
}

struct UebergabeAntwort: Decodable {
    struct Ergebnis: Decodable {
        let pfad: String
        let stand: String
    }
    let ergebnis: [Ergebnis]
}

enum LeitungsFehler: Error, CustomStringConvertible {
    case status(UInt, String)
    case adresse

    var description: String {
        switch self {
        case .status(let code, let weg): return "Der Server antwortet auf \(weg) mit \(code)."
        case .adresse: return "Der Pfad des Sockets lässt sich nicht als Adresse schreiben."
        }
    }
}

struct Leitung {
    let socket: String
    private let frist = TimeAmount.seconds(5)

    /// `http+unix://<Pfad des Sockets, kodiert>/<weg>`
    private func adresse(_ weg: String) throws -> String {
        var erlaubt = CharacterSet.urlHostAllowed
        erlaubt.remove(charactersIn: "/")
        guard let kodiert = socket.addingPercentEncoding(withAllowedCharacters: erlaubt) else { throw LeitungsFehler.adresse }
        return "http+unix://\(kodiert)\(weg)"
    }

    private func schicken(_ methode: HTTPMethod, _ weg: String, koerper: Data? = nil) async throws -> Data {
        var anfrage = HTTPClientRequest(url: try adresse(weg))
        anfrage.method = methode
        if let koerper {
            anfrage.headers.add(name: "content-type", value: "application/json")
            anfrage.body = .bytes(ByteBuffer(bytes: koerper))
        }
        let antwort = try await HTTPClient.shared.execute(anfrage, timeout: frist)
        guard (200..<300).contains(antwort.status.code) else { throw LeitungsFehler.status(antwort.status.code, weg) }
        let puffer = try await antwort.body.collect(upTo: 64 * 1024)
        return Data(puffer.readableBytesView)
    }

    func sperre() async throws { _ = try await schicken(.POST, "/sperre") }
    func entsperrt() async throws { _ = try await schicken(.POST, "/entsperrt") }

    func lebenszeichen() async throws -> Lebenszeichen {
        try JSONDecoder().decode(Lebenszeichen.self, from: try await schicken(.POST, "/lebenszeichen"))
    }

    func uebergeben(_ schluessel: [EinSchluessel]) async throws -> UebergabeAntwort {
        let koerper = try JSONEncoder().encode(Uebergabe(schluessel: schluessel))
        return try JSONDecoder().decode(UebergabeAntwort.self, from: try await schicken(.POST, "/schluessel", koerper: koerper))
    }
}

extension Data {
    /// Base64url ohne Auffüllung, wie der Server es liest.
    var base64url: String {
        base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
    }
}
