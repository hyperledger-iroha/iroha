import Foundation
@testable import IrohaSwift

func mobileKagemushaStructuralArchiveV1(
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
