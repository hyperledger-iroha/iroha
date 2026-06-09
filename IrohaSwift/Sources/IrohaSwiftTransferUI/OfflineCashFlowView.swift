import Foundation
import IrohaSwift
import SwiftUI

public enum IrohaOfflineCashFlowPhase: Equatable, Sendable {
    case setupRequired
    case ready
    case loading
    case sending
    case receiving
    case syncing
    case redeeming
    case frozen(String)
    case unavailable(String)
    case error(String)

    public var isBusy: Bool {
        switch self {
        case .loading, .sending, .receiving, .syncing, .redeeming:
            return true
        case .setupRequired, .ready, .frozen, .unavailable, .error:
            return false
        }
    }
}

public struct IrohaOfflineCashFlowState: Equatable, Sendable {
    public let phase: IrohaOfflineCashFlowPhase
    public let totalBalance: String
    public let spendableBalance: String
    public let pendingBalance: String
    public let pendingReceiptCount: Int
    public let latestReceivePayload: String?
    public let latestPaymentPayload: String?

    public init(
        phase: IrohaOfflineCashFlowPhase,
        totalBalance: String = "0",
        spendableBalance: String = "0",
        pendingBalance: String = "0",
        pendingReceiptCount: Int = 0,
        latestReceivePayload: String? = nil,
        latestPaymentPayload: String? = nil
    ) {
        self.phase = phase
        self.totalBalance = totalBalance
        self.spendableBalance = spendableBalance
        self.pendingBalance = pendingBalance
        self.pendingReceiptCount = pendingReceiptCount
        self.latestReceivePayload = latestReceivePayload
        self.latestPaymentPayload = latestPaymentPayload
    }
}

public enum IrohaOfflineCashFlowAction: Equatable, Sendable {
    case setup
    case load
    case send
    case receive
    case redeem
    case sync
    case refreshReceivePayload
    case scanTransfer
    case selectTransport(IrohaOfflineTransferTransportKind)
}

public struct IrohaOfflineTransferTransportPicker: View {
    private let capabilities: OfflineNoteTransferCapabilities
    private let selected: IrohaOfflineTransferTransportKind?
    private let isActive: Bool
    private let theme: IrohaOfflineTransferTheme
    private let onSelect: (IrohaOfflineTransferTransportKind) -> Void

    public init(
        capabilities: OfflineNoteTransferCapabilities,
        selected: IrohaOfflineTransferTransportKind? = nil,
        isActive: Bool = false,
        theme: IrohaOfflineTransferTheme = .neutral,
        onSelect: @escaping (IrohaOfflineTransferTransportKind) -> Void
    ) {
        self.capabilities = capabilities
        self.selected = selected
        self.isActive = isActive
        self.theme = theme
        self.onSelect = onSelect
    }

    public var body: some View {
        VStack(spacing: 10) {
            ForEach(IrohaOfflineTransferTransportKind.available(in: capabilities)) { kind in
                IrohaOfflineTransferTransportChoice(
                    kind: kind,
                    subtitle: subtitle(for: kind),
                    isSelected: selected == kind,
                    isActive: isActive && selected == kind,
                    theme: theme
                ) {
                    onSelect(kind)
                }
            }
        }
    }

    private func subtitle(for kind: IrohaOfflineTransferTransportKind) -> String {
        switch kind {
        case .qr:
            return "Show or scan an offline transfer QR."
        case .nearby:
            return "Use local nearby pairing when both devices are present."
        case .nfc:
            return "Use NFC only when the device and app entitlement support it."
        }
    }
}

public struct IrohaOfflineCashFlowView: View {
    private let state: IrohaOfflineCashFlowState
    private let capabilities: OfflineNoteTransferCapabilities
    private let selectedTransport: IrohaOfflineTransferTransportKind?
    private let theme: IrohaOfflineTransferTheme
    private let action: (IrohaOfflineCashFlowAction) -> Void

