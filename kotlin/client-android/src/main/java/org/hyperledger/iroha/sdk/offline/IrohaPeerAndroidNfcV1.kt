package org.hyperledger.iroha.sdk.offline

import android.nfc.Tag
import android.nfc.cardemulation.HostApduService
import android.nfc.tech.IsoDep
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import java.io.Closeable
import java.io.IOException
import java.lang.ref.WeakReference
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ScheduledThreadPoolExecutor
import java.util.concurrent.SynchronousQueue
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/** ISO/IEC 7816 status words used by the Iroha peer NFC V1 Android boundary. */
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
            entries.firstOrNull { it.code == code }
    }
}

/** Immutable NFC response (`data || SW1 || SW2`). */
class IrohaPeerNfcApduResponseV1(
    data: ByteArray = byteArrayOf(),
    val statusWord: IrohaPeerNfcStatusWordV1,
) {
    private val responseData = data.copyOf()
    val data: ByteArray get() = responseData.copyOf()
    val encoded: ByteArray
        get() = responseData + byteArrayOf(
            (statusWord.code ushr 8).toByte(),
            statusWord.code.toByte(),
        )

    override fun equals(other: Any?): Boolean = other is IrohaPeerNfcApduResponseV1 &&
        statusWord == other.statusWord && responseData.contentEquals(other.responseData)

    override fun hashCode(): Int = 31 * statusWord.hashCode() + responseData.contentHashCode()

    companion object {
        /** Parses a raw tag response and rejects unrecognized status words. */
        @JvmStatic fun decode(raw: ByteArray): IrohaPeerNfcApduResponseV1 {
            require(raw.size in 2..(IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES + 2)) {
                "NFC response length is outside the protocol bound"
            }
            val code = ((raw[raw.size - 2].toInt() and 0xff) shl 8) or
                (raw.last().toInt() and 0xff)
            val status = IrohaPeerNfcStatusWordV1.fromCode(code)
                ?: throw IllegalArgumentException("Unsupported NFC status word")
            return IrohaPeerNfcApduResponseV1(raw.copyOfRange(0, raw.size - 2), status)
        }
    }

    /** Maps the Android APDU result into the portable durable-reader API. */
    fun toReaderResponse(): IrohaPeerNfcReaderResponseV1 = IrohaPeerNfcReaderResponseV1(
        responseData,
        requireNotNull(IrohaPeerNfcReaderStatusV1.fromCode(statusWord.code)) {
            "Unsupported portable NFC status word"
        },
    )
}

/** Computes conservative command/response limits from Android IsoDep capabilities. */
object IrohaPeerIsoDepLimitsV1 {
    private const val SHORT_READ_RESPONSE_LIMIT = 256
    private const val SHORT_WRITE_OVERHEAD = 57 // CLA..Lc + session/hash/u32
    private const val EXTENDED_WRITE_OVERHEAD = 59 // CLA..00/Lc16 + session/hash/u32

    @JvmStatic fun derive(
        maximumTransceiveLength: Int,
        supportsExtendedLengthApdu: Boolean,
    ): IrohaPeerNfcLimitsV1 {
        require(maximumTransceiveLength > SHORT_WRITE_OVERHEAD) {
            "IsoDep transceive limit is too small for NFC V1"
        }
        val readTransportLimit = if (supportsExtendedLengthApdu) {
            maximumTransceiveLength - 2 // SW1/SW2
        } else {
            minOf(SHORT_READ_RESPONSE_LIMIT, maximumTransceiveLength - 2)
        }
        val writeTransportLimit = maximumTransceiveLength - if (supportsExtendedLengthApdu) {
            EXTENDED_WRITE_OVERHEAD
        } else {
            SHORT_WRITE_OVERHEAD
        }
        return IrohaPeerNfcLimitsV1(
            maximumReadChunkBytes = minOf(IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES, readTransportLimit),
            maximumWriteChunkBytes = minOf(IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES, writeTransportLimit,
                if (supportsExtendedLengthApdu) IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES else 203),
        )
    }
}

/** IsoDep accepted a command, but its response could not be observed. */
class IrohaPeerNfcAmbiguousTransceiveExceptionV1(
    cause: IOException,
) : IOException("NFC command response is unknown", cause),
    IrohaPeerNfcAmbiguousResponseErrorV1

