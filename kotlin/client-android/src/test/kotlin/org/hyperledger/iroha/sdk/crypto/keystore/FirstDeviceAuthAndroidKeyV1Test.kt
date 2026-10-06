package org.hyperledger.iroha.sdk.crypto.keystore

import android.security.keystore.KeyProperties
import java.math.BigInteger
import java.security.Key
import java.security.KeyPairGenerator
import java.security.KeyStoreException
import java.security.PrivateKey
import java.security.Signature
import java.security.MessageDigest
import java.security.cert.Certificate
import java.security.spec.ECGenParameterSpec
import java.util.Base64
import java.util.Date
import org.bouncycastle.asn1.x500.X500Name
import org.bouncycastle.asn1.ASN1Encodable
import org.bouncycastle.asn1.ASN1Enumerated
import org.bouncycastle.asn1.ASN1Integer
import org.bouncycastle.asn1.DEROctetString
import org.bouncycastle.asn1.DERSequence
import org.bouncycastle.asn1.DERSet
import org.bouncycastle.asn1.DERTaggedObject
import org.bouncycastle.asn1.ASN1ObjectIdentifier
import org.bouncycastle.cert.jcajce.JcaX509CertificateConverter
import org.bouncycastle.cert.jcajce.JcaX509v3CertificateBuilder
import org.bouncycastle.operator.jcajce.JcaContentSignerBuilder
import org.hyperledger.iroha.sdk.auth.FirstDeviceAuthProtocolV1 as Protocol
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationResult.SecurityLevel
import org.hyperledger.iroha.sdk.json.Json
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

/** Host metadata/probe/signature tests. No fake Android production classes or hardware claims. */
class FirstDeviceAuthAndroidKeyV1Test {
    private fun challenge() = Protocol.Challenge.parse(Json.obj(linkedMapOf(
        "schema" to Json.of("bpng.first-device-auth-challenge.v1"), "version" to Json.of(1),
        "policy_sha256" to Json.of(digest(1)), "operation_id" to Json.of(digest(2)),
        "client_nonce" to Json.of(digest(3)), "server_nonce" to Json.of(digest(4)),
        "alias_digest" to Json.of(digest(5)), "google_owner_binding" to Json.of(digest(6)),
        "google_token_original_sha256" to Json.of(digest(7)), "issued_at_ms" to Json.of(1000),
        "expires_at_ms" to Json.of(2000),
    )).toJsonBytes())

    private class Access(override val apiLevel: Int, private val key: () -> Key?) : FirstDeviceAuthKeyAccessV1 {
        var probes = 0
        override val entries = object : AndroidKeystoreEntriesV1 {
            override val apiLevel = this@Access.apiLevel
            override fun getKey(alias: String): Key? { probes++; return key() }
            override fun getCertificateChain(alias: String): List<Certificate>? = error("Unexpected chain read")
        }
        override fun facts(key: PrivateKey): FirstDeviceAuthKeyFactsV1 = error("Unexpected facts read")
        override fun sign(key: PrivateKey, message: ByteArray): ByteArray = error("Unexpected sign")
    }

    private fun facts(api: Int = 31, hardware: Boolean = true, security: Int? = if (api >= 31) 1 else null,
        usage: Int? = if (api >= 31) -1 else null, origin: Int = KeyProperties.ORIGIN_GENERATED, purpose: Int = KeyProperties.PURPOSE_SIGN,
        digests: Set<String> = setOf("SHA-256"), size: Int = 256, auth: Boolean = false,
        presence: Boolean? = if (api >= 28) false else null, confirmation: Boolean? = if (api >= 28) false else null) =
        FirstDeviceAuthKeyFactsV1(hardware, security, usage, origin, purpose, digests, size, auth, presence, confirmation)

