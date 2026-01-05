import Foundation
import IrohaSwift

enum Command: String {
    case capture
    case replay
    case inspect
}

let sidByteCount = 32

func decodeSidBase64Url(_ value: String) -> Data? {
    let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else { return nil }
    guard !trimmed.contains("=") else { return nil }
    var base64 = trimmed.replacingOccurrences(of: "-", with: "+")
        .replacingOccurrences(of: "_", with: "/")
    let remainder = base64.count % 4
    if remainder == 1 {
        return nil
    }
    if remainder == 2 {
        base64.append("==")
    } else if remainder == 3 {
        base64.append("=")
    }
    guard let decoded = Data(base64Encoded: base64) else { return nil }
    guard decoded.count == sidByteCount else { return nil }
    return decoded
}

struct CLI {
    let command: Command
    let sessionID: Data
    let diagnosticsRoot: URL
    let bundleDir: URL?

    init?() {
        var args = CommandLine.arguments.dropFirst()
        guard let cmdRaw = args.first, let cmd = Command(rawValue: cmdRaw) else {
            CLI.usage()
            return nil
        }
        args = args.dropFirst()
        guard let sidBase64Url = args.first else {
            CLI.usage()
            return nil
        }
        args = args.dropFirst()
        guard let sid = decodeSidBase64Url(sidBase64Url) else {
            print("invalid sid base64url")
            return nil
        }
        let root = ConnectSessionDiagnostics.defaultRootDirectory()
        let bundle = args.first.map { URL(fileURLWithPath: $0, isDirectory: true) }
        self.command = cmd
        self.sessionID = sid
        self.diagnosticsRoot = root
        self.bundleDir = bundle
    }

    static func usage() {
        print("""
        Usage:
          connect-cli capture <sid_base64url>
          connect-cli replay <sid_base64url> [bundle_dir]
          connect-cli inspect <sid_base64url> [bundle_dir]
        """)
    }
}

@main
struct ConnectCLI {
    static func main() async {
        guard let cli = CLI() else { return }
        switch cli.command {
        case .capture:
            await capture(cli: cli)
        case .replay:
            replay(cli: cli)
        case .inspect:
            inspect(cli: cli)
        }
    }

    private static func capture(cli: CLI) async {
        guard NoritoNativeBridge.shared.isAvailable else {
            print("Norito bridge unavailable; bundle NoritoBridge.xcframework")
            return
        }
        let journal = ConnectQueueJournal(sessionID: cli.sessionID,
                                          configuration: .init(rootDirectory: cli.diagnosticsRoot))
        // Simple stdin reader: hex-encoded ciphertext per line, app->wallet direction.
        print("Enter hex ciphertext lines; CTRL+D to finish.")
        while let line = readLine() {
            guard let data = Data(hexString: line.trimmingCharacters(in: .whitespacesAndNewlines)) else {
                print("invalid hex: \(line)")
                continue
            }
            do {
                try journal.append(direction: .appToWallet, sequence: ConnectQueueJournal.timestampNow(), ciphertext: data)
            } catch {
                print("append failed: \(error)")
            }
        }
        print("Capture complete. Queues stored under \(cli.diagnosticsRoot.path)")
    }

    private static func replay(cli: CLI) {
        let journal = ConnectQueueJournal(sessionID: cli.sessionID,
                                          configuration: .init(rootDirectory: cli.diagnosticsRoot))
        do {
            let app = try journal.records(direction: .appToWallet)
            let wallet = try journal.records(direction: .walletToApp)
            print("app_to_wallet (\(app.count)):")
            for record in app {
                print("  seq=\(record.sequence) len=\(record.ciphertext.count) expires=\(record.expiresAtMs)")
            }
            print("wallet_to_app (\(wallet.count)):")
            for record in wallet {
                print("  seq=\(record.sequence) len=\(record.ciphertext.count) expires=\(record.expiresAtMs)")
            }
        } catch {
            print("replay failed: \(error)")
        }
    }

    private static func inspect(cli: CLI) {
        let diagnostics = ConnectSessionDiagnostics(sessionID: cli.sessionID,
                                                    configuration: .init(rootDirectory: cli.diagnosticsRoot))
        let target = cli.bundleDir ?? cli.diagnosticsRoot.appendingPathComponent("connect-cli-bundle", isDirectory: true)
        do {
            let manifest = try diagnostics.exportJournalBundle(to: target)
            try diagnostics.exportQueueMetrics(to: target.appendingPathComponent("metrics.ndjson"))
            print("Exported bundle to \(target.path)")
            print("Files: \(manifest.files)")
            let snapshot = try diagnostics.snapshot()
            print("Snapshot state: \(snapshot.state), app depth \(snapshot.appToWallet.depth), wallet depth \(snapshot.walletToApp.depth)")
        } catch {
            print("inspect failed: \(error)")
        }
    }
}

private extension Data {
    init?(hexString: String) {
        let clean = hexString.trimmingCharacters(in: .whitespacesAndNewlines)
        var data = Data(capacity: clean.count / 2)
        var idx = clean.startIndex
        while idx < clean.endIndex {
            let next = clean.index(idx, offsetBy: 2)
            guard next <= clean.endIndex else { return nil }
            let byteString = clean[idx..<next]
            guard let byte = UInt8(byteString, radix: 16) else { return nil }
            data.append(byte)
            idx = next
        }
        self = data
    }
}