    public init(
        state: IrohaOfflineCashFlowState,
        capabilities: OfflineNoteTransferCapabilities,
        selectedTransport: IrohaOfflineTransferTransportKind? = nil,
        theme: IrohaOfflineTransferTheme = .neutral,
        action: @escaping (IrohaOfflineCashFlowAction) -> Void
    ) {
        self.state = state
        self.capabilities = capabilities
        self.selectedTransport = selectedTransport
        self.theme = theme
        self.action = action
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            statusBanner
            balanceGrid

            switch state.phase {
            case .setupRequired:
                primaryButton("Set Up", action: .setup)
            case .unavailable, .frozen:
                EmptyView()
            default:
                actionGrid
                IrohaOfflineTransferTransportPicker(
                    capabilities: capabilities,
                    selected: selectedTransport,
                    isActive: state.phase == .sending || state.phase == .receiving,
                    theme: theme
                ) { transport in
                    action(.selectTransport(transport))
                }
            }
        }
        .padding(16)
        .background(theme.surface)
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .stroke(theme.border, lineWidth: 1)
        )
    }

    private var statusBanner: some View {
        HStack(spacing: 10) {
            if state.phase.isBusy {
                ProgressView()
                    .controlSize(.small)
                    .accessibilityHidden(true)
            }
            VStack(alignment: .leading, spacing: 3) {
                Text(statusTitle)
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundColor(theme.primaryText)
                Text(statusSubtitle)
                    .font(.system(size: 13))
                    .foregroundColor(theme.secondaryText)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private var balanceGrid: some View {
        HStack(spacing: 12) {
            metric("Total", value: state.totalBalance)
            metric("Spendable", value: state.spendableBalance)
            metric("Pending", value: state.pendingBalance)
        }
    }

    private var actionGrid: some View {
        HStack(spacing: 10) {
            primaryButton("Load", action: .load)
            secondaryButton("Receive", action: .receive)
            secondaryButton("Send", action: .send)
            secondaryButton("Redeem", action: .redeem)
            if state.pendingReceiptCount > 0 {
                secondaryButton("Sync", action: .sync)
            }
        }
    }

    private func metric(_ title: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(title)
                .font(.system(size: 11, weight: .medium))
                .foregroundColor(theme.secondaryText)
            Text(value)
                .font(.system(size: 14, weight: .semibold))
                .foregroundColor(theme.primaryText)
                .lineLimit(1)
                .minimumScaleFactor(0.75)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func primaryButton(_ title: String, action flowAction: IrohaOfflineCashFlowAction) -> some View {
        Button(title) {
            action(flowAction)
        }
        .buttonStyle(.borderedProminent)
        .disabled(state.phase.isBusy)
    }

    private func secondaryButton(_ title: String, action flowAction: IrohaOfflineCashFlowAction) -> some View {
        Button(title) {
            action(flowAction)
        }
        .buttonStyle(.bordered)
        .disabled(state.phase.isBusy)
    }

    private var statusTitle: String {
        switch state.phase {
        case .setupRequired:
            return "Offline Cash Setup"
        case .ready:
            return "Offline Cash Ready"
        case .loading:
            return "Loading Offline Cash"
        case .sending:
            return "Sending Offline Cash"
        case .receiving:
            return "Receiving Offline Cash"
        case .syncing:
            return "Syncing Offline Cash"
        case .redeeming:
            return "Redeeming Offline Cash"
        case .frozen:
            return "Offline Cash Frozen"
        case .unavailable:
            return "Offline Cash Unavailable"
        case .error:
            return "Offline Cash Needs Attention"
        }
    }

    private var statusSubtitle: String {
        switch state.phase {
        case .setupRequired:
            return "Create a device-bound authority before loading or receiving cash."
        case .ready:
            return "Use QR, Nearby, or NFC when available for this device and app build."
        case .loading:
            return "Pending audit receipts are synced before more cash is issued."
        case .sending:
            return "Keep both devices on the selected local transfer path."
        case .receiving:
            return "Keep this screen open until the sender finishes."
        case .syncing:
            return "Submitting pending audit receipts."
        case .redeeming:
            return "Moving spendable offline cash back online."
        case let .frozen(message), let .unavailable(message), let .error(message):
            return message
        }
    }
}
