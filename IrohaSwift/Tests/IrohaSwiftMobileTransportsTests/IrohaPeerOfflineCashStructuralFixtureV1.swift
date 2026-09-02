import Foundation
@testable import IrohaSwift

func mobileOfflineCashStructuralArchiveV1(
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
