package org.hyperledger.iroha.sdk.offline

/** App-facing transfer modality for Offline Note payment-token handoff. */
enum class OfflineNoteTransferModality {
    QR_STREAMING,
    NFC,
    NEARBY,
}

/** NFC availability hint; apps still own platform permission and entitlement checks. */
class OfflineNoteNfcCapability private constructor(
    val supported: Boolean,
    val reason: String?,
) {
    companion object {
        @JvmStatic
        fun supported(): OfflineNoteNfcCapability = OfflineNoteNfcCapability(true, null)

        @JvmStatic
        fun unavailable(reason: String): OfflineNoteNfcCapability =
            OfflineNoteNfcCapability(false, reason)
    }
}

/** Capability hints for choosing a local transfer modality in app UI. */
class OfflineNoteTransferCapabilities @JvmOverloads constructor(
    val qrStreaming: Boolean = true,
    val nfc: OfflineNoteNfcCapability,
    val nearby: Boolean = true,
) {
    fun supportedModalities(): List<OfflineNoteTransferModality> {
        val modalities = ArrayList<OfflineNoteTransferModality>()
        if (qrStreaming) modalities.add(OfflineNoteTransferModality.QR_STREAMING)
        if (nfc.supported) modalities.add(OfflineNoteTransferModality.NFC)
        if (nearby) modalities.add(OfflineNoteTransferModality.NEARBY)
        return modalities
    }

    companion object {
        @JvmStatic
        @JvmOverloads
        fun current(
            androidHceSupported: Boolean = false,
            nearbyAvailable: Boolean = true,
        ): OfflineNoteTransferCapabilities {
            val nfc =
                if (androidHceSupported) {
                    OfflineNoteNfcCapability.supported()
                } else {
                    OfflineNoteNfcCapability.unavailable(
                        "Android NFC payment-token transfer requires device HCE support and an app HostApduService.",
                    )
                }
            return OfflineNoteTransferCapabilities(qrStreaming = true, nfc = nfc, nearby = nearbyAvailable)
        }
    }
}

/** Canonical payment-token bytes plus modality metadata for framework-specific transports. */
class OfflineNoteTransferPayload(
    val modality: OfflineNoteTransferModality,
    val contentType: String,
    payload: ByteArray,
) {
    private val _payload = payload.copyOf()

    fun payload(): ByteArray = _payload.copyOf()
}

/** Result of ingesting a streamed QR/NFC/Nearby frame. */
class OfflineNoteTransferStreamResult(
    payload: ByteArray?,
    val token: OfflineNotePaymentToken?,
    val receiveRequest: OfflineNoteReceiveRequest?,
    val receiptAck: OfflineNoteReceiptAck?,
    val receivedChunks: Int,
    val totalChunks: Int,
    val recoveredChunks: Int,
) {
    private val _payload = payload?.copyOf()
    val isComplete: Boolean get() = _payload != null
    val progress: Double get() = if (totalChunks == 0) 0.0 else receivedChunks / totalChunks.toDouble()

    fun payload(): ByteArray? = _payload?.copyOf()
}

/** Receiver for QR-compatible stream frames carried over camera, NFC APDUs, or nearby byte channels. */
class OfflineNoteTransferStreamReceiver {
    private val decoder = OfflineQrStream.Decoder()

    fun ingestFrame(frameBytes: ByteArray): OfflineNoteTransferStreamResult {
        val result = decoder.ingest(frameBytes)
        var token: OfflineNotePaymentToken? = null
        var receiveRequest: OfflineNoteReceiveRequest? = null
        var receiptAck: OfflineNoteReceiptAck? = null
        result.payload?.let { payload ->
            when (result.payloadKind) {
                OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN ->
                    token = OfflineNotePaymentTokenCodec.decodeQrPayload(payload)
                OfflineQrStream.PayloadKind.OFFLINE_RECEIVE_REQUEST ->
                    receiveRequest = OfflineNoteReceiveRequestCodec.decodeQrPayload(payload)
                OfflineQrStream.PayloadKind.OFFLINE_RECEIPT_ACK ->
                    receiptAck = OfflineNoteReceiptAckCodec.decodeQrPayload(payload)
                else -> throw IllegalArgumentException("QR stream payload kind is not an Offline Note payload")
            }
        }
        return OfflineNoteTransferStreamResult(
            payload = result.payload,
            token = token,
            receiveRequest = receiveRequest,
            receiptAck = receiptAck,
            receivedChunks = result.receivedChunks,
            totalChunks = result.totalChunks,
            recoveredChunks = result.recoveredChunks,
        )
    }
}

