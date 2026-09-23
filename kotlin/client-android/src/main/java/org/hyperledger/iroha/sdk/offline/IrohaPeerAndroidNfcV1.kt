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

/** Exact ISO SELECT AID envelope used before the KAGEMUSHA command inventory. */
internal object IrohaPeerNfcAidSelectionV1 {
    private val prefix = byteArrayOf(0x00, 0xa4.toByte(), 0x04, 0x00)
    private val success = byteArrayOf(0x90.toByte(), 0x00)

    fun command(): ByteArray = prefix + byteArrayOf(IrohaPeerNfcV1.APPLICATION_IDENTIFIER_SIZE.toByte()) +
        IrohaPeerNfcV1.applicationIdentifier() + byteArrayOf(0x00)

    fun isSelect(apdu: ByteArray): Boolean =
        apdu.size >= 2 && apdu[0] == prefix[0] && apdu[1] == prefix[1]

    fun accepts(apdu: ByteArray): Boolean {
        val expected = command()
        return apdu.contentEquals(expected) ||
            apdu.contentEquals(expected.copyOf(expected.size - 1))
    }

    fun succeeded(response: ByteArray): Boolean = response.contentEquals(success)
}

/** Thin Android IsoDep transceiver around the transport-neutral APDU codec. */
class IrohaPeerIsoDepTransceiverV1 private constructor(
    private val isoDep: IsoDep,
    private val operationTimeoutMillis: Int,
) : Closeable {
    init { require(operationTimeoutMillis in 1..120_000) }
    private var aidSelected = false

    val localLimits: IrohaPeerNfcLimitsV1
        get() = IrohaPeerIsoDepLimitsV1.derive(
            isoDep.maxTransceiveLength,
            isoDep.isExtendedLengthApduSupported,
        )

    @Throws(IOException::class)
    fun connect() {
        if (!isoDep.isConnected) {
            aidSelected = false
            isoDep.connect()
        }
        isoDep.timeout = operationTimeoutMillis
        if (!aidSelected) {
            val response = isoDep.transceive(IrohaPeerNfcAidSelectionV1.command())
            if (!IrohaPeerNfcAidSelectionV1.succeeded(response)) {
                throw IOException("KAGEMUSHA NFC application selection failed")
            }
            aidSelected = true
        }
    }

    @Throws(IOException::class)
    fun transceive(command: IrohaPeerNfcCommandV1): IrohaPeerNfcApduResponseV1 {
        if (!isoDep.isConnected || !aidSelected) throw IOException("KAGEMUSHA NFC application is not selected")
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
        aidSelected = false
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
                try {
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
                } catch (_: Exception) {
                    receiver.rejectPayment(context)
                    respond.respond(IrohaPeerNfcApduResponseV1(statusWord = IrohaPeerNfcStatusWordV1.STORAGE_FAILURE))
                }
            }
            else -> throw IllegalStateException("unknown NFC receiver disposition")
        }
    }

    override fun onDeactivated(reason: Int) {
        activation.invalidate()
        receiver.abandonPendingPayment()
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

/** Exactly one response crosses the synchronous/asynchronous HostApduService return boundary. */
internal class IrohaPeerNfcApduReplyGateV1(private val post: (ByteArray) -> Unit) {
    private val lock = Any()
    private var returned = false
    private var replied = false
    private var direct: ByteArray? = null

    fun deliver(encoded: ByteArray) {
        synchronized(lock) {
            if (replied) return
            replied = true
            if (returned) post(encoded) else direct = encoded
        }
    }

    fun finish(): ByteArray? = synchronized(lock) {
        returned = true
        direct
    }
}

/** Each SELECT or RF deactivation invalidates responses from the previous activation. */
internal class IrohaPeerNfcApduSelectionStateV1 {
    private var epoch = 0L
    var isSelected = false
        private set

    fun select(apdu: ByteArray): Boolean {
        epoch += 1
        isSelected = IrohaPeerNfcAidSelectionV1.accepts(apdu)
        return isSelected
    }

    fun currentEpoch(): Long? = if (isSelected) epoch else null

    fun isCurrent(expected: Long): Boolean = isSelected && epoch == expected

    fun deactivate() {
        epoch += 1
        isSelected = false
    }
}

/** HostApduService base which supports asynchronous durable staging. */
abstract class IrohaPeerAsyncHostApduServiceV1 : HostApduService() {
    private val responseHandler: Handler by lazy { Handler(Looper.getMainLooper()) }
    private val selection = IrohaPeerNfcApduSelectionStateV1()
    protected abstract val commandHandler: IrohaPeerNfcAsyncCommandHandlerV1

    final override fun processCommandApdu(commandApdu: ByteArray?, extras: Bundle?): ByteArray? {
        if (commandApdu == null) return failure(IrohaPeerNfcStatusWordV1.WRONG_LENGTH)
        if (IrohaPeerNfcAidSelectionV1.isSelect(commandApdu)) {
            val wasSelected = selection.isSelected
            val accepted = selection.select(commandApdu)
            if (wasSelected) commandHandler.onDeactivated(DEACTIVATION_DESELECTED)
            return failure(if (accepted) IrohaPeerNfcStatusWordV1.SUCCESS else IrohaPeerNfcStatusWordV1.NOT_FOUND)
        }
        val epoch = selection.currentEpoch()
            ?: return failure(IrohaPeerNfcStatusWordV1.CONDITIONS_NOT_SATISFIED)
        val command = try {
            IrohaPeerNfcAPDUCodecV1.decode(commandApdu)
        } catch (_: IllegalArgumentException) {
            return failure(IrohaPeerNfcStatusWordV1.WRONG_DATA)
        }
        val gate = IrohaPeerNfcApduReplyGateV1 { encoded ->
            responseHandler.post {
                if (selection.isCurrent(epoch)) sendResponseApdu(encoded)
            }
        }
        try {
            commandHandler.handle(command) { response -> gate.deliver(response.encode()) }
        } catch (_: IllegalArgumentException) {
            gate.deliver(failure(IrohaPeerNfcStatusWordV1.WRONG_DATA))
        } catch (_: IllegalStateException) {
            gate.deliver(failure(IrohaPeerNfcStatusWordV1.CONDITIONS_NOT_SATISFIED))
        }
        return gate.finish()
    }

    final override fun onDeactivated(reason: Int) {
        selection.deactivate()
        commandHandler.onDeactivated(reason)
    }

    private fun failure(status: IrohaPeerNfcStatusWordV1): ByteArray =
        IrohaPeerNfcApduResponseV1(statusWord = status).encode()
}
