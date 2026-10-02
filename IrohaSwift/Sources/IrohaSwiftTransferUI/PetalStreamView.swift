#if canImport(SwiftUI)
import Foundation
import IrohaSwift
import SwiftUI

/// Draws one Petal Stream frame as a square, scaled to fit.
public struct PetalFrameView: View {
    private let drawList: PetalDrawList

    public init(cells: PetalFrameCells, palette: PetalPalette = .standard) {
        drawList = PetalDrawList(cells: cells, palette: palette)
    }

    public init(drawList: PetalDrawList) {
        self.drawList = drawList
    }

    public var body: some View {
        Canvas { context, size in
            let side = min(size.width, size.height)
            let rect = CGRect(
                x: (size.width - side) / 2,
                y: (size.height - side) / 2,
                width: side,
                height: side
            )
            context.withCGContext { cgContext in
                PetalCoreGraphicsRenderer.draw(drawList, in: cgContext, rect: rect)
            }
        }
        .aspectRatio(1, contentMode: .fit)
        .accessibilityLabel(Text("Petal Stream code"))
    }
}

/// Plays a Petal stream: shows frame `startFrame`, `startFrame + 1`, … at
/// `framesPerSecond`, looping over the 16-bit frame counter forever.
///
/// Any receiver can join at any moment: lane `D` repeats the beacon every
/// fourth frame and every other frame carries fresh fountain atoms. Keep the
/// rate well below the camera rate so each frame is captured whole at least
/// once; the default suits 30 fps phone cameras.
public struct PetalStreamView: View {
    /// Default display rate.
    public static let defaultFramesPerSecond = 8.0

    private let encoder: PetalStreamEncoder
    private let framesPerSecond: Double
    private let palette: PetalPalette
    private let startFrame: UInt16
    @State private var start = Date()

    /// - Parameters:
    ///   - encoder: The stream to show.
    ///   - framesPerSecond: Display rate, clamped to 1–60.
    ///   - palette: Frame colours.
    ///   - startFrame: The first frame number shown.
    public init(
        encoder: PetalStreamEncoder,
        framesPerSecond: Double = PetalStreamView.defaultFramesPerSecond,
        palette: PetalPalette = .standard,
        startFrame: UInt16 = 0
    ) {
        self.encoder = encoder
        self.framesPerSecond = Self.clampedRate(framesPerSecond)
        self.palette = palette
        self.startFrame = startFrame
    }

    public var body: some View {
        TimelineView(.periodic(from: start, by: 1 / framesPerSecond)) { timeline in
            let frame = Self.frameNumber(
                elapsed: timeline.date.timeIntervalSince(start),
                framesPerSecond: framesPerSecond,
                startFrame: startFrame
            )
            PetalFrameView(cells: encoder.cells(frame: frame), palette: palette)
        }
        .onChange(of: encoder.meta) { _ in start = Date() }
    }

    /// The frame shown `elapsed` seconds after the stream started.
    public static func frameNumber(
        elapsed: TimeInterval,
        framesPerSecond: Double,
        startFrame: UInt16 = 0
    ) -> UInt16 {
        let ticks = (elapsed * clampedRate(framesPerSecond)).rounded(.down)
        guard ticks.isFinite, ticks >= 0 else { return startFrame }
        let step = UInt64(min(ticks, 1e15))
        return startFrame &+ UInt16(truncatingIfNeeded: step)
    }

    static func clampedRate(_ framesPerSecond: Double) -> Double {
        guard framesPerSecond.isFinite else { return defaultFramesPerSecond }
        return min(max(framesPerSecond, 1), 60)
    }
}
#endif
