package org.hyperledger.iroha.sdk.offline

import android.nfc.Tag
import android.nfc.cardemulation.HostApduService
import android.nfc.tech.IsoDep
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import java.io.Closeable
import java.io.IOException
import java.util.concurrent.Executor
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.ThreadFactory
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Conservative command and response limits from Android IsoDep capabilities. Without extended
 * length a WRITE APDU carries at most 255 data bytes (`s ‖ hash ‖ offset` plus a 203-byte chunk)
 * and a read answers at most 256 bytes.
 */
object IrohaPeerIsoDepLimitsV1 {
    /** The largest fixed command, BEGIN_PAYMENT (`80 20 00 00 ‖ Lc ‖` 132 bytes). */
    const val MINIMUM_TRANSCEIVE_LENGTH = 137
    private const val CHUNK_PREFIX = IrohaPeerNfcV1.SESSION_ID_BYTES + IrohaPeerNfcV1.HASH_BYTES + 4

    @JvmStatic
    fun derive(maximumTransceiveLength: Int, supportsExtendedLengthApdu: Boolean): IrohaPeerNfcLimitsV1 {
        require(maximumTransceiveLength >= MINIMUM_TRANSCEIVE_LENGTH) {
            "IsoDep transceive length $maximumTransceiveLength cannot carry BEGIN_PAYMENT"
        }
        val read = if (supportsExtendedLengthApdu) {
            minOf(IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES, maximumTransceiveLength - 2)
        } else {
            minOf(256, maximumTransceiveLength - 2)
        }
        val write = if (supportsExtendedLengthApdu) {
            minOf(IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES, maximumTransceiveLength - 7 - CHUNK_PREFIX)
        } else {
            minOf(0xff - CHUNK_PREFIX, maximumTransceiveLength - 5 - CHUNK_PREFIX)
        }
        return IrohaPeerNfcLimitsV1(
            maximumReadChunkBytes = read,
            maximumWriteChunkBytes = write,
        )
    }
}

/** IsoDep accepted a command, but its response could not be observed. */
class IrohaPeerNfcAmbiguousTransceiveExceptionV1(cause: IOException) :
    IOException("NFC command response is unknown", cause), IrohaPeerNfcAmbiguousResponseErrorV1

/** Durable wallet callbacks may outlast ordinary radio commands without slowing contact failure. */
internal class IrohaPeerIsoDepTimeoutsV1(
    val ordinaryMillis: Int = IrohaPeerIsoDepTransceiverV1.DEFAULT_OPERATION_TIMEOUT_MILLIS,
    private val durableMillis: Int = maxOf(ordinaryMillis, IrohaPeerIsoDepTransceiverV1.DEFAULT_DURABLE_OPERATION_TIMEOUT_MILLIS),
) {
    init {
        require(ordinaryMillis in 1..120_000 && durableMillis in ordinaryMillis..120_000) {
            "IsoDep timeouts must be 1..120000 ms and the durable wait must cover the ordinary wait"
        }
    }

    fun forOperation(operation: IrohaPeerNfcOperationV1): Int = when (operation) {
        IrohaPeerNfcOperationV1.BEGIN_PAYMENT, IrohaPeerNfcOperationV1.COMMIT -> durableMillis
        else -> ordinaryMillis
    }
}

/**
 * One reader contact's lifetime. [close] is terminal and always reaches the tag: the platform's
 * close is what interrupts a connect or transceive blocked on another thread, and it does so
 * whether or not the tag has reported a connection yet. Nothing reconnects a closed contact.
 */
internal class IrohaPeerIsoDepContactV1(
    private val isConnected: () -> Boolean,
    private val connectTag: () -> Unit,
    private val closeTag: () -> Unit,
) {
    private val closed = AtomicBoolean(false)

    @Throws(IOException::class)
    fun requireOpen() {
        if (closed.get()) throw IOException(CLOSED)
    }

    /** Connects unless the tag is already connected; a close that raced the connect wins. */
    @Throws(IOException::class)
    fun ensureConnected() {
        requireOpen()
        if (!isConnected()) connectTag()
        if (closed.get()) {
            closeQuietly()
            throw IOException(CLOSED)
        }
    }

    fun close() {
        closed.set(true)
        closeQuietly()
    }

    private fun closeQuietly() {
        try {
            closeTag()
        } catch (_: IOException) {
            // The contact is over either way; a blocked call still returns with its own IOException.
        } catch (_: RuntimeException) {
            // A tag that already left the field can refuse the platform call (SecurityException).
        }
    }

    companion object {
        const val CLOSED = "Iroha peer NFC contact was closed"
    }
}

