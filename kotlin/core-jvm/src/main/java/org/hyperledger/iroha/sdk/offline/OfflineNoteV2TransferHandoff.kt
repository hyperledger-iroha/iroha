package org.hyperledger.iroha.sdk.offline

/** App-facing transfer modality for Offline Note V2 payment-token handoff. */
enum class OfflineNoteV2TransferModality {
    QR_STREAMING,
    NFC,
    NEARBY,
}

/** NFC availability hint; apps still own platform permission and entitlement checks. */
class OfflineNoteV2NfcCapability private constructor(
    val supported: Boolean,
    val reason: String?,
) {
    companion object {
        @JvmStatic
        fun supported(): OfflineNoteV2NfcCapability = OfflineNoteV2NfcCapability(true, null)

        @JvmStatic
        fun unavailable(reason: String): OfflineNoteV2NfcCapability =
            OfflineNoteV2NfcCapability(false, reason)
    }
}

/** Capability hints for choosing a local transfer modality in app UI. */
class OfflineNoteV2TransferCapabilities @JvmOverloads constructor(
    val qrStreaming: Boolean = true,
    val nfc: OfflineNoteV2NfcCapability,
    val nearby: Boolean = true,
) {
    fun supportedModalities(): List<OfflineNoteV2TransferModality> {
        val modalities = ArrayList<OfflineNoteV2TransferModality>()
        if (qrStreaming) modalities.add(OfflineNoteV2TransferModality.QR_STREAMING)
        if (nfc.supported) modalities.add(OfflineNoteV2TransferModality.NFC)
        if (nearby) modalities.add(OfflineNoteV2TransferModality.NEARBY)
        return modalities
    }

    companion object {
        @JvmStatic
        @JvmOverloads
        fun current(
            androidHceSupported: Boolean = false,
            nearbyAvailable: Boolean = true,
        ): OfflineNoteV2TransferCapabilities {
            val nfc =
                if (androidHceSupported) {
                    OfflineNoteV2NfcCapability.supported()
                } else {
                    OfflineNoteV2NfcCapability.unavailable(
                        "Android NFC payment-token transfer requires device HCE support and an app HostApduService.",
                    )
                }
            return OfflineNoteV2TransferCapabilities(qrStreaming = true, nfc = nfc, nearby = nearbyAvailable)
        }
    }
}

/** Canonical payment-token bytes plus modality metadata for framework-specific transports. */
class OfflineNoteV2TransferPayload(
    val modality: OfflineNoteV2TransferModality,
    val contentType: String,
    payload: ByteArray,
) {
    private val _payload = payload.copyOf()

    fun payload(): ByteArray = _payload.copyOf()
}

/** Result of ingesting a streamed QR/NFC/Nearby frame. */
class OfflineNoteV2TransferStreamResult(
    payload: ByteArray?,
    val token: OfflineNoteV2PaymentToken?,
    val receivedChunks: Int,
    val totalChunks: Int,
    val recoveredChunks: Int,
) {
    private val _payload = payload?.copyOf()
    val isComplete: Boolean get() = token != null
    val progress: Double get() = if (totalChunks == 0) 0.0 else receivedChunks / totalChunks.toDouble()

    fun payload(): ByteArray? = _payload?.copyOf()
}

/** Receiver for QR-compatible stream frames carried over camera, NFC APDUs, or nearby byte channels. */
class OfflineNoteV2TransferStreamReceiver {
    private val decoder = OfflineQrStream.Decoder()

    fun ingestFrame(frameBytes: ByteArray): OfflineNoteV2TransferStreamResult {
        val result = decoder.ingest(frameBytes)
        val token = result.payload?.let {
            require(result.payloadKind == OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN_V2) {
                "QR stream payload kind is not a payment token"
            }
            OfflineNoteV2PaymentTokenCodec.decodeQrPayload(it)
        }
        return OfflineNoteV2TransferStreamResult(
            payload = result.payload,
            token = token,
            receivedChunks = result.receivedChunks,
            totalChunks = result.totalChunks,
            recoveredChunks = result.recoveredChunks,
        )
    }
}

/** Canonical Offline Note V2 payment-token handoff helpers for QR, NFC, and nearby transports. */
object OfflineNoteV2TransferHandoff {
    const val PAYMENT_TOKEN_CONTENT_TYPE: String = "application/vnd.iroha.offline.payment-token-v2+norito"
    const val RECEIVE_CHALLENGE_CONTENT_TYPE: String =
        "application/vnd.iroha.offline.receive-challenge-v1+octet-stream"
    const val RECEIPT_ACK_CONTENT_TYPE: String = "application/vnd.iroha.offline.receipt-ack-v1+octet-stream"
    const val NEARBY_SERVICE_NAME: String = "iroha-pay-v2"
    const val NFC_EXTERNAL_TYPE: String = "org.hyperledger.iroha:offline-payment-v2"
    const val DEFAULT_NFC_AID_HEX: String = OfflineNoteV2NfcApduProtocol.AID_HEX
    const val QR_FRAME_CADENCE_MS: Int = 500

