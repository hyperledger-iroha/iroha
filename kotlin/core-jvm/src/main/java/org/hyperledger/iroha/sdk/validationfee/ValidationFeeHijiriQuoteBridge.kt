package org.hyperledger.iroha.sdk.validationfee

import java.nio.charset.StandardCharsets

/**
 * Native Norito boundary for the live Hijiri validation-fee quote route.
 *
 * No managed JSON encoder or decoder participates in the wire exchange. The native bridge creates
 * the exact V1 request and validates canonical response encoding, request echoes, live next-height
 * semantics, all hashes, and fee arithmetic before returning a typed projection.
 */
class ValidationFeeHijiriQuoteBridge private constructor() {
    companion object {
        private const val LIBRARY_NAME = "connect_norito_bridge"
        private const val REQUIRED_BRIDGE_ABI_VERSION = 23

        private val nativeLoadResult: Result<Unit> by lazy {
            runCatching {
                System.loadLibrary(LIBRARY_NAME)
                val actualAbi = nativeBridgeAbiVersion()
                check(actualAbi == REQUIRED_BRIDGE_ABI_VERSION) {
                    "native Hijiri quote bridge ABI mismatch: expected " +
                        "$REQUIRED_BRIDGE_ABI_VERSION, found $actualAbi"
                }
            }
        }

        /** Encode an exact canonical bare-Norito V1 request. */
        @JvmStatic
        fun encodeRequestV1(request: ValidationFeeHijiriQuoteRequestV1): ByteArray {
            requireNative()
            val encoded = invokeRequiredQuoteNative("nativeEncodeRequestV1") {
                nativeEncodeRequestV1(
                    request.accountId.toByteArray(StandardCharsets.UTF_8),
                    request.qualifyingTransferCount,
                )
            }
            require(encoded.isNotEmpty() &&
                encoded.size <= VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES_V1) {
                "native Hijiri quote encoder returned an invalid request size"
            }
            return encoded.copyOf()
        }

        /**
         * Verify a canonical Norito response against the exact request bytes sent to Torii.
         *
         * The returned object is constructed only after native semantic validation succeeds.
         */
        @JvmStatic
        fun verifyResponseV1(
            responseNorito: ByteArray,
            requestNorito: ByteArray,
        ): ValidationFeeHijiriQuoteV1 {
            require(responseNorito.isNotEmpty() &&
                responseNorito.size <= VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES_V1) {
                "responseNorito must contain 1.." +
                    "$VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES_V1 bytes"
            }
            require(requestNorito.isNotEmpty() &&
                requestNorito.size <= VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES_V1) {
                "requestNorito must contain 1.." +
                    "$VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES_V1 bytes"
            }
            requireNative()
            val projectionJson = invokeRequiredQuoteNative("nativeVerifyResponseV1") {
                nativeVerifyResponseV1(
                    responseNorito.copyOf(),
                    requestNorito.copyOf(),
                )
            }
            return ValidationFeeHijiriQuoteProjectionParser.parse(projectionJson.copyOf())
        }

        internal fun <T> invokeRequiredQuoteNative(method: String, invocation: () -> T): T =
            try {
                invocation()
            } catch (failure: UnsatisfiedLinkError) {
                throw IllegalStateException(
                    "native Hijiri validation-fee quote bridge is unavailable: " +
                        "required ABI-23 method $method is missing",
                    failure,
                )
            }

        private fun requireNative() {
            nativeLoadResult.getOrElse { failure ->
                throw IllegalStateException(
                    "native Hijiri validation-fee quote bridge is unavailable",
                    failure,
                )
            }
        }

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativeEncodeRequestV1(
            accountIdUtf8: ByteArray,
            qualifyingTransferCount: Int,
        ): ByteArray

        @JvmStatic
        private external fun nativeVerifyResponseV1(
            responseNorito: ByteArray,
            requestNorito: ByteArray,
        ): ByteArray
    }
}

internal interface ValidationFeeHijiriQuoteCodec {
    fun encode(request: ValidationFeeHijiriQuoteRequestV1): ByteArray

    fun verify(responseNorito: ByteArray, requestNorito: ByteArray): ValidationFeeHijiriQuoteV1
}

internal object NativeValidationFeeHijiriQuoteCodec : ValidationFeeHijiriQuoteCodec {
    override fun encode(request: ValidationFeeHijiriQuoteRequestV1): ByteArray =
        ValidationFeeHijiriQuoteBridge.encodeRequestV1(request)

    override fun verify(
        responseNorito: ByteArray,
        requestNorito: ByteArray,
    ): ValidationFeeHijiriQuoteV1 =
        ValidationFeeHijiriQuoteBridge.verifyResponseV1(responseNorito, requestNorito)
}