/**
 * Reader-side IsoDep transceiver of one tag, selecting [applicationIdentifier] (the identifier
 * both phones registered). Pass [transceive] to [IrohaPeerNfcReaderExchangeV1.readRequest] to read
 * the receiver's open Request, retain the connection during review and approval, then continue
 * with [IrohaPeerNfcReaderExchangeV1.run] using [localLimits]. Each phase begins with SELECT for the
 * same identifier without closing this transceiver. A transport failure after a command was sent
 * is an [IrohaPeerNfcAmbiguousTransceiveExceptionV1].
 */
class IrohaPeerIsoDepTransceiverV1 private constructor(
    private val isoDep: IsoDep,
    @JvmField val applicationIdentifier: IrohaPeerNfcApplicationIdentifierV1,
    operationTimeoutMillis: Int,
    durableOperationTimeoutMillis: Int,
) : Closeable, IrohaPeerNfcReaderTransceiverV1 {
    private val timeouts = IrohaPeerIsoDepTimeoutsV1(operationTimeoutMillis, durableOperationTimeoutMillis)

    @Volatile private var aidSelected = false
    private val contact = IrohaPeerIsoDepContactV1(
        isConnected = { isoDep.isConnected },
        connectTag = {
            aidSelected = false
            isoDep.connect()
        },
        closeTag = { isoDep.close() },
    )

    /** The chunk limits this tag can carry; intersect them with the card's advertised limits. */
    val localLimits: IrohaPeerNfcLimitsV1
        get() = IrohaPeerIsoDepLimitsV1.derive(isoDep.maxTransceiveLength, isoDep.isExtendedLengthApduSupported)

    /** Connect to the tag and apply the operation timeout; SELECT follows through [transceive]. */
    @Throws(IOException::class)
    fun connect() {
        contact.ensureConnected()
        isoDep.timeout = timeouts.ordinaryMillis
    }

    /**
     * Send [command]. SELECT must name [applicationIdentifier]; every other command requires a
     * successful SELECT on this connection.
     */
    @Throws(IOException::class)
    override fun transceive(command: IrohaPeerNfcCommandV1): IrohaPeerNfcAPDUResponseV1 {
        contact.requireOpen()
        if (!isoDep.isConnected) connect()
        val select = command is IrohaPeerNfcCommandV1.SelectApplication
        if (command is IrohaPeerNfcCommandV1.SelectApplication) {
            require(command.applicationIdentifier == applicationIdentifier) {
                "SELECT must name this transceiver's application identifier"
            }
            aidSelected = false
        } else if (!aidSelected) {
            throw IOException("Iroha peer NFC application is not selected")
        }
        val encoded = IrohaPeerNfcAPDUCodecV1.encode(command)
        if (encoded.size > isoDep.maxTransceiveLength) throw IOException("NFC APDU exceeds tag limit")
        isoDep.timeout = timeouts.forOperation(command.operation)
        val raw = try {
            isoDep.transceive(encoded)
        } catch (failure: IOException) {
            throw IrohaPeerNfcAmbiguousTransceiveExceptionV1(failure)
        }
        val response = IrohaPeerNfcAPDUResponseV1.decode(raw)
        if (select) aidSelected = response.statusWord == IrohaPeerNfcStatusWordV1.SUCCESS && response.dataLength == 0
        return response
    }

    /** Ends this contact for good, including a connect or transceive blocked on another thread. */
    override fun close() {
        aidSelected = false
        contact.close()
    }

    companion object {
        const val DEFAULT_OPERATION_TIMEOUT_MILLIS = 10_000
        const val DEFAULT_DURABLE_OPERATION_TIMEOUT_MILLIS = 35_000

        /** [operationTimeoutMillis] bounds ordinary radio commands; only BEGIN_PAYMENT and COMMIT
         * use [durableOperationTimeoutMillis] while the receiver persists wallet work. */
        @JvmStatic
        @JvmOverloads
        fun from(
            tag: Tag,
            applicationIdentifier: IrohaPeerNfcApplicationIdentifierV1 = IrohaPeerNfcApplicationIdentifierV1.PROTOCOL,
            operationTimeoutMillis: Int = DEFAULT_OPERATION_TIMEOUT_MILLIS,
            durableOperationTimeoutMillis: Int = maxOf(operationTimeoutMillis, DEFAULT_DURABLE_OPERATION_TIMEOUT_MILLIS),
        ): IrohaPeerIsoDepTransceiverV1? =
            IsoDep.get(tag)?.let {
                IrohaPeerIsoDepTransceiverV1(it, applicationIdentifier, operationTimeoutMillis, durableOperationTimeoutMillis)
            }
    }
}

