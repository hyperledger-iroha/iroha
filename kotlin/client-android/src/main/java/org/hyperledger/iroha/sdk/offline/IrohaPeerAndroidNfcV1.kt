package org.hyperledger.iroha.sdk.offline

import android.nfc.Tag
import android.nfc.cardemulation.HostApduService
import android.nfc.tech.IsoDep
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import java.io.Closeable
import java.io.IOException

/** ISO/IEC 7816 status words used by the KAGEMUSHA V1 Android boundary. */
enum class IrohaPeerNfcStatusWordV1(val code: Int) {
    SUCCESS(0x9000),
    STORAGE_FAILURE(0x6581),
    WRONG_LENGTH(0x6700),
    SECURITY_STATUS_NOT_SATISFIED(0x6982),
    CONDITIONS_NOT_SATISFIED(0x6985),
    WRONG_DATA(0x6a80),
    NOT_FOUND(0x6a82),
    INSTRUCTION_NOT_SUPPORTED(0x6d00),
    CLASS_NOT_SUPPORTED(0x6e00);

    companion object {
        @JvmStatic fun fromCode(code: Int): IrohaPeerNfcStatusWordV1? =
            values().firstOrNull { it.code == code }
    }
}

/** Immutable NFC response (`data || SW1 || SW2`). */
class IrohaPeerNfcApduResponseV1(
    data: ByteArray = byteArrayOf(),
    @JvmField val statusWord: IrohaPeerNfcStatusWordV1,
) {
    private val body = data.copyOf()
    fun data(): ByteArray = body.copyOf()
    fun encode(): ByteArray = body + byteArrayOf(
        (statusWord.code ushr 8).toByte(),
        statusWord.code.toByte(),
    )

    fun toReaderResponse(): IrohaPeerNfcReaderResponseV1 = IrohaPeerNfcReaderResponseV1(
        body,
        requireNotNull(IrohaPeerNfcReaderStatusV1.fromCode(statusWord.code)),
    )

    companion object {
        @JvmStatic fun decode(raw: ByteArray): IrohaPeerNfcApduResponseV1 {
            require(raw.size >= 2 && raw.size <= IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES + 2)
            val code = ((raw[raw.size - 2].toInt() and 0xff) shl 8) or
                (raw[raw.size - 1].toInt() and 0xff)
            return IrohaPeerNfcApduResponseV1(
                raw.copyOfRange(0, raw.size - 2),
                requireNotNull(IrohaPeerNfcStatusWordV1.fromCode(code)),
            )
        }
    }
}

/** Computes conservative command/response limits from Android IsoDep capabilities. */
object IrohaPeerIsoDepLimitsV1 {
    @JvmStatic fun derive(
        maximumTransceiveLength: Int,
        supportsExtendedLengthApdu: Boolean,
    ): IrohaPeerNfcLimitsV1 {
        require(maximumTransceiveLength > 16)
        val envelope = if (supportsExtendedLengthApdu) 9 else 7
        return IrohaPeerNfcLimitsV1(
            maximumReadChunkBytes = minOf(
                IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES,
                maximumTransceiveLength - 2,
                if (supportsExtendedLengthApdu) Int.MAX_VALUE else 256,
            ),
            maximumWriteChunkBytes = minOf(
                IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES,
                maximumTransceiveLength - envelope - 4,
                if (supportsExtendedLengthApdu) Int.MAX_VALUE else 240,
            ),
        )
    }
}

/** IsoDep accepted a command, but its response could not be observed. */
class IrohaPeerNfcAmbiguousTransceiveExceptionV1(cause: IOException) :
    IOException("NFC command response is unknown", cause), IrohaPeerNfcAmbiguousResponseErrorV1

/** Thin Android IsoDep transceiver around the transport-neutral APDU codec. */
class IrohaPeerIsoDepTransceiverV1 private constructor(
    private val isoDep: IsoDep,
    private val operationTimeoutMillis: Int,
) : Closeable {
    init { require(operationTimeoutMillis in 1..120_000) }

    val localLimits: IrohaPeerNfcLimitsV1
        get() = IrohaPeerIsoDepLimitsV1.derive(
            isoDep.maxTransceiveLength,
            isoDep.isExtendedLengthApduSupported,
        )

    @Throws(IOException::class)
    fun connect() {
        if (!isoDep.isConnected) isoDep.connect()
        isoDep.timeout = operationTimeoutMillis
    }

    @Throws(IOException::class)
    fun transceive(command: IrohaPeerNfcCommandV1): IrohaPeerNfcApduResponseV1 {
        if (!isoDep.isConnected) throw IOException("IsoDep is not connected")
        val encoded = IrohaPeerNfcAPDUCodecV1.encode(command)
        if (encoded.size > isoDep.maxTransceiveLength) throw IOException("NFC APDU exceeds tag limit")
        val response = try {
            isoDep.transceive(encoded)
        } catch (failure: IOException) {
            throw IrohaPeerNfcAmbiguousTransceiveExceptionV1(failure)
        }
        return IrohaPeerNfcApduResponseV1.decode(response)
    }

    @Throws(IOException::class)
    fun transceiveForReader(command: IrohaPeerNfcCommandV1): IrohaPeerNfcReaderResponseV1 =
        transceive(command).toReaderResponse()

    override fun close() {
        if (isoDep.isConnected) isoDep.close()
    }

    companion object {
        @JvmStatic @JvmOverloads
        fun from(tag: Tag, operationTimeoutMillis: Int = 10_000): IrohaPeerIsoDepTransceiverV1? =
            IsoDep.get(tag)?.let { IrohaPeerIsoDepTransceiverV1(it, operationTimeoutMillis) }
    }
}

