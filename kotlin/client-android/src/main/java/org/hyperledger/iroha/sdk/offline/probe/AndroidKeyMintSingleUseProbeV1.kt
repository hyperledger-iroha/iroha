// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.MessageDigest
import java.security.PublicKey
import java.security.SecureRandom
import java.security.Signature
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import java.security.spec.ECFieldFp
import java.math.BigInteger

/**
 * Non-monetary diagnostic only. It never implements a wallet provider or native lifecycle bridge,
 * and an observed second-sign failure is not hardware qualification.
 */
internal object AndroidKeyMintSingleUseProbeV1 {
    fun run(context: Context): SingleUseProbeResultV1 =
        SingleUseProbeRunnerV1(AndroidSingleUseProbeDeviceV1(context)).run()
}

internal sealed interface SingleUseProbeResultV1 {
    data class Unavailable(val reason: String) : SingleUseProbeResultV1
    data class Failed(val stage: String, val exceptionClass: String, val cleanupExceptionClass: String?) :
        SingleUseProbeResultV1
    data class Attempts(
        val challenge: ByteArray,
        val publicKey: ByteArray,
        val certificateChain: List<ByteArray>,
        val first: SignAttemptV1,
        val second: SignAttemptV1,
        val cleanupExceptionClass: String?,
    ) : SingleUseProbeResultV1
}

internal sealed interface SignAttemptV1 {
    data class Signed(val signature: ByteArray) : SignAttemptV1
    data class Failed(val exceptionClass: String) : SignAttemptV1
}

internal data class ProbeKeyMaterialV1(
    val publicKey: ByteArray,
    val certificateChain: List<ByteArray>,
)

/** Mockable raw one-use key operations; no method grants monetary authority. */
internal interface SingleUseProbeDeviceV1 {
    val apiLevel: Int
    fun hasHardwareSingleUseFeature(): Boolean
    fun newChallenge(): ByteArray
    fun hasAlias(alias: String): Boolean
    fun generate(alias: String, challenge: ByteArray): ProbeKeyMaterialV1
    fun read(alias: String): ProbeKeyMaterialV1
    fun sign(alias: String, message: ByteArray): ByteArray
    fun delete(alias: String)
}

internal class SingleUseProbeRunnerV1(private val device: SingleUseProbeDeviceV1) {
    fun run(): SingleUseProbeResultV1 {
        if (device.apiLevel < 31) {
            return SingleUseProbeResultV1.Unavailable("Android API 31 is required")
        }
        val hardwareFeature = try {
            device.hasHardwareSingleUseFeature()
        } catch (error: Exception) {
            return SingleUseProbeResultV1.Failed(
                "feature", error.javaClass.name, null,
            )
        }
        if (!hardwareFeature) {
            return SingleUseProbeResultV1.Unavailable("hardware single-use feature absent")
        }
        val challenge = try {
            device.newChallenge().copyOf().also {
                require(it.size == 32 && it.any { byte -> byte != 0.toByte() }) {
                    "probe challenge must be a nonzero 32-byte nonce"
                }
            }
        } catch (error: Exception) {
            return SingleUseProbeResultV1.Failed(
                "challenge", error.javaClass.name, null,
            )
        }
        val alias = "iroha_keymint_probe_" + challenge.joinToString("") {
            "%02x".format(it.toInt() and 0xff)
        }
        var stage = "generate"
        var material: ProbeKeyMaterialV1? = null
        var first: SignAttemptV1? = null
        var second: SignAttemptV1? = null
        var failureClass: String? = null
        try {
            material = device.generate(alias, challenge).also {
                require(it.publicKey.isNotEmpty() && it.certificateChain.isNotEmpty()) {
                    "probe requires a public key and raw attestation chain"
                }
                require(it.certificateChain.all(ByteArray::isNotEmpty)) {
                    "probe attestation chain contains an empty certificate"
                }
            }
            stage = "sign"
            first = attempt(alias, message("first", challenge))
            second = attempt(alias, message("second", challenge))
        } catch (error: Exception) {
            failureClass = error.javaClass.name
        }
        val cleanupFailure = try {
            device.delete(alias)
            null
        } catch (error: Exception) {
            error.javaClass.name
        }
        if (failureClass != null || material == null || first == null || second == null) {
            return SingleUseProbeResultV1.Failed(
                stage, failureClass ?: IllegalStateException::class.java.name, cleanupFailure,
            )
        }
        return SingleUseProbeResultV1.Attempts(
            challenge = challenge,
            publicKey = material.publicKey.copyOf(),
            certificateChain = material.certificateChain.map(ByteArray::copyOf),
            first = first,
            second = second,
            cleanupExceptionClass = cleanupFailure,
        )
    }

    private fun attempt(alias: String, message: ByteArray): SignAttemptV1 =
        try {
            val signature = device.sign(alias, message)
            if (signature.isEmpty()) {
                SignAttemptV1.Failed("empty signature")
            } else {
                SignAttemptV1.Signed(signature.copyOf())
            }
        } catch (error: Exception) {
            SignAttemptV1.Failed(error.javaClass.name)
        }

    private fun message(label: String, challenge: ByteArray): ByteArray =
        MessageDigest.getInstance("SHA-256").digest(
            "iroha:kagemusha:keymint-one-use-probe:v1:$label\u0000".toByteArray(Charsets.UTF_8) +
                challenge,
        )
}

