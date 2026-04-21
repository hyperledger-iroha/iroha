package org.hyperledger.iroha.sdk.client

import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer
import org.hyperledger.iroha.sdk.address.decodePublicKeyLiteral
import org.hyperledger.iroha.sdk.crypto.IrohaHash

/** Client-side verification helper for identifier-resolution receipts. */
object IdentifierReceiptVerifier {
    @JvmStatic
    fun verify(receipt: IdentifierResolutionReceipt, policy: IdentifierPolicySummary): Boolean {
        require(receipt.policyId == policy.policyId) {
            "receipt policyId does not match the supplied policy"
        }
        require(receipt.attestation.kind == "signed") {
            "only signed identifier receipt attestations can be verified with a resolver public key"
        }
        val payloadBytes = IdentifierReceiptCanonicalEncoder.encodePayload(receipt.payload)
        val message = IrohaHash.prehash(payloadBytes)
        val signatureBytes = hexToBytes(
            requireNotNull(receipt.attestation.signature) {
                "signed attestation is missing signature"
            }
        )
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
            val verifier = Ed25519Signer()
            verifier.init(false, Ed25519PublicKeyParameters(publicKey, 0))
            verifier.update(message, 0, message.size)
            return verifier.verifySignature(signature)
        } catch (ex: Exception) {
            throw IllegalArgumentException(
                "failed to verify Ed25519 identifier receipt", ex
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