    @Test fun api26NullIsUnknownAndNeverAbsence() {
        for (api in 26..30) {
            val access = Access(api) { null }
            val result = FirstDeviceAuthAndroidKeyV1.openExistingWithAccess(ByteArray(32) { 1 }, challenge(), {}, access)
            assertEquals(FirstDeviceAuthAndroidKeyV1.Reason.KEY_ABSENCE_UNKNOWN,
                (result as FirstDeviceAuthAndroidKeyV1.Lookup.Unavailable).reason)
            assertEquals(1, access.probes)
        }
    }

    @Test fun api31NullIsDefinitiveAbsenceWithoutGeneration() {
        val access = Access(31) { null }
        assertSame(FirstDeviceAuthAndroidKeyV1.Lookup.Absent,
            FirstDeviceAuthAndroidKeyV1.openExistingWithAccess(ByteArray(32) { 2 }, challenge(), {}, access))
        assertEquals(1, access.probes)
    }

    @Test fun providerErrorNeverMeansAbsence() {
        for (api in listOf(26, 31)) {
            val access = Access(api) { throw KeyStoreException("Synthetic provider error") }
            val result = FirstDeviceAuthAndroidKeyV1.openExistingWithAccess(ByteArray(32) { 3 }, challenge(), {}, access)
            assertEquals(FirstDeviceAuthAndroidKeyV1.Reason.KEYSTORE_UNAVAILABLE,
                (result as FirstDeviceAuthAndroidKeyV1.Lookup.Unavailable).reason)
        }
    }

    @Test fun positiveSoftwarePrivateKeyIsUnusableWithoutChainOrReplacement() {
        val pair = keyPair()
        for (api in listOf(26, 31)) {
            val result = FirstDeviceAuthAndroidKeyV1.openExistingWithAccess(ByteArray(32) { 4 }, challenge(), {}, Access(api) { pair.private })
            assertEquals(FirstDeviceAuthAndroidKeyV1.Reason.KEY_UNUSABLE,
                (result as FirstDeviceAuthAndroidKeyV1.Lookup.Unavailable).reason)
        }
    }

    @Test fun ownerChangeDuringLookupCannotPublishAbsence() {
        var owner = true
        val access = Access(31) { owner = false; null }
        assertThrows(IllegalStateException::class.java) {
            FirstDeviceAuthAndroidKeyV1.openExistingWithAccess(ByteArray(32) { 5 }, challenge(), { check(owner) }, access)
        }
    }

    @Test fun authAliasCannotEqualPaymentOrAccountAliasAndIsBoundToEverySlotByte() {
        val slot = ByteArray(32) { 6 }; val alias = firstDeviceAuthAliasV1(slot)
        assertTrue(alias.startsWith("iroha-auth-a1-")); assertFalse(alias.startsWith("kgm-w1-"))
        assertNotEquals(alias, "primary-account")
        for (i in slot.indices) { val changed = slot.copyOf(); changed[i] = 7; assertNotEquals(alias, firstDeviceAuthAliasV1(changed)) }
        assertThrows(IllegalArgumentException::class.java) { firstDeviceAuthAliasV1(ByteArray(32)) }
    }

    @Test fun teeMetadataIsConsistentOnApi26And31WithoutSupplyingAuthority() {
        requireFirstDeviceAuthFactsV1(facts(26), 26, SecurityLevel.TRUSTED_ENVIRONMENT)
        requireFirstDeviceAuthFactsV1(facts(31), 31, SecurityLevel.TRUSTED_ENVIRONMENT)
    }

    @Test fun softwareOrMismatchedHardwareLevelsRefuse() {
        assertThrows(IllegalArgumentException::class.java) { requireFirstDeviceAuthFactsV1(facts(hardware = false), 31, SecurityLevel.TRUSTED_ENVIRONMENT) }
        assertThrows(IllegalArgumentException::class.java) { requireFirstDeviceAuthFactsV1(facts(), 31, SecurityLevel.SOFTWARE) }
        assertThrows(IllegalArgumentException::class.java) { requireFirstDeviceAuthFactsV1(facts(security = 0), 31, SecurityLevel.TRUSTED_ENVIRONMENT) }
        assertThrows(IllegalArgumentException::class.java) { requireFirstDeviceAuthFactsV1(facts(security = 1), 31, SecurityLevel.STRONG_BOX) }
        requireFirstDeviceAuthFactsV1(facts(security = 2), 31, SecurityLevel.STRONG_BOX)
    }