/** Thin Android IsoDep transceiver around the transport-neutral V1 APDU codec. */
class IrohaPeerIsoDepTransceiverV1 private constructor(
    private val isoDep: IsoDep,
    private val operationTimeoutMillis: Int,
) : Closeable {
    init {
        require(operationTimeoutMillis in 1..120_000)
    }

    val maximumTransceiveLength: Int get() = isoDep.maxTransceiveLength
    val supportsExtendedLengthApdu: Boolean get() = isoDep.isExtendedLengthApduSupported
    /** Pass this to request planning and the two-tap reducer. */
    val localLimits: IrohaPeerNfcLimitsV1
        get() = IrohaPeerIsoDepLimitsV1.derive(
            maximumTransceiveLength,
            supportsExtendedLengthApdu,
        )

    @Throws(IOException::class)
    fun connect() {
        if (!isoDep.isConnected) isoDep.connect()
        isoDep.timeout = operationTimeoutMillis
    }

    /** Transmits one typed command; non-success status words remain visible. */
    @Throws(IOException::class)
    fun transceive(command: IrohaPeerNfcCommandV1): IrohaPeerNfcApduResponseV1 {
        if (!isoDep.isConnected) throw IOException("IsoDep is not connected")
        val apdu = IrohaPeerNfcAPDUCodecV1.encode(command)
        if (apdu.size > isoDep.maxTransceiveLength) {
            throw IOException("NFC APDU exceeds this tag's transceive limit")
        }
        val rawResponse = try {
            isoDep.transceive(apdu)
        } catch (failure: IOException) {
            throw IrohaPeerNfcAmbiguousTransceiveExceptionV1(failure)
        }
        return IrohaPeerNfcApduResponseV1.decode(rawResponse)
    }

    /** Direct adapter for [IrohaPeerNfcReaderExchangeV1]. */
    @Throws(IOException::class)
    fun transceiveForReader(command: IrohaPeerNfcCommandV1): IrohaPeerNfcReaderResponseV1 =
        transceive(command).toReaderResponse()

    override fun close() {
        if (isoDep.isConnected) isoDep.close()
    }

    companion object {
        /** Returns null when the discovered tag does not expose ISO-DEP. */
        @JvmStatic
        @JvmOverloads
        fun from(tag: Tag, operationTimeoutMillis: Int = 10_000): IrohaPeerIsoDepTransceiverV1? =
            IsoDep.get(tag)?.let { IrohaPeerIsoDepTransceiverV1(it, operationTimeoutMillis) }
    }
}

fun interface IrohaPeerNfcApduResponseHandlerV1 {
    fun respond(response: IrohaPeerNfcApduResponseV1)
}

/**
 * One stable, deactivation-aware command handler owned by an async HCE service.
 * Implementations must never carry tap-scoped callbacks across [onDeactivated].
 */
interface IrohaPeerNfcAsyncCommandHandlerV1 {
    fun handle(
        command: IrohaPeerNfcCommandV1,
        respond: IrohaPeerNfcApduResponseHandlerV1,
    )

    fun onDeactivated(reason: Int)
}

/**
 * Async HCE boundary; returning null lets COMMIT wait for durable storage.
 * [commandHandler] must return the same handler for the service lifetime so RF
 * deactivation can invalidate the exact operation that accepted the command.
 */
abstract class IrohaPeerAsyncHostApduServiceV1 : HostApduService() {
    private val activationEpoch = IrohaPeerNfcActivationEpochV1()
    private val responseHandler: Handler by lazy { Handler(Looper.getMainLooper()) }

    protected abstract val commandHandler: IrohaPeerNfcAsyncCommandHandlerV1
    private val stableCommandHandler: IrohaPeerNfcAsyncCommandHandlerV1 by lazy {
        commandHandler
    }

    protected open fun didDeactivate(reason: Int) = Unit

    /**
     * Enqueues an asynchronous APDU response behind the current main-loop turn.
     * This guarantees that [sendResponseApdu] cannot run until the framework has
     * observed the null return from [processCommandApdu].
     */
    protected open fun postAsyncResponse(action: () -> Unit): Boolean =
        responseHandler.post(action)

    final override fun processCommandApdu(commandApdu: ByteArray?, extras: Bundle?): ByteArray? {
        val commandEpoch = activationEpoch.capture()
        if (commandApdu == null) return failure(IrohaPeerNfcStatusWordV1.WRONG_LENGTH)
        val command = try {
            IrohaPeerNfcAPDUCodecV1.decode(commandApdu)
        } catch (_: IllegalArgumentException) {
            return failure(IrohaPeerNfcApduFailureClassifierV1.classify(commandApdu))
        }
        val responseGate = IrohaPeerNfcHceResponseGateV1(
            commandEpoch,
            activationEpoch,
            post = ::postAsyncResponse,
            send = ::sendResponseApdu,
        )
        val failureResponse = try {
            stableCommandHandler.handle(command, responseGate)
            null
        } catch (_: SecurityException) {
            IrohaPeerNfcApduResponseV1(statusWord =
                IrohaPeerNfcStatusWordV1.SECURITY_STATUS_NOT_SATISFIED)
        } catch (_: IllegalStateException) {
            IrohaPeerNfcApduResponseV1(statusWord =
                IrohaPeerNfcStatusWordV1.CONDITIONS_NOT_SATISFIED)
        } catch (_: IllegalArgumentException) {
            IrohaPeerNfcApduResponseV1(statusWord = IrohaPeerNfcStatusWordV1.WRONG_DATA)
        } catch (_: IOException) {
            IrohaPeerNfcApduResponseV1(statusWord = IrohaPeerNfcStatusWordV1.STORAGE_FAILURE)
        } catch (_: Throwable) {
            IrohaPeerNfcApduResponseV1(statusWord = IrohaPeerNfcStatusWordV1.STORAGE_FAILURE)
        }
        return responseGate.finishInvocation(failureResponse)
    }

