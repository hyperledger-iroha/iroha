package org.hyperledger.iroha.sdk.client

import java.security.KeyFactory
import java.security.Signature
import java.security.spec.X509EncodedKeySpec
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer
import org.hyperledger.iroha.sdk.address.decodePublicKeyLiteral
import org.hyperledger.iroha.sdk.crypto.IrohaHash

/** Client-side verification helper for identifier-resolution receipts. */
object IdentifierReceiptVerifier {
    private val ED25519_SPKI_PREFIX = byteArrayOf(
        0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00
    )

    @JvmStatic
    fun verify(receipt: IdentifierResolutionReceipt, policy: IdentifierPolicySummary): Boolean {
        require(receipt.policyId == policy.policyId) {
            "receipt policyId does not match the supplied policy"
        }
        val payload = receipt.signaturePayload
        require(
            receipt.policyId == payload.policyId
                && receipt.opaqueId == payload.opaqueId
                && receipt.receiptHash == payload.receiptHash
                && receipt.uaid == payload.uaid
                && receipt.accountId == payload.accountId
                && receipt.resolvedAtMs == payload.resolvedAtMs
                && receipt.expiresAtMs == payload.expiresAtMs
        ) { "receipt top-level fields do not match signaturePayload" }
        val payloadBytes = hexToBytes(receipt.signaturePayloadHex)
        val message = IrohaHash.prehash(payloadBytes)
        val signatureBytes = hexToBytes(receipt.signature)
        val keyPayload = decodePublicKeyLiteral(policy.resolverPublicKey)
            ?: throw IllegalArgumentException("resolverPublicKey is not a valid multihash literal")
        return when (keyPayload.curveId) {
            0x01 -> verifyEd25519(keyPayload.keyBytes, message, signatureBytes)
            0x0F -> throw UnsupportedOperationException(
                "SM2 receipt verification is not available in the SDK"
            )
            0x02 -> throw UnsupportedOperationException(
                "ML-DSA receipt verification is not available in the SDK"
            )
            else -> throw UnsupportedOperationException(
                "Unsupported resolver key curve id: ${keyPayload.curveId}"
            )
        }
    }

    private fun verifyEd25519(publicKey: ByteArray, message: ByteArray, signature: ByteArray): Boolean {
        try {
            val spki = ByteArray(ED25519_SPKI_PREFIX.size + publicKey.size)
            System.arraycopy(ED25519_SPKI_PREFIX, 0, spki, 0, ED25519_SPKI_PREFIX.size)
            System.arraycopy(publicKey, 0, spki, ED25519_SPKI_PREFIX.size, publicKey.size)
            val keyFactory = KeyFactory.getInstance("Ed25519")
            val key = keyFactory.generatePublic(X509EncodedKeySpec(spki))
            val verifier = Signature.getInstance("Ed25519")
            verifier.initVerify(key)
            verifier.update(message)
            return verifier.verify(signature)
        } catch (ex: Exception) {
            return verifyEd25519WithBouncyCastle(publicKey, message, signature, ex)
        }
    }

    private fun verifyEd25519WithBouncyCastle(
        publicKey: ByteArray,
        message: ByteArray,
        signature: ByteArray,
        jcaFailure: Exception
    ): Boolean {
        try {
            val verifier = Ed25519Signer()
            verifier.init(false, Ed25519PublicKeyParameters(publicKey, 0))
            verifier.update(message, 0, message.size)
            return verifier.verifySignature(signature)
        } catch (fallbackFailure: Exception) {
            fallbackFailure.addSuppressed(jcaFailure)
            throw IllegalArgumentException(
                "failed to verify Ed25519 identifier receipt", fallbackFailure
            )
        }
    }

    private fun hexToBytes(hex: String): ByteArray {
        var trimmed = hex.trim()
        if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
            trimmed = trimmed.substring(2)
        }
        require(trimmed.length % 2 == 0) { "hex value must contain an even number of characters" }
        val out = ByteArray(trimmed.length / 2)
        for (i in trimmed.indices step 2) {
            val high = Character.digit(trimmed[i], 16)
            val low = Character.digit(trimmed[i + 1], 16)
            require(high >= 0 && low >= 0) { "hex value contains non-hex characters" }
            out[i / 2] = ((high shl 4) or low).toByte()
        }
        return out
    }
}