    @JvmField
    val QR_STREAMING_OPTIONS: OfflineQrStream.Options = OfflineQrStream.Options(180, 2)

    @JvmField
    val NFC_STREAMING_OPTIONS: OfflineQrStream.Options =
        OfflineQrStream.Options(OfflineNoteV2NfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES - 20, 0)

    @JvmField
    val NEARBY_STREAMING_OPTIONS: OfflineQrStream.Options = OfflineQrStream.Options(4096, 0)

    @JvmStatic
    fun rawPaymentTokenBytes(token: OfflineNoteV2PaymentToken): ByteArray =
        OfflineNoteV2PaymentTokenCodec.encodeNorito(token)

    @JvmStatic
    fun paymentTokenPayload(
        token: OfflineNoteV2PaymentToken,
        modality: OfflineNoteV2TransferModality,
    ): OfflineNoteV2TransferPayload =
        OfflineNoteV2TransferPayload(modality, PAYMENT_TOKEN_CONTENT_TYPE, rawPaymentTokenBytes(token))

    @JvmStatic
    fun decodePaymentToken(payload: OfflineNoteV2TransferPayload): OfflineNoteV2PaymentToken {
        require(payload.contentType == PAYMENT_TOKEN_CONTENT_TYPE) {
            "Transfer payload content type is not a payment token"
        }
        return OfflineNoteV2PaymentTokenCodec.decodeNorito(payload.payload())
    }

    @JvmStatic
    fun decodePaymentToken(rawPayload: ByteArray): OfflineNoteV2PaymentToken =
        OfflineNoteV2PaymentTokenCodec.decodeNorito(rawPayload)

    @JvmStatic
    fun qrStreamingFrameBytes(token: OfflineNoteV2PaymentToken): List<ByteArray> =
        qrStreamingFrameBytes(token, QR_STREAMING_OPTIONS)

    @JvmStatic
    fun qrStreamingFrameBytes(
        token: OfflineNoteV2PaymentToken,
        options: OfflineQrStream.Options,
    ): List<ByteArray> =
        OfflineNoteV2PaymentTokenCodec.encodeQrFrameBytes(token, options)

    @JvmStatic
    fun nfcFrameBytes(token: OfflineNoteV2PaymentToken): List<ByteArray> =
        nfcFrameBytes(token, NFC_STREAMING_OPTIONS)

    @JvmStatic
    fun nfcFrameBytes(
        token: OfflineNoteV2PaymentToken,
        options: OfflineQrStream.Options,
    ): List<ByteArray> = streamFrameBytes(token, options)

    @JvmStatic
    fun nfcPaymentTokenWriteApdus(token: OfflineNoteV2PaymentToken): List<ByteArray> =
        nfcPaymentTokenWriteApdus(token, OfflineNoteV2NfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES)

    @JvmStatic
    fun nfcPaymentTokenWriteApdus(
        token: OfflineNoteV2PaymentToken,
        maxChunkLength: Int,
    ): List<ByteArray> =
        OfflineNoteV2NfcApduProtocol.writePayloadApdus(
            OfflineNoteV2NfcPayloadKind.PAYMENT_TOKEN,
            rawPaymentTokenBytes(token),
            maxChunkLength,
        )

    @JvmStatic
    fun nearbyPayload(token: OfflineNoteV2PaymentToken): OfflineNoteV2TransferPayload =
        paymentTokenPayload(token, OfflineNoteV2TransferModality.NEARBY)

    @JvmStatic
    fun nearbyPaymentEnvelopeBytes(token: OfflineNoteV2PaymentToken): ByteArray =
        OfflineNoteV2NearbyEnvelope(
            kind = OfflineNoteV2NearbyMessageKind.PAYMENT,
            payload = rawPaymentTokenBytes(token),
            contentType = PAYMENT_TOKEN_CONTENT_TYPE,
        ).encoded()

    @JvmStatic
    fun decodeNearbyPaymentToken(envelopeBytes: ByteArray): OfflineNoteV2PaymentToken =
        OfflineNoteV2NearbyEnvelope.decode(envelopeBytes).paymentToken()

    @JvmStatic
    fun nearbyFrameBytes(token: OfflineNoteV2PaymentToken): List<ByteArray> =
        nearbyFrameBytes(token, NEARBY_STREAMING_OPTIONS)

    @JvmStatic
    fun nearbyFrameBytes(
        token: OfflineNoteV2PaymentToken,
        options: OfflineQrStream.Options,
    ): List<ByteArray> = streamFrameBytes(token, options)

    private fun streamFrameBytes(
        token: OfflineNoteV2PaymentToken,
        options: OfflineQrStream.Options,
    ): List<ByteArray> =
        OfflineQrStream.Encoder.encodeFrameBytes(
            rawPaymentTokenBytes(token),
            OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN_V2,
            options,
        )
}
