import Foundation
@testable import IrohaSwift

/// Canonical Offline Cash V1 framing with an intentionally opaque body.
///
/// This proves only the IPM framing layer. Typed protocol validation is tested
/// separately by the Offline Cash V1 codec suite.
func irohaPeerOfflineCashStructuralArchiveV1(
    kind: IrohaPeerWireKindV1,
    payload: Data
) -> Data {
    precondition(!payload.isEmpty)
    return noritoEncode(
        typeName: kind.requiredOfflineCashCanonicalSchema,
        payload: payload,
        flags: NoritoHeader.compactLen,
        payloadAlignment: 16
    )
}
