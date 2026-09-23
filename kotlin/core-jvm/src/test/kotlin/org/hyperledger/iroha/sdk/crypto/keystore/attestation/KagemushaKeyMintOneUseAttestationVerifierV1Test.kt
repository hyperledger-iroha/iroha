// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.crypto.keystore.attestation

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.MessageDigest
import java.security.Signature
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import org.bouncycastle.asn1.ASN1Boolean
import org.bouncycastle.asn1.ASN1Encodable
import org.bouncycastle.asn1.ASN1Enumerated
import org.bouncycastle.asn1.ASN1Integer
import org.bouncycastle.asn1.ASN1UTCTime
import org.bouncycastle.asn1.DERBitString
import org.bouncycastle.asn1.DERNull
import org.bouncycastle.asn1.DEROctetString
import org.bouncycastle.asn1.DERSequence
import org.bouncycastle.asn1.DERSet
import org.bouncycastle.asn1.DERTaggedObject
import org.bouncycastle.asn1.x500.X500Name
import org.bouncycastle.asn1.x509.AlgorithmIdentifier
import org.bouncycastle.asn1.x509.BasicConstraints
import org.bouncycastle.asn1.x509.Extension
import org.bouncycastle.asn1.x509.Extensions
import org.bouncycastle.asn1.x509.KeyUsage
import org.bouncycastle.asn1.x509.SubjectPublicKeyInfo
import org.bouncycastle.asn1.x9.X9ObjectIdentifiers
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test

class KagemushaKeyMintOneUseAttestationVerifierV1Test {
    @Test
    fun acceptsPinnedHardwareOneUseAndExactCoreSelection() {
        val fixture = fixture()
        val result = fixture.verifier.verify(fixture.expected, fixture.raw)
        assertEquals(AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT, result.attestationSecurityLevel)
        assertEquals(32, result.signatureSha256().size)
    }

    @Test
    fun softwareEnforcedOneUseIsRejectedEvenWithAValidCertificateChain() {
        val fixture = fixture(hardwareUsage = false, softwareUsage = true)
        rejects(fixture)
    }

    @Test
    fun softwareEnforcedRollbackTagIsRejectedEvenWithAValidCertificateChain() {
        val fixture = fixture(hardwareRollback = false, softwareRollback = true)
        rejects(fixture)
    }

    @Test
    fun duplicateCriticalTagInSoftwareAndHardwareIsRejected() {
        val fixture = fixture(softwareUsage = true)
        rejects(fixture)
    }

    @Test
    fun alteredAppSignerOrUnlockedBootIsRejected() {
        rejects(fixture(attestedSignerDigest = ByteArray(32) { 0x33 }))
        rejects(fixture(lockedBoot = false))
    }

    @Test
    fun unsupportedAuthorizationAndSoftwareSecurityLevelAreRejected() {
        rejects(fixture(unsupportedHardwareTag = true))
        rejects(fixture(securityLevel = 0))
    }

    @Test
    fun nonceLaneIndexPublicKeyAndSignatureAreBoundExactly() {
        val fixture = fixture()
        val changedNonce = fixture.raw.copy(nonce = ByteArray(32) { 0x44 })
        assertThrows(AttestationVerificationException::class.java) {
            fixture.verifier.verify(fixture.expected, changedNonce)
        }
        val changedLane = fixture.raw.copy(lane = ByteArray(32) { 0x45 })
        assertThrows(AttestationVerificationException::class.java) {
            fixture.verifier.verify(fixture.expected, changedLane)
        }
        val changedIndex = fixture.raw.copy(after = ByteArray(16) { 0x46 })
        assertThrows(AttestationVerificationException::class.java) {
            fixture.verifier.verify(fixture.expected, changedIndex)
        }
        val changedKey = fixture.raw.copy(publicKey = ByteArray(65) { 0x47 })
        assertThrows(AttestationVerificationException::class.java) {
            fixture.verifier.verify(fixture.expected, changedKey)
        }
        val changedSignature = fixture.raw.copy(signature = ByteArray(70) { 0x48 })
        assertThrows(AttestationVerificationException::class.java) {
            fixture.verifier.verify(fixture.expected, changedSignature)
        }
    }

    @Test
    fun unpinnedRootAndNonNextIndexAreRejected() {
        val fixture = fixture()
        val other = fixture()
        val wrongTrust = verifier(other.rootDer)
        assertThrows(AttestationVerificationException::class.java) {
            wrongTrust.verify(fixture.expected, fixture.raw)
        }
        assertThrows(IllegalArgumentException::class.java) {
            KagemushaKeyMintPinnedRootV1(fixture.rootDer, ByteArray(32) { 0x55 })
        }
        assertThrows(IllegalArgumentException::class.java) {
            KagemushaKeyMintExpectedSelectionV1(
                FRAME, LANE, ByteArray(16) { 0xff.toByte() }, ByteArray(16), NONCE,
                fixture.expected.committedPublicKeySec1(),
            )
        }
    }