    @Test fun authBindingFiniteUsageOrConfirmationRestrictionsRefuse() {
        for (f in listOf(facts(auth = true), facts(usage = 1), facts(presence = true), facts(confirmation = true)))
            assertThrows(IllegalArgumentException::class.java) { requireFirstDeviceAuthFactsV1(f, 31, SecurityLevel.TRUSTED_ENVIRONMENT) }
    }

    @Test fun wrongKeyShapeOriginPurposeOrDigestRefuses() {
        for (f in listOf(facts(origin = 2), facts(purpose = 8), facts(size = 384), facts(digests = setOf("NONE")), facts(digests = setOf("SHA-256", "SHA-512"))))
            assertThrows(IllegalArgumentException::class.java) { requireFirstDeviceAuthFactsV1(f, 31, SecurityLevel.TRUSTED_ENVIRONMENT) }
    }

    @Test fun unavailableApiSpecificFactsCannotBeInventedOrIgnored() {
        assertThrows(IllegalArgumentException::class.java) { requireFirstDeviceAuthFactsV1(facts(26, security = 1), 26, SecurityLevel.TRUSTED_ENVIRONMENT) }
        assertThrows(IllegalArgumentException::class.java) { requireFirstDeviceAuthFactsV1(facts(26), 26, SecurityLevel.STRONG_BOX) }
        assertThrows(IllegalArgumentException::class.java) { requireFirstDeviceAuthFactsV1(facts(31, security = null), 31, SecurityLevel.TRUSTED_ENVIRONMENT) }
        assertThrows(IllegalArgumentException::class.java) { requireFirstDeviceAuthFactsV1(facts(31, presence = null), 31, SecurityLevel.TRUSTED_ENVIRONMENT) }
    }

    @Test fun fullPossessionMessageSignatureIsVerifiedWithoutCroppingOrRehashingInput() {
        val pair = keyPair(); val leaf = certificate(pair)
        val message = "BPNG.FIRST_DEVICE.AUTH.POSSESSION.V1\u0000".toByteArray() + ByteArray(129) { it.toByte() }
        val der = Signature.getInstance("SHA256withECDSA").run { initSign(pair.private); update(message); sign() }
        verifyFirstDeviceAuthPossessionDerV1(leaf, message, der)
        assertThrows(IllegalArgumentException::class.java) { verifyFirstDeviceAuthPossessionDerV1(leaf, message.copyOf(32), der) }
        for (i in listOf(0, 32, message.lastIndex)) {
            val changed = message.copyOf(); changed[i] = (changed[i].toInt() xor 1).toByte()
            assertThrows(IllegalArgumentException::class.java) { verifyFirstDeviceAuthPossessionDerV1(leaf, changed, der) }
        }
    }

    @Test fun malformedOrWrongKeyPossessionDerCannotPass() {
        val pair = keyPair(); val leaf = certificate(pair); val other = certificate(keyPair())
        val message = ByteArray(164) { it.toByte() }
        val der = Signature.getInstance("SHA256withECDSA").run { initSign(pair.private); update(message); sign() }
        assertThrows(IllegalArgumentException::class.java) { verifyFirstDeviceAuthPossessionDerV1(other, message, der) }
        assertThrows(IllegalArgumentException::class.java) { verifyFirstDeviceAuthPossessionDerV1(leaf, message, der + 0.toByte()) }
    }

