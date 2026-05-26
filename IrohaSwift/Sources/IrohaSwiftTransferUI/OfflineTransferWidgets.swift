import Foundation
import IrohaSwift
import SwiftUI
#if canImport(UIKit)
import UIKit
#elseif canImport(AppKit)
import AppKit
#endif

public enum IrohaOfflineTransferTransportKind: String, CaseIterable, Identifiable, Sendable {
    case qr
    case nfc
    case nearby

    public var id: String { rawValue }

    public var defaultTitle: String {
        switch self {
        case .qr:
            return "QR"
        case .nfc:
            return "NFC"
        case .nearby:
            return "Nearby"
        }
    }

    public var systemImage: String {
        switch self {
        case .qr:
            return "qrcode"
        case .nfc:
            return "wave.3.right"
        case .nearby:
            return "dot.radiowaves.left.and.right"
        }
    }
}

public struct IrohaOfflineTransferTheme {
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

    public static let bpngStyle = IrohaOfflineTransferTheme(
        background: Color(red: 0.03, green: 0.03, blue: 0.03),
        surface: Color(red: 0.08, green: 0.08, blue: 0.08),
        surfaceAlt: Color(red: 0.16, green: 0.10, blue: 0.07),
        primaryText: .white,
        secondaryText: Color.white.opacity(0.72),
        accent: Color(red: 0.91, green: 0.04, blue: 0.06),
        warning: Color(red: 0.98, green: 0.74, blue: 0.08),
        success: Color(red: 0.10, green: 0.72, blue: 0.44),
        border: Color.white.opacity(0.14)
    )

    public static let neutral = IrohaOfflineTransferTheme(
        background: IrohaOfflinePlatformColor.systemBackground,
        surface: IrohaOfflinePlatformColor.secondarySystemBackground,
        surfaceAlt: IrohaOfflinePlatformColor.tertiarySystemBackground,
        primaryText: IrohaOfflinePlatformColor.label,
        secondaryText: IrohaOfflinePlatformColor.secondaryLabel,
        accent: IrohaOfflinePlatformColor.systemBlue,
        warning: IrohaOfflinePlatformColor.systemYellow,
        success: IrohaOfflinePlatformColor.systemGreen,
        border: IrohaOfflinePlatformColor.separator
    )
}

private enum IrohaOfflinePlatformColor {
    static var systemBackground: Color {
        #if canImport(UIKit)
        Color(UIColor.systemBackground)
        #elseif canImport(AppKit)
        Color(NSColor.windowBackgroundColor)
        #else
        Color.white
        #endif
    }

    static var secondarySystemBackground: Color {
        #if canImport(UIKit)
        Color(UIColor.secondarySystemBackground)
        #elseif canImport(AppKit)
        Color(NSColor.controlBackgroundColor)
        #else
        Color(red: 0.96, green: 0.96, blue: 0.96)
        #endif
    }

    static var tertiarySystemBackground: Color {
        #if canImport(UIKit)
        Color(UIColor.tertiarySystemBackground)
        #elseif canImport(AppKit)
        Color(NSColor.underPageBackgroundColor)
        #else
        Color(red: 0.92, green: 0.92, blue: 0.92)
        #endif
    }

    static var label: Color {
        #if canImport(UIKit)
        Color(UIColor.label)
        #elseif canImport(AppKit)
        Color(NSColor.labelColor)
        #else
        Color.black
        #endif
    }

    static var secondaryLabel: Color {
        #if canImport(UIKit)
        Color(UIColor.secondaryLabel)
        #elseif canImport(AppKit)
        Color(NSColor.secondaryLabelColor)
        #else
        Color.gray
        #endif
    }

    static var systemBlue: Color {
        #if canImport(UIKit)
        Color(UIColor.systemBlue)
        #elseif canImport(AppKit)
        Color(NSColor.systemBlue)
        #else
        Color.blue
        #endif
    }

    static var systemYellow: Color {
        #if canImport(UIKit)
        Color(UIColor.systemYellow)
        #elseif canImport(AppKit)
        Color(NSColor.systemYellow)
        #else
        Color.yellow
        #endif
    }

