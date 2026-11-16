// swift-tools-version:5.9
// Entwurf (Zweig mac-helfer): das Mac-Hilfsprogramm des Dashboards.
import PackageDescription

let package = Package(
    name: "dashboard-helfer",
    platforms: [.macOS(.v14)],
    dependencies: [
        // HTTP über den lokalen Socket: Apples eigener HTTP-Client auf SwiftNIO.
        // URLSession kann keinen lokalen Socket ansprechen.
        .package(url: "https://github.com/swift-server/async-http-client.git", from: "1.21.0"),
    ],
    targets: [
        .executableTarget(
            name: "dashboard-helfer",
            dependencies: [.product(name: "AsyncHTTPClient", package: "async-http-client")]
        ),
    ]
)