    @Test
    fun longLivedVerifierRejectsStaleSnapshotAndTrustedTimeRollback() {
        val fixture = fixture()
        var now = EVALUATION_TIME
        val verifier = verifier(fixture.rootDer) { now }
        verifier.verify(fixture.expected, fixture.raw)
        now += 86_400_000L
        assertThrows(AttestationVerificationException::class.java) {
            verifier.verify(fixture.expected, fixture.raw)
        }
        now = EVALUATION_TIME - 1L
        assertThrows(AttestationVerificationException::class.java) {
            verifier.verify(fixture.expected, fixture.raw)
        }
    }

    private fun rejects(fixture: Fixture) {
        assertThrows(AttestationVerificationException::class.java) {
            fixture.verifier.verify(fixture.expected, fixture.raw)
        }
    }

    private fun fixture(
        hardwareUsage: Boolean = true,
        softwareUsage: Boolean = false,
        hardwareRollback: Boolean = true,
        softwareRollback: Boolean = false,
        attestedSignerDigest: ByteArray = APP_SIGNER,
        lockedBoot: Boolean = true,
        securityLevel: Int = 1,
        unsupportedHardwareTag: Boolean = false,
    ): Fixture {
        val root = ecKeyPair()
        val leaf = ecKeyPair()
        val challenge = preparedChallengeV1(NONCE, LANE, BEFORE, AFTER)
        val appId = DERSequence(arrayOf(
            DERSet(DERSequence(arrayOf(
                DEROctetString(APP_PACKAGE.toByteArray(StandardCharsets.UTF_8)),
                ASN1Integer(APP_VERSION),
            ))),
            DERSet(DEROctetString(attestedSignerDigest)),
        )).encoded
        val software = mutableListOf<Pair<Int, ASN1Encodable>>()
        if (softwareRollback) software += 303 to DERNull.INSTANCE
        if (softwareUsage) software += 405 to ASN1Integer(1)
        software += 709 to DEROctetString(appId)
        val hardware = mutableListOf<Pair<Int, ASN1Encodable>>(
            1 to DERSet(ASN1Integer(2)),
            2 to ASN1Integer(3),
            3 to ASN1Integer(256),
            5 to DERSet(ASN1Integer(4)),
            10 to ASN1Integer(1),
        )
        if (hardwareRollback) hardware += 303 to DERNull.INSTANCE
        if (hardwareUsage) hardware += 405 to ASN1Integer(1)
        hardware += 702 to ASN1Integer(0)
        hardware += 704 to DERSequence(arrayOf(
            DEROctetString(ByteArray(32) { 0x11 }), ASN1Boolean.getInstance(lockedBoot),
            ASN1Enumerated(0), DEROctetString(ByteArray(32) { 0x22 }),
        ))
        if (unsupportedHardwareTag) hardware += 999 to ASN1Integer(1)
        val keyDescription = DERSequence(arrayOf(
            ASN1Integer(100), ASN1Enumerated(securityLevel), ASN1Integer(100),
            ASN1Enumerated(securityLevel), DEROctetString(challenge), DEROctetString(ByteArray(0)),
            authorizationList(software), authorizationList(hardware),
        )).encoded
        val rootDer = certificate(root, root, true, null)
        val leafDer = certificate(leaf, root, false, keyDescription)
        val key = sec1(leaf.public as ECPublicKey)
        val expected = KagemushaKeyMintExpectedSelectionV1(FRAME, LANE, BEFORE, AFTER, NONCE, key)
        val signed = Signature.getInstance("SHA256withECDSA").apply {
            initSign(leaf.private)
            update(FRAME)
        }.sign()
        val raw = KagemushaKeyMintRawSelectionEvidenceV1(
            FRAME, LANE, BEFORE, AFTER, NONCE, challenge, key, listOf(leafDer, rootDer), signed,
        )
        return Fixture(rootDer, expected, raw, verifier(rootDer))
    }

    private fun verifier(
        rootDer: ByteArray,
        trustedTime: () -> Long = { EVALUATION_TIME },
    ): KagemushaKeyMintOneUseAttestationVerifierV1 =
        KagemushaKeyMintOneUseAttestationVerifierV1(
            listOf(KagemushaKeyMintPinnedRootV1(rootDer, sha256(rootDer))),
            KagemushaKeyMintAppPinV1(APP_PACKAGE, APP_VERSION, listOf(APP_SIGNER)),
            revocationPolicy(),
            trustedTime,
        )

    private fun authorizationList(entries: List<Pair<Int, ASN1Encodable>>): DERSequence =
        DERSequence(entries.sortedBy { it.first }.map { (tag, value) ->
            DERTaggedObject(true, tag, value)
        }.toTypedArray())

