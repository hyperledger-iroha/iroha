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
import java.security.MessageDigest
import java.security.cert.X509Certificate
import java.security.spec.ECGenParameterSpec
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AndroidKeyAttestationOriginalV1

/** Local generation preference. The independent issuer still enforces the original enrolled policy. */
enum class KagemushaAndroidAppKeyHardwarePolicyV1 { TEE_OR_STRONGBOX, STRONGBOX_ONLY }

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
        val strongBox = context.packageManager.hasSystemFeature(PackageManager.FEATURE_STRONGBOX_KEYSTORE)
        check(strongBox || policy == KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX) { "Required StrongBox is unavailable" }
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
    private fun requireAvailable() { check(Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) { "Hardware app-key metadata is unavailable" } }
    private fun keyStore() = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
    private fun loadExact(store: KeyStore, alias: String, challenge: ByteArray,
        policy: KagemushaAndroidAppKeyHardwarePolicyV1): KagemushaAndroidHardwareAppKeyEvidenceV1 {
        require(alias.isNotBlank() && alias.length <= 128 && challenge.size == 32)
        val key = checkNotNull(store.getKey(alias, null)) { "Original app key disappeared" }
        val info = KeyFactory.getInstance("EC", "AndroidKeyStore").getKeySpec(key, KeyInfo::class.java)
        requirePersistentHardwareAppKeyV1(info.securityLevel, info.origin, info.purposes, info.digests.toSet(),
            info.remainingUsageCount, key.encoded != null, policy)
        val chain = checkNotNull(store.getCertificateChain(alias)).map { it as X509Certificate }
        check(MessageDigest.isEqual(challenge, AndroidKeyAttestationOriginalV1.challenge(chain))) { "Original app-key challenge differs" }
        val point = AndroidKeyAttestationOriginalV1.publicKeySec1(chain)
        val der = chain.map(X509Certificate::getEncoded)
        check(der.size in 2..8 && der.all { it.isNotEmpty() && it.size <= 16 * 1024 })
        return KagemushaAndroidHardwareAppKeyEvidenceV1(info.securityLevel, MessageDigest.getInstance("SHA-256").digest(point), point, der)
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
    check((level == KeyProperties.SECURITY_LEVEL_STRONGBOX ||
        (policy == KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX && level == KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT)) &&
        origin == KeyProperties.ORIGIN_GENERATED && purposes == KeyProperties.PURPOSE_SIGN &&
        digests == setOf(KeyProperties.DIGEST_SHA256) && remaining == KeyProperties.UNRESTRICTED_USAGE_COUNT && !exportable) {
        "The original persistent hardware app key differs or is unavailable"
    }
}
