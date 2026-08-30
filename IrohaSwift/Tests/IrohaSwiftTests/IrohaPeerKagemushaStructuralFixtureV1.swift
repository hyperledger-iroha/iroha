import Foundation
@testable import IrohaSwift

/// Canonical ABI-21 transport framing with an intentionally opaque body.
/// This proves the IPM layer only; callers must not treat it as a semantically
/// valid request/payment/acknowledgement or pass it to the typed adapter.
func irohaPeerKagemushaStructuralArchiveV1(
    kind: IrohaPeerWireKindV1,
    schemaVersion: UInt16 = IrohaPeerWireMessageV1.kagemushaLegacySchemaVersion,
    payload: Data
) -> Data {
    precondition(!payload.isEmpty)
    guard let schema = kind.requiredKagemushaCanonicalSchema(
        schemaVersion: schemaVersion
    ), let alignment = KagemushaRecursiveSpend.archivedPayloadAlignment(
        forWireName: schema
    ) else {
        preconditionFailure("Unsupported Kagemusha IPM1 kind/schema pair")
    }
    return noritoEncode(
        typeName: schema,
        payload: payload,
        flags: NoritoHeader.compactLen,
        payloadAlignment: alignment
    )
}