    private fun ecKeyPair(): KeyPair = KeyPairGenerator.getInstance("EC").apply {
        initialize(ECGenParameterSpec("secp256r1"))
    }.generateKeyPair()

    private fun certificate(subject: KeyPair, issuer: KeyPair, isRoot: Boolean, extension: ByteArray?): ByteArray {
        val signatureAlgorithm = AlgorithmIdentifier(X9ObjectIdentifiers.ecdsa_with_SHA256)
        val name = X500Name(if (isRoot) "CN=Kagemusha Test Root" else "CN=Kagemusha Test KeyMint Leaf")
        val issuerName = X500Name("CN=Kagemusha Test Root")
        val extensions = mutableListOf(
            Extension(Extension.basicConstraints, true,
                DEROctetString(BasicConstraints(isRoot).encoded)),
            Extension(Extension.keyUsage, true, DEROctetString(
                KeyUsage(if (isRoot) KeyUsage.keyCertSign else KeyUsage.digitalSignature).encoded)),
        )
        if (extension != null) {
            extensions += Extension(KEYMINT_OID, false, DEROctetString(extension))
        }
        val tbs = DERSequence(arrayOf<ASN1Encodable>(
            DERTaggedObject(true, 0, ASN1Integer(2)),
            ASN1Integer(if (isRoot) 1 else 2),
            signatureAlgorithm,
            issuerName,
            DERSequence(arrayOf(ASN1UTCTime("250101000000Z"), ASN1UTCTime("300101000000Z"))),
            name,
            SubjectPublicKeyInfo.getInstance(subject.public.encoded),
            DERTaggedObject(true, 3, Extensions(extensions.toTypedArray())),
        ))
        val signature = Signature.getInstance("SHA256withECDSA").apply {
            initSign(issuer.private)
            update(tbs.encoded)
        }.sign()
        return DERSequence(arrayOf(tbs, signatureAlgorithm, DERBitString(signature))).encoded
    }

    private fun sec1(key: ECPublicKey): ByteArray = byteArrayOf(0x04) +
        coordinate(key.w.affineX) + coordinate(key.w.affineY)

    private fun coordinate(value: BigInteger): ByteArray {
        val bytes = value.toByteArray()
        val unsigned = if (bytes.size == 33) bytes.copyOfRange(1, 33) else bytes
        return ByteArray(32 - unsigned.size) + unsigned
    }

    private fun revocationPolicy(): AndroidAttestationRevocationPolicyV1 {
        val snapshot = buildString {
            append(AndroidAttestationRevocationPolicyV1.SNAPSHOT_DOMAIN).append('\n')
            append("payload_sha256=").append("11".repeat(32)).append('\n')
            append("response_date_ms=").append(EVALUATION_TIME).append('\n')
            append("last_modified_ms=-\n")
            append("cache_max_age_seconds=86400\n")
            append("serial_count=0\n")
            append("tbs_sha256_count=0\n")
        }.toByteArray(StandardCharsets.US_ASCII)
        return AndroidAttestationRevocationPolicyV1.fromCanonicalSnapshot(snapshot, sha256(snapshot))
    }

    private fun sha256(value: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(value)

    private data class Fixture(
        val rootDer: ByteArray,
        val expected: KagemushaKeyMintExpectedSelectionV1,
        val raw: KagemushaKeyMintRawSelectionEvidenceV1,
        val verifier: KagemushaKeyMintOneUseAttestationVerifierV1,
    )

    private fun KagemushaKeyMintRawSelectionEvidenceV1.copy(
        lane: ByteArray = laneCommitment(),
        after: ByteArray = secureIndexAfterLittleEndian(),
        nonce: ByteArray = attestationNonce(),
        publicKey: ByteArray = publicKeySec1(),
        signature: ByteArray = signatureDer(),
    ) = KagemushaKeyMintRawSelectionEvidenceV1(
        canonicalSelectionFrame(), lane, secureIndexBeforeLittleEndian(), after, nonce,
        attestationChallenge(), publicKey, certificateChainDer(), signature,
    )

    companion object {
        private const val APP_PACKAGE = "org.hyperledger.iroha.wallet"
        private const val APP_VERSION = 7L
        private const val EVALUATION_TIME = 1764547200000L
        private val KEYMINT_OID = org.bouncycastle.asn1.ASN1ObjectIdentifier("1.3.6.1.4.1.11129.2.1.17")
        private val FRAME = "core-canonical-selection-v1".toByteArray(StandardCharsets.UTF_8)
        private val LANE = ByteArray(32) { 0x21 }
        private val BEFORE = ByteArray(16)
        private val AFTER = ByteArray(16).also { it[0] = 1 }
        private val NONCE = ByteArray(32) { 0x37 }
        private val APP_SIGNER = ByteArray(32) { 0x66 }
    }
}
