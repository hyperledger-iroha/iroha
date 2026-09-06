import Foundation
@testable import IrohaSwift

/// Canonical KAGEMUSHA V1 framing with an intentionally opaque body.
///
/// This proves only the IPM framing layer. Typed protocol validation is tested
/// separately by the KAGEMUSHA V1 codec suite.
func irohaPeerKagemushaStructuralArchiveV1(
    kind: IrohaPeerWireKindV1,
    payload: Data
) -> Data {
    precondition(!payload.isEmpty)
    return noritoEncode(
        typeName: kind.requiredKagemushaCanonicalSchema,
        payload: payload,
        flags: NoritoHeader.compactLen,
        payloadAlignment: kind.requiredKagemushaPayloadAlignment
    )
}