    final override fun onDeactivated(reason: Int) {
        // Invalidate the RF activation before cancelling tap-scoped storage
        // work, so no callback can leak into a later service activation.
        activationEpoch.invalidate()
        stableCommandHandler.onDeactivated(reason)
        didDeactivate(reason)
    }

    private fun failure(status: IrohaPeerNfcStatusWordV1): ByteArray =
        IrohaPeerNfcApduResponseV1(statusWord = status).encoded
}

/**
 * Race-proof handoff between an HCE command invocation and its response.
 * Responses produced before the handler returns are returned synchronously.
 * Later responses are posted, exactly once, and re-check the RF epoch at the
 * time the posted action executes.
 */
internal class IrohaPeerNfcHceResponseGateV1(
    private val capturedEpoch: Long,
    private val activationEpoch: IrohaPeerNfcActivationEpochV1,
    private val post: (() -> Unit) -> Boolean,
    private val send: (ByteArray) -> Unit,
) : IrohaPeerNfcApduResponseHandlerV1 {
    private val lock = Any()
    private var invocationOpen = true
    private var completed = false
    private var synchronousResponse: ByteArray? = null

    override fun respond(response: IrohaPeerNfcApduResponseV1) {
        val asynchronousResponse = synchronized(lock) {
            if (completed) return
            completed = true
            val encoded = response.encoded
            if (invocationOpen) {
                synchronousResponse = encoded
                null
            } else {
                encoded
            }
        }
        if (asynchronousResponse != null) {
            // A rejected post deliberately consumes this completion. Sending
            // inline could race the null framework return; the reader's bounded
            // timeout/retry path is the only safe recovery.
            post {
                activationEpoch.performIfCurrent(capturedEpoch) {
                    send(asynchronousResponse)
                }
            }
        }
    }

    /** Closes the invocation and returns its direct response, if one exists. */
    fun finishInvocation(fallback: IrohaPeerNfcApduResponseV1? = null): ByteArray? =
        synchronized(lock) {
            invocationOpen = false
            synchronousResponse?.copyOf()?.also { synchronousResponse = null }
                ?: if (!completed && fallback != null) {
                    completed = true
                    fallback.encoded
                } else {
                    null
                }
        }
}

/** Serializes asynchronous HCE callbacks against RF activation changes. */
internal class IrohaPeerNfcActivationEpochV1 {
    private val lock = Any()
    private var epoch = 0L

    fun capture(): Long = synchronized(lock) { epoch }

    fun invalidate() = synchronized(lock) {
        epoch += 1
    }

    fun performIfCurrent(capturedEpoch: Long, action: () -> Unit): Boolean = synchronized(lock) {
        if (capturedEpoch != epoch) return@synchronized false
        action()
        true
    }
}

/**
 * Maps strict portable-codec failures to the ISO/IEC 7816 status that identifies
 * the failing APDU layer. Semantic field failures remain WRONG_DATA.
 */
internal object IrohaPeerNfcApduFailureClassifierV1 {
    private data class Envelope(
        val data: ByteArray,
        val expectedLength: Int?,
    )

