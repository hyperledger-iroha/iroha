package org.hyperledger.iroha.sdk.musubi

import java.nio.file.Files
import java.nio.file.Paths
import java.util.Collections
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys

/** Immutable JVM list snapshots preserve validated Musubi public values and proof commitments. */
class MusubiImmutableSnapshotsTest {
    @Test
    fun `immutable snapshots copy inputs and refuse Java list mutation at every size`() {
        val mutations: List<Pair<String, (MutableList<String>) -> Unit>> = listOf(
            "clear" to { values -> values.clear() },
            "add" to { values -> values.add("changed"); Unit },
            "set" to { values -> values[0] = "changed" },
            "remove index" to { values -> values.removeAt(0); Unit },
            "remove value" to { values -> values.remove("entry-0"); Unit },
            "iterator remove" to { values ->
                val iterator = values.iterator()
                if (iterator.hasNext()) iterator.next()
                iterator.remove()
            },
            "list iterator add" to { values -> values.listIterator().add("changed") },
            "list iterator set" to { values ->
                val iterator = values.listIterator()
                if (iterator.hasNext()) iterator.next()
                iterator.set("changed")
            },
            "list iterator remove" to { values ->
                val iterator = values.listIterator()
                if (iterator.hasNext()) iterator.next()
                iterator.remove()
            },
            "sublist clear" to { values -> values.subList(0, values.size).clear() },
        )
        for (size in 0..2) {
            val input = MutableList(size) { "entry-$it" }
            val expected = input.toList()
            val snapshot = MusubiValidationV1.immutableList(input)
            input.clear()
            input.add("caller changed")
            assertEquals(expected, snapshot, "size $size retained its construction values")
            // MutableList maps directly to java.util.List methods on this JVM wrapper.
            val javaList = snapshot as MutableList<String>
            for ((method, mutation) in mutations) {
                assertFailsWith<UnsupportedOperationException>("size $size rejects $method") {
                    mutation(javaList)
                }
                assertEquals(expected, snapshot, "size $size stays fixed after $method")
            }
            if (size == 0) {
                // An empty list has no valid swap indices; the Java operation still cannot alter it.
                assertFailsWith<IndexOutOfBoundsException> { Collections.swap(javaList, 0, 0) }
            } else {
                assertFailsWith<UnsupportedOperationException> {
                    Collections.swap(javaList, 0, size - 1)
                }
            }
            assertEquals(expected, snapshot, "size $size stays fixed after Java swap")
        }
    }

    @Test
    fun `public metadata keywords keep equality hash and JSON after caller mutation`() {
        val input = mutableListOf(MusubiKeywordV1("alpha"), MusubiKeywordV1("beta"))
        val metadata = MusubiReleaseMetadataV1(keywords = input)
        val expected = MusubiReleaseMetadataV1(keywords = input.toList())
        val keywords = input.toList()
        val beforeJson = metadata.toJsonBytes()
        val beforeHash = metadata.hashCode()
        fun unchanged() {
            assertEquals(keywords, metadata.keywords)
            assertEquals(expected, metadata)
            assertEquals(beforeHash, metadata.hashCode())
            assertContentEquals(beforeJson, metadata.toJsonBytes())
        }
        input.clear()
        unchanged()
        val publicList = metadata.keywords as MutableList<MusubiKeywordV1>
        assertFailsWith<UnsupportedOperationException> { publicList.clear() }
        unchanged()
        assertFailsWith<UnsupportedOperationException> { publicList[0] = MusubiKeywordV1("changed") }
        unchanged()
        assertFailsWith<UnsupportedOperationException> { Collections.swap(publicList, 0, 1) }
        unchanged()
        assertFailsWith<IllegalArgumentException> {
            MusubiReleaseMetadataV1(keywords = keywords.reversed())
        }
        assertFailsWith<IllegalArgumentException> {
            MusubiReleaseMetadataV1(keywords = listOf(keywords[0], keywords[0]))
        }
    }

