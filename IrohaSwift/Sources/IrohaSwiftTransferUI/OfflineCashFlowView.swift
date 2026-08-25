import IrohaSwift
import SwiftUI

/// UI-only state. Peer handoff is final at `.sendingCommitted`; there is no
/// pending-acceptance, reconciliation, rollback, or receipt-sync phase.
public enum IrohaOfflineCashFlowPhase: Equatable, Sendable {
    case setupRequired
    case ready
    case toppingUp
    case sendingCommitted
    case receiving
    case redeeming
    case frozen(String)
    case unavailable(String)
    case error(String)

    public var isBusy: Bool {
        switch self {
        case .toppingUp, .sendingCommitted, .receiving, .redeeming:
            return true
        case .setupRequired, .ready, .frozen, .unavailable, .error:
            return false
        }
    }
}

public struct IrohaOfflineCashFlowState: Equatable, Sendable {
    public let phase: IrohaOfflineCashFlowPhase
    public let spendableBalance: String

    public init(
        phase: IrohaOfflineCashFlowPhase,
        spendableBalance: String = "0"
    ) {
        self.phase = phase
        self.spendableBalance = spendableBalance
    }
}

public enum IrohaOfflineCashFlowAction: Equatable, Sendable {
    case setup
    /// Online, chain-final public-to-confidential issuance.
    case topUp
    /// Irreversible sender-side peer cash handoff.
    case send
    case receive
    /// Online, chain-final confidential-to-public redemption.
    case redeem
    case selectTransport(IrohaOfflineTransferTransportKind)
}

public struct IrohaOfflineTransferTransportPicker: View {
    private let capabilities: IrohaOfflineTransferCapabilities
    private let selected: IrohaOfflineTransferTransportKind?
    private let theme: IrohaOfflineTransferTheme
    private let onSelect: (IrohaOfflineTransferTransportKind) -> Void

    public init(
        capabilities: IrohaOfflineTransferCapabilities,
        selected: IrohaOfflineTransferTransportKind? = nil,
        theme: IrohaOfflineTransferTheme = .boi,
        onSelect: @escaping (IrohaOfflineTransferTransportKind) -> Void
    ) {
        self.capabilities = capabilities
        self.selected = selected
        self.theme = theme
        self.onSelect = onSelect
    }

    public var body: some View {
        VStack(spacing: 10) {
            ForEach(capabilities.availableTransports) { kind in
                IrohaOfflineTransferTransportChoice(
                    kind: kind,
                    subtitle: subtitle(for: kind),
                    isSelected: selected == kind,
                    theme: theme
                ) { onSelect(kind) }
            }
        }
    }

    private func subtitle(for kind: IrohaOfflineTransferTransportKind) -> String {
        switch kind {
        case .qr: return "Show or scan a canonical Kagemusha V4 payment stream."
        case .nfc: return "Tap phones for an authenticated local handoff."
        case .nearby: return "Pair nearby phones with the matching picture."
        }
    }
}

public struct IrohaOfflineCashFlowView: View {
    private let state: IrohaOfflineCashFlowState
    private let capabilities: IrohaOfflineTransferCapabilities
    private let selectedTransport: IrohaOfflineTransferTransportKind?
    private let currencySymbol: String
    private let theme: IrohaOfflineTransferTheme
    private let action: (IrohaOfflineCashFlowAction) -> Void

    public init(
        state: IrohaOfflineCashFlowState,
        capabilities: IrohaOfflineTransferCapabilities,
        selectedTransport: IrohaOfflineTransferTransportKind? = nil,
        currencySymbol: String = "₪",
        theme: IrohaOfflineTransferTheme = .boi,
        action: @escaping (IrohaOfflineCashFlowAction) -> Void
    ) {
        self.state = state
        self.capabilities = capabilities
        self.selectedTransport = selectedTransport
        self.currencySymbol = currencySymbol
        self.theme = theme
        self.action = action
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(spacing: 10) {
                if state.phase.isBusy {
                    ProgressView().controlSize(.small).accessibilityHidden(true)
                }
                VStack(alignment: .leading, spacing: 3) {
                    Text(statusTitle)
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundColor(theme.primaryText)
                    Text(statusSubtitle)
                        .font(.system(size: 13))
                        .foregroundColor(theme.secondaryText)
                }
            }

            VStack(alignment: .leading, spacing: 3) {
                Text("Offline cash")
                    .font(.caption.weight(.medium))
                    .foregroundColor(theme.secondaryText)
                Text("\(currencySymbol)\(state.spendableBalance)")
                    .font(.system(size: 30, weight: .semibold, design: .rounded))
                    .foregroundColor(theme.primaryText)
            }

            if state.phase == .setupRequired {
                Button("Set up offline cash") { action(.setup) }
                    .buttonStyle(.borderedProminent)
            } else if !isUnavailable {
                HStack(spacing: 8) {
                    flowButton("Top up", .topUp)
                    flowButton("Pay", .send)
                    flowButton("Receive", .receive)
                    flowButton("Redeem", .redeem)
                }
                IrohaOfflineTransferTransportPicker(
                    capabilities: capabilities,
                    selected: selectedTransport,
                    theme: theme
                ) { action(.selectTransport($0)) }
            }
        }
        .padding(16)
        .background(theme.surface)
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .stroke(theme.border, lineWidth: 1)
        )
    }

    private func flowButton(
        _ title: String,
        _ flowAction: IrohaOfflineCashFlowAction
    ) -> some View {
        Button(title) { action(flowAction) }
            .buttonStyle(.bordered)
            .disabled(state.phase.isBusy)
    }

    private var isUnavailable: Bool {
        switch state.phase {
        case .frozen, .unavailable: return true
        default: return false
        }
    }

    private var statusTitle: String {
        switch state.phase {
        case .setupRequired: return "Offline cash setup"
        case .ready: return "Offline cash ready"
        case .toppingUp: return "Finalizing top-up"
        case .sendingCommitted: return "Cash handoff committed"
        case .receiving: return "Receiving offline cash"
        case .redeeming: return "Finalizing redemption"
        case .frozen: return "Offline cash frozen"
        case .unavailable: return "Offline cash unavailable"
        case .error: return "Offline cash needs attention"
        }
    }

    private var statusSubtitle: String {
        switch state.phase {
        case .setupRequired:
            return "Create the governed device spend authority before receiving cash."
        case .ready:
            return "Pay and receive without network connectivity using QR, NFC, or Nearby."
        case .toppingUp:
            return "Top-up completes only after online chain finality."
        case .sendingCommitted:
            return "The sender consumed and signed this cash before transport; a receipt cannot roll it back."
        case .receiving:
            return "Verifying and durably storing the exact peer payment."
        case .redeeming:
            return "Redemption completes only after online chain finality."
        case let .frozen(message), let .unavailable(message), let .error(message):
            return message
        }
    }
}
