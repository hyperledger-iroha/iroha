// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import android.content.Context
import java.security.KeyStore
import java.security.MessageDigest
import java.security.PrivateKey
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppSignaturePurposeV1
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidHardwareAppKeyStoreV1
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidPlayIntegrityTokenOriginalV1
import org.hyperledger.iroha.sdk.crypto.keystore.requireAppPlatformSigningMessageV1

/**
 * Callable physical-test support, without a test runner or an installation factory.
 * Every operation takes actual opaque Native holders. Detached observations below are comparison
 * data, never a way to reopen Native, admit Google/PKIX, or establish State/Guard readiness.
 *
 * TODO: wire an admitted product-host instrumentation entry to genuine installed runtime/account
 * acquisition. The SDK test application's identity must not stand in for that product's Play pin.
 */
internal object OrdinaryPersistentIdentityPhysicalHarnessV1 {
    /** Collect through the real SDK adapter; Native decides fresh versus exact-original recovery. */
    fun collectNativeOriginal(context: Context,
        identity: KagemushaNativePreparedOrdinaryAppIdentityV1): CollectedObservation {
        val signedC = identity.originalSignedPreparationBytes()
        val transcript = requireAndroidPreparation(identity)
        val collected = KagemushaAndroidHardwareAppKeyStoreV1(context).collectIdentity(identity)
        val observed = CollectedObservation(signedC, transcript, collected.originalKeyReference(),
            collected.publicKeySec1(), collected.originalAttestationBytes())
        requireCollectedOriginal(identity, observed)
        requireKeystoreOriginal(observed)
        requireCollectedOriginal(identity, observed)
        return observed
    }

    /**
     * Sign only the Native-owned E after genuine raw314 admission, then invoke the existing
     * same-coordinator C/raw/key/E/policy pairing check. A required token must be the original
     * actual provider result; this helper neither requests nor decodes it, nor saves its body.
     * The Native receipt returned by proveEnrollmentPossession is distinct from platform DER.
     */
    fun proveAndObserveBoundOriginal(context: Context,
        reservation: KagemushaNativeReservedOrdinaryAppIdentityV1,
        identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
        possession: KagemushaNativePreparedAppEnrollmentPossessionV1,
        collected: CollectedObservation,
        integrityOriginal: KagemushaAndroidPlayIntegrityTokenOriginalV1?): PossessionObservation {
        requireCollectedOriginal(identity, collected)
        val admission = checkNotNull(identity.recoverOriginalAdmission()) {
            "The same original requires genuine Native raw314 admission before possession"
        }
        requireAdmissionOriginal(admission, collected)
        val e = possession.signingBytes()
        requireEnrollmentE(e)
        val receipt = KagemushaAndroidHardwareAppKeyStoreV1(context).proveEnrollmentPossession(possession)
        same(receipt, checkNotNull(possession.recoverOriginalPossession()) {
            "Native did not retain the original possession receipt"
        }, "possession receipt")
        val der = possession.originalPlatformEvidence()
        // This sole SDK method rejects cross-coordinator/C/key/raw/scope/policy substitutions.
        // No HTTP call or token-bearing request body is exposed by the harness.
        val paired = possession.certificateRequestOriginal(reservation, identity, integrityOriginal)
        paired.requireCurrent()
        val observed = PossessionObservation(collected, admission.signedRawAdmissionTransport(),
            admission.pendingNativeScopeDigest(), e, der, receipt)
        requireRecoveredOriginal(identity, possession, observed)
        requireKeystoreOriginal(collected)
        paired.requireCurrent()
        return observed
    }

    /**
     * Compare a genuinely recovered owner with the first retained originals after lost result or
     * process restart. No collection/signing/provider/HTTP call is made here. Native recovery may
     * finish consumption of its already retained DER; it never invokes the platform signer again.
     * Missing originals or Keystore entry are errors; reinstall does not authorize replacement.
     */
    fun assertRecoveredOriginal(identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
        possession: KagemushaNativePreparedAppEnrollmentPossessionV1,
        expected: PossessionObservation) {
        requireRecoveredOriginal(identity, possession, expected)
        requireKeystoreOriginal(expected.collected())
        requireRecoveredOriginal(identity, possession, expected)
    }

    /** Run the installed shared service's real issuer/PI/wallet/FI path, with no readiness claim. */
    suspend fun beginOrResumeInstalledEnrollment(service: KagemushaAndroidOrdinaryHardwareServiceV1):
        KagemushaOrdinaryEnrollmentOriginalsV1 = service.beginOrResumeEnrollment()

    /** Same-service explicit retry must retain the first FI enrollment and certificate originals. */
    suspend fun assertSameRetainedEnrollment(service: KagemushaAndroidOrdinaryHardwareServiceV1,
        expected: KagemushaOrdinaryEnrollmentOriginalsV1) {
        val recovered = service.beginOrResumeEnrollment()
        same(expected.enrollmentId(), recovered.enrollmentId(), "FI enrollment ID")
        same(expected.originalRetailCertificate(), recovered.originalRetailCertificate(), "FI certificate")
    }

    private fun requireRecoveredOriginal(identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
        possession: KagemushaNativePreparedAppEnrollmentPossessionV1, expected: PossessionObservation) {
        requireCollectedOriginal(identity, expected.collected())
        val admission = checkNotNull(identity.recoverOriginalAdmission()) {
            "Native raw admission is missing; detached observations cannot recreate it"
        }
        requireAdmissionOriginal(admission, expected.collected())
        same(admission.signedRawAdmissionTransport(), expected.signedAdmission(), "signed raw314")
        same(admission.pendingNativeScopeDigest(), expected.pendingScope(), "pending Native scope")
        val e = possession.signingBytes()
        requireEnrollmentE(e)
        same(e, expected.enrollmentPossession(), "original E424")
        same(checkNotNull(possession.recoverOriginalPossession()) {
            "Original possession is unavailable; recovery must not invoke a new signature"
        }, expected.receipt(), "original Native receipt")
        same(possession.originalPlatformEvidence(), expected.platformDer(), "original platform DER")
        requireCollectedOriginal(identity, expected.collected())
        same(possession.signingBytes(), e, "current original E")
    }

