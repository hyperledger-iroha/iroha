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

/**
 * Android OMAPI discovery for a provisioned embedded-secure-element lifecycle applet.
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
     * Open without blocking the application thread.
     *
     * An absent service, denied AID, incomplete foundation applet, malformed capability frame,
     * ambiguous qualified readers, or any platform error completes with an online-only bridge.
     */
    @JvmStatic
    @JvmOverloads
    fun openAsync(
        context: Context,
        executor: Executor,
        configuration: Configuration = Configuration(),
    ): CompletableFuture<KagemushaDeviceLifecycleBridgeV1> {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.P) {
            return CompletableFuture.completedFuture(KagemushaDeviceLifecycleBridgeV1.onlineOnly())
        }
        val result = CompletableFuture<KagemushaDeviceLifecycleBridgeV1>()
        lateinit var service: SEService
        try {
            service = SEService(context.applicationContext, executor) {
                if (result.isDone) {
                    runCatching { service.shutdown() }
                    return@SEService
                }
                val admitted = mutableListOf<Pair<KagemushaDeviceLifecycleBridgeV1, OmapiChannel>>()
                try {
                    val readers = service.readers
                        .asSequence()
                        .filter { reader ->
                            configuration.readerName?.let { reader.name == it }
                                ?: reader.name.startsWith("eSE", ignoreCase = true)
                        }
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
                        service.shutdown()
                        result.complete(KagemushaDeviceLifecycleBridgeV1.onlineOnly())
                    }
                } catch (_: Exception) {
                    admitted.forEach { (_, owned) -> owned.close() }
                    runCatching { service.shutdown() }
                    result.complete(KagemushaDeviceLifecycleBridgeV1.onlineOnly())
                } catch (_: LinkageError) {
                    admitted.forEach { (_, owned) -> owned.close() }
                    runCatching { service.shutdown() }
                    result.complete(KagemushaDeviceLifecycleBridgeV1.onlineOnly())
                }
            }
        } catch (_: Exception) {
            result.complete(KagemushaDeviceLifecycleBridgeV1.onlineOnly())
        } catch (_: LinkageError) {
            result.complete(KagemushaDeviceLifecycleBridgeV1.onlineOnly())
        }
        return result
    }

    /** Return the fixed production/foundation AID without exposing mutable shared storage. */
    @JvmStatic
    fun defaultAppletAid(): ByteArray = DEFAULT_APPLET_AID.copyOf()

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
}