/** Delivers exactly one response for one command, from any thread. */
fun interface IrohaPeerNfcApduResponseHandlerV1 {
    fun respond(response: IrohaPeerNfcAPDUResponseV1)
}

/** Asynchronous card command handler behind [IrohaPeerAsyncHostApduServiceV1]. */
interface IrohaPeerNfcAsyncCommandHandlerV1 {
    /** Accepted SELECT starts a new RF activation, before any subsequent protocol command. */
    fun onActivated()

    /** Handle one decoded non-SELECT command; call [respond] exactly once, from any thread. */
    fun handle(command: IrohaPeerNfcCommandV1, respond: IrohaPeerNfcApduResponseHandlerV1)

    /** The reader deselected the card or the RF link was lost ([HostApduService] reasons). */
    fun onDeactivated(reason: Int)
}

/** One-shot completion of an asynchronous durable card callback; only the first call counts. */
interface IrohaPeerNfcCardCompletionV1<T> {
    /** Transport ownership only: this command still owns its activation and response deadline. */
    val isActive: Boolean

    /** The durable record was persisted before this call. */
    fun complete(value: T)

    /** Persistence or native admission failed; the card answers `6581`. */
    fun fail(error: Throwable)
}

/**
 * Application callbacks of the card. Each runs on the bridge's callback executor, never on the
 * HCE thread, and may complete later on any thread. The durable callbacks must be idempotent: a
 * late completion after RF deactivation or the deadline is dropped, and the reader's exact replay
 * later calls them again for the same context.
 */
interface IrohaPeerNfcCardCallbacksV1 {
    /**
     * BEGIN_PAYMENT: durably admit the Payment header of [context] and complete with its `IPA1`
     * record ([IrohaPeerNfcDurablePaymentAdmissionV1]) after persisting it.
     */
    fun admitPayment(
        context: IrohaPeerNfcPaymentAdmissionContextV1,
        completion: IrohaPeerNfcCardCompletionV1<IrohaPeerNfcDurablePaymentAdmissionV1>,
    )

    /**
     * COMMIT: ingest the Payment of [context] natively (with the payer companion its carrier holds,
     * `KagemushaWalletV1.receiveWithCompanion`), persist the Credited acknowledgement and complete
     * with its `IDA1` record ([IrohaPeerNfcDurableAcknowledgementV1]).
     */
    fun durableCommit(
        context: IrohaPeerNfcCommitContextV1,
        completion: IrohaPeerNfcCardCompletionV1<IrohaPeerNfcDurableAcknowledgementV1>,
    )
}

/** Why an application boundary of the card failed. */
enum class IrohaPeerNfcCardFailureV1 {
    PAYMENT_ADMISSION_REJECTED,
    PAYMENT_ADMISSION_PERSISTENCE_FAILED,
    PAYMENT_ADMISSION_TIMED_OUT,
    DURABLE_COMMIT_REJECTED,
    DURABLE_COMMIT_PERSISTENCE_FAILED,
    DURABLE_COMMIT_TIMED_OUT,
    INVALID_DURABLE_COMMIT_RESULT,
}

/** Observable card progress. Events are idempotent: an exact replay can report a state again. */
sealed class IrohaPeerNfcCardEventV1 {
    /** A reader selected this application. This is contact only, not Request delivery or payment. */
    object Activated : IrohaPeerNfcCardEventV1()

    /**
     * The payer's phone read the last byte of the hosted Request (its final READ_REQUEST chunk): the
     * payer is reviewing and approving it. A repeated read reports it again.
     */
    object RequestRead : IrohaPeerNfcCardEventV1()

    /** The Payment header is durably admitted. */
    object PaymentAdmitted : IrohaPeerNfcCardEventV1()

    /** The Payment is ingested and the Credited acknowledgement is durable. */
    object AcknowledgementReady : IrohaPeerNfcCardEventV1()

    /** The payer confirmed the durable acknowledgement. Its response may still be buffered:
     * do not tear down RF on this event. Retain the COMPLETE responder for ambiguous-response
     * replay until the application explicitly retires this presentation. */
    object AcknowledgementConfirmed : IrohaPeerNfcCardEventV1()

    class Failed(
        @JvmField val failure: IrohaPeerNfcCardFailureV1,
        @JvmField val cause: Throwable?,
    ) : IrohaPeerNfcCardEventV1() {
        override fun toString(): String = "Failed(${failure.name})"
    }

