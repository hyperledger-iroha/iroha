// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.crypto.keystore

import android.security.keystore.KeyProperties
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.security.Key
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.KeyStoreException
import java.security.PrivateKey
import java.security.PublicKey
import java.security.Signature
import java.security.cert.Certificate
import java.security.cert.CertificateFactory
import java.security.cert.X509Certificate
import java.security.spec.ECGenParameterSpec

/** A Keystore-like private key: usable for signing through its owner, never exportable. */
internal class TestKeystorePrivateKeyV1(val delegate: PrivateKey) : PrivateKey {
    override fun getAlgorithm(): String = delegate.algorithm
    override fun getFormat(): String? = null
    override fun getEncoded(): ByteArray? = null
}

/** Independent test encoding of a P-256 key: the 65-byte point that ends its X.509 SPKI. */
internal fun testKeystoreSec1V1(publicKey: PublicKey): ByteArray {
    val spki = publicKey.encoded
    return spki.copyOfRange(spki.size - 65, spki.size).also { check(it[0] == 4.toByte()) }
}

/**
 * AndroidKeyStore (keystore2) model with error injection. Generating under an occupied alias
 * replaces the entry and loses the old key, as keystore2 `rebind_alias` does, so a test fails if an
 * adapter ever generates over an existing key. Generated and seeded keys carry a synthetic
 * attestation chain whose original `KeyDescription` passes the persistent app-key parser.
 */
internal class TestAndroidKeystoreV1(override var apiLevel: Int = 31) : AndroidKeystoreV1 {
    class Entry(
        val pair: KeyPair,
        val key: TestKeystorePrivateKeyV1,
        val chain: List<X509Certificate>,
        val facts: AndroidKeystoreKeyFactsV1,
        val request: AndroidKeystoreEcKeyRequestV1?,
    )

    private val root = TestKeystoreAttestationV1.p256()
    private var serial = 2L
    val entries = LinkedHashMap<String, Entry>()
    val generated = ArrayList<AndroidKeystoreEcKeyRequestV1>()
    var getKeyCalls = 0
    var signCalls = 0
    var deleteCalls = 0

    /** Thrown by every `getKey`, as keystore2 does for a binder or daemon failure. */
    var getKeyFailure: Throwable? = null

    /** `getKey` starts throwing once this many generations were attempted. */
    var getKeyFailureAfterGenerations: Int = Int.MAX_VALUE
    var strongBoxAvailable = true

    /** A StrongBox attempt that reports unavailability but binds a key anyway. */
    var strongBoxFailureLeavesKey = false
    var deleteFailure: Throwable? = null

    /** Seed an entry no adapter generated in this test (for example from an earlier process). */
    fun seed(alias: String, challenge: ByteArray = ByteArray(32) { 9 }, strongBox: Boolean = false): Entry =
        newEntry(challenge, strongBox, null).also { entries[alias] = it }

    private fun newEntry(challenge: ByteArray, strongBox: Boolean, request: AndroidKeystoreEcKeyRequestV1?): Entry {
        val pair = TestKeystoreAttestationV1.p256()
        val level = if (strongBox) KeyProperties.SECURITY_LEVEL_STRONGBOX else KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT
        val facts = AndroidKeystoreKeyFactsV1(
            insideSecureHardware = true,
            securityLevel = level,
            remainingUsageCount = request?.maxUsageCount ?: KeyProperties.UNRESTRICTED_USAGE_COUNT,
            origin = KeyProperties.ORIGIN_GENERATED,
            purposes = KeyProperties.PURPOSE_SIGN,
            digests = setOf(KeyProperties.DIGEST_SHA256),
        )
        val chain = TestKeystoreAttestationV1.chain(pair.public, root, challenge, serial++, level)
        return Entry(pair, TestKeystorePrivateKeyV1(pair.private), chain, facts, request)
    }

    override fun getKey(alias: String): Key? {
        getKeyCalls += 1
        if (generated.size >= getKeyFailureAfterGenerations) throw KeyStoreException("keystore2 binder failure")
        getKeyFailure?.let { throw it }
        return entries[alias]?.key
    }

    override fun getCertificateChain(alias: String): List<Certificate>? = entries[alias]?.chain

    override fun generate(request: AndroidKeystoreEcKeyRequestV1): PublicKey {
        generated += request
        if (request.strongBox && !strongBoxAvailable) {
            if (strongBoxFailureLeavesKey) entries[request.alias] = newEntry(request.challenge(), false, request)
            throw AndroidKeystoreStrongBoxUnavailableV1(null)
        }
        val entry = newEntry(request.challenge(), request.strongBox, request)
        entries[request.alias] = entry // rebind_alias: any previous key under the alias is lost.
        return entry.pair.public
    }

    override fun facts(key: PrivateKey): AndroidKeystoreKeyFactsV1 = entries.values.first { it.key === key }.facts

    override fun sign(key: PrivateKey, message: ByteArray): ByteArray {
        signCalls += 1
        return Signature.getInstance("SHA256withECDSA").run {
            initSign((key as TestKeystorePrivateKeyV1).delegate)
            update(message)
            sign()
        }
    }

    override fun deleteEntry(alias: String) {
        deleteCalls += 1
        deleteFailure?.let { throw it }
        entries.remove(alias)
    }
}

