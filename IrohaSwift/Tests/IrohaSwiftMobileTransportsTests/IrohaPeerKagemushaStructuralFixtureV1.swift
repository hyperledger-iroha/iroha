import Foundation
@testable import IrohaSwift

func mobileKagemushaStructuralArchiveV1(
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
