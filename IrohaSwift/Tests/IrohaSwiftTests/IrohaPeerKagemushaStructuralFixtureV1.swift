import Foundation
@testable import IrohaSwift

/// Canonical bridge ABI22 / Kagemusha data ABI V4 transport framing with an intentionally opaque body.
/// This proves the IPM layer only; callers must not treat it as a semantically
/// valid request/payment/acknowledgement or pass it to the typed adapter.
func irohaPeerKagemushaStructuralArchiveV1(
    kind: IrohaPeerWireKindV1,
    payload: Data
) -> Data {
    precondition(!payload.isEmpty)
    let schema: String
    let alignment: Int
    switch kind {
    case .receiveRequest:
        schema = KagemushaRecursiveSpend.recipientReceiveOfferWireName
        alignment = 16
    case .payment:
        schema = KagemushaRecursiveSpend.peerPaymentWireNameV4
        alignment = 16
    case .acknowledgement:
        schema = KagemushaRecursiveSpend.acknowledgementWireName
        alignment = 8
    }
    return noritoEncode(
        typeName: schema,
        payload: payload,
        flags: NoritoHeader.compactLen,
        payloadAlignment: alignment
    )
}