    fun classify(apdu: ByteArray): IrohaPeerNfcStatusWordV1 {
        if (apdu.size < 4) return IrohaPeerNfcStatusWordV1.WRONG_LENGTH
        val cla = apdu[0].toInt() and 0xff
        val instruction = apdu[1].toInt() and 0xff
        val p1 = apdu[2].toInt() and 0xff
        val p2 = apdu[3].toInt() and 0xff
        val envelope = decodeEnvelope(apdu) ?: return IrohaPeerNfcStatusWordV1.WRONG_LENGTH

        if (cla == 0 && instruction == 0xa4) {
            if (p1 != 0x04 || p2 != 0) return IrohaPeerNfcStatusWordV1.WRONG_DATA
            if (envelope.data.size != IrohaPeerNfcV1.APPLICATION_IDENTIFIER_SIZE ||
                envelope.expectedLength != 256
            ) {
                return IrohaPeerNfcStatusWordV1.WRONG_LENGTH
            }
            return if (IrohaPeerNfcV1.matchesApplicationIdentifier(envelope.data)) {
                IrohaPeerNfcStatusWordV1.WRONG_DATA
            } else {
                IrohaPeerNfcStatusWordV1.NOT_FOUND
            }
        }
        if (cla != IrohaPeerNfcV1.COMMAND_CLASS) {
            return IrohaPeerNfcStatusWordV1.CLASS_NOT_SUPPORTED
        }
        val typedInstruction = IrohaPeerNfcInstructionV1.fromCode(instruction)
            ?: return IrohaPeerNfcStatusWordV1.INSTRUCTION_NOT_SUPPORTED
        if (p1 != 0 || p2 != 0) return IrohaPeerNfcStatusWordV1.WRONG_DATA

        val structurallyValid = when (typedInstruction) {
            IrohaPeerNfcInstructionV1.GET_INFO ->
                envelope.data.isEmpty() && envelope.expectedLength == IrohaPeerNfcV1.INFO_BYTES
            IrohaPeerNfcInstructionV1.READ_REQUEST,
            IrohaPeerNfcInstructionV1.READ_ACKNOWLEDGEMENT ->
                envelope.data.size == 52 &&
                    envelope.expectedLength?.let {
                        it in 1..IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES
                    } == true
            IrohaPeerNfcInstructionV1.BEGIN_PAYMENT ->
                envelope.data.size == 48 + IrohaPeerWireMessageV1.HEADER_LENGTH &&
                    envelope.expectedLength == null
            IrohaPeerNfcInstructionV1.WRITE ->
                envelope.data.size in 53..(52 + IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES) &&
                    envelope.expectedLength == null
            IrohaPeerNfcInstructionV1.COMMIT,
            IrohaPeerNfcInstructionV1.CONFIRM_ACKNOWLEDGEMENT ->
                envelope.data.size == 80 && envelope.expectedLength == null
            IrohaPeerNfcInstructionV1.GET_STATUS ->
                envelope.data.size == 48 &&
                    envelope.expectedLength == IrohaPeerNfcV1.STATUS_BYTES
        }
        return if (structurallyValid) {
            IrohaPeerNfcStatusWordV1.WRONG_DATA
        } else {
            IrohaPeerNfcStatusWordV1.WRONG_LENGTH
        }
    }

    private fun decodeEnvelope(apdu: ByteArray): Envelope? {
        if (apdu.size < 4) return null
        if (apdu.size == 4) return Envelope(byteArrayOf(), null)
        val firstLength = apdu[4].toInt() and 0xff
        if (firstLength != 0) {
            if (apdu.size == 5) return Envelope(byteArrayOf(), firstLength)
            if (apdu.size == 5 + firstLength) {
                return Envelope(apdu.copyOfRange(5, apdu.size), null)
            }
            if (apdu.size == 6 + firstLength) {
                val rawLength = apdu.last().toInt() and 0xff
                return Envelope(
                    apdu.copyOfRange(5, 5 + firstLength),
                    if (rawLength == 0) 256 else rawLength,
                )
            }
            return null
        }
        if (apdu.size == 5) return Envelope(byteArrayOf(), 256)
        if (apdu.size < 7) return null
        val extendedLength = ((apdu[5].toInt() and 0xff) shl 8) or
            (apdu[6].toInt() and 0xff)
        if (apdu.size == 7) {
            if (extendedLength in 1..256) return null
            return Envelope(byteArrayOf(), if (extendedLength == 0) 65_536 else extendedLength)
        }
        if (extendedLength == 0) return null
        if (apdu.size == 7 + extendedLength) {
            if (extendedLength <= 0xff) return null
            return Envelope(apdu.copyOfRange(7, apdu.size), null)
        }
        if (apdu.size == 9 + extendedLength) {
            val offset = 7 + extendedLength
            val rawLength = ((apdu[offset].toInt() and 0xff) shl 8) or
                (apdu[offset + 1].toInt() and 0xff)
            val expectedLength = if (rawLength == 0) 65_536 else rawLength
            if (extendedLength <= 0xff && expectedLength <= 256) return null
            return Envelope(
                apdu.copyOfRange(7, offset),
                expectedLength,
            )
        }
        return null
    }
}

fun interface IrohaPeerNfcDurableAdmissionCompletionV1 {
    fun complete(record: IrohaPeerNfcDurablePaymentAdmissionV1?, error: Throwable?)
}

fun interface IrohaPeerNfcDurableAdmissionHandlerV1 {
    /**
     * Atomically store the exact 244-byte IPA1 with first-write-wins semantics,
     * then return the decoded stored record. A conflicting existing value must
     * fail instead of being replaced. Timeout makes completion ambiguous, so
     * this callback must be idempotent.
     */
    fun admit(
        context: IrohaPeerNfcPaymentAdmissionContextV1,
        completion: IrohaPeerNfcDurableAdmissionCompletionV1,
    )
}

