package org.hyperledger.iroha.sdk.crypto.keystore

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.security.keystore.KeyProperties
import java.security.PrivateKey
import java.security.Signature
import java.security.interfaces.ECPublicKey
import org.hyperledger.iroha.sdk.offline.KagemushaNativePreparedOrdinaryAppIdentityV1
import org.hyperledger.iroha.sdk.offline.KagemushaNativeCollectedAppIdentityOriginalV1
import org.hyperledger.iroha.sdk.offline.KagemushaNativePreparedAppApprovalV1
import org.hyperledger.iroha.sdk.offline.KagemushaNativePreparedAppEnrollmentPossessionV1
import org.hyperledger.iroha.sdk.offline.KagemushaNativePreparedOrdinaryBootstrapApprovalV1
import org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryIdentityOriginalTransportV1
import org.hyperledger.iroha.sdk.offline.KagemushaNativeRawAppIdentityAdmissionV1
import java.security.MessageDigest
import java.security.cert.X509Certificate
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AndroidKeyAttestationOriginalV1
import org.hyperledger.iroha.sdk.offline.KagemushaPlatformAttestationOriginalV1

/** Local generation preference. The independent issuer still enforces the original enrolled policy. */
enum class KagemushaAndroidAppKeyHardwarePolicyV1 { TEE_OR_STRONGBOX, STRONGBOX_ONLY, TEE_ONLY }

/** Original platform evidence, without app-integrity, StateGuard or offline counter qualification. */
class KagemushaAndroidHardwareAppKeyEvidenceV1 internal constructor(val securityLevel: Int,
    keyId: ByteArray, publicKey: ByteArray, chain: List<ByteArray>) {
    private val id = keyId.copyOf(); private val point = publicKey.copyOf()
    private val platformOriginal = KagemushaPlatformAttestationOriginalV1.android(chain)
    fun attestedKeyId(): ByteArray = id.copyOf()
    fun publicKeySec1(): ByteArray = point.copyOf()
    fun certificateChainDer(): List<ByteArray> = checkNotNull(platformOriginal.androidCertificateChainDer())
    /** Complete shared canonical original archive; data only, without a native or issuer grant. */
    fun platformAttestationOriginal(): ByteArray = platformOriginal.canonicalBytes()
    /** SHA-256 of the complete canonical archive, including original order and framing. */
    fun platformAttestationOriginalSha256(): ByteArray = platformOriginal.canonicalDigest()
}

/** Shared ordinary app-key adapter. No custom applet or finite-use key is required to enroll.
 * StrongBox is preferred when available; only an independently permitted TEE policy may use TEE.
 * The original alias must be recovered after uncertainty, never replaced by a new identity.
 *
 * Every existence decision is the tri-state `getKey` probe ([probeAndroidKeystoreAliasV1]): a
 * Keystore error stops the call and is never read as absence, a key is generated only after a
 * definitive absence, and nothing here deletes or regenerates a key in response to an error.
 * Custody requires keystore2 (API 31+). keystore1 cannot prove that an alias is empty, so every
 * call refuses below API 31, as the KAGEMUSHA wallet payment key does.
 */
