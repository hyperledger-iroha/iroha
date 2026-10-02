package org.hyperledger.iroha.sdk.crypto.keystore.attestation

import java.math.BigInteger
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.MessageDigest
import java.security.Signature
import java.security.spec.ECGenParameterSpec
import org.bouncycastle.asn1.*
import org.bouncycastle.asn1.x500.X500Name
import org.bouncycastle.asn1.x509.*
import org.bouncycastle.asn1.x9.X9ObjectIdentifiers
import org.hyperledger.iroha.sdk.crypto.keystore.KeyAttestation
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

/** Synthetic signed PKIX chains exercise selection; they grant no device qualification. */
class AttestationOriginalCertificateV1Test {
    private val challenge = ByteArray(32) { 7 }
    private val time = 1_800_000_000_000L
    private val oid = ASN1ObjectIdentifier("1.3.6.1.4.1.11129.2.1.17")

    @Test fun rootNearestDescriptionWinsOverANewerLeafDescription() {
        val f = fixture(original = description(challenge), leaf = description(ByteArray(32) { 8 }, 0, 0))
        val result = f.verifier.verify(f.attestation, challenge)
        assertEquals(AttestationResult.SecurityLevel.STRONG_BOX, result.attestationSecurityLevel)
        assertArrayEquals(challenge, result.attestationChallenge())
        assertEquals("CN=Original Attestation", result.attestationCertificate.subjectX500Principal.name)
        assertEquals("CN=Alias Leaf", result.leafCertificate.subjectX500Principal.name)
    }

    @Test fun validLeafCannotOverrideWrongOriginalChallenge() {
        val f = fixture(original = description(ByteArray(32) { 8 }), leaf = description(challenge))
        assertThrows(AttestationVerificationException::class.java) { f.verifier.verify(f.attestation, challenge) }
    }

    @Test fun rootNearestCertificateMustDescribeTheAliasLeafPublicKey() {
        val f = fixture(original = description(challenge), leaf = description(challenge), differentOriginalKey = true)
        assertThrows(AttestationVerificationException::class.java) { f.verifier.verify(f.attestation, challenge) }
    }

    @Test fun strongBoxPolicyRequiresBothKeyAndAttestationLevelTwo() {
        for ((attestation, key) in listOf(2 to 1, 1 to 2, 1 to 1, 0 to 2)) {
            val f = fixture(original = description(challenge, attestation, key))
            assertThrows(AttestationVerificationException::class.java) { f.verifier.verify(f.attestation, challenge) }
        }
    }

    @Test fun leafOnlyOriginalRemainsBoundToTheSameAliasKey() {
        val f = fixture(original = null, leaf = description(challenge))
        val result = f.verifier.verify(f.attestation, challenge)
        assertEquals(result.leafCertificate, result.attestationCertificate)
    }

    @Test fun missingOriginalOrMalformedOriginalCannotBeReplacedByLeafEvidence() {
        for (f in listOf(fixture(original = null), fixture(original = byteArrayOf(0), leaf = description(challenge)))) {
            assertThrows(Exception::class.java) { f.verifier.verify(f.attestation, challenge) }
        }
    }

    @Test fun completeOriginalCertificateRejectsEachRetiredDerShapeNegative() {
        val f = fixture(original = description(challenge))
        f.verifier.verify(f.attestation, challenge) // Real signed positive precedes mutation.
        val chain = f.attestation.certificateChain()
        val leaf = chain[0]
        // All eight former archive-parser negatives now exercise the sole actual verifier.
        val malformed = listOf(leaf + 0, leaf.copyOfRange(0, 4), byteArrayOf(0x31, 0),
            byteArrayOf(0x30, 0x80.toByte(), 0, 0), byteArrayOf(0x30, 0x81.toByte(), 0),
            byteArrayOf(0x30, 0x82.toByte(), 0, 1, 0), byteArrayOf(0x30, 0x81.toByte(), 1, 0),
            ByteArray(16 * 1024 + 1))
        for (bad in malformed) assertThrows(AttestationVerificationException::class.java) {
            f.verifier.verify(KeyAttestation("original-alias", listOf(bad, chain[1], chain[2])), challenge)
        }
    }

