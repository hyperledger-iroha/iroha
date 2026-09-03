import Combine
import Foundation
import IrohaSwift
import SwiftUI
#if canImport(UIKit)
import UIKit
#elseif canImport(AppKit)
import AppKit
#endif

/// Peer transports supported by KAGEMUSHA V1.
public enum IrohaKagemushaTransportKind: String, CaseIterable, Identifiable, Sendable {
    case qr
    case nfc
    case nearby

    public var id: String { rawValue }

    public var defaultTitle: String {
        switch self {
        case .qr: return "QR"
        case .nfc: return "NFC"
        case .nearby: return "Nearby"
        }
    }

    public var systemImage: String {
        switch self {
        case .qr: return "qrcode"
        case .nfc: return "wave.3.right"
        case .nearby: return "dot.radiowaves.left.and.right"
        }
    }
}

/// App-provided availability snapshot; this type does not infer entitlements.
public struct IrohaKagemushaCapabilities: Equatable, Sendable {
    public let qr: Bool
    public let nfc: Bool
    public let nearby: Bool

    public init(qr: Bool = true, nfc: Bool, nearby: Bool) {
        self.qr = qr
        self.nfc = nfc
        self.nearby = nearby
    }

    public var availableTransports: [IrohaKagemushaTransportKind] {
        IrohaKagemushaTransportKind.allCases.filter {
            switch $0 {
            case .qr: return qr
            case .nfc: return nfc
            case .nearby: return nearby
            }
        }
    }
}

public struct IrohaKagemushaTheme {
    public let background: Color
    public let surface: Color
    public let surfaceAlt: Color
    public let primaryText: Color
    public let secondaryText: Color
    public let accent: Color
    public let warning: Color
    public let success: Color
    public let border: Color

    public init(
        background: Color,
        surface: Color,
        surfaceAlt: Color,
        primaryText: Color,
        secondaryText: Color,
        accent: Color,
        warning: Color,
        success: Color,
        border: Color
    ) {
        self.background = background
        self.surface = surface
        self.surfaceAlt = surfaceAlt
        self.primaryText = primaryText
        self.secondaryText = secondaryText
        self.accent = accent
        self.warning = warning
        self.success = success
        self.border = border
    }

    /// Bank-of-Israel-friendly light theme.
    public static let boi = IrohaKagemushaTheme(
        background: Color(red: 0.96, green: 0.98, blue: 1.00),
        surface: .white,
        surfaceAlt: Color(red: 0.91, green: 0.95, blue: 1.00),
        primaryText: Color(red: 0.03, green: 0.08, blue: 0.14),
        secondaryText: Color(red: 0.24, green: 0.34, blue: 0.48),
        accent: Color(red: 0.03, green: 0.29, blue: 0.67),
        warning: Color(red: 0.88, green: 0.57, blue: 0.04),
        success: Color(red: 0.06, green: 0.55, blue: 0.35),
        border: Color(red: 0.73, green: 0.82, blue: 0.94)
    )

    public static let neutral = IrohaKagemushaTheme(
        background: IrohaKagemushaPlatformColor.systemBackground,
        surface: IrohaKagemushaPlatformColor.secondarySystemBackground,
        surfaceAlt: IrohaKagemushaPlatformColor.tertiarySystemBackground,
        primaryText: IrohaKagemushaPlatformColor.label,
        secondaryText: IrohaKagemushaPlatformColor.secondaryLabel,
        accent: IrohaKagemushaPlatformColor.systemBlue,
        warning: IrohaKagemushaPlatformColor.systemYellow,
        success: IrohaKagemushaPlatformColor.systemGreen,
        border: IrohaKagemushaPlatformColor.separator
    )
}

private enum IrohaKagemushaPlatformColor {
    static var systemBackground: Color {
        #if canImport(UIKit)
        Color(UIColor.systemBackground)
        #elseif canImport(AppKit)
        Color(NSColor.windowBackgroundColor)
        #else
        .white
        #endif
    }

    static var secondarySystemBackground: Color {
        #if canImport(UIKit)
        Color(UIColor.secondarySystemBackground)
        #elseif canImport(AppKit)
        Color(NSColor.controlBackgroundColor)
        #else
        Color(white: 0.96)
        #endif
    }

    static var tertiarySystemBackground: Color {
        #if canImport(UIKit)
        Color(UIColor.tertiarySystemBackground)
        #elseif canImport(AppKit)
        Color(NSColor.underPageBackgroundColor)
        #else
        Color(white: 0.92)
        #endif
    }

    static var label: Color {
        #if canImport(UIKit)
        Color(UIColor.label)
        #elseif canImport(AppKit)
        Color(NSColor.labelColor)
        #else
        .black
        #endif
    }

    static var secondaryLabel: Color {
        #if canImport(UIKit)
        Color(UIColor.secondaryLabel)
        #elseif canImport(AppKit)
        Color(NSColor.secondaryLabelColor)
        #else
        .gray
        #endif
    }

    static var systemBlue: Color {
        #if canImport(UIKit)
        Color(UIColor.systemBlue)
        #elseif canImport(AppKit)
        Color(NSColor.systemBlue)
        #else
        .blue
        #endif
    }