    static var systemGreen: Color {
        #if canImport(UIKit)
        Color(UIColor.systemGreen)
        #elseif canImport(AppKit)
        Color(NSColor.systemGreen)
        #else
        Color.green
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

public struct IrohaOfflineTransferTransportChoice: View {
    private let kind: IrohaOfflineTransferTransportKind
    private let title: String
    private let subtitle: String
    private let status: String?
    private let isSelected: Bool
    private let isActive: Bool
    private let isDisabled: Bool
    private let theme: IrohaOfflineTransferTheme
    private let action: () -> Void

    public init(
        kind: IrohaOfflineTransferTransportKind,
        title: String? = nil,
        subtitle: String,
        status: String? = nil,
        isSelected: Bool = false,
        isActive: Bool = false,
        isDisabled: Bool = false,
        theme: IrohaOfflineTransferTheme = .bpngStyle,
        action: @escaping () -> Void
    ) {
        self.kind = kind
        self.title = title ?? kind.defaultTitle
        self.subtitle = subtitle
        self.status = status
        self.isSelected = isSelected
        self.isActive = isActive
        self.isDisabled = isDisabled
        self.theme = theme
        self.action = action
    }

    public var body: some View {
        Button(action: action) {
            HStack(spacing: 14) {
                IrohaOfflineTransferTransportAnimation(
                    kind: kind,
                    isActive: isActive || isSelected,
                    theme: theme
                )
                .frame(width: 72, height: 58)
                .accessibilityHidden(true)

                VStack(alignment: .leading, spacing: 5) {
                    HStack(spacing: 8) {
                        Label(title, systemImage: kind.systemImage)
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundColor(theme.primaryText)
                            .lineLimit(1)
                            .minimumScaleFactor(0.78)

                        if let status {
                            Text(status)
                                .font(.system(size: 10, weight: .bold))
                                .foregroundColor(isActive ? theme.background : theme.warning)
                                .padding(.horizontal, 7)
                                .padding(.vertical, 3)
                                .background(
                                    Capsule()
                                        .fill(isActive ? theme.warning : theme.warning.opacity(0.16))
                                )
                                .lineLimit(1)
                        }
                    }

                    Text(subtitle)
                        .font(.system(size: 12))
                        .foregroundColor(theme.secondaryText)
                        .lineLimit(2)
                        .fixedSize(horizontal: false, vertical: true)
                }

                Spacer(minLength: 0)
            }
            .padding(12)
            .frame(maxWidth: .infinity, minHeight: 86, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(isSelected ? theme.surfaceAlt : theme.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .stroke(isSelected ? theme.warning : theme.border, lineWidth: isSelected ? 1.5 : 1)
            )
            .opacity(isDisabled ? 0.55 : 1)
        }
        .buttonStyle(.plain)
        .disabled(isDisabled)
    }
}

public struct IrohaOfflineTransferTransportAnimation: View {
    private let kind: IrohaOfflineTransferTransportKind
    private let isActive: Bool
    private let theme: IrohaOfflineTransferTheme

    public init(
        kind: IrohaOfflineTransferTransportKind,
        isActive: Bool,
        theme: IrohaOfflineTransferTheme = .bpngStyle
    ) {
        self.kind = kind
        self.isActive = isActive
        self.theme = theme
    }

    public var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30.0, paused: !isActive)) { timeline in
            let phase = isActive ? timeline.date.timeIntervalSinceReferenceDate : 0
            ZStack {
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(theme.background)
                    .overlay(
                        RoundedRectangle(cornerRadius: 8, style: .continuous)
                            .stroke(theme.border, lineWidth: 1)
                    )

                switch kind {
                case .qr:
                    qrGlyph(phase: phase)
                case .nfc:
                    nfcGlyph(phase: phase)
                case .nearby:
                    nearbyGlyph(phase: phase)
                }
            }
        }
    }

    private func qrGlyph(phase: TimeInterval) -> some View {
        let offset = CGFloat((sin(phase * 2.6) + 1) / 2) * 32 - 16
        return ZStack {
            VStack(spacing: 4) {
                ForEach(0..<3, id: \.self) { row in
                    HStack(spacing: 4) {
                        ForEach(0..<3, id: \.self) { column in
                            RoundedRectangle(cornerRadius: 2)
                                .fill((row + column).isMultiple(of: 2) ? theme.warning : theme.primaryText.opacity(0.72))
                                .frame(width: 7, height: 7)
                        }
                    }
                }
            }
            Rectangle()
                .fill(theme.accent)
                .frame(width: 42, height: 2)
                .offset(y: offset)
                .shadow(color: theme.accent.opacity(0.7), radius: 4)
        }
    }

