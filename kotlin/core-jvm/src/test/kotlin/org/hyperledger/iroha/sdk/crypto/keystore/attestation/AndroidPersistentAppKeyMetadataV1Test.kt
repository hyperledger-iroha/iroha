// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.crypto.keystore.attestation

import java.io.ByteArrayInputStream
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.Signature
import java.security.cert.CertificateFactory
import java.security.cert.X509Certificate
import java.security.spec.ECGenParameterSpec
import org.bouncycastle.asn1.*
import org.bouncycastle.asn1.x500.X500Name
import org.bouncycastle.asn1.x509.*
import org.bouncycastle.asn1.x9.X9ObjectIdentifiers
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertFails

/** Synthetic signed metadata checks only; no device, root trust, issuer or native admission. */
class AndroidPersistentAppKeyMetadataV1Test {
    private val challenge = ByteArray(32) { 7 }
    @Test fun `exact original TEE and StrongBox metadata remain usable without finite usage metadata`() {
        for ((level, expected) in listOf(1 to AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT,
            2 to AttestationResult.SecurityLevel.STRONG_BOX)) {
            assertEquals(expected, AndroidKeyAttestationOriginalV1.persistentAppHardwareSecurityLevel(fixture(description(level)), challenge))
        }
    }
    @Test fun `unknown software or mismatched attestation hardware levels fail`() {
        for ((attestation, key) in listOf(0 to 0, 3 to 3, 2 to 1, 1 to 2, 0 to 2)) {
            assertFails { AndroidKeyAttestationOriginalV1.persistentAppHardwareSecurityLevel(fixture(description(attestation, key)), challenge) }
        }
    }
    @Test fun `finite tag405 in hardware or software is not a persistent identity key`() {
        for (hardware in listOf(true, false)) {
            val original = description(1, finiteHardware = hardware, finiteSoftware = !hardware)
            assertFails { AndroidKeyAttestationOriginalV1.persistentAppHardwareSecurityLevel(fixture(original), challenge) }
        }
    }
    @Test fun `recovery remains exact challenge leaf point and root nearest original`() {
        val chain = fixture(description(1), leaf = description(0))
        assertEquals(AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT,
            AndroidKeyAttestationOriginalV1.persistentAppHardwareSecurityLevel(chain, challenge))
        assertFails { AndroidKeyAttestationOriginalV1.persistentAppHardwareSecurityLevel(chain, ByteArray(32) { 8 }) }
        assertFails { AndroidKeyAttestationOriginalV1.persistentAppHardwareSecurityLevel(fixture(description(1), differentOriginalKey = true), challenge) }
        assertFails { AndroidKeyAttestationOriginalV1.persistentAppHardwareSecurityLevel(fixture(byteArrayOf(0)), challenge) }
    }
    @Test fun `software enforced duplicated or wrong hardware purpose curve and origin are refused`() {
        for (original in listOf(description(1, softwareSign = true), description(1, duplicate = true),
            description(1, purpose = 3), description(1, curve = 2), description(1, origin = 2),
            description(1, digest = 2), description(1, keySize = 384))) {
            assertFails { AndroidKeyAttestationOriginalV1.persistentAppHardwareSecurityLevel(fixture(original), challenge) }
        }
    }
    @Test fun `missing or truncated authorization lists cannot be replaced by a leaf description`() {
        for (original in listOf(description(1, missingOrigin = true), description(1).copyOfRange(0, 20))) {
            assertFails { AndroidKeyAttestationOriginalV1.persistentAppHardwareSecurityLevel(fixture(original, description(1)), challenge) }
        }
    }
    private fun description(attestation: Int, key: Int = attestation, finiteHardware: Boolean = false,
        finiteSoftware: Boolean = false, softwareSign: Boolean = false, duplicate: Boolean = false,
        purpose: Int = 2, digest: Int = 4, keySize: Int = 256, curve: Int = 1, origin: Int = 0,
        missingOrigin: Boolean = false): ByteArray {
        fun tag(number: Int, value: ASN1Encodable) = DERTaggedObject(true, number, value)
        val hardware = mutableListOf<ASN1Encodable>(tag(1, DERSet(ASN1Integer(purpose.toLong()))), tag(2, ASN1Integer(3)),
            tag(3, ASN1Integer(keySize.toLong())), tag(5, DERSet(ASN1Integer(digest.toLong()))), tag(10, ASN1Integer(curve.toLong())))
        if (duplicate) hardware.add(1, tag(1, DERSet(ASN1Integer(purpose.toLong()))))
        if (finiteHardware) hardware += tag(405, ASN1Integer(1))
        if (!missingOrigin) hardware += tag(702, ASN1Integer(origin.toLong()))
        val software = mutableListOf<ASN1Encodable>()
        if (softwareSign) software += tag(1, DERSet(ASN1Integer(2)))
        if (finiteSoftware) software += tag(405, ASN1Integer(1))
        return DERSequence(arrayOf(ASN1Integer(100), ASN1Enumerated(attestation), ASN1Integer(100), ASN1Enumerated(key),
            DEROctetString(challenge), DEROctetString(ByteArray(0)), DERSequence(software.toTypedArray()), DERSequence(hardware.toTypedArray()))).encoded
    }
    private fun fixture(original: ByteArray, leaf: ByteArray? = null, differentOriginalKey: Boolean = false): List<X509Certificate> {
        val root = key(); val alias = key(); val attested = if (differentOriginalKey) key() else alias
        return listOf(certificate(alias, attested, "CN=Alias", "CN=Original", false, 3, leaf),
            certificate(attested, root, "CN=Original", "CN=Root", true, 2, original),
            certificate(root, root, "CN=Root", "CN=Root", true, 1, null))
    }
    private fun key() = KeyPairGenerator.getInstance("EC").apply { initialize(ECGenParameterSpec("secp256r1")) }.generateKeyPair()
    private fun certificate(subject: KeyPair, issuer: KeyPair, subjectName: String, issuerName: String,
        ca: Boolean, serial: Long, description: ByteArray?): X509Certificate {
        val algorithm = AlgorithmIdentifier(X9ObjectIdentifiers.ecdsa_with_SHA256)
        val extensions = mutableListOf(Extension(Extension.basicConstraints, true, DEROctetString(BasicConstraints(ca).encoded)),
            Extension(Extension.keyUsage, true, DEROctetString(KeyUsage(if (ca) KeyUsage.keyCertSign else KeyUsage.digitalSignature).encoded)))
        description?.let { extensions += Extension(ASN1ObjectIdentifier(AndroidKeyAttestationOriginalV1.KEY_DESCRIPTION_OID), false, DEROctetString(it)) }
        val tbs = DERSequence(arrayOf<ASN1Encodable>(DERTaggedObject(true, 0, ASN1Integer(2)), ASN1Integer(serial), algorithm,
            X500Name(issuerName), DERSequence(arrayOf(ASN1UTCTime("250101000000Z"), ASN1UTCTime("300101000000Z"))), X500Name(subjectName),
            SubjectPublicKeyInfo.getInstance(subject.public.encoded), DERTaggedObject(true, 3, Extensions(extensions.toTypedArray()))))
        val signature = Signature.getInstance("SHA256withECDSA").run { initSign(issuer.private); update(tbs.encoded); sign() }
        val bytes = DERSequence(arrayOf(tbs, algorithm, DERBitString(signature))).encoded
        return CertificateFactory.getInstance("X.509").generateCertificate(ByteArrayInputStream(bytes)) as X509Certificate
    }
}