    @Test fun independentlySelectedRootRejectsAppendedOriginalDer() {
        val f = fixture(original = description(challenge))
        f.verifier.verify(f.attestation, challenge)
        val root = f.attestation.certificateChain().last()
        AttestationVerifier.builder(f.policy, time).addTrustedRoot(root).build()
        for (bad in listOf(root + 0, root + root)) {
            assertThrows(AttestationVerificationException::class.java) {
                AttestationVerifier.builder(f.policy, time).addTrustedRoot(bad)
            }
        }
    }

    private fun description(challenge: ByteArray, attestation: Int = 2, key: Int = 2) = DERSequence(arrayOf(
        ASN1Integer(100), ASN1Enumerated(attestation), ASN1Integer(100), ASN1Enumerated(key),
        DEROctetString(challenge), DEROctetString(ByteArray(0)), DERSequence(), DERSequence(),
    )).encoded

    private class Fixture(val verifier: AttestationVerifier, val attestation: KeyAttestation,
        val policy: AndroidAttestationRevocationPolicyV1)
    private fun fixture(original: ByteArray?, leaf: ByteArray? = null, differentOriginalKey: Boolean = false): Fixture {
        val root = key(); val alias = key(); val originalKey = if (differentOriginalKey) key() else alias
        val rootDer = certificate(root, root, "CN=Root", "CN=Root", true, 1, null)
        val originalDer = certificate(originalKey, root, "CN=Original Attestation", "CN=Root", true, 2, original)
        val leafDer = certificate(alias, originalKey, "CN=Alias Leaf", "CN=Original Attestation", false, 3, leaf)
        val snapshot = (AndroidAttestationRevocationPolicyV1.SNAPSHOT_DOMAIN + "\n" +
            "payload_sha256=" + "11".repeat(32) + "\nresponse_date_ms=$time\nlast_modified_ms=-\n" +
            "cache_max_age_seconds=86400\nserial_count=0\ntbs_sha256_count=0\n").toByteArray(Charsets.US_ASCII)
        val policy = AndroidAttestationRevocationPolicyV1.fromCanonicalSnapshot(snapshot,
            MessageDigest.getInstance("SHA-256").digest(snapshot))
        return Fixture(AttestationVerifier.builder(policy, time).addTrustedRoot(rootDer).requireStrongBox(true).build(),
            KeyAttestation("original-alias", listOf(leafDer, originalDer, rootDer)), policy)
    }
    private fun key(): KeyPair = KeyPairGenerator.getInstance("EC").apply {
        initialize(ECGenParameterSpec("secp256r1"))
    }.generateKeyPair()
    private fun certificate(subject: KeyPair, issuer: KeyPair, subjectName: String, issuerName: String,
        ca: Boolean, serial: Long, description: ByteArray?): ByteArray {
        val algorithm = AlgorithmIdentifier(X9ObjectIdentifiers.ecdsa_with_SHA256)
        val extensions = mutableListOf(
            Extension(Extension.basicConstraints, true, DEROctetString(BasicConstraints(ca).encoded)),
            Extension(Extension.keyUsage, true, DEROctetString(KeyUsage(if (ca) KeyUsage.keyCertSign else KeyUsage.digitalSignature).encoded)),
        )
        description?.let { extensions += Extension(oid, false, DEROctetString(it)) }
        val tbs = DERSequence(arrayOf<ASN1Encodable>(DERTaggedObject(true, 0, ASN1Integer(2)), ASN1Integer(serial),
            algorithm, X500Name(issuerName), DERSequence(arrayOf(ASN1UTCTime("250101000000Z"), ASN1UTCTime("300101000000Z"))),
            X500Name(subjectName), SubjectPublicKeyInfo.getInstance(subject.public.encoded), DERTaggedObject(true, 3, Extensions(extensions.toTypedArray()))))
        val signature = Signature.getInstance("SHA256withECDSA").run { initSign(issuer.private); update(tbs.encoded); sign() }
        return DERSequence(arrayOf(tbs, algorithm, DERBitString(signature))).encoded
    }

