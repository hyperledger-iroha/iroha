package org.hyperledger.iroha.sdk.offline

/** Options for the SDK-owned Offline Cash V1 IQR1 stream. */
class OfflineCashQrStreamOptionsV1 @JvmOverloads constructor(
    val compressionPolicy: IrohaPeerWireCompressionPolicyV1 =
        IrohaPeerWireCompressionPolicyV1.PEER_OPTIMIZED,
)

/**
 * Offline Cash V1 QR framing over the shared, hardened IPM1/IQR1 transport.
 *
 * The canonical payload is the exact UTF-8 `kgm2:` text emitted by
 * [OfflineCashPeerAdapterV1]. Context-bound decoding remains the adapter's
 * responsibility after the stream completes.
 */
object OfflineCashQrStreamCodecV1 {
    const val NATIVE_TEXT_SCHEMA_VERSION: Int = 0x0100

    @JvmStatic
    @JvmOverloads
    fun encodePeerText(
        peerText: String,
        kind: IrohaPeerPayloadKind,
        options: OfflineCashQrStreamOptionsV1 = OfflineCashQrStreamOptionsV1(),
    ): List<String> {
        val bytes = peerText.toByteArray(Charsets.UTF_8)
        return try {
            val payload = IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.OFFLINE_CASH_V1,
                kind,
                NATIVE_TEXT_SCHEMA_VERSION,
                bytes,
            )
            IrohaPeerQRCodecV1.encode(
                IrohaPeerWireMessageV1(payload, options.compressionPolicy),
            )
        } finally {
            bytes.fill(0)
        }
    }

    @JvmStatic
    @JvmOverloads
    fun encodePaymentRequest(
        request: OfflineCashPaymentRequestV1,
        options: OfflineCashQrStreamOptionsV1 = OfflineCashQrStreamOptionsV1(),
    ): List<String> = encodePeerText(
        OfflineCashPeerAdapterV1.encodePaymentRequest(request),
        IrohaPeerPayloadKind.RECEIVE_REQUEST,
        options,
    )

    @JvmStatic
    @JvmOverloads
    fun encodePayment(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
        options: OfflineCashQrStreamOptionsV1 = OfflineCashQrStreamOptionsV1(),
    ): List<String> = encodePeerText(
        OfflineCashPeerAdapterV1.encodePayment(request, payment),
        IrohaPeerPayloadKind.PAYMENT,
        options,
    )

    @JvmStatic
    @JvmOverloads
    fun encodeAcknowledgement(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
        acknowledgement: OfflineCashAcknowledgementV1,
        options: OfflineCashQrStreamOptionsV1 = OfflineCashQrStreamOptionsV1(),
    ): List<String> = encodePeerText(
        OfflineCashPeerAdapterV1.encodeAcknowledgement(request, payment, acknowledgement),
        IrohaPeerPayloadKind.ACKNOWLEDGEMENT,
        options,
    )

    @JvmStatic
    fun decodeFrameText(frameText: String): IrohaPeerQRFrameV1 =
        IrohaPeerQRCodecV1.decodeFrame(frameText).also { frame ->
            require(frame.profile == IrohaPeerPayloadProfile.OFFLINE_CASH_V1) {
                "IQR1 frame is not an Offline Cash V1 stream"
            }
        }

    @JvmStatic
    fun completedPeerText(message: IrohaPeerWireMessageV1): String {
        require(message.canonicalPayload.profile == IrohaPeerPayloadProfile.OFFLINE_CASH_V1) {
            "IPM1 message is not an Offline Cash V1 stream"
        }
        require(message.canonicalPayload.schemaVersion == NATIVE_TEXT_SCHEMA_VERSION) {
            "Unsupported Offline Cash V1 QR schema"
        }
        val bytes = message.canonicalPayload.bytes
        return try {
            val text = bytes.toString(Charsets.UTF_8)
            require(text.toByteArray(Charsets.UTF_8).contentEquals(bytes)) {
                "Offline Cash V1 QR payload is not canonical UTF-8"
            }
            text
        } finally {
            bytes.fill(0)
        }
    }
}

/** One bounded QR scan update and, on completion, the exact `kgm2:` text. */
class OfflineCashQrStreamProgressV1 internal constructor(
    val completedPeerText: String?,
    val kind: IrohaPeerPayloadKind?,
    streamId: ByteArray,
    val receivedDataFrames: Int,
    val totalDataFrames: Int,
    val recoveredDataFrames: Int,
    val isDuplicate: Boolean,
) {
    private val stream = streamId.copyOf()
    val streamId: ByteArray get() = stream.copyOf()
    val isComplete: Boolean get() = completedPeerText != null
    val progress: Double get() = when {
        isComplete -> 1.0
        totalDataFrames > 0 ->
            minOf(1.0, receivedDataFrames.toDouble() / totalDataFrames.toDouble())
        else -> 0.0
    }
}

/** Stateful, reorder-tolerant Offline Cash V1 animated QR decoder. */
class OfflineCashQrStreamDecoderV1 @JvmOverloads constructor(
    expectedKind: IrohaPeerPayloadKind? = null,
    scanLimits: IrohaPeerQRScanLimitsV1 = IrohaPeerQRScanLimitsV1.STANDARD,
    clock: IrohaPeerQRClockV1 = IrohaPeerQRMonotonicClockV1,
) {
    private val decoder = IrohaPeerQRScanSessionV1(
        IrohaPeerPayloadProfile.OFFLINE_CASH_V1,
        expectedKind,
        OfflineCashQrStreamCodecV1.NATIVE_TEXT_SCHEMA_VERSION,
        scanLimits,
        clock,
    )

    val activeStreamCount: Int get() = decoder.activeStreamCount

    fun reset() = decoder.reset()

    /** Quarantines a stream after context-bound native adapter validation fails. */
    @JvmOverloads
    fun quarantine(streamId: ByteArray, nowMillis: Long? = null) {
        if (nowMillis == null) decoder.quarantine(streamId)
        else decoder.quarantine(streamId, nowMillis)
    }

    @JvmOverloads
    fun ingest(frameText: String, nowMillis: Long? = null): OfflineCashQrStreamProgressV1 {
        val result = if (nowMillis == null) decoder.ingest(frameText)
        else decoder.ingestAt(frameText, nowMillis)
        val message = result.message
        return OfflineCashQrStreamProgressV1(
            message?.let(OfflineCashQrStreamCodecV1::completedPeerText),
            result.payloadKind,
            result.streamId,
            result.receivedDataFrames,
            result.totalDataFrames,
            result.recoveredDataFrames,
            result.isDuplicate,
        )
    }
}