/** Synthetic attestation certificates carrying an original `KeyDescription` (JDK 8 APIs only). */
internal object TestKeystoreAttestationV1 {
    private const val KEY_DESCRIPTION_OID = "1.3.6.1.4.1.11129.2.1.17"
    private const val ECDSA_SHA256_OID = "1.2.840.10045.4.3.2"

    fun p256(): KeyPair = KeyPairGenerator.getInstance("EC").run {
        initialize(ECGenParameterSpec("secp256r1"))
        generateKeyPair()
    }

    /** Leaf first: the attested key at [level] (1 TEE, 2 StrongBox), then the self-signed root. */
    fun chain(subject: PublicKey, root: KeyPair, challenge: ByteArray, serial: Long, level: Int): List<X509Certificate> {
        // Hardware-enforced SIGN, EC, 256 bits, SHA-256, P-256, rollback resistance and GENERATED.
        val hardware = sequence(
            explicit(1, set(integer(2))),
            explicit(2, integer(3)),
            explicit(3, integer(256)),
            explicit(5, set(integer(4))),
            explicit(10, integer(1)),
            explicit(503, tlv(0x05, ByteArray(0))),
            explicit(702, integer(0)),
        )
        val keyDescription = sequence(
            integer(200), enumerated(level), integer(200), enumerated(level),
            octets(challenge), octets(ByteArray(0)), sequence(), hardware,
        )
        val extension = sequence(oid(KEY_DESCRIPTION_OID), octets(keyDescription))
        val leaf = certificate(serial, "attested key", "attestation root", subject, root.private, listOf(extension))
        val rootCertificate = certificate(1, "attestation root", "attestation root", root.public, root.private, emptyList())
        return listOf(leaf, rootCertificate)
    }

    private fun certificate(
        serial: Long,
        subject: String,
        issuer: String,
        publicKey: PublicKey,
        signer: PrivateKey,
        extensions: List<ByteArray>,
    ): X509Certificate {
        val algorithm = sequence(oid(ECDSA_SHA256_OID))
        val parts = arrayListOf(
            explicit(0, integer(2)), integer(serial), algorithm, name(issuer),
            sequence(tlv(0x17, "260101000000Z".toByteArray(Charsets.US_ASCII)),
                tlv(0x17, "491231235959Z".toByteArray(Charsets.US_ASCII))),
            name(subject), publicKey.encoded,
        )
        if (extensions.isNotEmpty()) parts.add(explicit(3, sequence(*extensions.toTypedArray())))
        val tbs = sequence(*parts.toTypedArray())
        val signature = Signature.getInstance("SHA256withECDSA").run {
            initSign(signer)
            update(tbs)
            sign()
        }
        val der = sequence(tbs, algorithm, tlv(0x03, byteArrayOf(0) + signature))
        return CertificateFactory.getInstance("X.509").generateCertificate(ByteArrayInputStream(der)) as X509Certificate
    }

    private fun name(commonName: String): ByteArray =
        sequence(set(sequence(oid("2.5.4.3"), tlv(0x0c, commonName.toByteArray(Charsets.UTF_8)))))

    private fun tlv(tag: Int, content: ByteArray): ByteArray = tlv(byteArrayOf(tag.toByte()), content)

    private fun tlv(identifier: ByteArray, content: ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(identifier)
        val size = content.size
        when {
            size < 0x80 -> out.write(size)
            size < 0x100 -> { out.write(0x81); out.write(size) }
            else -> { out.write(0x82); out.write(size ushr 8); out.write(size and 0xff) }
        }
        out.write(content)
        return out.toByteArray()
    }

    private fun sequence(vararg items: ByteArray): ByteArray = tlv(0x30, concat(items))
    private fun set(vararg items: ByteArray): ByteArray = tlv(0x31, concat(items))
    private fun integer(value: Long): ByteArray = tlv(0x02, BigInteger.valueOf(value).toByteArray())
    private fun enumerated(value: Int): ByteArray = tlv(0x0a, BigInteger.valueOf(value.toLong()).toByteArray())
    private fun octets(value: ByteArray): ByteArray = tlv(0x04, value)

    /** Constructed context-specific `[tagNumber] EXPLICIT`, with the high-tag form above 30. */
    private fun explicit(tagNumber: Int, content: ByteArray): ByteArray {
        if (tagNumber < 31) return tlv(0xa0 or tagNumber, content)
        val groups = ArrayList<Int>()
        var rest = tagNumber
        do {
            groups.add(rest and 0x7f)
            rest = rest ushr 7
        } while (rest != 0)
        val identifier = ByteArrayOutputStream()
        identifier.write(0xbf)
        for (index in groups.indices.reversed()) identifier.write(groups[index] or (if (index == 0) 0 else 0x80))
        return tlv(identifier.toByteArray(), content)
    }

    private fun oid(dotted: String): ByteArray {
        val arcs = dotted.split('.').map { it.toLong() }
        val out = ByteArrayOutputStream()
        out.write((arcs[0] * 40 + arcs[1]).toInt())
        for (arc in arcs.drop(2)) {
            val groups = ArrayList<Int>()
            var rest = arc
            do {
                groups.add((rest and 0x7f).toInt())
                rest = rest ushr 7
            } while (rest != 0L)
            for (index in groups.indices.reversed()) out.write(groups[index] or (if (index == 0) 0 else 0x80))
        }
        return tlv(0x06, out.toByteArray())
    }

    private fun concat(items: Array<out ByteArray>): ByteArray {
        val out = ByteArrayOutputStream()
        items.forEach { out.write(it) }
        return out.toByteArray()
    }
}