class KagemushaAndroidHardwareAppKeyStoreV1 internal constructor(
    private val keyStore: AndroidKeystoreV1,
    private val hasStrongBox: () -> Boolean,
) {
    constructor(context: Context) : this(AndroidSystemKeystoreV1(), kagemushaAppKeyStrongBoxFeatureV1(context.applicationContext))

    fun issueExact(alias: String, challenge: ByteArray, policy: KagemushaAndroidAppKeyHardwarePolicyV1,
        requireCurrent: () -> Unit): KagemushaAndroidHardwareAppKeyEvidenceV1 = synchronized(lock) {
        requireCurrent(); requireAvailable()
        val digest = challenge.copyOf(); require(alias.isNotBlank() && alias.length <= 128 && digest.size == 32)
        val strongBox = persistentHardwareAppKeyStrongBoxRequestedV1(keyStore.apiLevel, policy, hasStrongBox())
        requireDefinitelyAbsent(alias)
        fun generate(strong: Boolean) {
            requireCurrent(); requireDefinitelyAbsent(alias)
            keyStore.generate(AndroidKeystoreEcKeyRequestV1(alias, digest, strong))
        }
        if (strongBox) {
            try { generate(true) }
            catch (unavailable: AndroidKeystoreStrongBoxUnavailableV1) {
                // A definite StrongBox-unavailable result may use the policy-approved TEE under the
                // same original alias, after a second definitive absence. A retained key or a
                // Keystore that cannot answer stops instead.
                requireCurrent()
                if (policy != KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX) {
                    throw IllegalStateException("Required StrongBox is unavailable", unavailable)
                }
                generate(false)
            }
        } else generate(false)
        requireCurrent(); loadExact(alias, digest, policy).also { requireCurrent() }
    }
    fun recoverExact(alias: String, challenge: ByteArray, policy: KagemushaAndroidAppKeyHardwarePolicyV1,
        requireCurrent: () -> Unit): KagemushaAndroidHardwareAppKeyEvidenceV1? = synchronized(lock) {
        requireCurrent(); requireAvailable()
        // Null only for a definitive keystore2 absence; a Keystore that cannot answer throws.
        keyStore.probe(alias) ?: return@synchronized null
        loadExact(alias, challenge.copyOf(), policy).also { requireCurrent() }
    }
    /** Internal native-original signer. No key is generated, replaced or exported by this path. */
    internal fun originalSignatureDevice(): KagemushaAndroidAppSignatureDeviceV1 = object : KagemushaAndroidAppSignatureDeviceV1 {
        override fun signOriginal(original: KagemushaAndroidHeldAppSignatureV1): ByteArray = synchronized(lock) {
            original.requireOriginal(); requireAvailable()
            requireOriginalKey(original)
            val key = presentPrivateKey(original.alias)
            original.requireOriginal()
            keyStore.sign(key, original.signingBytes()).also { original.requireOriginal() }
        }
        override fun verifyOriginal(original: KagemushaAndroidHeldAppSignatureV1, signatureDer: ByteArray) = synchronized(lock) {
            original.requireOriginal(); requireAvailable(); requireCanonicalP256SignatureDerV1(signatureDer)
            requireOriginalKey(original)
            val publicKey = leafCertificate(original.alias).publicKey
            check(Signature.getInstance("SHA256withECDSA").run {
                initVerify(publicKey); update(original.signingBytes()); verify(signatureDer)
            }) { "Retained app signature does not belong to the original key and bytes" }
            original.requireOriginal()
        }
    }
    private fun requireOriginalKey(original: KagemushaAndroidHeldAppSignatureV1) {
        val evidence = loadExact(original.alias, original.attestationChallenge(), original.hardwarePolicy)
        check(MessageDigest.isEqual(evidence.attestedKeyId(), original.attestedKeyId())) { "Original app approval key changed" }
        original.requireOriginal()
    }
    /** Collect only the native-held C identity; issuer response intake is a separate explicit native step. */
    fun collectIdentity(prepared: KagemushaNativePreparedOrdinaryAppIdentityV1): KagemushaNativeCollectedAppIdentityOriginalV1 =
        prepared.performAndroidCollection { alias, challenge, policy, recoverOnly, guard ->
            if (recoverOnly) checkNotNull(recoverExact(alias, challenge, policy, guard)) {
                "Original app key is missing; an uncertain native identity cannot generate a replacement"
            }
            else issueExact(alias, challenge, policy, guard)
        }

    /** Collect only native-selected originals and let native Core admit the protected issuer reply. */
    suspend fun enroll(prepared: KagemushaNativePreparedOrdinaryAppIdentityV1,
        transport: KagemushaOrdinaryIdentityOriginalTransportV1): KagemushaNativeRawAppIdentityAdmissionV1 {
        collectIdentity(prepared)
        return prepared.admitOriginalAttestation(transport)
    }

    /** Sign only the exact Native-held purpose2 cash preparation. The internal validator runs
     * before the durable fence; the separate Bootstrap holder retains its fixed purpose1 path.
     * The returned original receipt grants no State proof, funds, finality or release qualification.
     */
    fun approve(prepared: KagemushaNativePreparedAppApprovalV1): ByteArray =
        approveNativeOrdinaryPreparationOriginalV1(prepared, ::signNativeOriginal)

    /** Capture the exact Bootstrap W only through its separate same-FI Native holder. */
    fun approveOrdinaryBootstrap(prepared: KagemushaNativePreparedOrdinaryBootstrapApprovalV1): ByteArray =
        prepared.performPlatformSigning { alias, generationChallenge, point, keyId, message, policy, guard ->
            signNativeOriginal(alias, generationChallenge, point, keyId, message, policy,
                KagemushaAndroidAppSignaturePurposeV1.ORDINARY_BOOTSTRAP_APPROVAL, guard)
        }.copyOf()

    /** Sign only the independently selected opaque purpose1 cash W1, after its real Reserve. */
    fun approveOrdinaryTerminal(prepared: org.hyperledger.iroha.sdk.offline.KagemushaNativePreparedOrdinaryTerminalApprovalV1): ByteArray =
        approveNativeOrdinaryTerminalOriginalV1(prepared, ::signNativeOriginal)

    /** Separate genuine Native incoming holder chooses W2 purpose2 or scoped W1 purpose1.
     * Full W/S correlation runs before Native fence; no raw message/alias can enter this API.
     */
    fun approveOrdinaryIncoming(prepared: org.hyperledger.iroha.sdk.offline.KagemushaNativePreparedOrdinaryIncomingApprovalV1): ByteArray =
        prepared.performPlatformSigning { alias, generationChallenge, point, keyId, message, policy, terminal, guard ->
            signNativeOriginal(alias, generationChallenge, point, keyId, message, policy,
                if (terminal) KagemushaAndroidAppSignaturePurposeV1.ORDINARY_INCOMING_TERMINAL_APPROVAL
                else KagemushaAndroidAppSignaturePurposeV1.ORDINARY_PREPARATION_APPROVAL, guard)
        }

    fun approveOrdinaryIntegrityRefresh(prepared:org.hyperledger.iroha.sdk.offline.KagemushaNativePreparedOrdinaryIntegrityRefreshV1):ByteArray =
        prepared.performPlatformSigning { alias,challenge,point,id,message,policy,guard ->
            signNativeOriginal(alias,challenge,point,id,message,policy,
                KagemushaAndroidAppSignaturePurposeV1.ORDINARY_INTEGRITY_REFRESH_POSSESSION,guard)
        }

    /** Dedicated actual Native Mint pre-debit purpose, separate from W2/W1 approval. */
    fun approveOrdinaryMintFunding(prepared:org.hyperledger.iroha.sdk.offline.KagemushaNativePreparedOrdinaryMintApprovalV1):ByteArray =
        prepared.performPlatformSigning {alias,challenge,point,id,message,policy,guard ->
            signNativeOriginal(alias,challenge,point,id,message,policy,
                KagemushaAndroidAppSignaturePurposeV1.ORDINARY_MINT_PRE_DEBIT_APPROVAL,guard)
        }

    /** Prove possession over the separate native-owned E without creating a monetary qualification. */
    fun proveEnrollmentPossession(prepared: KagemushaNativePreparedAppEnrollmentPossessionV1): ByteArray =
        prepared.performPlatformSigning { alias, generationChallenge, point, keyId, message, policy, guard ->
            signNativeOriginal(alias, generationChallenge, point, keyId, message, policy,
                KagemushaAndroidAppSignaturePurposeV1.IDENTITY_ENROLLMENT_POSSESSION, guard)
        }.copyOf()

    /** Distinct first-device E purpose; it cannot borrow financial W/E or raw DTO selectors. */
    fun proveFirstDeviceHardwarePossession(prepared: org.hyperledger.iroha.sdk.offline.KagemushaFirstDeviceHardwareEvidenceSessionV1) =
        prepared.performHardwarePossession { selected, guard ->
            signNativeOriginal(selected.key.alias, selected.key.challenge(), selected.point(), selected.keyId(),
                selected.message(), selected.key.policy, KagemushaAndroidAppSignaturePurposeV1.FIRST_DEVICE_HARDWARE_POSSESSION, guard)
        }

    // Only the native opaque prepared capability can enter this method. The native owner
    // durably fences invocation and retains original DER before consuming the approval.
    // Recovery never invokes this callback again for a started or completed attempt.
    private fun signNativeOriginal(alias: String, generationChallenge: ByteArray,
        expectedPoint: ByteArray, expectedKeyId: ByteArray, message: ByteArray,
        policy: KagemushaAndroidAppKeyHardwarePolicyV1, purpose: KagemushaAndroidAppSignaturePurposeV1,
        requireCurrent: () -> Unit): ByteArray = synchronized(lock) {
        requireCurrent(); requireAvailable()
        requireAppPlatformSigningMessageV1(message, purpose)
        // Null is a definitive keystore2 absence; a Keystore that cannot answer throws instead.
        checkNotNull(keyStore.probe(alias)) { "The enrolled app key is missing; native recovery is required" }
        val original = loadExact(alias, generationChallenge.copyOf(), policy)
        requireOriginalAppKeyBindingV1(original.publicKeySec1(), original.attestedKeyId(), expectedPoint, expectedKeyId)
        requireCurrent()
        val key = presentPrivateKey(alias)
        check(key.encoded == null) { "Original app key must remain nonexportable" }
        val certificate = leafCertificate(alias)
        val public = certificate.publicKey as? ECPublicKey
            ?: error("Original app-key certificate is not P-256")
        // Re-read the original alias after loading its private key and immediately before
        // the actual platform call; neither an endpoint projection nor an app DTO selects it.
        val rechecked = loadExact(alias, generationChallenge, policy)
        requireOriginalAppKeyBindingV1(rechecked.publicKeySec1(), rechecked.attestedKeyId(), expectedPoint, expectedKeyId)
        requireCurrent()
        val signature = signOriginalAndroidAppMessageV1(key, public, message, purpose, requireCurrent)
        try {
            val retained = loadExact(alias, generationChallenge, policy)
            requireOriginalAppKeyBindingV1(retained.publicKeySec1(), retained.attestedKeyId(), expectedPoint, expectedKeyId)
            requireCurrent()
            signature
        } catch (failure: Throwable) {
            signature.fill(0)
            throw failure
        }
    }

    private fun requireAvailable() {
        check(persistentHardwareAppKeyApiAvailableV1(keyStore.apiLevel)) { HARDWARE_APP_KEY_KEYSTORE2_REQUIRED_V1 }
    }
    /** Generation precondition: the tri-state probe reports a definitive absence. */
    private fun requireDefinitelyAbsent(alias: String) {
        check(keyStore.probe(alias) == null) { "Original app key already exists; recover it" }
    }
    /** The present private key of [alias]; a definitive absence and a Keystore error both stop. */
    private fun presentPrivateKey(alias: String): PrivateKey =
        checkNotNull(keyStore.probe(alias)) { "Original app signing key is unavailable" } as? PrivateKey
            ?: error("Original app signing key is unavailable")
    /** Leaf certificate of a present key; a missing chain is a Keystore error, never absence. */
    private fun leafCertificate(alias: String): X509Certificate =
        keyStore.getCertificateChain(alias)?.firstOrNull() as? X509Certificate
            ?: error("Original app-key certificate is unavailable")
    private fun loadExact(alias: String, challenge: ByteArray,
        policy: KagemushaAndroidAppKeyHardwarePolicyV1): KagemushaAndroidHardwareAppKeyEvidenceV1 {
        require(alias.isNotBlank() && alias.length <= 128 && challenge.size == 32)
        val key = checkNotNull(keyStore.probe(alias)) { "Original app key disappeared" } as? PrivateKey
            ?: error("Original app key is not a private key")
        val facts = keyStore.facts(key)
        val chain = checkNotNull(keyStore.getCertificateChain(alias)) { "Original app-key certificate chain is unavailable" }
            .map { it as? X509Certificate ?: error("Original app-key certificate is not X.509") }
        val der = chain.map(X509Certificate::getEncoded)
        check(der.size in 2..8 && der.all { it.isNotEmpty() && it.size <= 16 * 1024 })
        val attested = AndroidKeyAttestationOriginalV1.persistentAppHardwareSecurityLevel(chain, challenge)
        val level = requirePersistentAppHardwareMetadataV1(keyStore.apiLevel, facts.insideSecureHardware,
            attested.encodedValue, facts.securityLevel, facts.remainingUsageCount)
        requirePersistentHardwareAppKeyV1(level, facts.origin, facts.purposes, facts.digests,
            facts.remainingUsageCount, key.encoded != null, policy)
        val point = AndroidKeyAttestationOriginalV1.publicKeySec1(chain)
        return KagemushaAndroidHardwareAppKeyEvidenceV1(level, MessageDigest.getInstance("SHA-256").digest(point), point, der)
    }
    companion object {
        /** Provisioning API eligibility (keystore2, API 31+) only; actual key custody, attestation and Native admission are separate. */
        @JvmStatic fun isPlatformApiAvailable(): Boolean = persistentHardwareAppKeyApiAvailableV1(Build.VERSION.SDK_INT)
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

/** `FEATURE_STRONGBOX_KEYSTORE`; read only after the keystore2 check. */
private fun kagemushaAppKeyStrongBoxFeatureV1(context: Context): () -> Boolean =
    { context.packageManager.hasSystemFeature(PackageManager.FEATURE_STRONGBOX_KEYSTORE) }

private const val HARDWARE_APP_KEY_KEYSTORE2_REQUIRED_V1 =
    "Hardware app-key custody requires keystore2 (API 31+); keystore1 cannot prove an alias is empty"

/** keystore2 only: below API 31 `getKey` cannot report a definitive absence. */
internal fun persistentHardwareAppKeyApiAvailableV1(apiLevel: Int): Boolean = apiLevel >= ANDROID_KEYSTORE2_MIN_API_V1

internal fun persistentHardwareAppKeyStrongBoxRequestedV1(api: Int, policy: KagemushaAndroidAppKeyHardwarePolicyV1,
    hasStrongBox: Boolean): Boolean {
    check(persistentHardwareAppKeyApiAvailableV1(api)) { HARDWARE_APP_KEY_KEYSTORE2_REQUIRED_V1 }
    val strongBox = hasStrongBox && policy != KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY
    check(strongBox || policy != KagemushaAndroidAppKeyHardwarePolicyV1.STRONGBOX_ONLY) { "Required StrongBox is unavailable" }
    return strongBox
}

/** Exact persistent custody: a level [policy] admits, generated on the device, `PURPOSE_SIGN` and
 * `SHA-256` only, not exportable, and unlimited use.
 */
internal fun requirePersistentHardwareAppKeyV1(level: Int, origin: Int, purposes: Int, digests: Set<String>,
    remaining: Int, exportable: Boolean, policy: KagemushaAndroidAppKeyHardwarePolicyV1) {
    check(remaining == KeyProperties.UNRESTRICTED_USAGE_COUNT) { "Original persistent app-key usage metadata differs" }
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

/** keystore2 custody uses actual KeyInfo hardware residency, a KeyInfo security level equal to the
 * exact signed extension level, and unlimited use.
 */
internal fun requirePersistentAppHardwareMetadataV1(api: Int, insideSecureHardware: Boolean,
    attestedLevel: Int, reportedLevel: Int, remaining: Int): Int {
    check(persistentHardwareAppKeyApiAvailableV1(api)) { HARDWARE_APP_KEY_KEYSTORE2_REQUIRED_V1 }
    check(insideSecureHardware && attestedLevel in setOf(
        KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT, KeyProperties.SECURITY_LEVEL_STRONGBOX)) {
        "Persistent app key lacks actual hardware custody or an exact attested hardware level"
    }
    check(reportedLevel == attestedLevel && remaining == KeyProperties.UNRESTRICTED_USAGE_COUNT) {
        "Original modern hardware metadata differs from the signed persistent key"
    }
    return attestedLevel
}