fun interface IrohaPeerNfcDurableCommitCompletionV1 {
    fun complete(record: IrohaPeerNfcDurableAcknowledgementV1?, error: Throwable?)
}

fun interface IrohaPeerNfcDurableCommitHandlerV1 {
    /**
     * Persist the exact payment outcome and IDA1 ACK before completing. Retaps
     * retry the same context after timeout/RF loss, so storage must be
     * idempotent and return the previously persisted exact record.
     */
    fun commit(
        context: IrohaPeerNfcCommitContextV1,
        completion: IrohaPeerNfcDurableCommitCompletionV1,
    )
}

internal fun interface IrohaPeerNfcDurabilityTimeoutV1 {
    fun cancel()
}

internal fun interface IrohaPeerNfcDurabilityTimeoutSchedulerV1 {
    fun schedule(delayMillis: Long, action: () -> Unit): IrohaPeerNfcDurabilityTimeoutV1
}

private object IrohaPeerNfcDefaultDurabilityTimeoutSchedulerV1 :
    IrohaPeerNfcDurabilityTimeoutSchedulerV1 {
    private val executor = ScheduledThreadPoolExecutor(1) { runnable ->
        Thread(runnable, "iroha-peer-nfc-commit-timeout").apply { isDaemon = true }
    }.apply {
        setRemoveOnCancelPolicy(true)
    }

    override fun schedule(
        delayMillis: Long,
        action: () -> Unit,
    ): IrohaPeerNfcDurabilityTimeoutV1 {
        val future = executor.schedule(Runnable(action), delayMillis, TimeUnit.MILLISECONDS)
        return IrohaPeerNfcDurabilityTimeoutV1 { future.cancel(false) }
    }
}

internal fun interface IrohaPeerNfcDurabilityExecutorV1 {
    /** Returns false without blocking when the bounded worker is saturated. */
    fun execute(action: () -> Unit): Boolean
}

/**
 * One process-level application durability lease. A response timeout does not
 * release it: only the exact application callback (or synchronous throw) may
 * do so after persistence was invoked.
 */
internal class IrohaPeerNfcDurabilityLeaseV1 {
    private val lock = Any()
    private var owner: Any? = null

    fun acquire(token: Any): Boolean = synchronized(lock) {
        if (owner != null) return@synchronized false
        owner = token
        true
    }

    fun release(token: Any): Boolean = synchronized(lock) {
        if (owner !== token) return@synchronized false
        owner = null
        true
    }
}

private val irohaPeerNfcProcessDurabilityLeaseV1 = IrohaPeerNfcDurabilityLeaseV1()

private object IrohaPeerNfcDefaultDurabilityExecutorV1 : IrohaPeerNfcDurabilityExecutorV1 {
    private val executor = ThreadPoolExecutor(
        1,
        1,
        0,
        TimeUnit.MILLISECONDS,
        SynchronousQueue(),
        { runnable ->
            Thread(runnable, "iroha-peer-nfc-durability").apply { isDaemon = true }
        },
        ThreadPoolExecutor.AbortPolicy(),
    ).apply {
        // A SynchronousQueue has no capacity. Prestarting avoids a first-submit
        // handoff race and makes one-running-operation admission deterministic.
        prestartAllCoreThreads()
    }

    override fun execute(action: () -> Unit): Boolean = try {
        executor.execute(Runnable(action))
        true
    } catch (_: RejectedExecutionException) {
        false
    }
}

private object IrohaPeerNfcImmediateDurabilityExecutorV1 : IrohaPeerNfcDurabilityExecutorV1 {
    override fun execute(action: () -> Unit): Boolean {
        action()
        return true
    }
}

/**
 * Serialized receiver-session bridge for HCE. BEGIN_PAYMENT and COMMIT cannot
 * emit 9000 until their respective durable callbacks return the exact stored
 * admission or IDA1 record accepted by the portable receiver. A lost storage
 * callback fails closed before the reader timeout. RF deactivation and [reset]
 * invalidate the tap response without releasing the process durability lease.
 * Retries fail immediately until that exact callback or throw settles, so a
 * retained completion can never create an unbounded callback set.
 */