fun interface IrohaPeerNfcApduResponseHandlerV1 {
    fun respond(response: IrohaPeerNfcApduResponseV1)
}

interface IrohaPeerNfcAsyncCommandHandlerV1 {
    fun handle(command: IrohaPeerNfcCommandV1, respond: IrohaPeerNfcApduResponseHandlerV1)
    fun onDeactivated(reason: Int)
}

/** Completion for the one irreversible receiver transition. */
fun interface IrohaPeerNfcPaymentAdmissionCompletionV1 {
    fun complete(record: IrohaPeerNfcDurablePaymentAdmissionV1?, error: Throwable?)
}

/** Hardware-backed receiver staging. The completion may run asynchronously. */
fun interface IrohaPeerNfcDurableTransitionHandlerV1 {
    fun stagePayment(
        context: IrohaPeerNfcPaymentAdmissionContextV1,
        completion: IrohaPeerNfcPaymentAdmissionCompletionV1,
    )
}

/** Direct Request/Payment/ACK HCE bridge. */
class IrohaPeerNfcReceiverApduBridgeV1(
    private val receiver: IrohaPeerNfcReceiverSessionV1,
    private val durableTransitions: IrohaPeerNfcDurableTransitionHandlerV1,
) : IrohaPeerNfcAsyncCommandHandlerV1 {
    private val activation = ActivationEpoch()

    override fun handle(
        command: IrohaPeerNfcCommandV1,
        respond: IrohaPeerNfcApduResponseHandlerV1,
    ) {
        val epoch = activation.capture()
        val result = receiver.handle(command)
        when (result) {
            is ByteArray -> respond.respond(
                IrohaPeerNfcApduResponseV1(result, IrohaPeerNfcStatusWordV1.SUCCESS),
            )
            is IrohaPeerNfcPaymentAdmissionDispositionV1.Immediate -> respond.respond(
                IrohaPeerNfcApduResponseV1(statusWord = IrohaPeerNfcStatusWordV1.SUCCESS),
            )
            is IrohaPeerNfcPaymentAdmissionDispositionV1.Persist -> {
                val context = result.context
                durableTransitions.stagePayment(context) { record, error ->
                    activation.perform(epoch) {
                        val response = if (error != null || record == null) {
                            receiver.rejectPayment(context)
                            IrohaPeerNfcApduResponseV1(statusWord = IrohaPeerNfcStatusWordV1.STORAGE_FAILURE)
                        } else {
                            try {
                                receiver.completePayment(context, record)
                                IrohaPeerNfcApduResponseV1(statusWord = IrohaPeerNfcStatusWordV1.SUCCESS)
                            } catch (_: Throwable) {
                                receiver.rejectPayment(context)
                                IrohaPeerNfcApduResponseV1(statusWord = IrohaPeerNfcStatusWordV1.STORAGE_FAILURE)
                            }
                        }
                        respond.respond(response)
                    }
                }
            }
            else -> throw IllegalStateException("unknown NFC receiver disposition")
        }
    }

    override fun onDeactivated(reason: Int) {
        activation.invalidate()
    }

    private class ActivationEpoch {
        private var value = 0L
        @Synchronized fun capture(): Long = value
        @Synchronized fun invalidate() { value += 1 }
        @Synchronized fun perform(expected: Long, action: () -> Unit) {
            if (value == expected) action()
        }
    }
}

/** HostApduService base which supports asynchronous durable staging. */
abstract class IrohaPeerAsyncHostApduServiceV1 : HostApduService() {
    private val responseHandler: Handler by lazy { Handler(Looper.getMainLooper()) }
    protected abstract val commandHandler: IrohaPeerNfcAsyncCommandHandlerV1

    final override fun processCommandApdu(commandApdu: ByteArray?, extras: Bundle?): ByteArray? {
        if (commandApdu == null) return failure(IrohaPeerNfcStatusWordV1.WRONG_LENGTH)
        val command = try {
            IrohaPeerNfcAPDUCodecV1.decode(commandApdu)
        } catch (_: IllegalArgumentException) {
            return failure(IrohaPeerNfcStatusWordV1.WRONG_DATA)
        }
        var synchronous = true
        var direct: ByteArray? = null
        commandHandler.handle(command) { response ->
            val encoded = response.encode()
            if (synchronous) direct = encoded else responseHandler.post { sendResponseApdu(encoded) }
        }
        synchronous = false
        return direct
    }

    final override fun onDeactivated(reason: Int) {
        commandHandler.onDeactivated(reason)
    }

    private fun failure(status: IrohaPeerNfcStatusWordV1): ByteArray =
        IrohaPeerNfcApduResponseV1(statusWord = status).encode()
}