    static var systemYellow: Color {
        #if canImport(UIKit)
        Color(UIColor.systemYellow)
        #elseif canImport(AppKit)
        Color(NSColor.systemYellow)
        #else
        .yellow
        #endif
    }

    static var systemGreen: Color {
        #if canImport(UIKit)
        Color(UIColor.systemGreen)
        #elseif canImport(AppKit)
        Color(NSColor.systemGreen)
        #else
        .green
        #endif
    }

    static var separator: Color {
        #if canImport(UIKit)
        Color(UIColor.separator)
        #elseif canImport(AppKit)
        Color(NSColor.separatorColor)
        #else
        Color.gray.opacity(0.35)
        #endif
    }
}

public struct IrohaKagemushaTransportChoice: View {
    private let kind: IrohaKagemushaTransportKind
    private let title: String
    private let subtitle: String
    private let isSelected: Bool
    private let isDisabled: Bool
    private let theme: IrohaKagemushaTheme
    private let action: () -> Void

    public init(
        kind: IrohaKagemushaTransportKind,
        title: String? = nil,
        subtitle: String,
        isSelected: Bool = false,
        isDisabled: Bool = false,
        theme: IrohaKagemushaTheme = .boi,
        action: @escaping () -> Void
    ) {
        self.kind = kind
        self.title = title ?? kind.defaultTitle
        self.subtitle = subtitle
        self.isSelected = isSelected
        self.isDisabled = isDisabled
        self.theme = theme
        self.action = action
    }

    public var body: some View {
        Button(action: action) {
            HStack(spacing: 12) {
                Image(systemName: kind.systemImage)
                    .font(.system(size: 24, weight: .medium))
                    .foregroundColor(theme.accent)
                    .frame(width: 42)
                VStack(alignment: .leading, spacing: 3) {
                    Text(title)
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundColor(theme.primaryText)
                    Text(subtitle)
                        .font(.system(size: 12))
                        .foregroundColor(theme.secondaryText)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
            }
            .padding(12)
            .background(isSelected ? theme.surfaceAlt : theme.surface)
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .stroke(isSelected ? theme.accent : theme.border, lineWidth: isSelected ? 2 : 1)
            )
            .opacity(isDisabled ? 0.55 : 1)
        }
        .buttonStyle(.plain)
        .disabled(isDisabled)
    }
}

/// Generates either one complete QR envelope or an animated QR stream.
public enum IrohaKagemushaFountainPayloadFrames {
    public static let defaultSingleFrameLimitBytes = 320

    public static func frames(
        for message: IrohaPeerWireMessageV1,
        singleFrameLimitBytes: Int = defaultSingleFrameLimitBytes
    ) throws -> [String] {
        if let direct = try IrohaPeerQRCodecV1.staticCompleteTextCandidate(for: message),
           direct.utf8.count <= max(1, singleFrameLimitBytes) {
            return [direct]
        }
        return try IrohaPeerQRCodecV1.animatedFrameTexts(for: message)
    }
}

public struct IrohaKagemushaFountainPayloadView<Content: View>: View {
    private let message: IrohaPeerWireMessageV1
    private let interval: TimeInterval
    private let singleFrameLimitBytes: Int
    private let content: (String, Int, Int) -> Content
    @State private var frameIndex = 0

    public init(
        message: IrohaPeerWireMessageV1,
        interval: TimeInterval = 0.20,
        singleFrameLimitBytes: Int = IrohaKagemushaFountainPayloadFrames.defaultSingleFrameLimitBytes,
        @ViewBuilder content: @escaping (String, Int, Int) -> Content
    ) {
        self.message = message
        self.interval = interval
        self.singleFrameLimitBytes = singleFrameLimitBytes
        self.content = content
    }

    private var frames: [String] {
        (try? IrohaKagemushaFountainPayloadFrames.frames(
            for: message,
            singleFrameLimitBytes: singleFrameLimitBytes
        )) ?? []
    }

    public var body: some View {
        let visibleFrames = frames
        let index = visibleFrames.indices.contains(frameIndex) ? frameIndex : 0
        let visible = visibleFrames.indices.contains(index) ? visibleFrames[index] : ""
        content(visible, index, visibleFrames.count)
            .onChange(of: message) { _ in frameIndex = 0 }
            .onReceive(
                Timer.publish(every: max(interval, 0.05), on: .main, in: .common).autoconnect()
            ) { _ in
                guard interval > 0, visibleFrames.count > 1 else { return }
                frameIndex = (frameIndex + 1) % visibleFrames.count
            }
    }
}

public enum IrohaKagemushaNearbyPairingSymbol: String, CaseIterable, Codable, Sendable {
    case stars = "nearby_pairing_stars"
    case bird = "nearby_pairing_bird"
    case mask = "nearby_pairing_mask"
}

public struct IrohaKagemushaNearbyPairingChallenge: Equatable, Hashable, Codable, Sendable {
    public let symbol: IrohaKagemushaNearbyPairingSymbol

