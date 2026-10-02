// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

/** Product transport for an SDK-selected public original; replies remain untrusted.
 * Use the independently selected authenticated Core endpoint and recheck before every
 * request attempt and response exposure. Native signature/policy/custody intake is separate.
 */
fun interface KagemushaOrdinaryIdentityOriginalTransportV1 {
    suspend fun exchange(original: KagemushaOrdinaryIdentityHttpOriginalV1): ByteArray
}

/** Guarded public carrier selected only by a held native reservation/platform owner.
 * No public constructor accepts an account, nonce, C, key, policy or signing subject.
 */
class KagemushaOrdinaryIdentityHttpOriginalV1 private constructor(
    val requestId: String, val path: String, body: ByteArray, private val guard: () -> Unit,
) {
    private val originalBody = body.copyOf()
    fun body(): ByteArray { requireCurrent(); return originalBody.copyOf().also { requireCurrent() } }
    fun requireCurrent() = guard()
    internal companion object {
        fun preparation(fields: List<ByteArray>, guard: () -> Unit): KagemushaOrdinaryIdentityHttpOriginalV1 {
            guard()
            return KagemushaOrdinaryIdentityHttpOriginalV1(fields[7].toString(Charsets.US_ASCII),
                "/v1/kagemusha/enrollment/ordinary/prepare", KagemushaOrdinaryIdentityHttpCodecV1.preparationBody(fields), guard)
                .also { guard() }
        }
        fun rawAttestation(signedC: ByteArray, point: ByteArray, raw: ByteArray,
            guard: () -> Unit): KagemushaOrdinaryIdentityHttpOriginalV1 {
            guard()
            val c = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(signedC)
            return KagemushaOrdinaryIdentityHttpOriginalV1(
                KagemushaOrdinaryIdentityHttpCodecV1.rawAttestationRequestId(c.attestationChallenge()),
                "/v1/kagemusha/enrollment/ordinary/raw-attestation",
                KagemushaOrdinaryIdentityHttpCodecV1.rawAttestationBody(signedC, point, raw), guard).also { guard() }
        }
        fun certificate(attempt: ByteArray, body: ByteArray, guard: () -> Unit): KagemushaOrdinaryIdentityHttpOriginalV1 {
            guard()
            return KagemushaOrdinaryIdentityHttpOriginalV1(KagemushaOrdinaryIdentityHttpCodecV1.certificateRequestId(attempt),
                "/v1/kagemusha/enrollment/ordinary/certificate", body, guard).also { guard() }
        }
        fun retail(attempt: ByteArray, kind: String, body: ByteArray, guard: () -> Unit): KagemushaOrdinaryIdentityHttpOriginalV1 {
            guard()
            return KagemushaOrdinaryIdentityHttpOriginalV1(KagemushaOrdinaryIdentityHttpCodecV1.retailRequestId(attempt, kind),
                "/v1/kagemusha/enrollment/ordinary/$kind", body, guard).also { guard() }
        }
    }
}