    private func nfcGlyph(phase: TimeInterval) -> some View {
        let pulse = CGFloat((sin(phase * 4) + 1) / 2)
        return ZStack {
            HStack(spacing: 7) {
                phoneShape(color: theme.primaryText.opacity(0.90))
                    .rotationEffect(.degrees(-8))
                phoneShape(color: theme.warning)
                    .rotationEffect(.degrees(8))
            }
            ForEach(0..<3, id: \.self) { index in
                let opacity = max(0.12, 0.46 - Double(index) * 0.12 - Double(pulse) * 0.20)
                let size = 28 + CGFloat(index) * 10 + pulse * 8
                Circle()
                    .stroke(theme.accent.opacity(opacity), lineWidth: 2)
                    .frame(width: size, height: size)
            }
        }
    }

    private func nearbyGlyph(phase: TimeInterval) -> some View {
        let pulse = CGFloat((sin(phase * 3.2) + 1) / 2)
        return ZStack {
            HStack(spacing: 18) {
                phoneShape(color: theme.primaryText.opacity(0.88))
                phoneShape(color: theme.warning)
            }
            Circle()
                .fill(theme.accent.opacity(0.26 + 0.20 * pulse))
                .frame(width: 18 + pulse * 5, height: 18 + pulse * 5)
            Image(systemName: "checkmark")
                .font(.system(size: 10, weight: .bold))
                .foregroundColor(theme.primaryText)
        }
    }

    private func phoneShape(color: Color) -> some View {
        RoundedRectangle(cornerRadius: 5, style: .continuous)
            .fill(color)
            .frame(width: 18, height: 34)
            .overlay(
                RoundedRectangle(cornerRadius: 3)
                    .fill(theme.background.opacity(0.82))
                    .padding(3)
            )
    }
}

public enum IrohaOfflineFountainPayloadFrames {
    public static let defaultSingleFrameLimitBytes = 320

    public static func frames(
        for payload: String,
        singleFrameLimitBytes: Int = defaultSingleFrameLimitBytes,
        options: OfflineQrStreamOptions = OfflineQrStreamOptions(chunkSize: 128, parityGroup: 4)
    ) -> [String] {
        let bytes = Data(payload.utf8)
        guard bytes.count > singleFrameLimitBytes else {
            return [payload]
        }
        guard let frameBytes = try? OfflineQrStreamEncoder.encodeFrameBytes(
            payload: bytes,
            payloadKind: OfflineNoteTransferTextPayloadCodec.payloadKind(for: payload),
            options: options
        ) else {
            return [payload]
        }
        return frameBytes.map {
            OfflineQrStreamTextCodec.encode($0, encoding: .base64)
        }
    }
}

public struct IrohaOfflineFountainPayloadView<Content: View>: View {
    private let payload: String
    private let interval: TimeInterval
    private let singleFrameLimitBytes: Int
    private let options: OfflineQrStreamOptions
    private let content: (String, Int, Int) -> Content
    @State private var frameIndex = 0

    public init(
        payload: String,
        interval: TimeInterval = TimeInterval(OfflineNoteTransferHandoff.qrFrameCadenceMs) / 1000,
        singleFrameLimitBytes: Int = IrohaOfflineFountainPayloadFrames.defaultSingleFrameLimitBytes,
        options: OfflineQrStreamOptions = OfflineQrStreamOptions(chunkSize: 128, parityGroup: 4),
        @ViewBuilder content: @escaping (String, Int, Int) -> Content
    ) {
        self.payload = payload
        self.interval = interval
        self.singleFrameLimitBytes = singleFrameLimitBytes
        self.options = options
        self.content = content
    }

    private var frames: [String] {
        IrohaOfflineFountainPayloadFrames.frames(
            for: payload,
            singleFrameLimitBytes: singleFrameLimitBytes,
            options: options
        )
    }