    @Test fun positiveOriginalLookupOnApi26And31RetainsFullChainAndSignsFullPossession() {
        // Synthetic KeyInfo and signed KeyDescription DATA; this is no Android hardware verdict.
        for (api in listOf(26, 31)) {
            val c = challenge(); val access = OriginalAccess(api, c)
            val key = (FirstDeviceAuthAndroidKeyV1.openExistingWithAccess(ByteArray(32) { 8 }, c, {}, access)
                as FirstDeviceAuthAndroidKeyV1.Lookup.Present).key
            val chain = key.certificateChainDerBytes()
            assertEquals(2, chain.size)
            for (i in chain.indices) assertArrayEquals(access.chain[i].encoded, chain[i])
            val point = key.publicKeySec1Bytes(); assertEquals(65, point.size)
            chain[0][0] = 0; point[0] = 0
            assertArrayEquals(access.chain[0].encoded, key.certificateChainDerBytes()[0])
            assertEquals(4, key.publicKeySec1Bytes()[0].toInt())
            val raw = raw(c, access.chain)
            val der = key.signPossessionOriginal(raw)
            verifyFirstDeviceAuthPossessionDerV1(access.chain.first(), Protocol.possessionMessageBytes(c, raw), der)
            assertEquals(1, access.signatures)
        }
    }

    @Test fun changedOriginalCertificateChainCannotReuseHeldKey() {
        val c = challenge(); val access = OriginalAccess(31, c)
        val key = (FirstDeviceAuthAndroidKeyV1.openExistingWithAccess(ByteArray(32) { 9 }, c, {}, access)
            as FirstDeviceAuthAndroidKeyV1.Lookup.Present).key
        access.chain = originalChain(access.pair, c) + certificate(keyPair())
        assertThrows(IllegalArgumentException::class.java) { key.certificateChainDerBytes() }
        assertEquals(0, access.signatures)
    }

    @Test fun wrongOriginalAttestationChallengeCannotOpenExistingKey() {
        val c = challenge(); val access = OriginalAccess(31, c)
        access.chain = originalChain(access.pair, c, ByteArray(32) { 42 })
        val lookup = FirstDeviceAuthAndroidKeyV1.openExistingWithAccess(ByteArray(32) { 10 }, c, {}, access)
        assertEquals(FirstDeviceAuthAndroidKeyV1.Reason.KEY_UNUSABLE,
            (lookup as FirstDeviceAuthAndroidKeyV1.Lookup.Unavailable).reason)
        assertEquals(0, access.signatures)
    }

    @Test fun providerSignatureFromAnotherKeyCannotBePublished() {
        val c = challenge(); val access = OriginalAccess(31, c)
        val key = (FirstDeviceAuthAndroidKeyV1.openExistingWithAccess(ByteArray(32) { 11 }, c, {}, access)
            as FirstDeviceAuthAndroidKeyV1.Lookup.Present).key
        access.signingPair = keyPair()
        assertThrows(IllegalArgumentException::class.java) { key.signPossessionOriginal(raw(c, access.chain)) }
        assertEquals(1, access.signatures)
    }

    @Test fun changedVerifierChainDataRefusesBeforeSigning() {
        val c = challenge(); val access = OriginalAccess(31, c)
        val key = (FirstDeviceAuthAndroidKeyV1.openExistingWithAccess(ByteArray(32) { 12 }, c, {}, access)
            as FirstDeviceAuthAndroidKeyV1.Lookup.Present).key
        val other = originalChain(keyPair(), c)
        assertThrows(IllegalArgumentException::class.java) { key.signPossessionOriginal(raw(c, other)) }
        assertEquals(0, access.signatures)
    }

