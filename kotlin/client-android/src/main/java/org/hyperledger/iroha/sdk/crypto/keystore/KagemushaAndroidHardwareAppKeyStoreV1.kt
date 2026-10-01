package org.hyperledger.iroha.sdk.crypto.keystore

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyInfo
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import java.security.KeyFactory
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.PrivateKey
import java.security.Signature
import java.security.interfaces.ECPublicKey
import org.hyperledger.iroha.sdk.offline.KagemushaNativePreparedOrdinaryAppIdentityV1
import org.hyperledger.iroha.sdk.offline.KagemushaNativeRawAppIdentityAdmissionV1
import org.hyperledger.iroha.sdk.offline.KagemushaNativePreparedAppApprovalV1
import org.hyperledger.iroha.sdk.offline.KagemushaNativePreparedAppEnrollmentPossessionV1
import java.security.MessageDigest
import java.security.cert.X509Certificate
import java.security.spec.ECGenParameterSpec
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AndroidKeyAttestationOriginalV1

/** Local generation preference. The independent issuer still enforces the original enrolled policy. */
enum class KagemushaAndroidAppKeyHardwarePolicyV1 { TEE_OR_STRONGBOX, STRONGBOX_ONLY, TEE_ONLY }

/** Original platform evidence, without app-integrity, StateGuard or offline counter qualification. */
class KagemushaAndroidHardwareAppKeyEvidenceV1 internal constructor(val securityLevel: Int,
    keyId: ByteArray, publicKey: ByteArray, chain: List<ByteArray>) {
    private val id = keyId.copyOf(); private val point = publicKey.copyOf(); private val certificates = chain.map(ByteArray::copyOf)
    fun attestedKeyId(): ByteArray = id.copyOf()
    fun publicKeySec1(): ByteArray = point.copyOf()
    fun certificateChainDer(): List<ByteArray> = certificates.map(ByteArray::copyOf)
}

/** Shared ordinary app-key adapter. No custom applet or finite-use key is required to enroll.
 * StrongBox is preferred when available; only an independently permitted TEE policy may use TEE.
 * The original alias must be recovered after uncertainty, never replaced by a new identity.
 */