internal class AndroidSingleUseProbeDeviceV1(private val context: Context) : SingleUseProbeDeviceV1 {
    override val apiLevel: Int get() = Build.VERSION.SDK_INT

    override fun hasHardwareSingleUseFeature(): Boolean =
        context.packageManager.hasSystemFeature(PackageManager.FEATURE_KEYSTORE_SINGLE_USE_KEY) ||
            context.packageManager.hasSystemFeature(PackageManager.FEATURE_KEYSTORE_LIMITED_USE_KEY)

    override fun newChallenge(): ByteArray = ByteArray(32).also(SecureRandom()::nextBytes)

    override fun hasAlias(alias: String): Boolean =
        KeyStore.getInstance("AndroidKeyStore").apply { load(null) }.containsAlias(alias)

    override fun generate(alias: String, challenge: ByteArray): ProbeKeyMaterialV1 {
        check(Build.VERSION.SDK_INT >= Build.VERSION_CODES.S)
        check(!hasAlias(alias)) { "one-use alias already exists" }
        val spec = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_SIGN)
            .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
            .setDigests(KeyProperties.DIGEST_SHA256)
            .setAttestationChallenge(challenge.copyOf())
            .setMaxUsageCount(1)
            .build()
        val generator = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, "AndroidKeyStore")
        generator.initialize(spec)
        val pair = generator.generateKeyPair()
        val keyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val chain = keyStore.getCertificateChain(alias)?.map { it.encoded }
            ?: throw IllegalStateException("no attestation chain for one-use key")
        return ProbeKeyMaterialV1(uncompressedP256Sec1V1(pair.public), chain)
    }

    override fun read(alias: String): ProbeKeyMaterialV1 {
        val keyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val certificate = keyStore.getCertificate(alias)
            ?: throw IllegalStateException("one-use key unavailable")
        val chain = keyStore.getCertificateChain(alias)?.map { it.encoded }
            ?: throw IllegalStateException("one-use attestation chain unavailable")
        return ProbeKeyMaterialV1(uncompressedP256Sec1V1(certificate.publicKey), chain)
    }

    override fun sign(alias: String, message: ByteArray): ByteArray {
        val keyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val entry = keyStore.getEntry(alias, null) as? KeyStore.PrivateKeyEntry
            ?: throw IllegalStateException("one-use key unavailable")
        return Signature.getInstance("SHA256withECDSA").run {
            initSign(entry.privateKey)
            update(message)
            sign()
        }
    }

    override fun delete(alias: String) {
        val keyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        if (keyStore.containsAlias(alias)) keyStore.deleteEntry(alias)
    }

}

/** Core's first-release device key encoding, independent of Java's X.509 SPKI wrapper. */
internal fun uncompressedP256Sec1V1(publicKey: PublicKey): ByteArray {
    val point = publicKey as? ECPublicKey
        ?: throw IllegalStateException("one-use public key is not EC")
    val field = point.params.curve.field as? ECFieldFp
    require(field?.p == P256_FIELD_V1 &&
        point.params.curve.a == P256_FIELD_V1.subtract(BigInteger.valueOf(3)) &&
        point.params.curve.b == P256_B_V1 &&
        point.params.generator.affineX == P256_GENERATOR_X_V1 &&
        point.params.generator.affineY == P256_GENERATOR_Y_V1 &&
        point.params.order == P256_ORDER_V1 &&
        point.params.cofactor == 1) {
        "one-use public key is not secp256r1"
    }
    val x = point.w.affineX
    val y = point.w.affineY
    require(x.signum() >= 0 && x < P256_FIELD_V1 && y.signum() >= 0 && y < P256_FIELD_V1 &&
        y.modPow(BigInteger.valueOf(2), P256_FIELD_V1) ==
            x.modPow(BigInteger.valueOf(3), P256_FIELD_V1)
                .subtract(x.multiply(BigInteger.valueOf(3))).add(P256_B_V1).mod(P256_FIELD_V1)) {
        "one-use public key is not on secp256r1"
    }
    return byteArrayOf(0x04) + unsignedP256CoordinateV1(x.toByteArray()) +
        unsignedP256CoordinateV1(y.toByteArray())
}

private val P256_FIELD_V1 = BigInteger(
    "FFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF", 16,
)
private val P256_B_V1 = BigInteger(
    "5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B", 16,
)
private val P256_GENERATOR_X_V1 = BigInteger(
    "6B17D1F2E12C4247F8BCE6E563A440F277037D812DEB33A0F4A13945D898C296", 16,
)
private val P256_GENERATOR_Y_V1 = BigInteger(
    "4FE342E2FE1A7F9B8EE7EB4A7C0F9E162BCE33576B315ECECBB6406837BF51F5", 16,
)
private val P256_ORDER_V1 = BigInteger(
    "FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551", 16,
)

private fun unsignedP256CoordinateV1(encoded: ByteArray): ByteArray {
    val unsigned = if (encoded.size == 33 && encoded[0] == 0.toByte()) {
        encoded.copyOfRange(1, encoded.size)
    } else {
        encoded
    }
    require(unsigned.size in 1..32) { "one-use public key coordinate is not P-256" }
    return ByteArray(32 - unsigned.size) + unsigned
}