    public init(symbol: IrohaKagemushaNearbyPairingSymbol) {
        self.symbol = symbol
    }
}

public struct IrohaKagemushaNearbyPairingImageTile: View {
    private let challenge: IrohaKagemushaNearbyPairingChallenge
    private let size: CGFloat
    private let theme: IrohaKagemushaTheme

    public init(
        challenge: IrohaKagemushaNearbyPairingChallenge,
        size: CGFloat,
        theme: IrohaKagemushaTheme = .boi
    ) {
        self.challenge = challenge
        self.size = size
        self.theme = theme
    }

    public var body: some View {
        Image(challenge.symbol.rawValue, bundle: .main)
            .resizable()
            .scaledToFill()
            .frame(width: size, height: size)
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .stroke(theme.border, lineWidth: 1)
            )
            .accessibilityLabel(Text("Nearby pairing image \(challenge.symbol.rawValue)"))
    }
}

public struct IrohaKagemushaNearbyPairingChallengeDisplay: View {
    private let challenge: IrohaKagemushaNearbyPairingChallenge
    private let title: String
    private let tileSize: CGFloat
    private let theme: IrohaKagemushaTheme

    public init(
        challenge: IrohaKagemushaNearbyPairingChallenge,
        title: String,
        tileSize: CGFloat = 112,
        theme: IrohaKagemushaTheme = .boi
    ) {
        self.challenge = challenge
        self.title = title
        self.tileSize = tileSize
        self.theme = theme
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title).font(.headline).foregroundColor(theme.primaryText)
            IrohaKagemushaNearbyPairingImageTile(
                challenge: challenge,
                size: tileSize,
                theme: theme
            )
        }
    }
}

public struct IrohaKagemushaNearbyPairingChallengeChooser: View {
    private let title: String
    private let theme: IrohaKagemushaTheme
    private let onSelect: (IrohaKagemushaNearbyPairingChallenge) -> Void
    private let onCancel: () -> Void

    public init(
        title: String = "Tap the same picture on both phones.",
        theme: IrohaKagemushaTheme = .boi,
        onSelect: @escaping (IrohaKagemushaNearbyPairingChallenge) -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.title = title
        self.theme = theme
        self.onSelect = onSelect
        self.onCancel = onCancel
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title).font(.headline).foregroundColor(theme.primaryText)
            HStack(spacing: 10) {
                ForEach(IrohaKagemushaNearbyPairingSymbol.allCases, id: \.rawValue) { symbol in
                    let challenge = IrohaKagemushaNearbyPairingChallenge(symbol: symbol)
                    Button { onSelect(challenge) } label: {
                        IrohaKagemushaNearbyPairingImageTile(
                            challenge: challenge,
                            size: 78,
                            theme: theme
                        )
                    }
                    .buttonStyle(.plain)
                }
            }
            Button("Cancel", action: onCancel).buttonStyle(.bordered)
        }
    }
}

public enum IrohaKagemushaNfcGuidanceMode: Sendable {
    case send
    case receive
}

public enum IrohaKagemushaNfcExchangePhase: Sendable {
    case idle
    case waiting
    case committing
    case transferring
    case deliveryReceipt
    case failure
}

public struct IrohaKagemushaNfcGuidancePanel: View {
    private let mode: IrohaKagemushaNfcGuidanceMode
    private let phase: IrohaKagemushaNfcExchangePhase
    private let title: String
    private let message: String
    private let theme: IrohaKagemushaTheme

    public init(
        mode: IrohaKagemushaNfcGuidanceMode,
        phase: IrohaKagemushaNfcExchangePhase,
        title: String,
        message: String,
        theme: IrohaKagemushaTheme = .boi
    ) {
        self.mode = mode
        self.phase = phase
        self.title = title
        self.message = message
        self.theme = theme
    }

    public var body: some View {
        HStack(alignment: .top, spacing: 14) {
            Image(systemName: "wave.3.right.circle")
                .font(.system(size: 42))
                .foregroundColor(statusColor)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 5) {
                Text(title).font(.headline).foregroundColor(theme.primaryText)
                Text(message).font(.subheadline).foregroundColor(theme.secondaryText)
                Text(statusText)
                    .font(.caption.weight(.bold))
                    .foregroundColor(statusColor)
            }
            Spacer(minLength: 0)
        }
        .padding(14)
        .background(theme.surface)
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(theme.border, lineWidth: 1)
        )
    }

    private var statusColor: Color {
        switch phase {
        case .deliveryReceipt: return theme.success
        case .failure: return theme.accent
        case .idle: return theme.secondaryText
        case .waiting, .committing, .transferring: return theme.warning
        }
    }

    private var statusText: String {
        switch (mode, phase) {
        case (_, .idle): return "Ready"
        case (.send, .waiting): return "Hold near receiver"
        case (.receive, .waiting): return "Waiting for sender"
        case (.send, .committing): return "Committing KAGEMUSHA handoff"
        case (.receive, .committing): return "Verifying payment"
        case (_, .transferring): return "Transferring committed payment"
        case (_, .deliveryReceipt): return "Delivery receipt received"
        case (_, .failure): return "Needs attention"
        }
    }
}