class IrohaPeerNfcReceiverApduBridgeV1 private constructor(
    private val receiver: IrohaPeerNfcReceiverSessionV1,
    private val durableAdmission: IrohaPeerNfcDurableAdmissionHandlerV1,
    private val durableCommit: IrohaPeerNfcDurableCommitHandlerV1,
    private val timeoutScheduler: IrohaPeerNfcDurabilityTimeoutSchedulerV1,
    private val durabilityTimeoutMillis: Long,
    private val durabilityExecutor: IrohaPeerNfcDurabilityExecutorV1,
    private val durabilityLease: IrohaPeerNfcDurabilityLeaseV1,
) : IrohaPeerNfcAsyncCommandHandlerV1 {
    constructor(
        receiver: IrohaPeerNfcReceiverSessionV1,
        durableAdmission: IrohaPeerNfcDurableAdmissionHandlerV1,
        durableCommit: IrohaPeerNfcDurableCommitHandlerV1,
    ) : this(
        receiver,
        durableAdmission,
        durableCommit,
        IrohaPeerNfcDefaultDurabilityTimeoutSchedulerV1,
        DEFAULT_DURABILITY_TIMEOUT_MILLIS,
        IrohaPeerNfcDefaultDurabilityExecutorV1,
        irohaPeerNfcProcessDurabilityLeaseV1,
    )

    internal constructor(
        receiver: IrohaPeerNfcReceiverSessionV1,
        durableAdmission: IrohaPeerNfcDurableAdmissionHandlerV1,
        durableCommit: IrohaPeerNfcDurableCommitHandlerV1,
        timeoutScheduler: IrohaPeerNfcDurabilityTimeoutSchedulerV1,
    ) : this(
        receiver,
        durableAdmission,
        durableCommit,
        timeoutScheduler,
        DEFAULT_DURABILITY_TIMEOUT_MILLIS,
        IrohaPeerNfcImmediateDurabilityExecutorV1,
        IrohaPeerNfcDurabilityLeaseV1(),
    )

    internal constructor(
        receiver: IrohaPeerNfcReceiverSessionV1,
        durableAdmission: IrohaPeerNfcDurableAdmissionHandlerV1,
        durableCommit: IrohaPeerNfcDurableCommitHandlerV1,
        timeoutScheduler: IrohaPeerNfcDurabilityTimeoutSchedulerV1,
        durabilityExecutor: IrohaPeerNfcDurabilityExecutorV1,
    ) : this(
        receiver,
        durableAdmission,
        durableCommit,
        timeoutScheduler,
        DEFAULT_DURABILITY_TIMEOUT_MILLIS,
        durabilityExecutor,
        IrohaPeerNfcDurabilityLeaseV1(),
    )

    internal constructor(
        receiver: IrohaPeerNfcReceiverSessionV1,
        durableAdmission: IrohaPeerNfcDurableAdmissionHandlerV1,
        durableCommit: IrohaPeerNfcDurableCommitHandlerV1,
        timeoutScheduler: IrohaPeerNfcDurabilityTimeoutSchedulerV1,
        durabilityExecutor: IrohaPeerNfcDurabilityExecutorV1,
        durabilityLease: IrohaPeerNfcDurabilityLeaseV1,
    ) : this(
        receiver,
        durableAdmission,
        durableCommit,
        timeoutScheduler,
        DEFAULT_DURABILITY_TIMEOUT_MILLIS,
        durabilityExecutor,
        durabilityLease,
    )

    private sealed class PendingOperation(
        val id: Long,
        respond: IrohaPeerNfcApduResponseHandlerV1,
        val leaseToken: Any,
    ) {
        var respond: IrohaPeerNfcApduResponseHandlerV1? = respond
        var timeout: IrohaPeerNfcDurabilityTimeoutV1? = null
        var responseAttached = true
        var durabilityStarted = false
    }

    private class PendingAdmission(
        id: Long,
        respond: IrohaPeerNfcApduResponseHandlerV1,
        leaseToken: Any,
        val context: IrohaPeerNfcPaymentAdmissionContextV1,
    ) : PendingOperation(id, respond, leaseToken)

    private class PendingCommit(
        id: Long,
        respond: IrohaPeerNfcApduResponseHandlerV1,
        leaseToken: Any,
        val context: IrohaPeerNfcCommitContextV1,
    ) : PendingOperation(id, respond, leaseToken)

    private class CompletedOperation(
        val timeout: IrohaPeerNfcDurabilityTimeoutV1?,
        val respond: IrohaPeerNfcApduResponseHandlerV1?,
        val response: IrohaPeerNfcApduResponseV1,
    )

    private val lock = Any()
    private var pendingOperation: PendingOperation? = null
    private var nextOperationId = 1L

    override fun handle(
        command: IrohaPeerNfcCommandV1,
        respond: IrohaPeerNfcApduResponseHandlerV1,
    ) {
        val attempt = synchronized(lock) {
            if (pendingOperation != null) {
                respond.respond(IrohaPeerNfcApduResponseV1(
                    statusWord = IrohaPeerNfcStatusWordV1.CONDITIONS_NOT_SATISFIED,
                ))
                return
            }
            val pending = when (command.type) {
                IrohaPeerNfcCommandTypeV1.BEGIN_PAYMENT -> when (
                    val disposition = receiver.preparePaymentAdmission(command)
                ) {
                    IrohaPeerNfcPaymentAdmissionDispositionV1.AlreadyAdmitted -> {
                        respond.respond(IrohaPeerNfcApduResponseV1(
                            statusWord = IrohaPeerNfcStatusWordV1.SUCCESS,
                        ))
                        return
                    }
                    is IrohaPeerNfcPaymentAdmissionDispositionV1.RequiresDurableAdmission ->
                        PendingAdmission(nextOperationId++, respond, Any(), disposition.context)
                }
                IrohaPeerNfcCommandTypeV1.COMMIT -> when (
                    val disposition = receiver.prepareCommit(command)
                ) {
                    IrohaPeerNfcCommitDispositionV1.AlreadyCommitted -> {
                        respond.respond(IrohaPeerNfcApduResponseV1(
                            statusWord = IrohaPeerNfcStatusWordV1.SUCCESS,
                        ))
                        return
                    }
                    is IrohaPeerNfcCommitDispositionV1.RequiresDurableCommit ->
                        PendingCommit(nextOperationId++, respond, Any(), disposition.context)
                }
                else -> {
                    respond.respond(processSynchronous(command))
                    return
                }
            }
            if (!durabilityLease.acquire(pending.leaseToken)) {
                respond.respond(IrohaPeerNfcApduResponseV1(
                    statusWord = IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
                ))
                return
            }
            pendingOperation = pending
            pending
        }

        val bridgeReference = WeakReference(this)
        val operationId = attempt.id
        val timeout = try {
            timeoutScheduler.schedule(durabilityTimeoutMillis) {
                bridgeReference.get()?.timeOut(operationId)
            }
        } catch (_: RuntimeException) {
            failBeforeDurabilityStart(operationId)
            return
        }
        val stillPending = synchronized(lock) {
            if (pendingOperation !== attempt) {
                false
            } else {
                attempt.timeout = timeout
                true
            }
        }
        if (!stillPending) {
            timeout.cancel()
            durabilityLease.release(attempt.leaseToken)
            return
        }

        // The worker closure intentionally does not retain the pending response
        // or this bridge. An application may retain a completion indefinitely;
        // its completion therefore holds only a weak bridge reference and ID.
        val invokeDurability: () -> Unit = when (attempt) {
            is PendingAdmission -> {
                val context = attempt.context
                val handler = durableAdmission
                val lease = durabilityLease
                val leaseToken = attempt.leaseToken
                val completionOnce = AtomicBoolean(false)
                val completion = IrohaPeerNfcDurableAdmissionCompletionV1 { record, error ->
                    if (completionOnce.compareAndSet(false, true)) {
                        try {
                            bridgeReference.get()?.completeAdmission(operationId, record, error)
                        } finally {
                            lease.release(leaseToken)
                        }
                    }
                }
                val action: () -> Unit = {
                    if (bridgeReference.get()?.claimDurabilityStart(operationId) == true) {
                        try {
                            handler.admit(context, completion)
                        } catch (failure: Throwable) {
                            completion.complete(null, failure)
                        }
                    } else {
                        lease.release(leaseToken)
                    }
                }
                action
            }
            is PendingCommit -> {
                val context = attempt.context
                val handler = durableCommit
                val lease = durabilityLease
                val leaseToken = attempt.leaseToken
                val completionOnce = AtomicBoolean(false)
                val completion = IrohaPeerNfcDurableCommitCompletionV1 { record, error ->
                    if (completionOnce.compareAndSet(false, true)) {
                        try {
                            bridgeReference.get()?.completeCommit(operationId, record, error)
                        } finally {
                            lease.release(leaseToken)
                        }
                    }
                }
                val action: () -> Unit = {
                    if (bridgeReference.get()?.claimDurabilityStart(operationId) == true) {
                        try {
                            handler.commit(context, completion)
                        } catch (failure: Throwable) {
                            completion.complete(null, failure)
                        }
                    } else {
                        lease.release(leaseToken)
                    }
                }
                action
            }
        }
        val accepted = durabilityExecutor.execute(invokeDurability)
        if (!accepted) {
            failBeforeDurabilityStart(operationId)
        }
    }

    /**
     * Detaches the current RF response while preserving the bounded durability
     * response lease until its callback or five-second deadline. The separate
     * process durability lease remains held after that deadline until the exact
     * application callback/throw, preventing retained callbacks from growing.
     */
    fun reset() {
        synchronized(lock) {
            pendingOperation?.let { attempt ->
                attempt.responseAttached = false
                attempt.respond = null
            }
        }
    }

    override fun onDeactivated(reason: Int) {
        reset()
    }

    private fun completeAdmission(
        operationId: Long,
        record: IrohaPeerNfcDurablePaymentAdmissionV1?,
        error: Throwable?,
    ) = complete<PendingAdmission>(
        operationId,
        error,
        missingRecord = record == null,
        invalidRecord = { attempt -> record != null && record.context != attempt.context },
    ) { _ ->
        receiver.installPaymentAdmission(requireNotNull(record))
    }

    private fun completeCommit(
        operationId: Long,
        record: IrohaPeerNfcDurableAcknowledgementV1?,
        error: Throwable?,
    ) = complete<PendingCommit>(operationId, error, missingRecord = record == null) { _ ->
        receiver.installDurableAcknowledgement(requireNotNull(record))
    }

    private inline fun <reified Operation : PendingOperation> complete(
        operationId: Long,
        error: Throwable?,
        missingRecord: Boolean,
        noinline invalidRecord: (Operation) -> Boolean = { false },
        install: (Operation) -> Unit,
    ) {
        val completed = synchronized(lock) {
            val attempt = pendingOperation
            if (attempt !is Operation || attempt.id != operationId) return
            pendingOperation = null
            val response =
                if (error != null || missingRecord) {
                    IrohaPeerNfcApduResponseV1(
                        statusWord = IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
                    )
                } else if (invalidRecord(attempt)) {
                    IrohaPeerNfcApduResponseV1(
                        statusWord = IrohaPeerNfcStatusWordV1.SECURITY_STATUS_NOT_SATISFIED,
                    )
                } else {
                    try {
                        install(attempt)
                        IrohaPeerNfcApduResponseV1(
                            statusWord = IrohaPeerNfcStatusWordV1.SUCCESS,
                        )
                    } catch (_: IllegalArgumentException) {
                        IrohaPeerNfcApduResponseV1(
                            statusWord = IrohaPeerNfcStatusWordV1.SECURITY_STATUS_NOT_SATISFIED,
                        )
                    } catch (_: Throwable) {
                        IrohaPeerNfcApduResponseV1(
                            statusWord = IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
                        )
                    }
                }
            CompletedOperation(
                attempt.timeout,
                attempt.respond.takeIf { attempt.responseAttached },
                response,
            )
        }
        completed.timeout?.cancel()
        completed.respond?.respond(completed.response)
    }

    /** Releases a reservation only when application persistence never started. */
    private fun failBeforeDurabilityStart(operationId: Long) {
        val failed = synchronized(lock) {
            val attempt = pendingOperation
            if (attempt == null || attempt.id != operationId || attempt.durabilityStarted) return
            pendingOperation = null
            CompletedOperation(
                attempt.timeout,
                attempt.respond.takeIf { attempt.responseAttached },
                IrohaPeerNfcApduResponseV1(
                    statusWord = IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
                ),
            ) to attempt.leaseToken
        }
        durabilityLease.release(failed.second)
        failed.first.timeout?.cancel()
        failed.first.respond?.respond(failed.first.response)
    }

    private fun claimDurabilityStart(operationId: Long): Boolean = synchronized(lock) {
        val attempt = pendingOperation
        if (attempt == null ||
            attempt.id != operationId ||
            !attempt.responseAttached ||
            attempt.durabilityStarted
        ) {
            return@synchronized false
        }
        attempt.durabilityStarted = true
        true
    }

    private fun timeOut(operationId: Long) {
        val respond = synchronized(lock) {
            val attempt = pendingOperation
            if (attempt == null || attempt.id != operationId) return
            pendingOperation = null
            attempt.respond.takeIf { attempt.responseAttached }
        }
        respond?.respond(IrohaPeerNfcApduResponseV1(
            statusWord = IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
        ))
    }

    private fun processSynchronous(command: IrohaPeerNfcCommandV1): IrohaPeerNfcApduResponseV1 = try {
        IrohaPeerNfcApduResponseV1(
            receiver.handle(command),
            IrohaPeerNfcStatusWordV1.SUCCESS,
        )
    } catch (_: SecurityException) {
        IrohaPeerNfcApduResponseV1(statusWord =
            IrohaPeerNfcStatusWordV1.SECURITY_STATUS_NOT_SATISFIED)
    } catch (_: IllegalStateException) {
        IrohaPeerNfcApduResponseV1(statusWord =
            IrohaPeerNfcStatusWordV1.CONDITIONS_NOT_SATISFIED)
    } catch (_: IllegalArgumentException) {
        IrohaPeerNfcApduResponseV1(statusWord = IrohaPeerNfcStatusWordV1.WRONG_DATA)
    }

    companion object {
        /** Shorter than the reader-side 10-second default IsoDep timeout. */
        const val DEFAULT_DURABILITY_TIMEOUT_MILLIS: Long = 5_000
    }
}
