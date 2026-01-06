import Foundation

public struct ConnectFlowControlWindow: Sendable {
    public var appToWallet: UInt64
    public var walletToApp: UInt64

    public init(appToWallet: UInt64, walletToApp: UInt64) {
        self.appToWallet = appToWallet
        self.walletToApp = walletToApp
    }
}

final class ConnectFlowController {
    enum FlowError: Error {
        case exhausted(direction: ConnectDirection)

        var direction: ConnectDirection {
            switch self {
            case .exhausted(let dir):
                return dir
            }
        }
    }

    private var tokens: [ConnectDirection: UInt64]
    private let lock = NSLock()

    init(window: ConnectFlowControlWindow) {
        tokens = [
            .appToWallet: window.appToWallet,
            .walletToApp: window.walletToApp
        ]
    }

    func consume(direction: ConnectDirection) throws {
        lock.lock()
        defer { lock.unlock() }
        guard let remaining = tokens[direction], remaining > 0 else {
            throw FlowError.exhausted(direction: direction)
        }
        tokens[direction] = remaining - 1
    }

    func grant(direction: ConnectDirection, tokens amount: UInt64) {
        lock.lock()
        defer { lock.unlock() }
        let current = tokens[direction] ?? 0
        let (sum, overflow) = current.addingReportingOverflow(amount)
        tokens[direction] = overflow ? UInt64.max : sum
    }
}