    /** The RF link was lost or the reader deselected the card ([HostApduService] reasons). */
    class Deactivated(@JvmField val reason: Int) : IrohaPeerNfcCardEventV1() {
        override fun toString(): String = "Deactivated($reason)"
    }
}

/** Receives card events and their activation epoch, on the thread that caused them.
 * Before applying a queued event, check [IrohaPeerNfcCardApduBridgeV1.isCurrentActivation]. */
fun interface IrohaPeerNfcCardEventListenerV1 {
    fun onEvent(event: IrohaPeerNfcCardEventV1, activation: Long)
}

/**
 * Host card emulation bridge of one [IrohaPeerNfcCardEndpointV1] (the receiver's open Request, or a
 * restored session) and its application [callbacks].
 *
 * Every command but BEGIN_PAYMENT and COMMIT is answered synchronously by the endpoint. BEGIN_PAYMENT
 * and COMMIT answer only after [IrohaPeerNfcCardCallbacksV1.admitPayment] or
 * [IrohaPeerNfcCardCallbacksV1.durableCommit] returned its persisted record within the durable
 * deadline; otherwise the card answers `6581`. A durable completion that arrives after RF
 * deactivation or the deadline is dropped, and the reader's exact replay later re-stages it.
 */
class IrohaPeerNfcCardApduBridgeV1 @JvmOverloads constructor(
    @JvmField val endpoint: IrohaPeerNfcCardEndpointV1,
    private val callbacks: IrohaPeerNfcCardCallbacksV1,
    private val listener: IrohaPeerNfcCardEventListenerV1 = IrohaPeerNfcCardEventListenerV1 { _, _ -> },
    @JvmField val durableDeadlineMillis: Long = DEFAULT_DURABLE_DEADLINE_MILLIS,
    private val callbackExecutor: Executor = SHARED_CALLBACK_EXECUTOR,
    private val deadlineScheduler: ScheduledExecutorService = SHARED_DEADLINE_SCHEDULER,
) : IrohaPeerNfcAsyncCommandHandlerV1 {
    init {
        require(durableDeadlineMillis in 1..MAXIMUM_DURABLE_DEADLINE_MILLIS) {
            "NFC durable deadline must be 1..$MAXIMUM_DURABLE_DEADLINE_MILLIS ms"
        }
    }

    private val activation = IrohaPeerNfcActivationEpochV1()
    private val requestLength = endpoint.receiver.receiveRequest.byteCount.toLong()

    /** Whether a queued observation still belongs to this card's current RF activation. */
    fun isCurrentActivation(epoch: Long): Boolean = activation.isCurrent(epoch)

    /** The current observation epoch, including the inactive epoch after RF loss. */
    val currentActivation: Long get() = activation.current()

    override fun onActivated() { activation.activate { epoch -> emit(IrohaPeerNfcCardEventV1.Activated, epoch) } }

    override fun handle(command: IrohaPeerNfcCommandV1, respond: IrohaPeerNfcApduResponseHandlerV1) {
        // Direct bridge clients also use SELECT; HostApduService dispatches the hook itself.
        if (command is IrohaPeerNfcCommandV1.SelectApplication) {
            if (command.applicationIdentifier != endpoint.applicationIdentifier) {
                onDeactivated(HostApduService.DEACTIVATION_DESELECTED)
                return respond.respond(IrohaPeerNfcAPDUResponseV1.failure(IrohaPeerNfcErrorV1.INVALID_APPLICATION_IDENTIFIER))
            }
            onActivated()
        }
        val epoch = activation.capture() ?: return respond.respond(
            IrohaPeerNfcAPDUResponseV1.of(IrohaPeerNfcStatusWordV1.CONDITIONS_NOT_SATISFIED))
        when (command) {
            is IrohaPeerNfcCommandV1.BeginPayment -> beginPayment(command, epoch, respond)
            is IrohaPeerNfcCommandV1.Commit -> commit(command, epoch, respond)
            else -> activation.ifCurrent(epoch) {
                val response = answer { IrohaPeerNfcAPDUResponseV1(endpoint.handle(command), IrohaPeerNfcStatusWordV1.SUCCESS) }
                respond.respond(response)
                if (response.statusWord == IrohaPeerNfcStatusWordV1.SUCCESS) {
                    if (command is IrohaPeerNfcCommandV1.ReadRequest && command.offset + response.dataLength == requestLength)
                        emit(IrohaPeerNfcCardEventV1.RequestRead, epoch)
                    else if (command is IrohaPeerNfcCommandV1.ConfirmAcknowledgement)
                        emit(IrohaPeerNfcCardEventV1.AcknowledgementConfirmed, epoch)
                }
            }
        }
    }

    override fun onDeactivated(reason: Int) {
        activation.invalidate { epoch -> emit(IrohaPeerNfcCardEventV1.Deactivated(reason), epoch) }
    }

    private fun publish(epoch: Long, respond: IrohaPeerNfcApduResponseHandlerV1,
        response: IrohaPeerNfcAPDUResponseV1, event: IrohaPeerNfcCardEventV1) {
        activation.ifCurrent(epoch) {
            respond.respond(response)
            emit(event, epoch)
        }
    }

    private fun beginPayment(
        command: IrohaPeerNfcCommandV1.BeginPayment,
        epoch: Long,
        respond: IrohaPeerNfcApduResponseHandlerV1,
    ) {
        val disposition = try {
            endpoint.preparePaymentAdmission(command)
        } catch (failure: RuntimeException) {
            publish(epoch, respond, durableFailureResponse(failure),
                IrohaPeerNfcCardEventV1.Failed(IrohaPeerNfcCardFailureV1.PAYMENT_ADMISSION_REJECTED, failure))
            return
        }
        when (disposition) {
            is IrohaPeerNfcPaymentAdmissionDispositionV1.AlreadyAdmitted -> {
                publish(epoch, respond, SUCCESS, IrohaPeerNfcCardEventV1.PaymentAdmitted)
            }
            is IrohaPeerNfcPaymentAdmissionDispositionV1.RequiresDurableAdmission -> {
                val context = disposition.context
                val operation = DurableOperation<IrohaPeerNfcDurablePaymentAdmissionV1>(
                    epoch,
                    respond,
                    IrohaPeerNfcCardFailureV1.PAYMENT_ADMISSION_PERSISTENCE_FAILED,
                    IrohaPeerNfcCardFailureV1.PAYMENT_ADMISSION_TIMED_OUT,
                ) install@{ record ->
                    if (record.context != context) {
                        return@install Outcome(
                            IrohaPeerNfcAPDUResponseV1.failure(IrohaPeerNfcErrorV1.CONTINUITY_MISMATCH),
                            IrohaPeerNfcCardEventV1.Failed(IrohaPeerNfcCardFailureV1.PAYMENT_ADMISSION_REJECTED, null),
                        )
                    }
                    try {
                        endpoint.installPaymentAdmission(record)
                    } catch (failure: RuntimeException) {
                        return@install Outcome(
                            durableFailureResponse(failure),
                            IrohaPeerNfcCardEventV1.Failed(IrohaPeerNfcCardFailureV1.PAYMENT_ADMISSION_REJECTED, failure),
                        )
                    }
                    Outcome(SUCCESS, IrohaPeerNfcCardEventV1.PaymentAdmitted)
                }
                operation.start { callbacks.admitPayment(context, operation) }
            }
        }
    }

    private fun commit(command: IrohaPeerNfcCommandV1.Commit, epoch: Long, respond: IrohaPeerNfcApduResponseHandlerV1) {
        val disposition = try {
            endpoint.prepareCommit(command)
        } catch (failure: RuntimeException) {
            publish(epoch, respond, durableFailureResponse(failure),
                IrohaPeerNfcCardEventV1.Failed(IrohaPeerNfcCardFailureV1.DURABLE_COMMIT_REJECTED, failure))
            return
        }
        when (disposition) {
            is IrohaPeerNfcCommitDispositionV1.AlreadyCommitted -> {
                publish(epoch, respond, SUCCESS, IrohaPeerNfcCardEventV1.AcknowledgementReady)
            }
            is IrohaPeerNfcCommitDispositionV1.RequiresDurableCommit -> {
                val context = disposition.context
                val operation = DurableOperation<IrohaPeerNfcDurableAcknowledgementV1>(
                    epoch,
                    respond,
                    IrohaPeerNfcCardFailureV1.DURABLE_COMMIT_PERSISTENCE_FAILED,
                    IrohaPeerNfcCardFailureV1.DURABLE_COMMIT_TIMED_OUT,
                ) install@{ record ->
                    try {
                        endpoint.installDurableAcknowledgement(record)
                    } catch (failure: RuntimeException) {
                        return@install Outcome(
                            durableFailureResponse(failure),
                            IrohaPeerNfcCardEventV1.Failed(IrohaPeerNfcCardFailureV1.INVALID_DURABLE_COMMIT_RESULT, failure),
                        )
                    }
                    Outcome(SUCCESS, IrohaPeerNfcCardEventV1.AcknowledgementReady)
                }
                operation.start { callbacks.durableCommit(context, operation) }
            }
        }
    }

    /** The card's answer to a durable completion and the event it reports. */
    private class Outcome(val response: IrohaPeerNfcAPDUResponseV1, val event: IrohaPeerNfcCardEventV1)

    /**
     * One pending durable callback. The first of completion, failure or deadline settles it; a
     * completion is installed only while its RF activation is current.
     */
    private inner class DurableOperation<T>(
        private val epoch: Long,
        private val respond: IrohaPeerNfcApduResponseHandlerV1,
        private val persistenceFailure: IrohaPeerNfcCardFailureV1,
        private val timeoutFailure: IrohaPeerNfcCardFailureV1,
        private val install: (T) -> Outcome,
    ) : IrohaPeerNfcCardCompletionV1<T> {
        private val settled = AtomicBoolean(false)
        @Volatile private var deadline: ScheduledFuture<*>? = null

        override val isActive: Boolean get() = !settled.get() && activation.isCurrent(epoch)

        fun start(invoke: () -> Unit) {
            deadline = deadlineScheduler.schedule(
                Runnable {
                    settle { Outcome(STORAGE_FAILURE, IrohaPeerNfcCardEventV1.Failed(timeoutFailure, null)) }
                },
                durableDeadlineMillis,
                TimeUnit.MILLISECONDS,
            )
            if (settled.get()) deadline?.cancel(false)
            dispatch(epoch, persistenceFailure) {
                if (!isActive) {
                    settled.set(true)
                    deadline?.cancel(false)
                    return@dispatch
                }
                try {
                    invoke()
                } catch (failure: Exception) {
                    fail(failure)
                }
            }
        }

        // A late completion after RF deactivation is dropped; the reader's exact replay re-stages it.
        override fun complete(value: T) = settle { install(value) }

        override fun fail(error: Throwable) =
            settle { Outcome(STORAGE_FAILURE, IrohaPeerNfcCardEventV1.Failed(persistenceFailure, error)) }

        private fun settle(outcome: () -> Outcome) {
            if (!settled.compareAndSet(false, true)) return
            deadline?.cancel(false)
            // Invalidation cannot interleave installation, response and event publication.
            activation.ifCurrent(epoch) {
                val settledOutcome = outcome()
                respond.respond(settledOutcome.response)
                emit(settledOutcome.event, epoch)
            }
        }
    }

    private fun dispatch(epoch: Long, failureKind: IrohaPeerNfcCardFailureV1, action: () -> Unit) {
        try {
            callbackExecutor.execute {
                try {
                    action()
                } catch (failure: Exception) {
                    activation.ifCurrent(epoch) { emit(IrohaPeerNfcCardEventV1.Failed(failureKind, failure), epoch) }
                }
            }
        } catch (failure: RuntimeException) {
            activation.ifCurrent(epoch) { emit(IrohaPeerNfcCardEventV1.Failed(failureKind, failure), epoch) }
        }
    }

    private fun emit(event: IrohaPeerNfcCardEventV1, epoch: Long) {
        try {
            listener.onEvent(event, epoch)
        } catch (_: RuntimeException) {
            // A listener failure never changes the card's protocol answer.
        }
    }

    private inline fun answer(body: () -> IrohaPeerNfcAPDUResponseV1): IrohaPeerNfcAPDUResponseV1 = try {
        body()
    } catch (failure: RuntimeException) {
        failureResponse(failure)
    }

    companion object {
        const val DEFAULT_DURABLE_DEADLINE_MILLIS = 30_000L
        const val MAXIMUM_DURABLE_DEADLINE_MILLIS = 30_000L

        private val SUCCESS = IrohaPeerNfcAPDUResponseV1.of(IrohaPeerNfcStatusWordV1.SUCCESS)
        private val STORAGE_FAILURE = IrohaPeerNfcAPDUResponseV1.of(IrohaPeerNfcStatusWordV1.STORAGE_FAILURE)

        private fun daemonThreads(name: String): ThreadFactory = ThreadFactory { runnable ->
            Thread(runnable, name).apply { isDaemon = true }
        }

        private val SHARED_CALLBACK_EXECUTOR: Executor by lazy {
            Executors.newCachedThreadPool(daemonThreads("iroha-peer-nfc-card-callback"))
        }

        private val SHARED_DEADLINE_SCHEDULER: ScheduledExecutorService by lazy {
            Executors.newSingleThreadScheduledExecutor(daemonThreads("iroha-peer-nfc-card-deadline"))
        }

        /** An NFC failure answers its status word; any other failure is malformed data. */
        private fun failureResponse(failure: RuntimeException): IrohaPeerNfcAPDUResponseV1 =
            if (failure is IrohaPeerNfcExceptionV1) {
                IrohaPeerNfcAPDUResponseV1.failure(failure.error)
            } else {
                IrohaPeerNfcAPDUResponseV1.of(IrohaPeerNfcStatusWordV1.WRONG_DATA)
            }

        /** At a durable boundary any non-NFC failure is a persistence failure (`6581`). */
        private fun durableFailureResponse(failure: RuntimeException): IrohaPeerNfcAPDUResponseV1 =
            if (failure is IrohaPeerNfcExceptionV1) IrohaPeerNfcAPDUResponseV1.failure(failure.error) else STORAGE_FAILURE
    }
}