    @Test
    fun `public provider approvals cannot invalidate registered record commitments`() {
        val fixtureRecord = providerRecordFixture()
        val first = fixtureRecord.attestation.approvals.single()
        val second = secondApproval(fixtureRecord.attestation.payload)
        val sorted = listOf(first, second).sortedWith(Comparator { left, right ->
            MusubiValidationV1.compareUnsignedBytes(left.publicKeyPayload, right.publicKeyPayload)
        })
        val input = sorted.toMutableList()
        val attestation = MusubiProviderBundleVerificationAttestationV1(
            fixtureRecord.attestation.payload,
            input,
        )
        val record = MusubiProviderBundleAttestationRecordV1(
            fixtureRecord.key,
            MusubiProviderBundleAttestationDigestV1(musubiProviderBundleAttestationDigestV1(attestation)),
            attestation,
            fixtureRecord.registeredBy,
            fixtureRecord.registeredAtHeight,
        )
        val instruction = MusubiInstructionsV1.RegisterMusubiProviderBundleAttestationV1(
            attestation,
            java.math.BigInteger.ONE,
        )
        val beforeRecordJson = record.toJsonBytes()
        val beforeAttestationJson = attestation.toJsonBytes()
        val beforeFrame = instruction.concreteFrame()
        val beforeDigest = record.attestationDigest.bytes()
        val beforeRecordHash = record.hashCode()
        val beforeAttestationHash = attestation.hashCode()
        val beforeOrder = sorted.map { it.publicKey }
        val expectedRecord = MusubiProviderBundleAttestationRecordV1.fromJsonBytes(beforeRecordJson)
        fun unchanged() {
            assertEquals(beforeOrder, attestation.approvals.map { it.publicKey })
            assertEquals(expectedRecord, record)
            assertEquals(beforeRecordHash, record.hashCode())
            assertEquals(beforeAttestationHash, attestation.hashCode())
            assertContentEquals(beforeAttestationJson, attestation.toJsonBytes())
            assertContentEquals(beforeRecordJson, record.toJsonBytes())
            assertContentEquals(beforeFrame, instruction.concreteFrame())
            assertContentEquals(beforeDigest, record.attestationDigest.bytes())
            assertContentEquals(beforeDigest, musubiProviderBundleAttestationDigestV1(attestation))
        }
        input.clear()
        unchanged()
        val publicList = attestation.approvals as MutableList<MusubiProviderBundleVerificationApprovalV1>
        assertFailsWith<UnsupportedOperationException> { publicList.clear() }
        unchanged()
        assertFailsWith<UnsupportedOperationException> { publicList[0] = sorted[1] }
        unchanged()
        assertFailsWith<UnsupportedOperationException> { Collections.swap(publicList, 0, 1) }
        unchanged()
        assertFailsWith<IllegalArgumentException> {
            MusubiProviderBundleVerificationAttestationV1(attestation.payload, sorted.reversed())
        }
        assertFailsWith<IllegalArgumentException> {
            MusubiProviderBundleVerificationAttestationV1(attestation.payload, listOf(first, first))
        }
        assertFailsWith<IllegalArgumentException> {
            MusubiProviderBundleVerificationAttestationV1(attestation.payload, emptyList())
        }
        assertFailsWith<IllegalArgumentException> {
            MusubiProviderBundleVerificationAttestationV1(attestation.payload, List(65) { first })
        }
    }

    private fun secondApproval(
        payload: MusubiProviderBundleVerificationPayloadV1,
    ): MusubiProviderBundleVerificationApprovalV1 {
        val seed = 0x72
        val challenge = payload.toJsonBytes()
        val signer = Ed25519Signer()
        signer.init(true, Ed25519PrivateKeyParameters(ByteArray(32) { seed.toByte() }, 0))
        signer.update(challenge, 0, challenge.size)
        val signature = signer.generateSignature().joinToString("") { byte ->
            val value = byte.toInt() and 0xff
            "0123456789ABCDEF"[value ushr 4].toString() + "0123456789ABCDEF"[value and 0x0f]
        }
        // A real second-key signature supplies nonzero canonical structural approval bytes.
        // This snapshot test does not assert native signer-policy or protocol signature admission.
        return MusubiProviderBundleVerificationApprovalV1(
            encodePublicKeyMultihash(0x01, TestEd25519Keys.publicKey(seed)),
            signature,
        )
    }

    private fun providerRecordFixture(): MusubiProviderBundleAttestationRecordV1 {
        var root = Paths.get("").toAbsolutePath()
        while (!Files.isRegularFile(root.resolve("fixtures/musubi/sdk_v1.json"))) {
            root = requireNotNull(root.parent) { "native Musubi fixture is absent" }
        }
        val fixture = objectValue(JsonParser.parse(String(
            Files.readAllBytes(root.resolve("fixtures/musubi/sdk_v1.json")),
            Charsets.UTF_8,
        )))
        val routes = fixture.getValue("routes") as List<*>
        val route = routes.map(::objectValue).single {
            it["path"] == "/v1/musubi/queries/provider-bundle-attestation"
        }
        return MusubiProviderBundleAttestationRecordV1.fromJsonBytes(
            JsonEncoder.encode(route["response"]).toByteArray(Charsets.UTF_8),
        )
    }

    @Suppress("UNCHECKED_CAST")
    private fun objectValue(value: Any?): Map<String, Any?> = value as Map<String, Any?>
}
