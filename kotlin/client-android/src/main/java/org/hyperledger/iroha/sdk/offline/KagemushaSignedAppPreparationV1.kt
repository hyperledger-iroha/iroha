// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest

/** The sole Android JNI endpoint for exact issuer-signed pre-KeyMint preparation. */
internal interface KagemushaSignedAppPreparationEndpointV1 {
    fun contract(): IntArray?
    fun verify(
        signedPreparation: ByteArray,
        canonicalPolicy: ByteArray,
        pinnedPolicySha256: ByteArray,
        accountI105: ByteArray,
        clientNonce: ByteArray,
        releaseId: ByteArray,
        profileId: ByteArray,
        laneId: ByteArray,
        trustedNowMs: Long,
    ): ByteArray?
}

/**
 * Preflight the issuer's signed Android challenge before generating a KeyMint key.
 *
 * The policy digest must come from independently authenticated app configuration, never the
 * issuer response. [trustedNowMs] must come from the enrollment service's trusted clock. This
 * result is only the signed server nonce: it does not qualify hardware, admit a credential or
 * move money. The process-owned native enrollment ceremony rechecks the same preparation.
 */
class KagemushaSignedAppPreparationV1 private constructor(
    canonicalPolicy: ByteArray,
    pinnedPolicySha256: ByteArray,
    private val endpoint: KagemushaSignedAppPreparationEndpointV1,
) {
    private val policy = canonicalPolicy.copyOf()
    private val policySha256 = pinnedPolicySha256.copyOf()

    /** Return the exact verified 32-byte server nonce for the retained native selection. */
    fun verifyForKeyMint(
        selection: KagemushaNativeEnrollmentPhasesV1.Selection,
        signedPreparation: ByteArray,
        trustedNowMs: Long,
    ): ByteArray {
        require(signedPreparation.size == TOKEN_BYTES && trustedNowMs > 0) {
            "Invalid signed app preparation or trusted time"
        }
        // The process-owned phase-1 selection already required canonical I105. Rust
        // independently parses and re-encodes the account before signature verification.
        val account = selection.accountI105.toByteArray(Charsets.UTF_8)
        require(account.size in 1..ACCOUNT_MAX_BYTES &&
            !selection.accountI105.contains('@') && '\u0000' !in selection.accountI105)
        val token = signedPreparation.copyOf()
        val clientNonce = selection.clientNonce()
        val serverNonce = try {
            endpoint.verify(token, policy.copyOf(), policySha256.copyOf(), account,
                clientNonce, selection.releaseId(), selection.profileId(), selection.laneId(),
                trustedNowMs)?.copyOf()
                ?: throw IllegalStateException("Issuer-signed app preparation was rejected")
        } catch (error: LinkageError) {
            throw IllegalStateException("Issuer-signed app preparation JNI is unavailable", error)
        }
        check(serverNonce.size == 32 && serverNonce.any { it != 0.toByte() } &&
            !serverNonce.contentEquals(clientNonce) &&
            serverNonce.contentEquals(signedPreparation.copyOfRange(49, 81))) {
            "Issuer-signed app preparation JNI returned an invalid server nonce"
        }
        return serverNonce
    }

    companion object {
        private const val TOKEN_BYTES = 273
        private const val POLICY_MAX_BYTES = 8 * 1024
        private const val ACCOUNT_MAX_BYTES = 512
        private val CONTRACT = intArrayOf(1, TOKEN_BYTES, POLICY_MAX_BYTES, ACCOUNT_MAX_BYTES)

        /** Load the native verifier with an independently pinned canonical issuer policy. */
        @JvmStatic
        fun open(
            canonicalPolicy: ByteArray,
            pinnedPolicySha256: ByteArray,
        ): KagemushaSignedAppPreparationV1 {
            try {
                System.loadLibrary("connect_norito_bridge")
                return openEndpoint(canonicalPolicy, pinnedPolicySha256,
                    KagemushaSignedAppPreparationJniV1)
            } catch (error: LinkageError) {
                throw IllegalStateException("Issuer-signed app preparation JNI is unavailable", error)
            }
        }

        internal fun openEndpoint(
            canonicalPolicy: ByteArray,
            pinnedPolicySha256: ByteArray,
            endpoint: KagemushaSignedAppPreparationEndpointV1,
        ): KagemushaSignedAppPreparationV1 {
            require(canonicalPolicy.size in 1..POLICY_MAX_BYTES &&
                pinnedPolicySha256.size == 32 && pinnedPolicySha256.any { it != 0.toByte() } &&
                MessageDigest.getInstance("SHA-256").digest(canonicalPolicy)
                    .contentEquals(pinnedPolicySha256)) {
                "Issuer policy does not match the independent pin"
            }
            val contract = try { endpoint.contract() } catch (error: LinkageError) {
                throw IllegalStateException("Issuer-signed app preparation JNI is unavailable", error)
            }
            check(contract?.contentEquals(CONTRACT) == true) {
                "Issuer-signed app preparation JNI contract mismatch"
            }
            return KagemushaSignedAppPreparationV1(canonicalPolicy, pinnedPolicySha256, endpoint)
        }
    }
}

/** Exact Rust verifier exports; there is no Kotlin signature-verification fallback. */
internal object KagemushaSignedAppPreparationJniV1 : KagemushaSignedAppPreparationEndpointV1 {
    override fun contract(): IntArray? = nativeContractV1()
    override fun verify(
        signedPreparation: ByteArray,
        canonicalPolicy: ByteArray,
        pinnedPolicySha256: ByteArray,
        accountI105: ByteArray,
        clientNonce: ByteArray,
        releaseId: ByteArray,
        profileId: ByteArray,
        laneId: ByteArray,
        trustedNowMs: Long,
    ): ByteArray? = nativeVerifyV1(signedPreparation, canonicalPolicy, pinnedPolicySha256,
        accountI105, clientNonce, releaseId, profileId, laneId, trustedNowMs)

    @JvmStatic private external fun nativeContractV1(): IntArray?
    @JvmStatic private external fun nativeVerifyV1(
        signedPreparation: ByteArray,
        canonicalPolicy: ByteArray,
        pinnedPolicySha256: ByteArray,
        accountI105: ByteArray,
        clientNonce: ByteArray,
        releaseId: ByteArray,
        profileId: ByteArray,
        laneId: ByteArray,
        trustedNowMs: Long,
    ): ByteArray?
}