/** RF activation epoch of the card bridge: deactivation invalidates every pending completion. */
internal class IrohaPeerNfcActivationEpochV1 {
    private var value = 0L
    private var active = true

    @Synchronized fun current(): Long = value
    @Synchronized fun capture(): Long? = value.takeIf { active }

    @Synchronized fun activate(publish: (Long) -> Unit) {
        value += 1
        active = true
        publish(value)
    }

    @Synchronized fun invalidate(publish: (Long) -> Unit) {
        value += 1
        active = false
        publish(value)
    }

    @Synchronized fun isCurrent(expected: Long): Boolean = value == expected

    /** Runs [action] under the epoch lock while [expected] is active, else returns `null`. */
    @Synchronized fun <T> ifCurrent(expected: Long, action: () -> T): T? =
        if (active && value == expected) action() else null
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

/** Exact SELECT of one application identifier. */
internal class IrohaPeerNfcAidSelectionV1(@JvmField val applicationIdentifier: IrohaPeerNfcApplicationIdentifierV1) {
    /** `00 A4 04 00 ‖ Lc ‖ AID ‖ 00`. */
    fun command(): ByteArray =
        IrohaPeerNfcAPDUCodecV1.encode(IrohaPeerNfcCommandV1.SelectApplication(applicationIdentifier))