class KagemushaAndroidHardwareAppKeyStoreV1(context: Context) {
    private val context = context.applicationContext
    fun issueExact(alias: String, challenge: ByteArray, policy: KagemushaAndroidAppKeyHardwarePolicyV1,
        requireCurrent: () -> Unit): KagemushaAndroidHardwareAppKeyEvidenceV1 = synchronized(lock) {
        requireCurrent(); requireAvailable()
        val digest = challenge.copyOf(); require(alias.isNotBlank() && alias.length <= 128 && digest.size == 32)
        val store = keyStore(); check(!store.containsAlias(alias)) { "Original app key already exists; recover it" }
        val strongBox = policy != KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY &&
            context.packageManager.hasSystemFeature(PackageManager.FEATURE_STRONGBOX_KEYSTORE)
        check(strongBox || policy != KagemushaAndroidAppKeyHardwarePolicyV1.STRONGBOX_ONLY) { "Required StrongBox is unavailable" }
        fun generate(strong: Boolean) {
            requireCurrent(); check(!store.containsAlias(alias))
            KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, "AndroidKeyStore").apply {
                initialize(persistentHardwareAppKeyParametersV1(alias, digest, strong))
            }.generateKeyPair()
        }
        try { generate(strongBox) }
        catch (unavailable: StrongBoxUnavailableException) {
            // A definite StrongBox-unavailable result may use the policy-approved TEE under the
            // same original alias. Any retained key or another uncertain error stops instead.
            requireCurrent()
            if (!strongBox || policy != KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX || store.containsAlias(alias)) throw unavailable
            generate(false)
        }
        requireCurrent(); loadExact(store, alias, digest, policy).also { requireCurrent() }
    }
    fun recoverExact(alias: String, challenge: ByteArray, policy: KagemushaAndroidAppKeyHardwarePolicyV1,
        requireCurrent: () -> Unit): KagemushaAndroidHardwareAppKeyEvidenceV1? = synchronized(lock) {
        requireCurrent(); requireAvailable(); val store = keyStore()
        if (!store.containsAlias(alias)) return@synchronized null
        loadExact(store, alias, challenge.copyOf(), policy).also { requireCurrent() }
    }
    /** Internal native-original signer. No key is generated, replaced or exported by this path. */
    internal fun originalSignatureDevice(): KagemushaAndroidAppSignatureDeviceV1 = object : KagemushaAndroidAppSignatureDeviceV1 {
        override fun signOriginal(original: KagemushaAndroidHeldAppSignatureV1): ByteArray = synchronized(lock) {
            original.requireOriginal(); requireAvailable()
            val store = keyStore(); requireOriginalKey(store, original)
            val key = checkNotNull(store.getKey(original.alias, null)) as PrivateKey
            original.requireOriginal()
            Signature.getInstance("SHA256withECDSA").run {
                initSign(key); update(original.signingBytes()); sign()
            }.also { original.requireOriginal() }
        }
        override fun verifyOriginal(original: KagemushaAndroidHeldAppSignatureV1, signatureDer: ByteArray) = synchronized(lock) {
            original.requireOriginal(); requireAvailable(); requireCanonicalP256SignatureDerV1(signatureDer)
            val store = keyStore(); requireOriginalKey(store, original)
            val publicKey = checkNotNull(store.getCertificate(original.alias)).publicKey
            check(Signature.getInstance("SHA256withECDSA").run {
                initVerify(publicKey); update(original.signingBytes()); verify(signatureDer)
            }) { "Retained app signature does not belong to the original key and bytes" }
            original.requireOriginal()
        }
    }
    private fun requireOriginalKey(store: KeyStore, original: KagemushaAndroidHeldAppSignatureV1) {
        val evidence = loadExact(store, original.alias, original.attestationChallenge(), original.hardwarePolicy)
        check(MessageDigest.isEqual(evidence.attestedKeyId(), original.attestedKeyId())) { "Original app approval key changed" }
        original.requireOriginal()
    }
    /** Generate/recover only the native-held ordinary C identity; the native issuer admits its raw chain. */
    fun enroll(prepared: KagemushaNativePreparedOrdinaryAppIdentityV1): KagemushaNativeRawAppIdentityAdmissionV1 =
        prepared.performAndroidEnrollment { alias, challenge, policy, recoverOnly, guard ->
            if (recoverOnly) checkNotNull(recoverExact(alias, challenge, policy, guard)) {
                "Original app key is missing; an uncertain native identity cannot generate a replacement"
            }
            else issueExact(alias, challenge, policy, guard)
        }

    /** Sign the exact native-owned W once; returned bytes are the original non-monetary native receipt. */
    fun approve(prepared: KagemushaNativePreparedAppApprovalV1): ByteArray =
        prepared.performPlatformSigning { alias, generationChallenge, point, keyId, message, policy, guard ->
            signNativeOriginal(alias, generationChallenge, point, keyId, message, policy,
                KagemushaAndroidAppSignaturePurposeV1.OPERATION_APPROVAL, guard)
        }.copyOf()

    /** Prove possession over the separate native-owned E without creating a monetary qualification. */
    fun proveEnrollmentPossession(prepared: KagemushaNativePreparedAppEnrollmentPossessionV1): ByteArray =
        prepared.performPlatformSigning { alias, generationChallenge, point, keyId, message, policy, guard ->
            signNativeOriginal(alias, generationChallenge, point, keyId, message, policy,
                KagemushaAndroidAppSignaturePurposeV1.IDENTITY_ENROLLMENT_POSSESSION, guard)
        }.copyOf()

    // Only the native opaque prepared capability can enter this method. The native owner
    // durably fences invocation and retains original DER before consuming the approval.
    // Recovery never invokes this callback again for a started or completed attempt.
    private fun signNativeOriginal(alias: String, generationChallenge: ByteArray,
        expectedPoint: ByteArray, expectedKeyId: ByteArray, message: ByteArray,
        policy: KagemushaAndroidAppKeyHardwarePolicyV1, purpose: KagemushaAndroidAppSignaturePurposeV1,
        requireCurrent: () -> Unit): ByteArray = synchronized(lock) {
        requireCurrent(); requireAvailable()
        requireAppPlatformSigningMessageV1(message, purpose)
        val store = keyStore()
        check(store.containsAlias(alias)) { "The enrolled app key is missing; native recovery is required" }
        val original = loadExact(store, alias, generationChallenge.copyOf(), policy)
        requireOriginalAppKeyBindingV1(original.publicKeySec1(), original.attestedKeyId(), expectedPoint, expectedKeyId)
        requireCurrent()
        val key = store.getKey(alias, null) as? PrivateKey
            ?: error("Original app signing key is unavailable")
        check(key.encoded == null) { "Original app key must remain nonexportable" }
        val certificate = store.getCertificate(alias) as? X509Certificate
            ?: error("Original app-key certificate is unavailable")
        val public = certificate.publicKey as? ECPublicKey
            ?: error("Original app-key certificate is not P-256")
        // Re-read the original alias after loading its private key and immediately before
        // the actual platform call; neither an endpoint projection nor an app DTO selects it.
        val rechecked = loadExact(store, alias, generationChallenge, policy)
        requireOriginalAppKeyBindingV1(rechecked.publicKeySec1(), rechecked.attestedKeyId(), expectedPoint, expectedKeyId)
        requireCurrent()
        val signature = signOriginalAndroidAppMessageV1(key, public, message, purpose, requireCurrent)
        try {
            val retained = loadExact(store, alias, generationChallenge, policy)
            requireOriginalAppKeyBindingV1(retained.publicKeySec1(), retained.attestedKeyId(), expectedPoint, expectedKeyId)
            requireCurrent()
            signature
        } catch (failure: Throwable) {
            signature.fill(0)
            throw failure
        }
    }

    private fun requireAvailable() { check(Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) { "Hardware app-key API28 support is unavailable" } }
    private fun keyStore() = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
    private fun loadExact(store: KeyStore, alias: String, challenge: ByteArray,
        policy: KagemushaAndroidAppKeyHardwarePolicyV1): KagemushaAndroidHardwareAppKeyEvidenceV1 {
        require(alias.isNotBlank() && alias.length <= 128 && challenge.size == 32)
        val key = checkNotNull(store.getKey(alias, null)) { "Original app key disappeared" }
        val info = KeyFactory.getInstance("EC", "AndroidKeyStore").getKeySpec(key, KeyInfo::class.java)
        val chain = checkNotNull(store.getCertificateChain(alias)).map { it as X509Certificate }
        val der = chain.map(X509Certificate::getEncoded)
        check(der.size in 2..8 && der.all { it.isNotEmpty() && it.size <= 16 * 1024 })
        val attested = AndroidKeyAttestationOriginalV1.persistentAppHardwareSecurityLevel(chain, challenge)
        val modern = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
        val reportedLevel = if (modern) info.securityLevel else null
        val remaining = if (modern) info.remainingUsageCount else null
        val level = requirePersistentAppHardwareMetadataV1(Build.VERSION.SDK_INT, info.isInsideSecureHardware,
            attested.encodedValue, reportedLevel, remaining)
        if (modern) {
            requirePersistentHardwareAppKeyV1(level, info.origin, info.purposes, info.digests.toSet(),
                checkNotNull(remaining), key.encoded != null, policy)
        } else {
            // The original signed extension rejected finite tag405 above. API28–30 provides no
            // remainingUsageCount readback; do not substitute a handset/software count for it.
            requirePersistentHardwareAppKeyWithoutUsageMetadataV1(level, info.origin, info.purposes,
                info.digests.toSet(), key.encoded != null, policy)
        }
        val point = AndroidKeyAttestationOriginalV1.publicKeySec1(chain)
        return KagemushaAndroidHardwareAppKeyEvidenceV1(level, MessageDigest.getInstance("SHA-256").digest(point), point, der)
    }
    companion object {
        private val lock = Any()
        @JvmStatic fun originalAlias(account: String, clientNonceHex: String, signedPreparation: ByteArray): String {
            require(account.isNotBlank() && account.toByteArray(Charsets.UTF_8).size <= 512 &&
                clientNonceHex.matches(Regex("[0-9a-f]{64}")) && clientNonceHex.any { it != '0' } && signedPreparation.size == 273)
            val bytes = account.toByteArray(Charsets.UTF_8) + byteArrayOf(0) + clientNonceHex.toByteArray(Charsets.US_ASCII) + signedPreparation.copyOf()
            return "kagemusha-app-enrollment-v1-" + MessageDigest.getInstance("SHA-256").digest(bytes)
                .joinToString("") { "%02x".format(it.toInt() and 0xff) }
        }
    }
}