/** Canonical Offline Note payment-token handoff helpers for QR, NFC, and nearby transports. */
object OfflineNoteTransferHandoff {
    const val PAYMENT_TOKEN_CONTENT_TYPE: String = "application/vnd.iroha.offline.payment-token+norito"
    const val RECEIVE_REQUEST_CONTENT_TYPE: String =
        "application/vnd.iroha.offline.receive-request+norito"
    const val RECEIPT_ACK_CONTENT_TYPE: String = "application/vnd.iroha.offline.receipt-ack+norito"
    const val TEXT_PAYMENT_TOKEN_CONTENT_TYPE: String = "text/vnd.iroha.offline.payment-token"
    const val TEXT_RECEIVE_REQUEST_CONTENT_TYPE: String = "text/vnd.iroha.offline.receive-request"
    const val TEXT_RECEIPT_ACK_CONTENT_TYPE: String = "text/vnd.iroha.offline.receipt-ack"
    const val NEARBY_SERVICE_NAME: String = "iroha-pay"
    const val NFC_EXTERNAL_TYPE: String = "org.hyperledger.iroha:offline-payment"
    const val DEFAULT_NFC_AID_HEX: String = OfflineNoteNfcApduProtocol.AID_HEX
    const val QR_FRAME_CADENCE_MS: Int = 500

    @JvmField
    val QR_STREAMING_OPTIONS: OfflineQrStream.Options = OfflineQrStream.Options(180, 2)

    @JvmField
    val NFC_STREAMING_OPTIONS: OfflineQrStream.Options =
        OfflineQrStream.Options(OfflineNoteNfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES - 20, 0)

    @JvmField
    val NEARBY_STREAMING_OPTIONS: OfflineQrStream.Options = OfflineQrStream.Options(4096, 0)

    @JvmStatic
    fun rawPaymentTokenBytes(token: OfflineNotePaymentToken): ByteArray =
        OfflineNotePaymentTokenCodec.encodeNorito(token)

    @JvmStatic
    fun paymentTokenPayload(
        token: OfflineNotePaymentToken,
        modality: OfflineNoteTransferModality,
    ): OfflineNoteTransferPayload =
        OfflineNoteTransferPayload(modality, PAYMENT_TOKEN_CONTENT_TYPE, rawPaymentTokenBytes(token))

    @JvmStatic
    fun decodePaymentToken(payload: OfflineNoteTransferPayload): OfflineNotePaymentToken {
        require(payload.contentType == PAYMENT_TOKEN_CONTENT_TYPE) {
            "Transfer payload content type is not a payment token"
        }
        return OfflineNotePaymentTokenCodec.decodeNorito(payload.payload())
    }

    @JvmStatic
    fun decodePaymentToken(rawPayload: ByteArray): OfflineNotePaymentToken =
        OfflineNotePaymentTokenCodec.decodeNorito(rawPayload)

    @JvmStatic
    fun rawReceiveRequestBytes(request: OfflineNoteReceiveRequest): ByteArray =
        OfflineNoteReceiveRequestCodec.encodeNorito(request)

    @JvmStatic
    fun receiveRequestPayload(
        request: OfflineNoteReceiveRequest,
        modality: OfflineNoteTransferModality,
    ): OfflineNoteTransferPayload =
        OfflineNoteTransferPayload(modality, RECEIVE_REQUEST_CONTENT_TYPE, rawReceiveRequestBytes(request))

    @JvmStatic
    fun decodeReceiveRequest(payload: OfflineNoteTransferPayload): OfflineNoteReceiveRequest {
        require(payload.contentType == RECEIVE_REQUEST_CONTENT_TYPE) {
            "Transfer payload content type is not a receive request"
        }
        return OfflineNoteReceiveRequestCodec.decodeNorito(payload.payload())
    }

    @JvmStatic
    fun decodeReceiveRequest(rawPayload: ByteArray): OfflineNoteReceiveRequest =
        OfflineNoteReceiveRequestCodec.decodeNorito(rawPayload)

    @JvmStatic
    fun rawReceiptAckBytes(ack: OfflineNoteReceiptAck): ByteArray =
        OfflineNoteReceiptAckCodec.encodeNorito(ack)

    @JvmStatic
    fun receiptAckPayload(
        ack: OfflineNoteReceiptAck,
        modality: OfflineNoteTransferModality,
    ): OfflineNoteTransferPayload =
        OfflineNoteTransferPayload(modality, RECEIPT_ACK_CONTENT_TYPE, rawReceiptAckBytes(ack))

    @JvmStatic
    fun decodeReceiptAck(payload: OfflineNoteTransferPayload): OfflineNoteReceiptAck {
        require(payload.contentType == RECEIPT_ACK_CONTENT_TYPE) {
            "Transfer payload content type is not a receipt ACK"
        }
        return OfflineNoteReceiptAckCodec.decodeNorito(payload.payload())
    }

    @JvmStatic
    fun decodeReceiptAck(rawPayload: ByteArray): OfflineNoteReceiptAck =
        OfflineNoteReceiptAckCodec.decodeNorito(rawPayload)

    @JvmStatic
    fun qrStreamingFrameBytes(token: OfflineNotePaymentToken): List<ByteArray> =
        qrStreamingFrameBytes(token, QR_STREAMING_OPTIONS)