    /** Whether [apdu] is the canonical SELECT of exactly this identifier. */
    fun accepts(apdu: ByteArray): Boolean = try {
        IrohaPeerNfcAPDUCodecV1.decode(apdu, applicationIdentifier) is IrohaPeerNfcCommandV1.SelectApplication
    } catch (_: RuntimeException) {
        false
    }

    companion object {
        fun isSelect(apdu: ByteArray): Boolean = apdu.size >= 2 && apdu[0] == 0x00.toByte() && apdu[1] == 0xa4.toByte()

        fun succeeded(response: ByteArray): Boolean = response.contentEquals(byteArrayOf(0x90.toByte(), 0x00))
    }
}

/** Each SELECT or RF deactivation invalidates responses from the previous activation. */
internal class IrohaPeerNfcApduSelectionStateV1(applicationIdentifier: IrohaPeerNfcApplicationIdentifierV1) {
    private val aid = IrohaPeerNfcAidSelectionV1(applicationIdentifier)
    private var epoch = 0L

    val applicationIdentifier: IrohaPeerNfcApplicationIdentifierV1 get() = aid.applicationIdentifier

    var isSelected = false
        @Synchronized get
        private set

    @Synchronized
    fun select(apdu: ByteArray): Boolean {
        epoch += 1
        isSelected = aid.accepts(apdu)
        return isSelected
    }

