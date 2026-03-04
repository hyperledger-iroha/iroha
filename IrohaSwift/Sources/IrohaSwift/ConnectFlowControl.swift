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
    private let queue = DispatchQueue(label: "org.hyperledger.iroha.connect-flow-control")

    init(window: ConnectFlowControlWindow) {
        tokens = [
            .appToWallet: window.appToWallet,
            .walletToApp: window.walletToApp
        ]
    }

    func consume(direction: ConnectDirection) throws {
        let error: FlowError? = queue.sync {
            guard let remaining = tokens[direction], remaining > 0 else {
                return FlowError.exhausted(direction: direction)
            }
            tokens[direction] = remaining - 1
            return nil
        }
        if let error {
            throw error
        }
    }

    func grant(direction: ConnectDirection, tokens amount: UInt64) {
        queue.sync {
            let current = tokens[direction] ?? 0
            let (sum, overflow) = current.addingReportingOverflow(amount)
            tokens[direction] = overflow ? UInt64.max : sum
        }
    }
}
