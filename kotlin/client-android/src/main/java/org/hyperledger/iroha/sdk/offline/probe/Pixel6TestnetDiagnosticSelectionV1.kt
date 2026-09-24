// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.security.MessageDigest

/** JNI endpoint for a model-canonical, non-monetary Pixel 6 testnet selection. */
internal interface Pixel6TestnetDiagnosticSelectionEndpointV1 {
    fun contract(): IntArray?
    fun create(networkId: ByteArray, ownerScope: ByteArray): ByteArray?
}

/**
 * Construct a repeatable raw StrongBox probe subject through the Rust KAGEMUSHA data model.
 *
 * All identifiers are diagnostic-domain hashes. This subject has no issuer release, enrolled
 * credential, real transition statement, no-fork guarantee, or offline monetary authority.
 */
class Pixel6TestnetDiagnosticSelectionV1 private constructor(
    private val endpoint: Pixel6TestnetDiagnosticSelectionEndpointV1,
) {
    /** Return the exact 460-byte Rust-model subject for one current wallet/network owner. */
    fun create(networkId: ByteArray, ownerScope: ByteArray): ByteArray {
        val network = networkId.copyOf()
        val owner = ownerScope.copyOf()
        require(network.size == 32 && network.any { it != 0.toByte() } &&
            (network[31].toInt() and 1) == 1) {
            "Pixel 6 diagnostic network must be an exact marked NetworkId"
        }
        require(owner.size in 1..OWNER_MAX_BYTES)
        val frame = try {
            checkNotNull(endpoint.create(network, owner)) {
                "Pixel 6 diagnostic selection is unavailable"
            }.copyOf()
        } catch (error: LinkageError) {
            throw IllegalStateException("Pixel 6 diagnostic selection JNI is unavailable", error)
        }
        try {
            require(frame.size == FRAME_BYTES) { "Pixel 6 diagnostic selection size mismatch" }
            AndroidPixel6TestnetStrongBoxObservationV1.requireCanonicalFrame(
                network, frame.copyOfRange(59, 91), frame, frame.copyOfRange(219, 251),
                frame.copyOfRange(428, 444), frame.copyOfRange(444, 460),
            )
            val domain = "iroha:kagemusha:v1:hardware-transition-selection\u0000"
                .toByteArray(Charsets.US_ASCII)
            require(frame.copyOfRange(0, 49).contentEquals(domain))
            require(frame[331] == 5.toByte()) { "Pixel 6 diagnostic operation is not Rotate" }
            for ((tag, start) in intArrayOf(59, 91, 123, 155, 219, 251, 291, 332).withIndex()) {
                require(frame.copyOfRange(start, start + 32)
                    .contentEquals(diagnosticField(network, owner, tag + 1))) {
                    "Pixel 6 diagnostic selection has a non-diagnostic identity"
                }
            }
            require(frame.copyOfRange(283, 291).contentEquals(byteArrayOf(1) + ByteArray(7)) &&
                frame.copyOfRange(323, 331).contentEquals(byteArrayOf(1) + ByteArray(7)) &&
                frame.copyOfRange(364, 444).all { it == 0.toByte() } &&
                frame.copyOfRange(444, 460).contentEquals(byteArrayOf(1) + ByteArray(15))) {
                "Pixel 6 diagnostic selection carries a non-diagnostic transition"
            }
        } catch (error: IllegalArgumentException) {
            throw IllegalStateException("Pixel 6 diagnostic selection JNI returned an invalid frame", error)
        }
        return frame
    }

    private fun diagnosticField(network: ByteArray, owner: ByteArray, tag: Int): ByteArray =
        MessageDigest.getInstance("SHA-256").digest(
            DIAGNOSTIC_DOMAIN + byteArrayOf(tag.toByte()) + network +
                byteArrayOf(owner.size.toByte(), (owner.size ushr 8).toByte()) + owner,
        )

    companion object {
        private const val OWNER_MAX_BYTES = 2_048
        private const val FRAME_BYTES = 460
        private val CONTRACT = intArrayOf(1, 32, OWNER_MAX_BYTES, FRAME_BYTES)
        private val DIAGNOSTIC_DOMAIN =
            "iroha:kagemusha:v1:pixel6-testnet-diagnostic-selection\u0000"
                .toByteArray(Charsets.US_ASCII)

        /** Load only the diagnostic JNI constructor; no coordinator or monetary gate is opened. */
        @JvmStatic fun open(): Pixel6TestnetDiagnosticSelectionV1 {
            try {
                System.loadLibrary("connect_norito_bridge")
                return openEndpoint(Pixel6TestnetDiagnosticSelectionJniV1)
            } catch (error: LinkageError) {
                throw IllegalStateException("Pixel 6 diagnostic selection JNI is unavailable", error)
            }
        }

        internal fun openEndpoint(
            endpoint: Pixel6TestnetDiagnosticSelectionEndpointV1,
        ): Pixel6TestnetDiagnosticSelectionV1 {
            val contract = try { endpoint.contract() } catch (error: LinkageError) {
                throw IllegalStateException("Pixel 6 diagnostic selection JNI is unavailable", error)
            }
            check(contract?.contentEquals(CONTRACT) == true) {
                "Pixel 6 diagnostic selection contract mismatch"
            }
            return Pixel6TestnetDiagnosticSelectionV1(endpoint)
        }
    }
}

/** Rust JNI exports, with no Kotlin frame-construction fallback. */
internal object Pixel6TestnetDiagnosticSelectionJniV1 : Pixel6TestnetDiagnosticSelectionEndpointV1 {
    override fun contract(): IntArray? = nativeContractV1()
    override fun create(networkId: ByteArray, ownerScope: ByteArray): ByteArray? =
        nativeCreateV1(networkId, ownerScope)

    @JvmStatic private external fun nativeContractV1(): IntArray?
    @JvmStatic private external fun nativeCreateV1(networkId: ByteArray, ownerScope: ByteArray): ByteArray?
}
