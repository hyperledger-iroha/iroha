// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.security.MessageDigest

/** Existing checked private mailbox operations; publication must preserve exact immutable bytes. */
internal interface KagemushaProbeReviewCustodyIo {
    fun exists(path: String): Boolean
    fun read(path: String, maximum: Int): ByteArray
    fun mkdir(path: String)
    fun publish(path: String, bytes: ByteArray)
}

/** Diagnostic original custody only. Native remains the sole request and monetary authority. */
internal object KagemushaProbeReviewCustody {
    fun expectation(
        io: KagemushaProbeReviewCustodyIo,
        requestId: ByteArray,
        family: String,
        originals: Map<String, ByteArray>,
        continuation: Boolean,
    ): KagemushaWalletReviewExpectationV1 {
        val identity = requestId.copyOf()
        probeRequire(identity.size == 32 && identity.any { it != 0.toByte() })
        val bundles = when (family) {
            "review-send" -> setOf(setOf("request", "destination"))
            "review-unload" -> setOf(setOf("amount"), setOf("amount", "quote", "certificates", "beneficiary"))
            else -> throw EnrollmentProbeFailure("input")
        }
        probeRequire(originals.keys in bundles)
        val exact = originals.mapValues { (_, bytes) -> bytes.copyOf() }.toSortedMap()
        for ((name, bytes) in exact) {
            val maximum = when (name) {
                "request", "certificates" -> 10_034
                "destination", "beneficiary" -> 4_096
                "quote" -> 1_024
                "amount" -> 16
                else -> throw EnrollmentProbeFailure("input")
            }
            probeRequire(bytes.size in 1..maximum && (name != "amount" || bytes.size == 16))
        }
        exact["request-id"] = identity
        exact["family"] = family.toByteArray(Charsets.UTF_8)
        val names = exact.keys.joinToString("\n").toByteArray(Charsets.UTF_8)
        val digest = MessageDigest.getInstance("SHA-256").digest(identity)
            .joinToString("") { "%02x".format(it.toInt() and 255) }
        val root = "private/intents/executed-$digest"
        val directory = "private/intents/review-originals-$digest"
        if (continuation) {
            // Never recreate an absent old record from the newly offered inputs.
            probeRequire(io.exists(root) && io.exists("$directory/names"), "recovery-required")
            probeRequire(io.read("$directory/names", 1_024).contentEquals(names), "recovery-required")
            for ((name, bytes) in exact) {
                val path = "$directory/$name"
                probeRequire(io.exists(path) && io.read(path, bytes.size).contentEquals(bytes), "recovery-required")
            }
            val original = io.read(root, KagemushaWalletReviewProjectionV1.MAXIMUM_BYTES)
            val expectation = KagemushaWalletReviewExpectationV1.OriginalUnstarted(original)
            val kind = if (family == "review-send") KagemushaWalletReviewProjectionV1.Kind.SEND
                else KagemushaWalletReviewProjectionV1.Kind.UNLOAD
            probeRequire(expectation.projection().kind == kind, "recovery-required")
            return expectation
        }
        probeRequire(!io.exists(root), "recovery-required")
        io.mkdir(directory)
        for ((name, bytes) in exact) io.publish("$directory/$name", bytes)
        io.publish("$directory/names", names)
        return KagemushaWalletReviewExpectationV1.Fresh
    }
}