    // Draft executable DATA controls: reuse only this class's existing synthetic signed-PKIX
    // fixture. C bytes are opaque nonce DATA, not a signed issuer/manifest/Google fixture.
    // None of these tests establish installed policy, hardware custody or issuer authority.
    private val firstDeviceCData = byteArrayOf(0x43, 0x2d, 0x44, 0x41, 0x54, 0x41, 1)
    private val firstDevicePackage = "test.known.public"
    private val firstDeviceSignerData = ByteArray(32) { 0x44 }

    @Test fun firstDeviceDataJoinsTheActualVerifiedP256AndCompleteCNonce() {
        val f = fixture(original = firstDeviceDescription())
        val result = firstDeviceVerify(f)
        val actual = AndroidKeyAttestationOriginalV1.publicKeySec1(
            f.verifier.verify(f.attestation, firstDeviceNonce()).certificateChain(),
        )
        assertArrayEquals(firstDeviceNonce(), result.challengeOriginalSha256())
        assertArrayEquals(actual, result.publicKeySec1())
        assertArrayEquals(MessageDigest.getInstance("SHA-256").digest(actual), result.attestedKeyId())
        assertEquals(AttestationResult.SecurityLevel.STRONG_BOX, result.securityLevel)
        assertEquals(firstDevicePackage, result.packageName)
        assertEquals(BigInteger.ONE, result.versionCode)
        assertArrayEquals(firstDeviceSignerData, result.signingIdentitySha256())
        assertEquals(time, result.evaluationTimeEpochMillis)
        assertEquals(3, result.chainLength)
    }

    @Test fun firstDevicePermittedTeeDoesNotRelaxTheHeldStrongBoxRequirement() {
        val f = fixture(original = firstDeviceDescription(attestationLevel = 1, keyLevel = 1))
        val teeVerifier = AttestationVerifier.builder(f.policy, time)
            .addTrustedRoot(f.attestation.certificateChain().last()).build()
        val result = firstDeviceVerify(f, verifier = teeVerifier,
            levels = setOf(AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT))
        assertEquals(AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT, result.securityLevel)
        assertThrows(AttestationVerificationException::class.java) {
            firstDeviceVerify(f, levels = setOf(AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT,
                AttestationResult.SecurityLevel.STRONG_BOX))
        }
    }

    @Test fun firstDeviceNonceCannotIgnoreAnyCompleteOriginalByteOrBounds() {
        val f = fixture(original = firstDeviceDescription())
        for (changed in listOf(firstDeviceCData + 0, firstDeviceCData.copyOfRange(0, firstDeviceCData.size - 1),
            firstDeviceCData.copyOf().also { it[0] = 0 })) {
            assertThrows(AttestationVerificationException::class.java) { firstDeviceVerify(f, cOriginal = changed) }
        }
        for (bad in listOf(ByteArray(0), ByteArray(192 * 1024 + 1))) {
            assertThrows(IllegalArgumentException::class.java) { firstDeviceVerify(f, cOriginal = bad) }
        }
        val boundary = ByteArray(192 * 1024) { 0x51 }
        val bounded = fixture(original = firstDeviceDescription(nonce = firstDeviceNonce(boundary)))
        assertArrayEquals(firstDeviceNonce(boundary), firstDeviceVerify(bounded, cOriginal = boundary).challengeOriginalSha256())
    }

