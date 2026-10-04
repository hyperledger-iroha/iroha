package org.hyperledger.iroha.sdk.musubi

import java.math.BigInteger
import java.nio.file.Files
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys

/** Structural SDK controls; only the native fixture producer supplies canonical proof bytes. */
class MusubiCompletionAuthorityTest {
    private fun account(seed: Int): String = AccountAddress.fromAccount(
        TestEd25519Keys.publicKey(seed), "ed25519",
    ).toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

    @Test
    fun `completion uses the distinct signer rather than provider owner`() {
        val owner = account(0x61)
        val signer = account(0x62)
        val policy = MusubiProviderIngestCompletionSignerPolicyV1(
            ByteArray(32) { 1 }, BigInteger.ONE, null, ByteArray(32) { 2 },
        )
        val authority = MusubiProviderIngestCompletionAuthorityV1(owner, signer, policy)
        assertNotEquals(owner, signer)
        val digest = MusubiDigest32V1(ByteArray(32) { 3 })
        fun binding(completedBy: String) = MusubiProviderBundleVerificationBindingV1(
            NetworkId.fromBytes(ByteArray(32) { 7 }), "05".repeat(32), completedBy, authority,
            digest, BigInteger.ONE, BigInteger.ONE,
            MusubiProviderIngestFinalizedAnchorV1(BigInteger.ONE, ByteArray(32) { 6 }),
            digest, digest, digest, digest, digest, digest,
        )
        assertEquals(signer, binding(signer).completedBy)
        assertEquals(owner, binding(signer).completionAuthority.providerOwner)
        assertEquals(signer, binding(signer).completionAuthority.completionSigner)
        assertFailsWith<IllegalArgumentException> { binding(owner) }
        assertFailsWith<IllegalArgumentException> { binding(account(0x63)) }
        assertFailsWith<IllegalArgumentException> {
            MusubiProviderIngestCompletionAuthorityV1(owner, " $signer", policy)
        }
        assertNotEquals(authority, MusubiProviderIngestCompletionAuthorityV1(owner, account(0x63), policy))
    }

    @Test
    fun `canonical record and key factories retain fixtures and reject retired authority fields`() {
        var root = Paths.get("").toAbsolutePath()
        while (!Files.isRegularFile(root.resolve("fixtures/musubi/sdk_v1.json"))) {
            root = requireNotNull(root.parent) { "native Musubi fixture is absent" }
        }
        val fixture = objectValue(JsonParser.parse(String(Files.readAllBytes(
            root.resolve("fixtures/musubi/sdk_v1.json"),
        ), Charsets.UTF_8)))
        val routes = fixture.getValue("routes") as List<*>
        val route = routes.map(::objectValue).single {
            it["path"] == "/v1/musubi/queries/provider-bundle-attestation"
        }
        val key = MusubiProviderBundleAttestationKeyV1.fromJsonBytes(json(route["request"]))
        val record = MusubiProviderBundleAttestationRecordV1.fromJsonBytes(json(route["response"]))
        record.requireMatches(key)
        assertEquals(record, MusubiProviderBundleAttestationRecordV1.fromJsonBytes(record.toJsonBytes()))
        assertEquals(key, MusubiProviderBundleAttestationKeyV1.fromJsonBytes(key.toJsonBytes()))
        for (change in listOf<(MutableMap<String, Any?>) -> Unit>(
            { it.remove("completion_signer") },
            { it["completion_signer"] = null },
            { it["completion_signer"] = "invalid" },
            { it["extra"] = 1 },
        )) {
            val changed = objectValue(JsonParser.parse(String(json(route["response"]), Charsets.UTF_8)))
            val attestation = objectValue(changed["attestation"])
            val payload = objectValue(attestation["payload"])
            val binding = objectValue(payload["binding"])
            change(objectValue(binding["completion_authority"]))
            assertFailsWith<IllegalArgumentException> {
                MusubiProviderBundleAttestationRecordV1.fromJsonBytes(json(changed))
            }
        }
        val missingKey = objectValue(JsonParser.parse(String(key.toJsonBytes(), Charsets.UTF_8)))
        missingKey.remove("provider_id")
        assertFailsWith<IllegalArgumentException> {
            MusubiProviderBundleAttestationKeyV1.fromJsonBytes(json(missingKey))
        }
    }

    private fun json(value: Any?): ByteArray = JsonEncoder.encode(value).toByteArray(Charsets.UTF_8)

    @Suppress("UNCHECKED_CAST")
    private fun objectValue(value: Any?): MutableMap<String, Any?> = value as MutableMap<String, Any?>
}
