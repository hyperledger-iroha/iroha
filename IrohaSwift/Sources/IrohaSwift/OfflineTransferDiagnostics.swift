import Foundation

public enum IrohaOfflineTransferDiagnosticComparison: String, Equatable, Sendable {
    case missing
    case invalid
    case lessThan = "less_than"
    case equal
    case greaterThan = "greater_than"
}

public struct IrohaOfflineTransferPrecheckDiagnostics: Equatable, Sendable {
    public let amountPresent: String
    public let balancePresent: String
    public let lockedPresent: String
    public let spendablePresent: String
    public let maxTxPresent: String
    public let maxBalancePresent: String
    public let amountToSpendable: IrohaOfflineTransferDiagnosticComparison
    public let amountToMaxTx: IrohaOfflineTransferDiagnosticComparison
    public let balanceToMaxBalance: IrohaOfflineTransferDiagnosticComparison

    public var logFields: [String: String] {
        [
            "amount_present": amountPresent,
            "balance_present": balancePresent,
            "locked_present": lockedPresent,
            "spendable_present": spendablePresent,
            "max_tx_present": maxTxPresent,
            "max_balance_present": maxBalancePresent,
            "amount_vs_spendable": amountToSpendable.rawValue,
            "amount_vs_max_tx": amountToMaxTx.rawValue,
            "balance_vs_max_balance": balanceToMaxBalance.rawValue,
        ]
    }
}

public enum IrohaOfflineTransferDiagnosticPolicy {
    public static func presence(_ rawValue: String?) -> String {
        let trimmed = rawValue?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return trimmed.isEmpty ? "missing" : "present"
    }

    public static func redactedIdentifier(_ rawValue: String?, prefixLength: Int = 12) -> String {
        let trimmed = rawValue?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard !trimmed.isEmpty else { return "empty" }
        let digest = IrohaHash.hash(Data(trimmed.utf8))
            .map { String(format: "%02x", $0) }
            .joined()
        return String(digest.prefix(max(1, prefixLength)))
    }

    public static func compareAmountsForDiagnostics(
        _ lhs: String?,
        to rhs: String?
    ) -> IrohaOfflineTransferDiagnosticComparison {
        let lhsValue = diagnosticAmount(lhs)
        let rhsValue = diagnosticAmount(rhs)
        switch (lhsValue, rhsValue) {
        case (.missing, _), (_, .missing):
            return .missing
        case (.invalid, _), (_, .invalid):
            return .invalid
        case (.value(let lhsAmount), .value(let rhsAmount)):
            switch try? ToriiOfflineCashCodec.compareAmounts(lhsAmount, rhsAmount) {
            case .orderedAscending:
                return .lessThan
            case .orderedSame:
                return .equal
            case .orderedDescending:
                return .greaterThan
            case nil:
                return .invalid
            }
        }
    }

    public static func precheckDiagnostics(
        amount: String?,
        balance: String?,
        locked: String?,
        spendable: String?,
        maxTx: String?,
        maxBalance: String?
    ) -> IrohaOfflineTransferPrecheckDiagnostics {
        IrohaOfflineTransferPrecheckDiagnostics(
            amountPresent: presence(amount),
            balancePresent: presence(balance),
            lockedPresent: presence(locked),
            spendablePresent: presence(spendable),
            maxTxPresent: presence(maxTx),
            maxBalancePresent: presence(maxBalance),
            amountToSpendable: compareAmountsForDiagnostics(amount, to: spendable),
            amountToMaxTx: compareAmountsForDiagnostics(amount, to: maxTx),
            balanceToMaxBalance: compareAmountsForDiagnostics(balance, to: maxBalance)
        )
    }

    private enum DiagnosticAmount {
        case missing
        case invalid
        case value(String)
    }

    private static func diagnosticAmount(_ rawValue: String?) -> DiagnosticAmount {
        let trimmed = rawValue?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard !trimmed.isEmpty else { return .missing }
        guard let canonicalAmount = try? ToriiOfflineCashCodec.canonicalAmountString(trimmed),
              let zeroComparison = try? ToriiOfflineCashCodec.compareAmounts(canonicalAmount, "0"),
              zeroComparison != .orderedAscending else {
            return .invalid
        }
        return .value(canonicalAmount)
    }
}