    /** An explicit host-only provider double; genuine software JCA signing, never hardware evidence. */
    private inner class OriginalAccess(override val apiLevel: Int, c: Protocol.Challenge) : FirstDeviceAuthKeyAccessV1 {
        val pair = keyPair()
        var signingPair = pair
        var chain = originalChain(pair, c)
        var signatures = 0
        private val handle = object : PrivateKey {
            override fun getAlgorithm() = "EC"
            override fun getFormat(): String? = null
            override fun getEncoded(): ByteArray? = null
        }
        override val entries = object : AndroidKeystoreEntriesV1 {
            override val apiLevel = this@OriginalAccess.apiLevel
            override fun getKey(alias: String): Key = handle
            override fun getCertificateChain(alias: String): List<Certificate> = chain.toList()
        }
        override fun facts(key: PrivateKey): FirstDeviceAuthKeyFactsV1 = facts(apiLevel)
        override fun sign(key: PrivateKey, message: ByteArray): ByteArray {
            signatures++
            return Signature.getInstance("SHA256withECDSA").run {
                initSign(signingPair.private); update(message); sign()
            }
        }
    }

    private fun originalChain(pair: java.security.KeyPair, c: Protocol.Challenge,
        attestationChallenge: ByteArray = c.attestationChallengeBytes()): List<java.security.cert.X509Certificate> {
        val hardware = DERSequence(arrayOf<ASN1Encodable>(
            DERTaggedObject(true, 1, DERSet(ASN1Integer(2))),
            DERTaggedObject(true, 2, ASN1Integer(3)),
            DERTaggedObject(true, 3, ASN1Integer(256)),
            DERTaggedObject(true, 5, DERSet(ASN1Integer(4))),
            DERTaggedObject(true, 10, ASN1Integer(1)),
            DERTaggedObject(true, 702, ASN1Integer(0)),
        ))
        val description = DERSequence(arrayOf<ASN1Encodable>(ASN1Integer(3), ASN1Enumerated(1),
            ASN1Integer(41), ASN1Enumerated(1), DEROctetString(attestationChallenge),
            DEROctetString(ByteArray(0)), DERSequence(), hardware))
        val name = X500Name("CN=Synthetic original attestation DATA only")
        val builder = JcaX509v3CertificateBuilder(name, BigInteger.ONE, Date(0), Date(2000000000000), name, pair.public)
        builder.addExtension(ASN1ObjectIdentifier("1.3.6.1.4.1.11129.2.1.17"), false, description)
        val leaf = JcaX509CertificateConverter().getCertificate(builder.build(JcaContentSignerBuilder("SHA256withECDSA").build(pair.private)))
        return listOf(leaf, certificate(keyPair()))
    }

    private fun raw(c: Protocol.Challenge, chain: List<java.security.cert.X509Certificate>) = Protocol.RawOriginal.parse(
        Json.obj(linkedMapOf("schema" to Json.of("bpng.first-device-auth-raw.v1"), "version" to Json.of(1),
            "config_sha256" to Json.of(digest(8)), "challenge_digest" to Json.of(sha(c.transcriptBytes())),
            "raw_request_sha256" to Json.of(digest(9)),
            "app_public_key_sec1_base64" to Json.of(Base64.getEncoder().encodeToString(
                org.hyperledger.iroha.sdk.crypto.keystore.attestation.AndroidKeyAttestationOriginalV1.publicKeySec1(chain))),
            "security_level" to Json.of(1), "checked_at_ms" to Json.of(1500),
            "original_chain_base64" to Json.array(chain.map { Json.of(Base64.getEncoder().encodeToString(it.encoded)) }))).toJsonBytes())

    private fun sha(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes)
        .joinToString("") { "%02x".format(it.toInt() and 255) }

    private fun keyPair() = KeyPairGenerator.getInstance("EC").run { initialize(ECGenParameterSpec("secp256r1")); generateKeyPair() }
    private fun certificate(pair: java.security.KeyPair): java.security.cert.X509Certificate {
        val name = X500Name("CN=Host structural test only")
        val builder = JcaX509v3CertificateBuilder(name, BigInteger.ONE, Date(0), Date(2000000000000), name, pair.public)
        return JcaX509CertificateConverter().getCertificate(builder.build(JcaContentSignerBuilder("SHA256withECDSA").build(pair.private)))
    }
    private fun digest(byte: Int) = "%02x".format(byte).repeat(32)
}
