package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

internal object IrohaPeerKagemushaStructuralTestV1 {
    fun message(
        kind: IrohaPeerPayloadKind,
        payload: ByteArray,
        schemaVersion: Int = IrohaPeerWireMessageV1.KAGEMUSHA_LEGACY_SCHEMA_VERSION,
    ): IrohaPeerWireMessageV1 = IrohaPeerWireMessageV1(
        IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            kind,
            schemaVersion,
            archive(kind, payload, schemaVersion),
        ),
    )

    fun archive(
        kind: IrohaPeerPayloadKind,
        payload: ByteArray,
        schemaVersion: Int = IrohaPeerWireMessageV1.KAGEMUSHA_LEGACY_SCHEMA_VERSION,
    ): ByteArray {
        val schema = when {
            kind == IrohaPeerPayloadKind.PAYMENT &&
                schemaVersion == IrohaPeerWireMessageV1.KAGEMUSHA_ELIGIBILITY_PAYMENT_SCHEMA_VERSION ->
                "iroha_data_model::offline::model::KagemushaEligibilityPaymentEnvelopeV1"
            schemaVersion != IrohaPeerWireMessageV1.KAGEMUSHA_LEGACY_SCHEMA_VERSION ->
                error("Unsupported Kagemusha IPM1 kind/schema pair")
            kind == IrohaPeerPayloadKind.RECEIVE_REQUEST ->
                "iroha_torii_shared::offline_api::OfflineRecipientReceiveOfferV2"
            kind == IrohaPeerPayloadKind.PAYMENT ->
                "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4"
            else ->
                "iroha_data_model::offline::model::KagemushaReceiverAcknowledgementV2"
        }
        val padding = when (kind) {
            IrohaPeerPayloadKind.RECEIVE_REQUEST ->
                ByteArray(8)
            IrohaPeerPayloadKind.PAYMENT -> ByteArray(8)
            IrohaPeerPayloadKind.ACKNOWLEDGEMENT -> byteArrayOf()
        }
        val header = NoritoHeader(
            SchemaHash.hash16(schema),
            payload.size,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE,
        )
        return header.encode() + padding + payload
    }
}
