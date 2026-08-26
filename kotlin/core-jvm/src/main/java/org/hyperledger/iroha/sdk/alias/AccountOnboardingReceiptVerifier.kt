package org.hyperledger.iroha.sdk.alias

import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.crypto.Ed25519PublicKeyAdmission
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.crypto.NativeSignerBridge
import org.hyperledger.iroha.sdk.crypto.SigningAlgorithm
import org.hyperledger.iroha.sdk.core.model.NetworkId

/** Canonical hash and onboarding-authority signature verification for stateless receipts. */
object AccountOnboardingReceiptVerifier {
    private val HASH_DOMAIN = "iroha:account-onboarding-plan-receipt:v1\u0000"
        .toByteArray(StandardCharsets.UTF_8)

    /** Computes the exact domain-separated hash signed by the onboarding authority. */
    @JvmStatic
    fun canonicalHash(body: AccountOnboardingPlanBodyV1): ByteArray =
        IrohaHash.prehash(HASH_DOMAIN + AliasNoritoCodec.encodeOnboardingPlanBody(body))

    /** Verifies the receipt against the exact local network and onboarding authority. */
    @JvmStatic
    fun verify(
        receipt: AccountOnboardingPlanReceiptV1,
        expectedNetworkId: NetworkId,
        expectedAuthority: String,
    ): Boolean {
        if (receipt.body.networkId != expectedNetworkId) return false
        val canonicalExpected = try {
            requireCanonicalI105Address(expectedAuthority, "expectedAuthority")
        } catch (_: IllegalArgumentException) {
            return false
        }
        if (!sameAccountIdentity(receipt.body.authority, canonicalExpected)) return false
        val carriedHash = AliasHashText.decode(receipt.planHash) ?: return false
        if (!MessageDigest.isEqual(carriedHash, canonicalHash(receipt.body))) return false
        val signature = decodeHex(receipt.signature) ?: return false
        return verifyAuthoritySignature(receipt.body.authority, carriedHash, signature)
    }

    /** Verifies an exact digest/message against the single-key authority encoded by an I105 account. */
    @JvmStatic
    fun verifyAuthoritySignature(authority: String, message: ByteArray, signatureHex: String): Boolean {
        val signature = decodeHex(signatureHex) ?: return false
        return verifyAuthoritySignature(authority, message, signature)
    }

    /** Compares universal account identities independently of the I105 display discriminant. */
    @JvmStatic
    fun sameAccountIdentity(left: String, right: String): Boolean = try {
        val leftBytes = AccountAddress.fromI105(
            requireCanonicalI105Address(left, "left"),
            null,
        ).canonicalBytes
        val rightBytes = AccountAddress.fromI105(
            requireCanonicalI105Address(right, "right"),
            null,
        ).canonicalBytes
        MessageDigest.isEqual(leftBytes, rightBytes)
    } catch (_: Exception) {
        false
    }

    internal fun verifyAuthoritySignature(authority: String, message: ByteArray, signature: ByteArray): Boolean =
        try {
            val address = AccountAddress.fromI105(authority, null)
            val signatory = address.singleKeyPayloadIgnoringCurveSupport() ?: return false
            when (signatory.curveId) {
                0x01 -> verifyEd25519(signatory.publicKey, message, signature)
                else -> verifyNative(signatory.curveId, signatory.publicKey, message, signature)
            }
        } catch (_: Exception) {
            false
        }

    /** Requires a valid receipt for the exact local network and authority. */
    @JvmStatic
    fun requireValid(
        receipt: AccountOnboardingPlanReceiptV1,
        expectedNetworkId: NetworkId,
        expectedAuthority: String,
    ): AccountOnboardingPlanReceiptV1 {
        require(verify(receipt, expectedNetworkId, expectedAuthority)) {
            "account onboarding receipt network, hash, or authority signature is invalid"
        }
        return receipt
    }

    /** Binds a network-pinned receipt to the exact canonical request sent by the caller. */
    @JvmStatic
    fun requireValidForRequest(
        request: AccountOnboardingPlanRequestV1,
        receipt: AccountOnboardingPlanReceiptV1,
        expectedNetworkId: NetworkId,
        expectedAuthority: String,
    ): AccountOnboardingPlanReceiptV1 {
        require(receipt.body.request == request) {
            "account onboarding receipt does not match the exact normalized request"
        }
        return requireValid(receipt, expectedNetworkId, expectedAuthority)
    }

    private fun verifyEd25519(publicKey: ByteArray, message: ByteArray, signature: ByteArray): Boolean =
        if (!Ed25519PublicKeyAdmission.isValid(publicKey)) {
            false
        } else {
            try {
                val verifier = Ed25519Signer()
                verifier.init(false, Ed25519PublicKeyParameters(publicKey, 0))
                verifier.update(message, 0, message.size)
                verifier.verifySignature(signature)
            } catch (_: RuntimeException) {
                false
            }
        }

    private fun verifyNative(
        curveId: Int,
        publicKey: ByteArray,
        message: ByteArray,
        signature: ByteArray,
    ): Boolean {
        val algorithm = signingAlgorithm(curveId) ?: return false
        if (!NativeSignerBridge.isNativeAvailable()) return false
        return try {
            NativeSignerBridge.verifyDetached(algorithm, publicKey, message, signature)
        } catch (_: RuntimeException) {
            false
        }
    }

    private fun signingAlgorithm(curveId: Int): SigningAlgorithm? = when (curveId) {
        0x02 -> SigningAlgorithm.ML_DSA
        0x03 -> SigningAlgorithm.BLS_NORMAL
        0x04 -> SigningAlgorithm.SECP256K1
        0x05 -> SigningAlgorithm.BLS_SMALL
        0x0A -> SigningAlgorithm.GOST_2012_256_A
        0x0B -> SigningAlgorithm.GOST_2012_256_B
        0x0C -> SigningAlgorithm.GOST_2012_256_C
        0x0D -> SigningAlgorithm.GOST_2012_512_A
        0x0E -> SigningAlgorithm.GOST_2012_512_B
        0x0F -> SigningAlgorithm.SM2
        else -> null
    }

    private fun decodeHex(value: String): ByteArray? {
        if (value.isEmpty() || value.length % 2 != 0) return null
        val result = ByteArray(value.length / 2)
        for (index in result.indices) {
            val high = Character.digit(value[index * 2], 16)
            val low = Character.digit(value[index * 2 + 1], 16)
            if (high < 0 || low < 0) return null
            result[index] = ((high shl 4) or low).toByte()
        }
        return result
    }
}
