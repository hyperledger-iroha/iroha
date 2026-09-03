// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

/** Exact asset-neutral KAGEMUSHA V1 readiness projection. */
class KagemushaReadinessV1(
    @JvmField val kagemushaHandoffCapability: String,
    @JvmField val wireVersion: Int,
    @JvmField val deviceLifecycleVersion: Int,
    @JvmField val ready: Boolean,
)

/** Closed reserve-operation catalog exposed by the canonical KAGEMUSHA routes. */
enum class KagemushaReserveOperationKindV1(@JvmField val wireName: String) {
    TOP_UP("top_up"),
    REDEMPTION("redemption"),
}

/** Pollable state of one idempotent reserve operation. */
enum class KagemushaReserveOperationStateV1(@JvmField val wireName: String) {
    PENDING("pending"),
    APPLIED("applied"),
    REJECTED("rejected"),
}

/** Exact terminal rejection catalog for KAGEMUSHA V1 reserve operations. */
enum class KagemushaOperationRejectionCodeV1(@JvmField val wireName: String) {
    INVALID_REQUEST("invalid_request"),
    UNAUTHORIZED("unauthorized"),
    INSUFFICIENT_ONLINE_BALANCE("insufficient_online_balance"),
    INVALID_PROOF("invalid_proof"),
    HARDWARE_POLICY_REJECTED("hardware_policy_rejected"),
    IDENTITY_CONFLICT("identity_conflict"),
    RESERVE_UNDERFLOW("reserve_underflow"),
    ARITHMETIC_OVERFLOW("arithmetic_overflow"),
    INTERNAL_FAILURE("internal_failure"),
}

/** Public rejection metadata that carries no monetary result. */
class KagemushaOperationRejectionV1(
    @JvmField val code: KagemushaOperationRejectionCodeV1,
    detailDigest: ByteArray,
) {
    private val detailDigestValue = requireNonzero32(detailDigest, "detailDigest")

    fun detailDigest(): ByteArray = detailDigestValue.copyOf()
}

/** Verifier boundary that must authenticate applied results against caller-pinned finality. */
fun interface KagemushaOperationStatusVerifierV1<A : Any, R> {
    fun verify(canonicalJson: ByteArray, trustAnchor: A): R
}

/**
 * Structurally validated operation metadata that withholds an applied monetary result.
 *
 * The complete response can leave this type only through [verifyAgainst], which requires an
 * independently supplied trust anchor and verifier.
 */
class UnverifiedKagemushaOperationStatusV1 internal constructor(
    operationId: ByteArray,
    @JvmField val kind: KagemushaReserveOperationKindV1,
    @JvmField val state: KagemushaReserveOperationStateV1,
    @JvmField val rejection: KagemushaOperationRejectionV1?,
    canonicalJson: ByteArray,
) {
    private val operationIdValue = requireNonzero32(operationId, "operationId")
    private val canonicalJsonValue = canonicalJson.copyOf()

    fun operationId(): ByteArray = operationIdValue.copyOf()

    fun <A : Any, R> verifyAgainst(
        trustAnchor: A,
        verifier: KagemushaOperationStatusVerifierV1<A, R>,
    ): R = verifier.verify(canonicalJsonValue.copyOf(), trustAnchor)

    override fun toString(): String =
        "UnverifiedKagemushaOperationStatusV1(kind=$kind, state=$state, result=[WITHHELD])"
}

internal fun requireNonzero32(value: ByteArray, field: String): ByteArray {
    val copy = value.copyOf()
    require(copy.size == 32 && copy.any { it != 0.toByte() }) {
        "$field must be one nonzero 32-byte value"
    }
    return copy
}
