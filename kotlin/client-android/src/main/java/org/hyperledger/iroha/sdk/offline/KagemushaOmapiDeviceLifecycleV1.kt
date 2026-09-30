// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import android.annotation.TargetApi
import android.content.Context
import android.os.Build
import android.se.omapi.Channel
import android.se.omapi.Reader
import android.se.omapi.SEService
import android.se.omapi.Session
import java.util.concurrent.CompletableFuture
import java.util.concurrent.Executor
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Android OMAPI discovery for a provisioned secure-element lifecycle applet.
 *
 * OMAPI access rules authenticate the calling package/signing certificate for the selected AID.
 * Discovery still returns an available bridge only when that applet supplies the complete exact
 * ABI-25 capability frame. StrongBox or an eSE feature flag alone never enables the wallet.
 */
object KagemushaOmapiDeviceLifecycleV1 {
    enum class DiscoveryStatus { AVAILABLE, API_UNSUPPORTED, NO_EMBEDDED_READER, UNAVAILABLE, AMBIGUOUS, TIMED_OUT, SERVICE_FAILED }
    enum class FailureReason { ACCESS_DENIED, APPLET_NOT_FOUND, NO_LOGICAL_CHANNEL, SECURE_ELEMENT_ABSENT, PLATFORM_IO, PLATFORM_FAILURE, CAPABILITY_REJECTED }

    /** Original platform failure remains available to the owner; string rendering exposes no body. */
    class DiscoveryFailure internal constructor(
        val readerName: String?, val reason: FailureReason, val cause: Throwable?,
    ) {
        override fun toString(): String = "DiscoveryFailure(reader=$readerName, reason=$reason)"
    }

    /** Discovery is transport evidence only and grants no release or monetary authority. */
    class DiscoveryResult internal constructor(
        val bridge: KagemushaDeviceLifecycleBridgeV1,
        val status: DiscoveryStatus,
        failures: List<DiscoveryFailure>,
        private val closeUndelivered: () -> Unit = {},
    ) {
        private val disposalRequested = AtomicBoolean(false)
        val failures: List<DiscoveryFailure> = java.util.Collections.unmodifiableList(failures.toList())

        internal fun discardIfUndelivered() {
            if (status == DiscoveryStatus.AVAILABLE && disposalRequested.compareAndSet(false, true)) {
                closeUndelivered()
            }
        }
        init {
            require((status == DiscoveryStatus.AVAILABLE) ==
                (bridge.availability == KagemushaDeviceLifecycleBridgeV1.Availability.AVAILABLE))
        }
    }

    internal fun classifyFailure(readerName: String?, failure: Throwable): DiscoveryFailure =
        DiscoveryFailure(readerName, when (failure) {
            is SecurityException -> FailureReason.ACCESS_DENIED
            is java.util.NoSuchElementException -> FailureReason.APPLET_NOT_FOUND
            is UnsupportedOperationException -> FailureReason.NO_LOGICAL_CHANNEL
            is java.io.IOException -> FailureReason.PLATFORM_IO
            else -> FailureReason.PLATFORM_FAILURE
        }, failure)

    private fun unavailable(status: DiscoveryStatus, failures: List<DiscoveryFailure> = emptyList()) =
        DiscoveryResult(KagemushaDeviceLifecycleBridgeV1.onlineOnly(), status, failures)

    @JvmStatic
    @JvmOverloads
    fun openAsync(context: Context, executor: Executor, configuration: Configuration = Configuration(),
        discoveryTimeoutMillis: Long = DEFAULT_DISCOVERY_TIMEOUT_MILLIS): CompletableFuture<KagemushaDeviceLifecycleBridgeV1> {
        val discovery = openWithDiagnosticsAsync(context, executor, configuration, discoveryTimeoutMillis)
        return projectDiscovery(discovery)
    }

    /** Transfer discovery ownership only when the caller-facing completion succeeds. */
    internal fun projectDiscovery(
        discovery: CompletableFuture<DiscoveryResult>,
    ): CompletableFuture<KagemushaDeviceLifecycleBridgeV1> {
        val bridge = CompletableFuture<KagemushaDeviceLifecycleBridgeV1>()
        bridge.whenComplete { _, _ -> if (bridge.isCancelled) discovery.cancel(false) }
        discovery.whenComplete { discovered, failure ->
            if (failure != null) {
                bridge.completeExceptionally(failure)
            } else if (!bridge.complete(discovered.bridge)) {
                // Discovery may already have won while cancellation prevents transfer to the caller.
                discovered.discardIfUndelivered()
            }
        }
        return bridge
    }

