// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.attested

import java.math.BigInteger
import java.security.AlgorithmParameters
import java.security.GeneralSecurityException
import java.security.KeyFactory
import java.security.MessageDigest
import java.security.PublicKey
import java.security.Signature
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import java.security.spec.ECParameterSpec
import java.security.spec.ECPoint
import java.security.spec.ECPublicKeySpec
import org.hyperledger.iroha.sdk.offline.KagemushaP256Codec

/**
 * Domain separators of the attested-app suite: `iroha:kagemusha:v1:attested-app:<purpose>\0`.
 */
enum class KagemushaAttestedDomain(val purpose: String) {
    SCHEME_ID("scheme-id"),
    DESCRIPTOR("descriptor"),
    DEVICE_ID("device-id"),
    CERT("cert"),
    TRANSITION("transition"),
    REQUEST("request"),
    ACK("ack"),
    VOUCHER("voucher"),
    CRL("crl"),
    CRL_DELTA("crl-delta"),
    DELIVERY("delivery"),
    ENROLL("enroll"),
    SYNC("sync"),
    ACCOUNT_PROOF("account-proof"),
    LOAD_BINDING("load-binding"),
    REDEMPTION_ID("redemption-id"),
    ;

    private val encoded: ByteArray =
        "iroha:kagemusha:v1:attested-app:$purpose\u0000".toByteArray(Charsets.US_ASCII)

    /** Exact domain bytes, including the trailing NUL. */
    fun bytes(): ByteArray = encoded.copyOf()

    /** `SHA-256(domain || parts...)`. */
    fun hash(vararg parts: ByteArray): ByteArray = KagemushaAttestedCrypto.sha256(encoded, *parts)

    /** `domain || parts...`, the exact message signed with ECDSA P-256/SHA-256. */
    fun message(vararg parts: ByteArray): ByteArray {
        val output = java.io.ByteArrayOutputStream()
        output.write(encoded)
        parts.forEach { output.write(it) }
        return output.toByteArray()
    }
}

/**
 * A non-exportable P-256 device key.
 *
 * Production implementations live in a TEE, StrongBox or the Secure Enclave and are usable only
 * by the authentic app. There is no raw-sign or key-export API on the wallet surface.
 */
interface KagemushaDeviceSigner {
    /** True only for hardware-backed keys; software keys are admitted solely for test schemes. */
    val hardwareBacked: Boolean

    /** The 65-byte uncompressed SEC1 public key. */
    fun publicKey(): ByteArray

    /**
     * ECDSA P-256 with SHA-256 over [message], returned as strict ASN.1 DER exactly as Android
     * Keystore and JCA produce it. The SDK converts it to canonical raw low-S before persisting.
     */
    fun sign(message: ByteArray): ByteArray
}

/** P-256/SHA-256 helpers shared by every attested-app verifier. */
object KagemushaAttestedCrypto {
    const val PUBLIC_KEY_BYTES: Int = 65
    const val SIGNATURE_BYTES: Int = 64
    const val DIGEST_BYTES: Int = 32

    private val ORDER =
        BigInteger("FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551", 16)
    private val HALF_ORDER: BigInteger = ORDER.shiftRight(1)

    private val p256: ECParameterSpec by lazy {
        AlgorithmParameters.getInstance("EC").run {
            init(ECGenParameterSpec("secp256r1"))
            getParameterSpec(ECParameterSpec::class.java)
        }
    }

    /** `SHA-256(parts...)`. */
    @JvmStatic
    fun sha256(vararg parts: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").run {
        parts.forEach(::update)
        digest()
    }

    /** Validate one 65-byte uncompressed SEC1 P-256 public key and return its JCA form. */
    @JvmStatic
    fun publicKey(sec1: ByteArray): ECPublicKey {
        val value = KagemushaP256Codec.requireUncompressedPublicKey(sec1)
        val point = ECPoint(BigInteger(1, value.copyOfRange(1, 33)), BigInteger(1, value.copyOfRange(33, 65)))
        return KeyFactory.getInstance("EC").generatePublic(ECPublicKeySpec(point, p256)) as ECPublicKey
    }

    /** Encode a JCA P-256 public key as 65-byte uncompressed SEC1. */
    @JvmStatic
    fun sec1(publicKey: PublicKey): ByteArray {
        require(publicKey is ECPublicKey && publicKey.params.curve == p256.curve) {
            "KAGEMUSHA attested device keys must be P-256"
        }
        val out = ByteArray(PUBLIC_KEY_BYTES)
        out[0] = 0x04
        fixed32(publicKey.w.affineX).copyInto(out, 1)
        fixed32(publicKey.w.affineY).copyInto(out, 33)
        return KagemushaP256Codec.requireUncompressedPublicKey(out)
    }

    /** True when [signature] is a canonical 64-byte `r || s` with `0 < r, s < n` and low S. */
    @JvmStatic
    fun isCanonicalLowS(signature: ByteArray): Boolean {
        if (signature.size != SIGNATURE_BYTES) return false
        val r = BigInteger(1, signature.copyOfRange(0, 32))
        val s = BigInteger(1, signature.copyOfRange(32, 64))
        return r.signum() > 0 && r < ORDER && s.signum() > 0 && s <= HALF_ORDER
    }

    /**
     * Convert a strict DER platform signature to canonical raw low-S `r || s`.
     * High-S values are folded to `n - s`, as signers must; verifiers never fold.
     */
    @JvmStatic
    fun normalizeDerSignature(der: ByteArray): ByteArray = KagemushaP256Codec.rawLowSFromStrictDer(der)

    /**
     * Verify ECDSA P-256/SHA-256 over [message]. High-S, malformed keys and malformed signatures
     * return false; nothing is normalized on the verifying side.
     */
    @JvmStatic
    fun verify(publicKeySec1: ByteArray, message: ByteArray, signature: ByteArray): Boolean {
        if (!isCanonicalLowS(signature)) return false
        return try {
            val verifier = Signature.getInstance("SHA256withECDSA")
            verifier.initVerify(publicKey(publicKeySec1))
            verifier.update(message)
            verifier.verify(KagemushaP256Codec.strictDerFromRawLowS(signature))
        } catch (failure: GeneralSecurityException) {
            false
        } catch (failure: IllegalArgumentException) {
            false
        }
    }

    /** Sign with [signer], normalize to low-S and confirm the result verifies under its key. */
    @JvmStatic
    fun signAndCheck(signer: KagemushaDeviceSigner, publicKey: ByteArray, message: ByteArray): ByteArray {
        val signature = normalizeDerSignature(signer.sign(message))
        check(verify(publicKey, message, signature)) {
            "KAGEMUSHA device key produced a signature that does not verify under the enrolled key"
        }
        return signature
    }

    private fun fixed32(value: BigInteger): ByteArray {
        val signed = value.toByteArray()
        val unsigned = if (signed.size > 32) signed.copyOfRange(signed.size - 32, signed.size) else signed
        require(unsigned.size <= 32 && (signed.size <= 32 || signed.copyOfRange(0, signed.size - 32).all { it.toInt() == 0 })) {
            "P-256 scalar exceeds 32 bytes"
        }
        return ByteArray(32).also { unsigned.copyInto(it, 32 - unsigned.size) }
    }
}