    private fun requireAndroidPreparation(identity: KagemushaNativePreparedOrdinaryAppIdentityV1): ByteArray {
        val c = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(identity.originalSignedPreparationBytes())
        check(c.platformClass == KagemushaHardwarePlatformClassV1.ANDROID_KEYMINT) { "Android C is required" }
        val original = identity.originalChallengeSigningBytes()
        check(original.size == 512) { "Original C signing transcript must be 512 bytes" }
        same(c.canonicalSigningBytes(), original, "full signed C transcript")
        return original
    }

    private fun requireCollectedOriginal(identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
        expected: CollectedObservation) {
        same(identity.originalSignedPreparationBytes(), expected.signedPreparation(), "signed C515")
        same(requireAndroidPreparation(identity), expected.challengeTranscript(), "C signing transcript")
        val recovered = checkNotNull(identity.recoverOriginalAttestation()) {
            "Native original attestation is missing; recovery must not generate another key"
        }
        check(recovered.originalKeyReference() == expected.keyReference()) { "Original app key alias changed" }
        val point = recovered.publicKeySec1()
        check(point.size == 65 && point[0] == 4.toByte()) { "Original full P-256 point is required" }
        same(point, expected.publicKeySec1(), "original P-256 point")
        val raw = recovered.originalAttestationBytes()
        same(raw, expected.platformOriginal(), "full platform original")
        val canonical = KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(raw)
        checkNotNull(canonical.androidCertificateChainDer()) { "The retained canonical original must be Android" }
        same(canonical.canonicalBytes(), raw, "canonical platform framing")
    }

    private fun requireAdmissionOriginal(admission: KagemushaNativeRawAppIdentityAdmissionV1,
        expected: CollectedObservation) {
        check(admission.originalKeyReference() == expected.keyReference()) { "Admitted alias changed" }
        same(admission.publicKeySec1(), expected.publicKeySec1(), "admitted original point")
        same(admission.originalAttestationBytes(), expected.platformOriginal(), "admitted full original")
        check(admission.signedRawAdmissionTransport().size == 314) { "Exact Native-admitted raw314 is required" }
        val scope = admission.pendingNativeScopeDigest()
        check(scope.size == 32 && scope.any { it != 0.toByte() }) { "Actual pending scope is required" }
    }

    private fun requireEnrollmentE(e: ByteArray) {
        requireAppPlatformSigningMessageV1(e, KagemushaAndroidAppSignaturePurposeV1.IDENTITY_ENROLLMENT_POSSESSION)
        check(e.size == 424) { "Native E must be exactly 424 bytes" }
    }

    /** Existence/full-chain corroboration only; server policy and future key usability stay separate. */
    private fun requireKeystoreOriginal(expected: CollectedObservation) {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        // keystore2 getKey: null is a definitive absence; a Keystore error throws instead.
        check(store.getKey(expected.keyReference(), null) is PrivateKey) {
            "Original Android Keystore entry is missing; authenticated recovery, rotation or retirement is required"
        }
        val chain = checkNotNull(store.getCertificateChain(expected.keyReference())) {
            "Original generation-time certificate chain is missing"
        }.map { it.encoded }
        val observed = KagemushaPlatformAttestationOriginalV1.android(chain).canonicalBytes()
        same(observed, expected.platformOriginal(), "Keystore original certificate chain")
    }

    private fun same(first: ByteArray, second: ByteArray, label: String) {
        check(MessageDigest.isEqual(first, second)) { "$label changed" }
    }

    /** Immutable, detached DATA used only as an expected comparison, without serialization/owner admission. */
    class CollectedObservation internal constructor(signedC: ByteArray, challenge: ByteArray,
        private val reference: String, point: ByteArray, raw: ByteArray) {
        private val c = signedC.copyOf()
        private val transcript = challenge.copyOf()
        private val keyPoint = point.copyOf()
        private val platform = raw.copyOf()
        fun signedPreparation(): ByteArray = c.copyOf()
        fun challengeTranscript(): ByteArray = transcript.copyOf()
        fun keyReference(): String = reference
        fun publicKeySec1(): ByteArray = keyPoint.copyOf()
        fun platformOriginal(): ByteArray = platform.copyOf()
    }

    /** Expected DATA retains the first C/raw/E/DER/receipt; it cannot manufacture a native capability. */
    class PossessionObservation internal constructor(private val identity: CollectedObservation,
        signedRaw: ByteArray, scope: ByteArray, e: ByteArray, der: ByteArray, receipt: ByteArray) {
        private val admission = signedRaw.copyOf()
        private val pending = scope.copyOf()
        private val possession = e.copyOf()
        private val evidence = der.copyOf()
        private val nativeReceipt = receipt.copyOf()
        fun collected(): CollectedObservation = identity
        fun signedAdmission(): ByteArray = admission.copyOf()
        fun pendingScope(): ByteArray = pending.copyOf()
        fun enrollmentPossession(): ByteArray = possession.copyOf()
        fun platformDer(): ByteArray = evidence.copyOf()
        fun receipt(): ByteArray = nativeReceipt.copyOf()
    }
}