    public var body: some View {
        let visibleFrames = frames
        let currentPayload = visibleFrames.indices.contains(frameIndex) ? visibleFrames[frameIndex] : payload
        content(currentPayload, frameIndex, visibleFrames.count)
            .onChange(of: payload) { _ in
                frameIndex = 0
            }
            .onReceive(Timer.publish(every: max(interval, 0.05), on: .main, in: .common).autoconnect()) { _ in
                let count = frames.count
                guard interval > 0, count > 1 else { return }
                frameIndex = (frameIndex + 1) % count
            }
    }
}

public struct IrohaOfflineNearbyPairingChallengeDisplay: View {
    private let challenge: OfflineNoteNearbyPairingChallenge
    private let title: String
    private let tileSize: CGFloat
    private let theme: IrohaOfflineTransferTheme

    public init(
        challenge: OfflineNoteNearbyPairingChallenge,
        title: String,
        tileSize: CGFloat = 112,
        theme: IrohaOfflineTransferTheme = .bpngStyle
    ) {
        self.challenge = challenge
        self.title = title
        self.tileSize = tileSize
        self.theme = theme
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title)
                .font(.system(size: 14, weight: .semibold))
                .foregroundColor(theme.primaryText)
            IrohaOfflineNearbyPairingImageTile(challenge: challenge, size: tileSize, theme: theme)
        }
    }
}

public struct IrohaOfflineNearbyPairingChallengeChooser: View {
    private let challenge: OfflineNoteNearbyPairingChallenge?
    private let title: String
    private let cancelTitle: String
    private let theme: IrohaOfflineTransferTheme
    private let onSelect: (OfflineNoteNearbyPairingChallenge) -> Void
    private let onCancel: () -> Void

    public init(
        challenge: OfflineNoteNearbyPairingChallenge?,
        title: String = "Tap the same Nearby picture on this phone.",
        cancelTitle: String = "Cancel Nearby Transfer",
        theme: IrohaOfflineTransferTheme = .bpngStyle,
        onSelect: @escaping (OfflineNoteNearbyPairingChallenge) -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.challenge = challenge
        self.title = title
        self.cancelTitle = cancelTitle
        self.theme = theme
        self.onSelect = onSelect
        self.onCancel = onCancel
    }

    public var body: some View {
        if challenge != nil {
            VStack(alignment: .leading, spacing: 12) {
                Text(title)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundColor(theme.primaryText)

                HStack(spacing: 10) {
                    ForEach(OfflineNoteNearbyPairingChallenge.allChoices, id: \.assetName) { choice in
                        Button {
                            onSelect(choice)
                        } label: {
                            IrohaOfflineNearbyPairingImageTile(challenge: choice, size: 78, theme: theme)
                        }
                        .buttonStyle(.plain)
                    }
                }

                Button(cancelTitle, action: onCancel)
                    .buttonStyle(.bordered)
            }
        }
    }
}

public struct IrohaOfflineNearbyPairingImageTile: View {
    private let challenge: OfflineNoteNearbyPairingChallenge
    private let size: CGFloat
    private let theme: IrohaOfflineTransferTheme

    public init(
        challenge: OfflineNoteNearbyPairingChallenge,
        size: CGFloat,
        theme: IrohaOfflineTransferTheme = .bpngStyle
    ) {
        self.challenge = challenge
        self.size = size
        self.theme = theme
    }

    public var body: some View {
        Image(challenge.assetName, bundle: .main)
            .resizable()
            .scaledToFill()
            .frame(width: size, height: size)
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .stroke(theme.border, lineWidth: 1)
            )
            .accessibilityLabel(accessibilityLabel)
    }

    private var accessibilityLabel: Text {
        switch challenge.assetName {
        case "nearby_pairing_stars":
            return Text("Pairing image stars")
        case "nearby_pairing_bird":
            return Text("Pairing image bird")
        case "nearby_pairing_mask":
            return Text("Pairing image mask")
        default:
            return Text("Pairing image")
        }
    }
}

public enum IrohaOfflineNfcGuidanceMode: Sendable {
    case send
    case receive
}

