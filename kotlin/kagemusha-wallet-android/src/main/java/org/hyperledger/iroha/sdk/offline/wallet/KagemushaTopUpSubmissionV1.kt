package org.hyperledger.iroha.sdk.offline.wallet

import org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1
import org.hyperledger.iroha.sdk.offline.KagemushaTopUpRequestV1

/** Exact signed bytes natively authenticated against the complete original reviewed request.
 * Recreate through [prepare] when recovering persisted bytes. Session, MiBank approval, fee
 * review and persistence remain app-owned requirements before any network dispatch.
 */
class KagemushaPreparedTopUpSubmissionV1 private constructor(
    private val bytes: TopUpSubmissionBytesV1,
    val expectedRequest: KagemushaTopUpRequestV1,
) {
    fun signedTransactionBytes(): ByteArray = bytes.signedTransaction()
    fun canonicalRequestBytes(): ByteArray = bytes.canonicalRequest()
    companion object {
        @JvmStatic fun prepare(signedTransaction: ByteArray,
            expectedRequest: KagemushaTopUpRequestV1): KagemushaPreparedTopUpSubmissionV1 {
            val request = KagemushaNoritoV1.encodeTopUpRequestShape(expectedRequest)
            val bytes = TopUpSubmissionBytesV1(signedTransaction, request) { signed, expected ->
                KagemushaTopUpSubmissionNativeV1.validate(signed, expected)
            }
            return KagemushaPreparedTopUpSubmissionV1(bytes, expectedRequest)
        }
    }
}

// Only the fixed native validator is used by the public prepared-submission constructor.
// This internal byte owner supports isolated lifetime tests; their callbacks grant no authority.
internal class TopUpSubmissionBytesV1(signed: ByteArray, request: ByteArray,
    validate: (ByteArray, ByteArray) -> Unit) {
    private val signedBytes: ByteArray
    private val requestBytes: ByteArray
    init {
        require(signed.isNotEmpty() && signed.size <= 16 * 1024 * 1024)
        require(request.isNotEmpty() && request.size <= 16 * 1024)
        val signedCopy = signed.copyOf()
        val requestCopy = request.copyOf()
        validate(signedCopy.copyOf(), requestCopy.copyOf())
        signedBytes = signedCopy
        requestBytes = requestCopy
    }
    fun signedTransaction(): ByteArray = signedBytes.copyOf()
    fun canonicalRequest(): ByteArray = requestBytes.copyOf()
}

private object KagemushaTopUpSubmissionNativeV1 {
    private val available: Boolean by lazy {
        try {
            System.loadLibrary("connect_norito_bridge")
            KagemushaTopUpSubmissionJniV1.nativeBridgeAbiVersion() == 23 &&
                KagemushaTopUpSubmissionJniV1.nativeValidate(byteArrayOf(), byteArrayOf()) != 0
        } catch (_: LinkageError) { false } catch (_: RuntimeException) { false }
    }
    fun validate(signed: ByteArray, expected: ByteArray) {
        check(available) { "Native Offline signed request verifier is unavailable" }
        val result = try { KagemushaTopUpSubmissionJniV1.nativeValidate(signed, expected) }
            catch (_: LinkageError) { error("Native Offline signed request verifier is unavailable") }
        check(result == 0) { "The signed Offline transaction does not match its reviewed request" }
    }
}

internal object KagemushaTopUpSubmissionJniV1 {
    @JvmStatic external fun nativeBridgeAbiVersion(): Int
    @JvmStatic external fun nativeValidate(signedTransaction: ByteArray, expectedRequest: ByteArray): Int
}
