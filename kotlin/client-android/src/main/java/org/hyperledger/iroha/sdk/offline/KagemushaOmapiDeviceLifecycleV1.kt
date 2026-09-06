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
 * ABI-23 capability frame. StrongBox or an eSE feature flag alone never enables the wallet.
 */
object KagemushaOmapiDeviceLifecycleV1 {
    /** Exact applet selection and optional reader pin. */
    class Configuration @JvmOverloads constructor(
        readerName: String? = null,
        appletAid: ByteArray = DEFAULT_APPLET_AID,
    ) {
        internal val readerName: String? = readerName?.also {
            require(it.isNotEmpty() && it == it.trim()) { "readerName must be exact and non-empty" }
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
     * bridge. With no explicit reader pin, eSE, UICC and SD readers are all eligible; exactly one
     * applet must pass the complete capability contract.
     */
    @JvmStatic
    @JvmOverloads
    fun openAsync(
        context: Context,
        executor: Executor,
        configuration: Configuration = Configuration(),
        discoveryTimeoutMillis: Long = DEFAULT_DISCOVERY_TIMEOUT_MILLIS,
    ): CompletableFuture<KagemushaDeviceLifecycleBridgeV1> {
        require(discoveryTimeoutMillis in 1..MAXIMUM_DISCOVERY_TIMEOUT_MILLIS) {
            "discoveryTimeoutMillis must be in 1..$MAXIMUM_DISCOVERY_TIMEOUT_MILLIS"
        }
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.P) {
            return CompletableFuture.completedFuture(KagemushaDeviceLifecycleBridgeV1.onlineOnly())
        }
        val result = CompletableFuture<KagemushaDeviceLifecycleBridgeV1>()
        val service = CompletableFuture<SEService>()
        val shutdownRequested = AtomicBoolean(false)
        val shutdownService: () -> Unit = {
            if (shutdownRequested.compareAndSet(false, true)) {
                service.whenCompleteAsync(
                    { connected, _ -> runCatching { connected?.shutdown() } },
                    executor,
                )
            }
        }
        val timeout = discoveryTimeoutExecutor.schedule(
            {
                completeUnavailableUnlessResolved(result, shutdownService)
            },
            discoveryTimeoutMillis,
            TimeUnit.MILLISECONDS,
        )
        result.whenComplete { bridge, _ ->
            timeout.cancel(false)
            if (bridge?.availability != KagemushaDeviceLifecycleBridgeV1.Availability.AVAILABLE) {
                shutdownService()
            }
        }
        try {
            val connecting = SEService(context.applicationContext, executor) {
                service.whenCompleteAsync(
                    { connected, failure ->
                        if (connected == null || failure != null) {
                            result.complete(KagemushaDeviceLifecycleBridgeV1.onlineOnly())
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
        } catch (_: Exception) {
            service.completeExceptionally(IllegalStateException("OMAPI service creation failed"))
            result.complete(KagemushaDeviceLifecycleBridgeV1.onlineOnly())
        } catch (_: LinkageError) {
            service.completeExceptionally(IllegalStateException("OMAPI service linkage failed"))
            result.complete(KagemushaDeviceLifecycleBridgeV1.onlineOnly())
        }
        return result
    }

    private fun discoverConnectedService(
        service: SEService,
        configuration: Configuration,
        result: CompletableFuture<KagemushaDeviceLifecycleBridgeV1>,
        shutdownService: () -> Unit,
    ) {
        if (result.isDone) {
            shutdownService()
            return
        }
        val admitted = mutableListOf<Pair<KagemushaDeviceLifecycleBridgeV1, OmapiChannel>>()
        try {
            val readers = service.readers
                .asSequence()
                .filter { reader -> acceptsReaderName(reader.name, configuration.readerName) }
                .sortedBy(Reader::getName)
                .toList()
            for (reader in readers) {
                val owned = openChannel(reader, configuration.appletAid) ?: continue
                try {
                    val endpoint = KagemushaSecureElementApduEndpointV1(owned)
                    val bridge = KagemushaDeviceLifecycleBridgeV1.withSecureElementEndpoint(endpoint)
                    admitted += bridge to owned
                } catch (_: RuntimeException) {
                    owned.close()
                } catch (_: LinkageError) {
                    owned.close()
                }
            }
            if (admitted.size == 1) {
                val (bridge, owned) = admitted.single()
                owned.attach(service)
                if (!result.complete(bridge)) owned.close()
            } else {
                admitted.forEach { (_, owned) -> owned.close() }
                shutdownService()
                result.complete(KagemushaDeviceLifecycleBridgeV1.onlineOnly())
            }
        } catch (_: Exception) {
            admitted.forEach { (_, owned) -> owned.close() }
            shutdownService()
            result.complete(KagemushaDeviceLifecycleBridgeV1.onlineOnly())
        } catch (_: LinkageError) {
            admitted.forEach { (_, owned) -> owned.close() }
            shutdownService()
            result.complete(KagemushaDeviceLifecycleBridgeV1.onlineOnly())
        }
    }

    /** Return the fixed production/foundation AID without exposing mutable shared storage. */
    @JvmStatic
    fun defaultAppletAid(): ByteArray = DEFAULT_APPLET_AID.copyOf()

    internal fun acceptsReaderName(candidate: String, configured: String?): Boolean =
        configured == null || candidate == configured

    internal fun completeUnavailableUnlessResolved(
        result: CompletableFuture<KagemushaDeviceLifecycleBridgeV1>,
        onTimeout: () -> Unit,
    ): Boolean {
        val completed = result.complete(KagemushaDeviceLifecycleBridgeV1.onlineOnly())
        if (completed) onTimeout()
        return completed
    }

    @TargetApi(Build.VERSION_CODES.P)
    private fun openChannel(reader: Reader, aid: ByteArray): OmapiChannel? {
        var session: Session? = null
        var channel: Channel? = null
        return try {
            if (!reader.isSecureElementPresent) return null
            session = reader.openSession()
            channel = session.openLogicalChannel(aid)
            if (channel == null) {
                session.close()
                return null
            }
            OmapiChannel(session, channel)
        } catch (_: Exception) {
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

    const val DEFAULT_DISCOVERY_TIMEOUT_MILLIS: Long = 10_000
    private const val MAXIMUM_DISCOVERY_TIMEOUT_MILLIS: Long = 60_000

    private val discoveryTimeoutExecutor: ScheduledExecutorService =
        Executors.newSingleThreadScheduledExecutor { runnable ->
            Thread(runnable, "kagemusha-omapi-timeout").apply { isDaemon = true }
        }
}