internal fun persistentHardwareAppKeyParametersV1(alias: String, challenge: ByteArray, strongBox: Boolean): KeyGenParameterSpec {
    require(alias.isNotBlank() && challenge.size == 32)
    return KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_SIGN).setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
        .setDigests(KeyProperties.DIGEST_SHA256).setIsStrongBoxBacked(strongBox).setAttestationChallenge(challenge.copyOf()).build()
}
internal fun requirePersistentHardwareAppKeyV1(level: Int, origin: Int, purposes: Int, digests: Set<String>,
    remaining: Int, exportable: Boolean, policy: KagemushaAndroidAppKeyHardwarePolicyV1) {
    check(remaining == KeyProperties.UNRESTRICTED_USAGE_COUNT) { "Original persistent app-key usage metadata differs" }
    requirePersistentHardwareAppKeyWithoutUsageMetadataV1(level, origin, purposes, digests, exportable, policy)
}

/** API28–30 checks actual custody/parameters after exact signed-extension finite-tag rejection. */
internal fun requirePersistentHardwareAppKeyWithoutUsageMetadataV1(level: Int, origin: Int, purposes: Int,
    digests: Set<String>, exportable: Boolean, policy: KagemushaAndroidAppKeyHardwarePolicyV1) {
    val admittedLevel = when (policy) {
        KagemushaAndroidAppKeyHardwarePolicyV1.STRONGBOX_ONLY -> level == KeyProperties.SECURITY_LEVEL_STRONGBOX
        KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY -> level == KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT
        KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX -> level == KeyProperties.SECURITY_LEVEL_STRONGBOX ||
            level == KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT
    }
    check(admittedLevel && origin == KeyProperties.ORIGIN_GENERATED && purposes == KeyProperties.PURPOSE_SIGN &&
        digests == setOf(KeyProperties.DIGEST_SHA256) && !exportable) {
        "The original persistent hardware app key differs or is unavailable"
    }
}

/** Legacy custody uses actual KeyInfo hardware residency plus the exact signed extension level.
 * Null modern-only fields represent an unavailable API, never a software usage-count substitute.
 */
internal fun requirePersistentAppHardwareMetadataV1(api: Int, insideSecureHardware: Boolean,
    attestedLevel: Int, reportedLevel: Int?, remaining: Int?): Int {
    check(api >= 28 && insideSecureHardware && attestedLevel in setOf(
        KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT, KeyProperties.SECURITY_LEVEL_STRONGBOX)) {
        "Persistent app key lacks actual hardware custody or an exact attested hardware level"
    }
    if (api >= 31) {
        check(reportedLevel == attestedLevel && remaining == KeyProperties.UNRESTRICTED_USAGE_COUNT) {
            "Original modern hardware metadata differs from the signed persistent key"
        }
    } else {
        check(reportedLevel == null && remaining == null) { "Legacy hardware metadata was invented" }
    }
    return attestedLevel
}
