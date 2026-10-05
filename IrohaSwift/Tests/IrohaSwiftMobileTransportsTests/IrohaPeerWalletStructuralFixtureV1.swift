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