    /** Exact applet selection and optional reader pin. */
    class Configuration @JvmOverloads constructor(
        readerName: String? = null,
        appletAid: ByteArray = DEFAULT_APPLET_AID,
    ) {
        internal val readerName: String? = readerName?.also {
            require(isEmbeddedReaderName(it)) {
                "readerName must name an embedded secure-element reader"
            }
        }
        internal val appletAid: ByteArray = appletAid.copyOf().also {
            require(it.size in 5..16) { "appletAid must contain 5..16 bytes" }
            require(it.any { byte -> byte != 0.toByte() }) { "appletAid must be non-zero" }
        }
    }

    /**
     * Open without blocking the application thread and resolve within [discoveryTimeoutMillis].
     *
     * An absent service, denied AID, incomplete foundation applet, malformed capability frame,
     * ambiguous qualified readers, timeout, or any platform error completes with an online-only
     * bridge. With no explicit reader pin, only embedded eSE readers are eligible; exactly one
     * applet must pass the complete capability contract.
     */
    @JvmStatic
    @JvmOverloads
    fun openWithDiagnosticsAsync(
        context: Context,
        executor: Executor,
        configuration: Configuration = Configuration(),
        discoveryTimeoutMillis: Long = DEFAULT_DISCOVERY_TIMEOUT_MILLIS,
    ): CompletableFuture<DiscoveryResult> {
        require(discoveryTimeoutMillis in 1..MAXIMUM_DISCOVERY_TIMEOUT_MILLIS) {
            "discoveryTimeoutMillis must be in 1..$MAXIMUM_DISCOVERY_TIMEOUT_MILLIS"
        }
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.P) {
            return CompletableFuture.completedFuture(unavailable(DiscoveryStatus.API_UNSUPPORTED))
        }
        val result = CompletableFuture<DiscoveryResult>()
        val service = CompletableFuture<SEService>()
        val shutdownRequested = AtomicBoolean(false)
        val shutdownService: () -> Unit = {
            if (shutdownRequested.compareAndSet(false, true)) {
                closeServiceWhenReady(service) { connected -> connected.shutdown() }
            }
        }
        val timeout = discoveryTimeoutExecutor.schedule(
            {
                completeDiagnosticTimeoutUnlessResolved(result, shutdownService)
            },
            discoveryTimeoutMillis,
            TimeUnit.MILLISECONDS,
        )
        result.whenComplete { discovered, _ ->
            timeout.cancel(false)
            if (discovered?.status != DiscoveryStatus.AVAILABLE) {
                shutdownService()
            }
        }
        try {
            val connecting = SEService(context.applicationContext, executor) {
                service.whenCompleteAsync(
                    { connected, failure ->
                        if (connected == null || failure != null) {
                            result.complete(unavailable(DiscoveryStatus.SERVICE_FAILED,
                                listOf(classifyFailure(null, failure ?: IllegalStateException("OMAPI service missing")))))
                        } else {
                            discoverConnectedService(
                                connected,
                                configuration,
                                result,
                                shutdownService,
                            )
                        }
                    },
                    executor,
                )
            }
            service.complete(connecting)
        } catch (error: Exception) {
            service.completeExceptionally(error)
            result.complete(unavailable(DiscoveryStatus.SERVICE_FAILED, listOf(classifyFailure(null, error))))
        } catch (error: LinkageError) {
            service.completeExceptionally(error)
            result.complete(unavailable(DiscoveryStatus.SERVICE_FAILED, listOf(classifyFailure(null, error))))
        }
        return result
    }

    private fun discoverConnectedService(
        service: SEService,
        configuration: Configuration,
        result: CompletableFuture<DiscoveryResult>,
        shutdownService: () -> Unit,
    ) {
        if (result.isDone) {
            shutdownService()
            return
        }
        val admitted = mutableListOf<Pair<KagemushaDeviceLifecycleBridgeV1, OmapiChannel>>()
        val failures = mutableListOf<DiscoveryFailure>()
        try {
            val readers = service.readers
                .asSequence()
                .filter { reader -> acceptsReaderName(reader.name, configuration.readerName) }
                .sortedBy(Reader::getName)
                .toList()
            for (reader in readers) {
                val owned = openChannel(reader, configuration.appletAid, failures) ?: continue
                try {
                    val endpoint = KagemushaSecureElementApduEndpointV1(owned)
                    val bridge = KagemushaDeviceLifecycleBridgeV1.withSecureElementEndpoint(endpoint)
                    admitted += bridge to owned
                } catch (error: RuntimeException) {
                    failures += DiscoveryFailure(reader.name, FailureReason.CAPABILITY_REJECTED, error)
                    owned.close()
                } catch (error: LinkageError) {
                    failures += DiscoveryFailure(reader.name, FailureReason.CAPABILITY_REJECTED, error)
                    owned.close()
                }
            }
            if (admitted.size == 1) {
                val (bridge, owned) = admitted.single()
                owned.attach(service)
                if (!result.complete(DiscoveryResult(bridge, DiscoveryStatus.AVAILABLE, failures, owned::close))) {
                    owned.close()
                }
            } else {
                admitted.forEach { (_, owned) -> owned.close() }
                shutdownService()
                result.complete(unavailable(when {
                    admitted.size > 1 -> DiscoveryStatus.AMBIGUOUS
                    readers.isEmpty() -> DiscoveryStatus.NO_EMBEDDED_READER
                    else -> DiscoveryStatus.UNAVAILABLE
                }, failures))
            }
        } catch (error: Exception) {
            admitted.forEach { (_, owned) -> owned.close() }
            shutdownService()
            result.complete(unavailable(DiscoveryStatus.SERVICE_FAILED, failures + classifyFailure(null, error)))
        } catch (error: LinkageError) {
            admitted.forEach { (_, owned) -> owned.close() }
            shutdownService()
            result.complete(unavailable(DiscoveryStatus.SERVICE_FAILED, failures + classifyFailure(null, error)))
        }
    }

    /** Return the fixed production/foundation AID without exposing mutable shared storage. */
    @JvmStatic
    fun defaultAppletAid(): ByteArray = DEFAULT_APPLET_AID.copyOf()

    internal fun acceptsReaderName(candidate: String, configured: String?): Boolean =
        isEmbeddedReaderName(candidate) && (configured == null || candidate == configured)

    // Android OMAPI names embedded readers eSE, eSE1, eSE2, ...; SIM/SD readers can be removable.
    private fun isEmbeddedReaderName(candidate: String): Boolean =
        candidate == "eSE" || EMBEDDED_READER_NAME.matches(candidate)

    /** Cleanup must still run if the caller shuts down its executor after discovery returns. */
    internal fun <T : Any> closeServiceWhenReady(
        service: CompletableFuture<T>,
        close: (T) -> Unit,
    ) {
        service.whenCompleteAsync(
            { connected, _ -> if (connected != null) runCatching { close(connected) } },
            serviceShutdownExecutor,
        )
    }

    internal fun completeDiagnosticTimeoutUnlessResolved(result: CompletableFuture<DiscoveryResult>,
        onTimeout: () -> Unit): Boolean {
        val completed = result.complete(unavailable(DiscoveryStatus.TIMED_OUT))
        if (completed) onTimeout()
        return completed
    }

    @TargetApi(Build.VERSION_CODES.P)
    private fun openChannel(reader: Reader, aid: ByteArray, failures: MutableList<DiscoveryFailure>): OmapiChannel? {
        var session: Session? = null
        var channel: Channel? = null
        return try {
            if (!reader.isSecureElementPresent) {
                failures += DiscoveryFailure(reader.name, FailureReason.SECURE_ELEMENT_ABSENT, null)
                return null
            }
            session = reader.openSession()
            channel = session.openLogicalChannel(aid)
            if (channel == null) {
                failures += DiscoveryFailure(reader.name, FailureReason.NO_LOGICAL_CHANNEL, null)
                session.close()
                return null
            }
            OmapiChannel(session, channel)
        } catch (error: Exception) {
            failures += classifyFailure(reader.name, error)
            runCatching { channel?.close() }
            runCatching { session?.close() }
            null
        }
    }

    @TargetApi(Build.VERSION_CODES.P)
    private class OmapiChannel(
        private val session: Session,
        private val channel: Channel,
    ) : KagemushaSecureElementApduEndpointV1.Channel {
        private var owner: SEService? = null

        fun attach(service: SEService) {
            check(owner == null)
            owner = service
        }

        override fun transmit(command: ByteArray): ByteArray = try {
            channel.transmit(command)
        } catch (error: Exception) {
            throw IllegalStateException("OMAPI lifecycle applet transceive failed", error)
        }

        override fun close() {
            runCatching { channel.close() }
            runCatching { session.close() }
            runCatching { owner?.shutdown() }
            owner = null
        }
    }

    private val DEFAULT_APPLET_AID = byteArrayOf(
        0xf0.toByte(), 0x4f, 0x44, 0x4a, 0x52, 0x4e, 0x00, 0x01,
    )
    private val EMBEDDED_READER_NAME = Regex("eSE[1-9][0-9]*")

    const val DEFAULT_DISCOVERY_TIMEOUT_MILLIS: Long = 10_000
    private const val MAXIMUM_DISCOVERY_TIMEOUT_MILLIS: Long = 60_000

    private val discoveryTimeoutExecutor: ScheduledExecutorService =
        Executors.newSingleThreadScheduledExecutor { runnable ->
            Thread(runnable, "kagemusha-omapi-timeout").apply { isDaemon = true }
        }

    private val serviceShutdownExecutor: Executor =
        Executors.newCachedThreadPool { runnable ->
            Thread(runnable, "kagemusha-omapi-shutdown").apply { isDaemon = true }
        }
}
