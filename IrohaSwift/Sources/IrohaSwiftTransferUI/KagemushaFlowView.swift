import IrohaSwift
import SwiftUI

/// UI-only state. Peer handoff is final at `.sendingCommitted`; there is no
/// pending-acceptance, reconciliation, rollback, or receipt-sync phase.
public enum IrohaKagemushaFlowPhase: Equatable, Sendable {
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

public struct IrohaKagemushaFlowState: Equatable, Sendable {
    public let phase: IrohaKagemushaFlowPhase
    public let spendableBalance: String

    public init(
        phase: IrohaKagemushaFlowPhase,
        spendableBalance: String = "0"
    ) {
        self.phase = phase
        self.spendableBalance = spendableBalance
    }
}

public enum IrohaKagemushaFlowAction: Equatable, Sendable {
    case setup
    /// Online, chain-final public-to-confidential issuance.
    case topUp
    /// Irreversible sender-side peer cash handoff.
    case send
    case receive
    /// Online, chain-final confidential-to-public redemption.
    case redeem
    case selectTransport(IrohaKagemushaTransportKind)
}

public struct IrohaKagemushaTransportPicker: View {
    private let capabilities: IrohaKagemushaCapabilities
    private let selected: IrohaKagemushaTransportKind?
    private let theme: IrohaKagemushaTheme
    private let onSelect: (IrohaKagemushaTransportKind) -> Void

    public init(
        capabilities: IrohaKagemushaCapabilities,
        selected: IrohaKagemushaTransportKind? = nil,
        theme: IrohaKagemushaTheme = .boi,
        onSelect: @escaping (IrohaKagemushaTransportKind) -> Void
    ) {
        self.capabilities = capabilities
        self.selected = selected
        self.theme = theme
        self.onSelect = onSelect
    }

    public var body: some View {
        VStack(spacing: 10) {
            ForEach(capabilities.availableTransports) { kind in
                IrohaKagemushaTransportChoice(
                    kind: kind,
                    subtitle: subtitle(for: kind),
                    isSelected: selected == kind,
                    theme: theme
                ) { onSelect(kind) }
            }
        }
    }

    private func subtitle(for kind: IrohaKagemushaTransportKind) -> String {
        switch kind {
        case .qr: return "Show or scan a canonical ABI-21 payment stream."
        case .nfc: return "Tap phones for an authenticated local handoff."
        case .nearby: return "Pair nearby phones with the matching picture."
        }
    }
}

public struct IrohaKagemushaFlowView: View {
    private let state: IrohaKagemushaFlowState
    private let capabilities: IrohaKagemushaCapabilities
    private let selectedTransport: IrohaKagemushaTransportKind?
    private let currencySymbol: String
    private let theme: IrohaKagemushaTheme
    private let action: (IrohaKagemushaFlowAction) -> Void

    public init(
        state: IrohaKagemushaFlowState,
        capabilities: IrohaKagemushaCapabilities,
        selectedTransport: IrohaKagemushaTransportKind? = nil,
        currencySymbol: String = "₪",
        theme: IrohaKagemushaTheme = .boi,
        action: @escaping (IrohaKagemushaFlowAction) -> Void
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
                Text("Kagemusha")
                    .font(.caption.weight(.medium))
                    .foregroundColor(theme.secondaryText)
                Text("\(currencySymbol)\(state.spendableBalance)")
                    .font(.system(size: 30, weight: .semibold, design: .rounded))
                    .foregroundColor(theme.primaryText)
            }

            if state.phase == .setupRequired {
                Button("Set up Kagemusha") { action(.setup) }
                    .buttonStyle(.borderedProminent)
            } else if !isUnavailable {
                HStack(spacing: 8) {
                    flowButton("Top up", .topUp)
                    flowButton("Pay", .send)
                    flowButton("Receive", .receive)
                    flowButton("Redeem", .redeem)
                }
                IrohaKagemushaTransportPicker(
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
        _ flowAction: IrohaKagemushaFlowAction
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
        case .setupRequired: return "Kagemusha setup"
        case .ready: return "Kagemusha ready"
        case .toppingUp: return "Finalizing top-up"
        case .sendingCommitted: return "Kagemusha payment committed"
        case .receiving: return "Receiving Kagemusha"
        case .redeeming: return "Finalizing redemption"
        case .frozen: return "Kagemusha frozen"
        case .unavailable: return "Kagemusha unavailable"
        case .error: return "Kagemusha needs attention"
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