    @Test fun firstDeviceOwnsActualPkixRootAndGovernedRevocationTimeChecks() {
        val f = fixture(original = firstDeviceDescription())
        val other = fixture(original = firstDeviceDescription())
        val untrusted = AttestationVerifier.builder(f.policy, time)
            .addTrustedRoot(other.attestation.certificateChain().last()).requireStrongBox(true).build()
        assertThrows(AttestationVerificationException::class.java) { firstDeviceVerify(f, verifier = untrusted) }
        val stale = AttestationVerifier.builder(f.policy, time + 86_400_001L)
            .addTrustedRoot(f.attestation.certificateChain().last()).requireStrongBox(true).build()
        assertThrows(AttestationVerificationException::class.java) { firstDeviceVerify(f, verifier = stale) }
        // Same maintained snapshot grammar, with the real synthetic leaf serial marked revoked.
        val snapshot = (AndroidAttestationRevocationPolicyV1.SNAPSHOT_DOMAIN + "\n" +
            "payload_sha256=" + "11".repeat(32) + "\nresponse_date_ms=$time\nlast_modified_ms=-\n" +
            "cache_max_age_seconds=86400\nserial_count=1\nserial=3\ntbs_sha256_count=0\n").toByteArray(Charsets.US_ASCII)
        val revoked = AndroidAttestationRevocationPolicyV1.fromCanonicalSnapshot(snapshot,
            MessageDigest.getInstance("SHA-256").digest(snapshot))
        val rejecting = AttestationVerifier.builder(revoked, time)
            .addTrustedRoot(f.attestation.certificateChain().last()).requireStrongBox(true).build()
        assertThrows(AttestationVerificationException::class.java) { firstDeviceVerify(f, verifier = rejecting) }
    }

    @Test fun firstDeviceAppTupleMustMatchEachIndependentPolicyMember() {
        val f = fixture(original = firstDeviceDescription())
        assertThrows(IllegalArgumentException::class.java) { firstDeviceVerify(f, packageName = "other.application") }
        assertThrows(IllegalArgumentException::class.java) { firstDeviceVerify(f, version = BigInteger.valueOf(2)) }
        assertThrows(IllegalArgumentException::class.java) { firstDeviceVerify(f, signer = ByteArray(32) { 0x45 }) }
        assertThrows(IllegalArgumentException::class.java) {
            firstDeviceVerify(f, levels = setOf(AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT))
        }
        for (bad in listOf(BigInteger.ZERO, BigInteger.valueOf(-1), BigInteger.ONE.shiftLeft(64))) {
            assertThrows(IllegalArgumentException::class.java) { firstDeviceVerify(f, version = bad) }
        }
        for (bad in listOf(ByteArray(0), ByteArray(31), ByteArray(32))) {
            assertThrows(IllegalArgumentException::class.java) { firstDeviceVerify(f, signer = bad) }
        }
        for (bad in listOf(emptySet(), setOf(AttestationResult.SecurityLevel.SOFTWARE))) {
            assertThrows(IllegalArgumentException::class.java) { firstDeviceVerify(f, levels = bad) }
        }
    }

    @Test fun firstDeviceClosedTag709RejectsMissingDuplicatesAndExtraIdentities() {
        val originals = listOf(
            firstDeviceDescription(applicationId = null),
            firstDeviceDescription(hardwareApplicationId = true),
            firstDeviceDescription(duplicateSoftwareApplicationId = true),
            firstDeviceDescription(applicationId = firstDeviceApplicationId(packageCount = 2)),
            firstDeviceDescription(applicationId = firstDeviceApplicationId(signerCount = 2)),
            firstDeviceDescription(applicationId = firstDeviceApplicationId(packageCount = 0)),
            firstDeviceDescription(applicationId = firstDeviceApplicationId(signerCount = 0)),
        )
        for (original in originals) {
            val f = fixture(original = original)
            assertThrows(Exception::class.java) { firstDeviceVerify(f) }
        }
    }