    @Synchronized
    fun currentEpoch(): Long? = if (isSelected) epoch else null

    @Synchronized
    fun isCurrent(expected: Long): Boolean = isSelected && epoch == expected

    @Synchronized
    fun deactivate() {
        epoch += 1
        isSelected = false
    }
}

/**
 * The HostApduService boundary without Android types: SELECT of exactly the card's identifier
 * (`9000`, any other `6A82`), `6985` before selection, strict decoding and one gated reply per
 * command. Replies after RF deactivation or a newer SELECT are never sent.
 */
internal class IrohaPeerNfcHostApduDispatcherV1(
    applicationIdentifier: IrohaPeerNfcApplicationIdentifierV1,
    private val commandHandler: () -> IrohaPeerNfcAsyncCommandHandlerV1,
    private val post: (Runnable) -> Unit,
    private val send: (ByteArray) -> Unit,
) {
    val selection = IrohaPeerNfcApduSelectionStateV1(applicationIdentifier)
    private var selectedHandler: IrohaPeerNfcAsyncCommandHandlerV1? = null

    fun process(commandApdu: ByteArray?): ByteArray? {
        if (commandApdu == null) return status(IrohaPeerNfcStatusWordV1.WRONG_LENGTH)
        if (IrohaPeerNfcAidSelectionV1.isSelect(commandApdu)) {
            val previous = selectedHandler
            val wasSelected = selection.isSelected
            val accepted = selection.select(commandApdu)
            if (wasSelected) previous?.onDeactivated(HostApduService.DEACTIVATION_DESELECTED)
            selectedHandler = if (accepted) commandHandler() else null
            selectedHandler?.onActivated()
            return status(if (accepted) IrohaPeerNfcStatusWordV1.SUCCESS else IrohaPeerNfcStatusWordV1.NOT_FOUND)
        }
        val epoch = selection.currentEpoch() ?: return status(IrohaPeerNfcStatusWordV1.CONDITIONS_NOT_SATISFIED)
        val handler = selectedHandler ?: return status(IrohaPeerNfcStatusWordV1.CONDITIONS_NOT_SATISFIED)
        if (commandHandler() !== handler) {
            selection.deactivate()
            selectedHandler = null
            handler.onDeactivated(HostApduService.DEACTIVATION_DESELECTED)
            return status(IrohaPeerNfcStatusWordV1.CONDITIONS_NOT_SATISFIED)
        }
        val command = try {
            IrohaPeerNfcAPDUCodecV1.decode(commandApdu, selection.applicationIdentifier)
        } catch (failure: IrohaPeerNfcExceptionV1) {
            return IrohaPeerNfcAPDUResponseV1.failure(failure.error).encode()
        } catch (_: RuntimeException) {
            return status(IrohaPeerNfcStatusWordV1.WRONG_DATA)
        }
        val gate = IrohaPeerNfcApduReplyGateV1 { encoded ->
            post(Runnable { if (selection.isCurrent(epoch) && commandHandler() === handler) send(encoded) })
        }
        try {
            handler.handle(command) { response -> gate.deliver(response.encode()) }
        } catch (failure: IrohaPeerNfcExceptionV1) {
            gate.deliver(IrohaPeerNfcAPDUResponseV1.failure(failure.error).encode())
        } catch (_: RuntimeException) {
            gate.deliver(status(IrohaPeerNfcStatusWordV1.WRONG_DATA))
        }
        return gate.finish()
    }

    fun deactivated(reason: Int) {
        selection.deactivate()
        val previous = selectedHandler
        selectedHandler = null
        previous?.onDeactivated(reason)
    }

    private fun status(statusWord: IrohaPeerNfcStatusWordV1): ByteArray = IrohaPeerNfcAPDUResponseV1.of(statusWord).encode()
}

/**
 * HostApduService base of the card. Register the service with an aid-filter for
 * [applicationIdentifier] (the protocol identifier template is `res/xml/iroha_peer_nfc_v1_aids.xml`;
 * an application with its own identifier ships its own copy) and route commands to
 * [commandHandler], normally an [IrohaPeerNfcCardApduBridgeV1]. Responses completed after the
 * method returned are posted to the main thread and sent only while that SELECT is current.
 */
abstract class IrohaPeerAsyncHostApduServiceV1 : HostApduService() {
    private val responseHandler: Handler by lazy { Handler(Looper.getMainLooper()) }

    /** The identifier this card answers; it must equal the service's aid-filter. */
    protected open val applicationIdentifier: IrohaPeerNfcApplicationIdentifierV1
        get() = IrohaPeerNfcApplicationIdentifierV1.PROTOCOL

    protected abstract val commandHandler: IrohaPeerNfcAsyncCommandHandlerV1

    // Lazy: subclasses override applicationIdentifier, which construction must not read.
    private val dispatcher: IrohaPeerNfcHostApduDispatcherV1 by lazy {
        IrohaPeerNfcHostApduDispatcherV1(
            applicationIdentifier,
            { commandHandler },
            { runnable -> responseHandler.post(runnable) },
            { encoded -> sendResponseApdu(encoded) },
        )
    }

    final override fun processCommandApdu(commandApdu: ByteArray?, extras: Bundle?): ByteArray? =
        dispatcher.process(commandApdu)

    final override fun onDeactivated(reason: Int) {
        dispatcher.deactivated(reason)
    }
}
