package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

internal object IrohaPeerKagemushaStructuralAndroidTestV1 {
    fun message(
        kind: IrohaPeerPayloadKind,
        payload: ByteArray,
    ): IrohaPeerWireMessageV1 {
        val schema = when (kind) {
            IrohaPeerPayloadKind.RECEIVE_REQUEST ->
                "iroha_torii_shared::offline_api::OfflineRecipientReceiveOfferV2"
            IrohaPeerPayloadKind.PAYMENT ->
                "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4"
            IrohaPeerPayloadKind.ACKNOWLEDGEMENT ->
                "iroha_data_model::offline::model::KagemushaReceiverAcknowledgementV2"
        }
        val padding = when (kind) {
            IrohaPeerPayloadKind.RECEIVE_REQUEST, IrohaPeerPayloadKind.PAYMENT -> ByteArray(8)
            IrohaPeerPayloadKind.ACKNOWLEDGEMENT -> byteArrayOf()
        }
        val header = NoritoHeader(
            SchemaHash.hash16(schema),
            payload.size,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE,
        )
        return IrohaPeerWireMessageV1(
            IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                kind,
                0x0102,
                header.encode() + padding + payload,
            ),
        )
    }
}