    @Test fun firstDeviceTag709RequiresExactDerUtf8TypesAndUnsignedVersion() {
        val canonical = firstDeviceApplicationId()
        val noncanonical = BERSequence(arrayOf<ASN1Encodable>(
            DERSet(DERSequence(arrayOf<ASN1Encodable>(DEROctetString(firstDevicePackage.toByteArray(Charsets.US_ASCII)), ASN1Integer(1)))),
            DERSet(DEROctetString(firstDeviceSignerData)),
        )).encoded
        val malformed = DERSequence(arrayOf<ASN1Encodable>(DEROctetString(byteArrayOf(1)), DERSet())).encoded
        val originals = listOf(
            firstDeviceDescription(applicationId = canonical + 0),
            firstDeviceDescription(applicationId = noncanonical),
            firstDeviceDescription(applicationId = malformed),
            firstDeviceDescription(applicationId = firstDeviceApplicationId(packageBytes = byteArrayOf(0xc3.toByte(), 0x28))),
            firstDeviceDescription(applicationId = firstDeviceApplicationId(version = BigInteger.ZERO)),
            firstDeviceDescription(applicationId = firstDeviceApplicationId(version = BigInteger.valueOf(-1))),
            firstDeviceDescription(applicationId = firstDeviceApplicationId(version = BigInteger.ONE.shiftLeft(64))),
        )
        for (original in originals) assertThrows(Exception::class.java) { firstDeviceVerify(fixture(original = original)) }
        val max = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
        val f = fixture(original = firstDeviceDescription(applicationId = firstDeviceApplicationId(version = max)))
        assertEquals(max, firstDeviceVerify(f, version = max).versionCode)
    }

    @Test fun firstDevicePersistentSubsetStillRefusesFiniteUseAndSoftwareHardwareTags() {
        for (original in listOf(firstDeviceDescription(finiteUse = true), firstDeviceDescription(softwarePurpose = true),
            firstDeviceDescription(hardwarePurpose = 3), firstDeviceDescription(attestationLevel = 2, keyLevel = 1))) {
            assertThrows(Exception::class.java) { firstDeviceVerify(fixture(original = original)) }
        }
    }

    @Test fun firstDeviceImmutableDataCopiesDoNotClaimCertificateAliasAuthority() {
        val f = fixture(original = firstDeviceDescription())
        val chain = f.attestation.certificateChain()
        // Canonical external label is DATA: certificates do not authenticate Keystore aliases.
        val selected = KeyAttestation("externally-selected-label", chain)
        chain[0].fill(0)
        val originalC = firstDeviceCData.copyOf()
        val signer = firstDeviceSignerData.copyOf()
        val result = f.verifier.verifyFirstDevicePersistentAppOriginals(selected, originalC, firstDevicePackage,
            BigInteger.ONE, signer, setOf(AttestationResult.SecurityLevel.STRONG_BOX))
        val cDigest = result.challengeOriginalSha256()
        val point = result.publicKeySec1()
        originalC.fill(0); signer.fill(0)
        result.challengeOriginalSha256().fill(0); result.publicKeySec1().fill(0); result.attestedKeyId().fill(0)
        result.signingIdentitySha256().fill(0); result.leafSpkiSha256().fill(0)
        assertArrayEquals(cDigest, result.challengeOriginalSha256())
        assertArrayEquals(point, result.publicKeySec1())
        assertArrayEquals(MessageDigest.getInstance("SHA-256").digest(point), result.attestedKeyId())
        assertArrayEquals(firstDeviceSignerData, result.signingIdentitySha256())
        assertTrue(result.leafSpkiSha256().any { it != 0.toByte() })
    }

    @Test fun legacyPersistentSubsetDoesNotAcquireTheFirstDeviceAppPolicy() {
        val f = fixture(original = firstDeviceDescription(applicationId = null))
        val verified = f.verifier.verify(f.attestation, firstDeviceNonce())
        assertEquals(AttestationResult.SecurityLevel.STRONG_BOX,
            AndroidKeyAttestationOriginalV1.persistentAppHardwareSecurityLevel(verified.certificateChain(), firstDeviceNonce()))
        assertThrows(IllegalArgumentException::class.java) { firstDeviceVerify(f) }
    }