    @JvmStatic
    fun qrStreamingFrameBytes(
        token: OfflineNotePaymentToken,
        options: OfflineQrStream.Options,
    ): List<ByteArray> =
        OfflineNotePaymentTokenCodec.encodeQrFrameBytes(token, options)

    @JvmStatic
    fun qrStreamingFrameBytes(request: OfflineNoteReceiveRequest): List<ByteArray> =
        qrStreamingFrameBytes(request, QR_STREAMING_OPTIONS)

    @JvmStatic
    fun qrStreamingFrameBytes(
        request: OfflineNoteReceiveRequest,
        options: OfflineQrStream.Options,
    ): List<ByteArray> =
        OfflineNoteReceiveRequestCodec.encodeQrFrameBytes(request, options)

    @JvmStatic
    fun qrStreamingFrameBytes(ack: OfflineNoteReceiptAck): List<ByteArray> =
        qrStreamingFrameBytes(ack, QR_STREAMING_OPTIONS)

    @JvmStatic
    fun qrStreamingFrameBytes(
        ack: OfflineNoteReceiptAck,
        options: OfflineQrStream.Options,
    ): List<ByteArray> =
        OfflineNoteReceiptAckCodec.encodeQrFrameBytes(ack, options)

    @JvmStatic
    fun nfcFrameBytes(token: OfflineNotePaymentToken): List<ByteArray> =
        nfcFrameBytes(token, NFC_STREAMING_OPTIONS)

    @JvmStatic
    fun nfcFrameBytes(
        token: OfflineNotePaymentToken,
        options: OfflineQrStream.Options,
    ): List<ByteArray> = streamFrameBytes(token, options)

    @JvmStatic
    fun nfcPaymentTokenWriteApdus(token: OfflineNotePaymentToken): List<ByteArray> =
        nfcPaymentTokenWriteApdus(token, OfflineNoteNfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES)

    @JvmStatic
    fun nfcPaymentTokenWriteApdus(
        token: OfflineNotePaymentToken,
        maxChunkLength: Int,
    ): List<ByteArray> =
        OfflineNoteNfcApduProtocol.writePayloadApdus(
            OfflineNoteNfcPayloadKind.PAYMENT_TOKEN,
            rawPaymentTokenBytes(token),
            maxChunkLength,
        )

    @JvmStatic
    fun nfcReceiptAckWriteApdus(ack: OfflineNoteReceiptAck): List<ByteArray> =
        nfcReceiptAckWriteApdus(ack, OfflineNoteNfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES)

    @JvmStatic
    fun nfcReceiptAckWriteApdus(
        ack: OfflineNoteReceiptAck,
        maxChunkLength: Int,
    ): List<ByteArray> =
        OfflineNoteNfcApduProtocol.writePayloadApdus(
            OfflineNoteNfcPayloadKind.RECEIPT_ACK,
            rawReceiptAckBytes(ack),
            maxChunkLength,
        )

    @JvmStatic
    fun nearbyPayload(token: OfflineNotePaymentToken): OfflineNoteTransferPayload =
        paymentTokenPayload(token, OfflineNoteTransferModality.NEARBY)

    @JvmStatic
    fun nearbyPaymentEnvelopeBytes(token: OfflineNotePaymentToken): ByteArray =
        OfflineNoteNearbyEnvelope(
            kind = OfflineNoteNearbyMessageKind.PAYMENT,
            payload = rawPaymentTokenBytes(token),
            contentType = PAYMENT_TOKEN_CONTENT_TYPE,
        ).encoded()

    @JvmStatic
    fun decodeNearbyPaymentToken(envelopeBytes: ByteArray): OfflineNotePaymentToken =
        OfflineNoteNearbyEnvelope.decode(envelopeBytes).paymentToken()

    @JvmStatic
    fun nearbyReceiptAckEnvelopeBytes(ack: OfflineNoteReceiptAck): ByteArray =
        OfflineNoteNearbyEnvelope(
            kind = OfflineNoteNearbyMessageKind.RECEIPT_ACK,
            payload = rawReceiptAckBytes(ack),
            contentType = RECEIPT_ACK_CONTENT_TYPE,
        ).encoded()

    @JvmStatic
    fun decodeNearbyReceiptAck(envelopeBytes: ByteArray): OfflineNoteReceiptAck =
        OfflineNoteNearbyEnvelope.decode(envelopeBytes).receiptAck()

    @JvmStatic
    fun nearbyFrameBytes(token: OfflineNotePaymentToken): List<ByteArray> =
        nearbyFrameBytes(token, NEARBY_STREAMING_OPTIONS)

    @JvmStatic
    fun nearbyFrameBytes(
        token: OfflineNotePaymentToken,
        options: OfflineQrStream.Options,
    ): List<ByteArray> = streamFrameBytes(token, options)

    private fun streamFrameBytes(
        token: OfflineNotePaymentToken,
        options: OfflineQrStream.Options,
    ): List<ByteArray> =
        OfflineQrStream.Encoder.encodeFrameBytes(
            rawPaymentTokenBytes(token),
            OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN,
            options,
        )
}
