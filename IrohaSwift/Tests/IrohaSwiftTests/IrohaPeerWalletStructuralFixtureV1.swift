import Foundation
@testable import IrohaSwift

/// Scheme named by every structural wallet envelope built here.
let irohaPeerWalletStructuralSchemeV1 = Data(repeating: 0x5c, count: 32)

/// Canonical KAGEMUSHA wallet V1 envelope frame of `kind` that carries `payload` as an opaque
/// field.
///
/// The frame has the header, versions, message tag and scheme field that
/// `KagemushaWalletWireV1.inspectEnvelope` checks, but it is not a valid typed message. It
/// proves only the IPM framing layer; typed wallet validation is tested by
/// `KagemushaWalletVectorsV1Tests`.
func irohaPeerWalletStructuralEnvelopeV1(
    kind: IrohaPeerWireKindV1,
    payload: Data
) -> Data {
    let version = irohaPeerWalletField([1, 0])
    let leaf = version + irohaPeerWalletField([UInt8](irohaPeerWalletStructuralSchemeV1))
        + irohaPeerWalletField([UInt8](payload))
    // The variant mirrors the version and scheme field paths of the real message layouts.
    let variant: [UInt8]
    switch kind {
    case .offer, .request:
        variant = irohaPeerWalletField(irohaPeerWalletField(leaf))
    case .payment, .lineage:
        variant = irohaPeerWalletField(version + irohaPeerWalletField(irohaPeerWalletField(leaf)))
    case .credited, .sessionControl, .policyData:
        variant = irohaPeerWalletField(leaf)
    }
    let tag = withUnsafeBytes(of: UInt32(kind.rawValue).littleEndian, Array.init)
    let envelope = version + irohaPeerWalletField(tag + variant)
    return noritoEncode(
        typeName: KagemushaWalletWireV1.envelopeFrameName,
        payload: Data(envelope),
        flags: NoritoHeader.compactLen,
        payloadAlignment: KagemushaWalletWireV1.envelopePayloadAlignment
    )
}

/// Structural wallet envelope of `kind` whose complete frame is exactly `frameBytes` long; its
/// opaque field is `filler(count)` for the count that reaches that size.
func irohaPeerWalletStructuralEnvelopeV1(
    kind: IrohaPeerWireKindV1,
    frameBytes: Int,
    filler: (Int) -> Data = { Data(repeating: 0xA5, count: $0) }
) -> Data {
    var count = 0
    for _ in 0..<16 {
        let frame = irohaPeerWalletStructuralEnvelopeV1(kind: kind, payload: filler(count))
        if frame.count == frameBytes { return frame }
        count += frameBytes - frame.count
        precondition(count >= 0, "\(kind) envelope cannot be \(frameBytes) bytes")
    }
    preconditionFailure("\(kind) envelope cannot be exactly \(frameBytes) bytes")
}

/// `[compact length][content]` with a canonical unsigned LEB128 length.
private func irohaPeerWalletField(_ content: [UInt8]) -> [UInt8] {
    var length = UInt64(content.count)
    var prefix: [UInt8] = []
    repeat {
        var byte = UInt8(length & 0x7f)
        length >>= 7
        if length != 0 { byte |= 0x80 }
        prefix.append(byte)
    } while length != 0
    return prefix + content
}

/// Canonical envelope frames of `fixtures/kagemusha/wallet_v1_vectors.json` in vector order,
/// with the IPM1 kind of each frame's message tag.
func irohaPeerWalletVectorEnvelopesV1(
    filePath: String = #filePath
) throws -> [(variant: String, kind: IrohaPeerWireKindV1, frame: Data)] {
    var directory = URL(fileURLWithPath: filePath).deletingLastPathComponent()
    var fixture: URL?
    for _ in 0..<8 {
        let candidate = directory.appendingPathComponent("fixtures/kagemusha/wallet_v1_vectors.json")
        if FileManager.default.fileExists(atPath: candidate.path) {
            fixture = candidate
            break
        }
        directory.deleteLastPathComponent()
    }
    guard let fixture,
          let root = try JSONSerialization.jsonObject(with: Data(contentsOf: fixture))
            as? [String: Any],
          let envelopes = root["envelopes"] as? [[String: Any]] else {
        throw CocoaError(.fileReadCorruptFile)
    }
    return try envelopes.map { vector in
        guard let variant = vector["variant"] as? String,
              let tag = vector["tag"] as? Int,
              let kind = IrohaPeerWireKindV1(rawValue: UInt8(tag)),
              let hex = vector["canonical_hex"] as? String,
              hex.count.isMultiple(of: 2) else {
            throw CocoaError(.fileReadCorruptFile)
        }
        var frame = Data(capacity: hex.count / 2)
        var cursor = hex.startIndex
        while cursor < hex.endIndex {
            let next = hex.index(cursor, offsetBy: 2)
            guard let byte = UInt8(hex[cursor..<next], radix: 16) else {
                throw CocoaError(.fileReadCorruptFile)
            }
            frame.append(byte)
            cursor = next
        }
        return (variant, kind, frame)
    }
}