    @Test fun firstDeviceOriginalCertificateWidthsAreCheckedBeforePkix() {
        val f = fixture(original = firstDeviceDescription())
        val held = f.attestation.certificateChain()
        for (chain in listOf(emptyList(), listOf(held[0]), List(9) { held[0] }, listOf(ByteArray(16_385), held[1], held[2]))) {
            val wrong = KeyAttestation("selected-label", chain)
            assertThrows(IllegalArgumentException::class.java) {
                f.verifier.verifyFirstDevicePersistentAppOriginals(wrong, firstDeviceCData, firstDevicePackage,
                    BigInteger.ONE, firstDeviceSignerData, setOf(AttestationResult.SecurityLevel.STRONG_BOX))
            }
        }
    }

    private fun firstDeviceNonce(original: ByteArray = firstDeviceCData): ByteArray =
        MessageDigest.getInstance("SHA-256").digest(original)

    private fun firstDeviceVerify(
        fixture: Fixture,
        cOriginal: ByteArray = firstDeviceCData,
        packageName: String = firstDevicePackage,
        version: BigInteger = BigInteger.ONE,
        signer: ByteArray = firstDeviceSignerData,
        levels: Set<AttestationResult.SecurityLevel> = setOf(AttestationResult.SecurityLevel.STRONG_BOX),
        verifier: AttestationVerifier = fixture.verifier,
    ): AttestationVerifier.FirstDevicePersistentAppVerificationV1 = verifier.verifyFirstDevicePersistentAppOriginals(
        fixture.attestation, cOriginal, packageName, version, signer, levels,
    )

    private fun firstDeviceApplicationId(
        packageBytes: ByteArray = firstDevicePackage.toByteArray(Charsets.US_ASCII),
        version: BigInteger = BigInteger.ONE,
        signer: ByteArray = firstDeviceSignerData,
        packageCount: Int = 1,
        signerCount: Int = 1,
    ): ByteArray = DERSequence(arrayOf<ASN1Encodable>(
        DERSet(Array<ASN1Encodable>(packageCount) { DERSequence(arrayOf<ASN1Encodable>(DEROctetString(packageBytes), ASN1Integer(version))) }),
        DERSet(Array<ASN1Encodable>(signerCount) { DEROctetString(signer) }),
    )).encoded

    private fun firstDeviceDescription(
        nonce: ByteArray = firstDeviceNonce(),
        applicationId: ByteArray? = firstDeviceApplicationId(),
        hardwareApplicationId: Boolean = false,
        duplicateSoftwareApplicationId: Boolean = false,
        finiteUse: Boolean = false,
        softwarePurpose: Boolean = false,
        hardwarePurpose: Long = 2,
        attestationLevel: Int = 2,
        keyLevel: Int = 2,
    ): ByteArray {
        val software = mutableListOf<ASN1Encodable>()
        if (softwarePurpose) software += DERTaggedObject(true, 1, DERSet(ASN1Integer(2)))
        applicationId?.let {
            software += DERTaggedObject(true, 709, DEROctetString(it))
            if (duplicateSoftwareApplicationId) software += DERTaggedObject(true, 709, DEROctetString(it))
        }
        val hardware = mutableListOf<ASN1Encodable>(
            DERTaggedObject(true, 1, DERSet(ASN1Integer(hardwarePurpose))), DERTaggedObject(true, 2, ASN1Integer(3)),
            DERTaggedObject(true, 3, ASN1Integer(256)), DERTaggedObject(true, 5, DERSet(ASN1Integer(4))),
            DERTaggedObject(true, 10, ASN1Integer(1)),
        )
        if (finiteUse) hardware += DERTaggedObject(true, 405, ASN1Integer(1))
        hardware += DERTaggedObject(true, 702, ASN1Integer(0))
        if (hardwareApplicationId && applicationId != null) hardware += DERTaggedObject(true, 709, DEROctetString(applicationId))
        return DERSequence(arrayOf<ASN1Encodable>(ASN1Integer(100), ASN1Enumerated(attestationLevel),
            ASN1Integer(100), ASN1Enumerated(keyLevel), DEROctetString(nonce), DEROctetString(ByteArray(0)),
            DERSequence(software.toTypedArray()), DERSequence(hardware.toTypedArray()))).encoded
    }
}