public enum IrohaOfflineNfcExchangePhase: Sendable {
    case idle
    case waiting
    case transferring
    case success
    case failure
}

public struct IrohaOfflineNfcGuidancePanel: View {
    private let mode: IrohaOfflineNfcGuidanceMode
    private let phase: IrohaOfflineNfcExchangePhase
    private let title: String
    private let message: String
    private let theme: IrohaOfflineTransferTheme

    public init(
        mode: IrohaOfflineNfcGuidanceMode,
        phase: IrohaOfflineNfcExchangePhase,
        title: String,
        message: String,
        theme: IrohaOfflineTransferTheme = .bpngStyle
    ) {
        self.mode = mode
        self.phase = phase
        self.title = title
        self.message = message
        self.theme = theme
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .top, spacing: 14) {
                IrohaOfflineNfcHoldAnimation(
                    phase: phase,
                    theme: theme
                )
                .frame(width: 92, height: 74)
                .accessibilityHidden(true)

                VStack(alignment: .leading, spacing: 6) {
                    Text(title)
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundColor(theme.primaryText)
                        .lineLimit(2)
                    Text(message)
                        .font(.system(size: 13))
                        .foregroundColor(theme.secondaryText)
                        .lineLimit(3)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }

            HStack(spacing: 8) {
                Circle()
                    .fill(statusColor)
                    .frame(width: 8, height: 8)
                    .shadow(color: statusColor.opacity(0.5), radius: 4)
                Text(statusText)
                    .font(.system(size: 11, weight: .bold))
                    .foregroundColor(theme.secondaryText)
                    .textCase(.uppercase)
                    .lineLimit(1)
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(theme.background)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .stroke(theme.border, lineWidth: 1)
        )
    }

    private var statusColor: Color {
        switch phase {
        case .success:
            return theme.success
        case .failure:
            return theme.accent
        case .idle:
            return theme.secondaryText
        case .waiting, .transferring:
            return theme.warning
        }
    }

    private var statusText: String {
        switch (mode, phase) {
        case (_, .idle):
            return "Ready"
        case (.send, .waiting):
            return "Tap to receiver"
        case (.receive, .waiting):
            return "Waiting for sender"
        case (_, .transferring):
            return "Transferring"
        case (_, .success):
            return "Confirmed"
        case (_, .failure):
            return "Needs attention"
        }
    }
}

public struct IrohaOfflineNfcHoldAnimation: View {
    private let phase: IrohaOfflineNfcExchangePhase
    private let theme: IrohaOfflineTransferTheme

    public init(
        phase: IrohaOfflineNfcExchangePhase,
        theme: IrohaOfflineTransferTheme = .bpngStyle
    ) {
        self.phase = phase
        self.theme = theme
    }

    public var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30.0, paused: phase == .idle || phase == .failure || phase == .success)) { timeline in
            let progress = CGFloat((sin(timeline.date.timeIntervalSinceReferenceDate * 4) + 1) / 2)
            ZStack {
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(theme.surface)
                HStack(spacing: -4) {
                    phone(color: theme.primaryText.opacity(0.90))
                        .rotationEffect(.degrees(-10))
                    phone(color: theme.warning)
                        .rotationEffect(.degrees(10))
                }
                ForEach(0..<3, id: \.self) { index in
                    let opacity = max(0.10, 0.42 - Double(index) * 0.10 - Double(progress) * 0.16)
                    let size = 34 + CGFloat(index) * 14 + progress * 8
                    Circle()
                        .stroke(theme.accent.opacity(opacity), lineWidth: 2)
                        .frame(width: size, height: size)
                }
                if phase == .success {
                    Circle()
                        .fill(theme.success)
                        .frame(width: 22, height: 22)
                        .overlay(Image(systemName: "checkmark").font(.system(size: 11, weight: .bold)).foregroundColor(.white))
                        .offset(x: 30, y: -22)
                }
            }
        }
    }

    private func phone(color: Color) -> some View {
        RoundedRectangle(cornerRadius: 7, style: .continuous)
            .fill(color)
            .frame(width: 25, height: 46)
            .overlay(
                RoundedRectangle(cornerRadius: 5, style: .continuous)
                    .fill(theme.background.opacity(0.82))
                    .padding(4)
            )
    }
}
