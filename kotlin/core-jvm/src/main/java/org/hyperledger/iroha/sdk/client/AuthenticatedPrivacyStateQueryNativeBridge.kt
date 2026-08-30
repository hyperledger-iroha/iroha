// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import java.security.SecureRandom
import java.util.Arrays
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.privacy.PrivacyFinalizedStateProjectionV1
import org.hyperledger.iroha.sdk.privacy.PrivacyFinalizedStateRequestV1
import org.hyperledger.iroha.sdk.privacy.PrivacyFinalizedStateViewV1

/** ABI-22 native codec and verifier for authenticated finalized privacy-state IDs 97–104. */
class AuthenticatedPrivacyStateQueryNativeBridge private constructor() {
    /** Native-bound signed ID97–104 body plus the private preparation used to verify its result. */
    class SignedQueryV1 internal constructor(
        preparation: ByteArray,
        requestBody: ByteArray,
        internal val request: PrivacyFinalizedStateRequestV1,
        internal val networkId: NetworkId,
    ) {
        private val nativePreparation = preparation.copyOf()
        private val canonicalRequestBody = requestBody.copyOf()

        /** Canonical versioned `SignedQuery` bytes for exact `POST /v1/query`. */
        fun requestBody(): ByteArray = canonicalRequestBody.copyOf()

        internal fun preparation(): ByteArray = nativePreparation.copyOf()
    }

    companion object {
        const val REQUIRED_BRIDGE_ABI_VERSION: Int = 22
        const val RESPONSE_MAX_BYTES: Long = 256L * 1024L
        private const val DIGEST_BYTES = 32
        private const val NONCE_BYTES = 32
        private const val PREPARATION_MAX_BYTES = 64 * 1024
        private const val SIGNATURE_MAX_BYTES = 16 * 1024
        private const val SIGNED_QUERY_MAX_BYTES = 64 * 1024
        private val nonceRandom = SecureRandom()

        private val nativeLoadResult: Result<Unit> by lazy {
            runCatching {
                System.loadLibrary("connect_norito_bridge")
                val actual = nativeBridgeAbiVersion()
                check(actual == REQUIRED_BRIDGE_ABI_VERSION) {
                    "native finalized privacy-state ABI mismatch: expected " +
                        "$REQUIRED_BRIDGE_ABI_VERSION, found $actual"
                }
            }
        }

        /** Build and opaquely sign one fresh member of the closed ID97–104 query union. */
        @JvmStatic
        fun buildSignedPrivacyStateQueryV1(
            request: PrivacyFinalizedStateRequestV1,
            networkId: NetworkId,
            authorityAccountId: String,
            signer: IrohaQuerySignatureProvider,
        ): SignedQueryV1 {
            val nonce = ByteArray(NONCE_BYTES)
            repeat(16) {
                if (nonce.any { byte -> byte != 0.toByte() }) return@repeat
                nonceRandom.nextBytes(nonce)
            }
            check(nonce.any { it != 0.toByte() }) {
                "secure privacy state-query nonce generator repeatedly returned zero"
            }
            return buildSignedPrivacyStateQueryAtV1(
                request,
                networkId,
                authorityAccountId,
                signer,
                System.currentTimeMillis(),
                nonce,
            )
        }

        internal fun buildSignedPrivacyStateQueryAtV1(
            request: PrivacyFinalizedStateRequestV1,
            networkId: NetworkId,
            authorityAccountId: String,
            signer: IrohaQuerySignatureProvider,
            creationTimeMs: Long,
            nonce: ByteArray,
        ): SignedQueryV1 {
            requireNative()
            require(creationTimeMs > 0) { "creationTimeMs must be positive" }
            require(nonce.size == NONCE_BYTES && nonce.any { it != 0.toByte() }) {
                "nonce must contain exactly 32 nonzero bytes"
            }
            val binding = request.requestBinding()
            require(binding.isNotEmpty() && binding.size <= 128) {
                "request binding violates the closed native bound"
            }
            val prepared = try {
                nativePreparePrivacyStateQueryV1(
                    networkId.bytes(),
                    authorityAccountId.toByteArray(Charsets.UTF_8),
                    request.queryId,
                    request.protocolIndex,
                    binding.copyOf(),
                    creationTimeMs,
                    nonce.copyOf(),
                )
            } finally {
                Arrays.fill(binding, 0.toByte())
            }
            check(
                prepared.size == 2 &&
                    prepared[0].isNotEmpty() &&
                    prepared[0].size <= PREPARATION_MAX_BYTES &&
                    prepared[1].size == DIGEST_BYTES,
            ) { "native privacy state-query preparation returned an invalid shape" }
            val digest = prepared[1].copyOf()
            val signature = try {
                signer.signQueryDigest(digest.copyOf()).copyOf()
            } finally {
                Arrays.fill(digest, 0.toByte())
                Arrays.fill(prepared[1], 0.toByte())
            }
            require(signature.isNotEmpty() && signature.size <= SIGNATURE_MAX_BYTES) {
                "opaque query signer returned invalid signature bytes"
            }
            val requestBody = try {
                nativeFinalizePrivacyStateQueryV1(
                    prepared[0].copyOf(),
                    signature.copyOf(),
                )
            } finally {
                Arrays.fill(signature, 0.toByte())
            }
            check(requestBody.isNotEmpty() && requestBody.size <= SIGNED_QUERY_MAX_BYTES) {
                "native privacy state-query finalizer violated the request byte bound"
            }
            return SignedQueryV1(prepared[0], requestBody, request, networkId)
        }

        /** Natively verify and project the exact finalized response bound to [signedQuery]. */
        @JvmStatic
        fun projectPrivacyStateQueryV1(
            signedQuery: SignedQueryV1,
            responseNorito: ByteArray,
        ): PrivacyFinalizedStateViewV1 {
            requireNative()
            require(responseNorito.isNotEmpty() && responseNorito.size.toLong() <= RESPONSE_MAX_BYTES) {
                "responseNorito violates its closed byte bound"
            }
            val projection = nativeProjectPrivacyStateQueryV1(
                signedQuery.preparation(),
                responseNorito.copyOf(),
            )
            check(
                projection.isNotEmpty() &&
                    projection.size <= PrivacyFinalizedStateProjectionV1.MAX_PROJECTION_BYTES,
            ) { "native privacy state-query projection violated its byte bound" }
            return PrivacyFinalizedStateProjectionV1.parse(
                projection,
                signedQuery.request,
                signedQuery.networkId,
            )
        }

        private fun requireNative() {
            nativeLoadResult.getOrElse { failure ->
                throw IllegalStateException(
                    "native authenticated finalized privacy-state bridge is unavailable",
                    failure,
                )
            }
        }

        @JvmStatic private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic private external fun nativePreparePrivacyStateQueryV1(
            networkId: ByteArray,
            authorityAccountId: ByteArray,
            queryId: Int,
            protocolIndex: Int,
            requestBinding: ByteArray,
            creationTimeMs: Long,
            nonce: ByteArray,
        ): Array<ByteArray>

        @JvmStatic private external fun nativeFinalizePrivacyStateQueryV1(
            preparation: ByteArray,
            signature: ByteArray,
        ): ByteArray

        @JvmStatic private external fun nativeProjectPrivacyStateQueryV1(
            preparation: ByteArray,
            responseNorito: ByteArray,
        ): ByteArray
    }
}
