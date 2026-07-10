package org.hyperledger.iroha.sdk.client

import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

class SccpClientExactTest {
    @Test
    fun proofAndMessageDtosUseDisjointCanonicalArtifacts() {
        val artifact = canonicalArtifact()
        val proof = BridgeProofSubmitRequest("alice", artifact)
        assertEquals(setOf("authority", "message_bundle_b64"), proof.toJsonMap().keys)
        assertFalse(proof.toJsonMap().containsKey("private_key"))
        HttpClientTransport.preflightSccpBridgeSubmitJson(proof.toJsonBytes(), "/v1/bridge/proofs/submit")

        val message = BridgeMessageSubmitRequest("alice", artifact)
        assertEquals(setOf("authority", "native_proof_b64"), message.toJsonMap().keys)
        HttpClientTransport.preflightSccpBridgeSubmitJson(message.toJsonBytes(), "/v1/bridge/messages")

        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.preflightSccpBridgeSubmitJson(message.toJsonBytes(), "/v1/bridge/proofs/submit")
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.preflightSccpBridgeSubmitJson(proof.toJsonBytes(), "/v1/bridge/messages")
        }
    }

    @Test
    fun submitPreflightRejectsRetiredSelectorsSecretsAndUnknownFields() {
        val artifact = canonicalArtifact()
        val proofRetired = listOf(
            "private_key", "native_proof_b64", "burn_bundle", "message_bundle",
            "expected_destination_binding_hash_hex", "source_profile", "target_profile",
            "source_domain", "target_domain", "native_height",
        )
        for (field in proofRetired) {
            val value: Any = if (field.endsWith("_b64")) artifact else if (field.endsWith("bundle")) emptyMap<String, Any>() else "retired"
            val body = JsonEncoder.encode(linkedMapOf("authority" to "alice", "message_bundle_b64" to artifact, field to value)).toByteArray()
            assertFailsWith<IllegalArgumentException>(field) {
                HttpClientTransport.preflightSccpBridgeSubmitJson(body, "/v1/bridge/proofs/submit")
            }
        }
        val messageRetired = listOf(
            "private_key", "message_bundle_b64", "message_bundle", "burn_bundle", "receipt_lane",
            "settlement", "settlement_contract", "contract", "entrypoint", "payload", "mint",
            "asset_id", "amount", "recipient", "native_height",
        )
        for (field in messageRetired) {
            val value: Any = if (field.endsWith("_b64")) artifact else "retired"
            val body = JsonEncoder.encode(linkedMapOf("authority" to "alice", "native_proof_b64" to artifact, field to value)).toByteArray()
            assertFailsWith<IllegalArgumentException>(field) {
                HttpClientTransport.preflightSccpBridgeSubmitJson(body, "/v1/bridge/messages")
            }
        }
        for (body in listOf("[]", "null", "{", "42")) {
            assertFailsWith<IllegalArgumentException> {
                HttpClientTransport.preflightSccpBridgeSubmitJson(body.toByteArray(), "/v1/bridge/proofs/submit")
            }
        }
    }

    @Test
    fun canonicalArtifactValidationRejectsAliasesCorruptionAndTrailingBytes() {
        val canonical = canonicalArtifactBytes()
        val encoded = Base64.getEncoder().encodeToString(canonical)
        assertFailsWith<IllegalArgumentException> { BridgeProofSubmitRequest("alice", encoded.trimEnd('=')) }
        assertFailsWith<IllegalArgumentException> { BridgeProofSubmitRequest("alice", " $encoded") }
        val corrupted = canonical.copyOf().also { it[it.lastIndex] = (it.last() + 1).toByte() }
        assertFailsWith<IllegalArgumentException> { BridgeProofSubmitRequest("alice", Base64.getEncoder().encodeToString(corrupted)) }
        assertFailsWith<IllegalArgumentException> { BridgeProofSubmitRequest("alice", Base64.getEncoder().encodeToString(canonical + 0)) }
        val zeroSchema = canonical.copyOf().also { it.fill(0, 6, 22) }
        assertFailsWith<IllegalArgumentException> { BridgeProofSubmitRequest("alice", Base64.getEncoder().encodeToString(zeroSchema)) }
        val invalidMagic = canonical.copyOf().also { it[0] = 'X'.code.toByte() }
        assertFailsWith<IllegalArgumentException> { BridgeMessageSubmitRequest("alice", Base64.getEncoder().encodeToString(invalidMagic)) }
        BridgeMessageSubmitRequest("alice", Base64.getEncoder().encodeToString(canonicalArtifactBytes(8)))
        assertFailsWith<IllegalArgumentException> {
            BridgeProofSubmitRequest("alice", encoded, publicKeyHex = "01".repeat(32))
        }
        assertFailsWith<IllegalArgumentException> {
            BridgeProofSubmitRequest("alice", encoded, creationTimeMs = 0)
        }
    }

    @Test
    fun detachedSigningResponsesExposeAuditableTransactionPayloadOnly() {
        val transactionBytes = NoritoJavaCodecAdapter().encodeTransaction(TransactionPayload(creationTimeMs = 10))
        val transaction = Base64.getEncoder().encodeToString(transactionBytes)
        val signing = Base64.getEncoder().encodeToString(IrohaHash.prehash(transactionBytes))
        val unsigned = """{
          "submitted":false,"payload_kind":"transfer","message_id_hex":"${"40".repeat(32)}","backend":"bridge/sccp/outbound-v1",
          "counterparty_domain":2,"counterparty_chain":"bsc-mainnet","manifest_hash_hex":"${"41".repeat(32)}",
          "range_start_height":9,"range_end_height":9,"creation_time_ms":10,"tx_hash_hex":null,
          "transaction_payload_b64":"$transaction","signing_message_b64":"$signing"
        }"""
        val response = SccpBridgeSubmitResponseParser.parse(unsigned.toByteArray())
        assertFalse(response.submitted)
        assertEquals(transaction, response.transactionPayloadB64)
        assertEquals(signing, response.signingMessageB64)

        for (retired in listOf("transaction_scaffold_b64", "signed_transaction_b64")) {
            val malformed = unsigned.replace("\"signing_message_b64\"", "\"$retired\":\"$transaction\",\"signing_message_b64\"")
            assertFailsWith<IllegalArgumentException> { SccpBridgeSubmitResponseParser.parse(malformed.toByteArray()) }
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBridgeSubmitResponseParser.parse(unsigned.replace("\"transaction_payload_b64\":\"$transaction\"", "\"transaction_payload_b64\":null").toByteArray())
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBridgeSubmitResponseParser.parse(unsigned.replace(transaction, Base64.getEncoder().encodeToString(canonicalArtifactBytes() + 0)).toByteArray())
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBridgeSubmitResponseParser.parse(unsigned.replace("\"counterparty_chain\":\"bsc-mainnet\"", "\"counterparty_chain\":\"ethereum-mainnet\"").toByteArray())
        }
        assertFailsWith<IllegalArgumentException> {
            SccpBridgeSubmitResponseParser.parse(unsigned.replace("\"tx_hash_hex\":null,", "").toByteArray())
        }
        for (retired in listOf("ok", "proof_kind", "message_kind")) {
            val malformed = unsigned.replace("\"submitted\"", "\"$retired\":true,\"submitted\"")
            assertFailsWith<IllegalArgumentException> { SccpBridgeSubmitResponseParser.parse(malformed.toByteArray()) }
        }
    }

    @Test
    fun discoveryAndRecentParsersAreExactAndLaneAware() {
        val capabilities = SccpJsonParser.parseCapabilities(capabilitiesJson().toByteArray())
        assertEquals(1, capabilities.version)
        assertNull(capabilities.nativeMessageSubmitPath)
        assertEquals("/v1/sccp/messages/recent", capabilities.outbound.recentMessagesPath)
        assertTrue(capabilities.inboundLanes.isEmpty())

        val manifests = SccpJsonParser.parseProofManifests(manifestJson().toByteArray())
        val route = manifests.outboundDestinationRoutes.single()
        assertEquals("sora-nexus", route.sourceProfile)
        assertEquals("bsc-mainnet", route.targetProfile)
        assertEquals(SccpDestinationVerifierPlanV1.EVM_GROTH16_BN254_ADAPTER, route.verifierPlan)

        val recent = SccpJsonParser.parseRecentMessages(recentJson().toByteArray())
        assertEquals(2, recent.items.size)
        assertEquals(9, recent.items.first().height)
        assertEquals("bsc-mainnet", recent.items.first().targetProfile)

        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseCapabilities(capabilitiesJson().replace("\"version\":1", "\"version\":1,\"counterparties\":[]").toByteArray())
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseCapabilities(capabilitiesJson().replace("\"native_message_submit_path\":null", "\"native_proof_submit_path\":\"/retired\",\"native_message_submit_path\":null").toByteArray())
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseCapabilities(capabilitiesJson().replace("evm_address20", "evm_hex").toByteArray())
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseCapabilities(capabilitiesJson().replace("\"transfer\"", "\"burn\"").toByteArray())
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseProofManifests(manifestJson().replace("bsc-mainnet", "BSC-MAINNET").toByteArray())
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseProofManifests(manifestJson().replace("\"target_domain\":2", "\"target_domain\":1").toByteArray())
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRecentMessages(recentJson().replace("\"height\":9", "\"height\":7").toByteArray())
        }

        val inbound = SccpJsonParser.parseCapabilities(inboundCapabilitiesJson().toByteArray())
            .inboundLanes.single()
        assertTrue(inbound.sourceIdentity.emitter.identity.containsKey("route_config_hash"))
        assertFalse(inbound.sourceIdentity.emitter.identity.containsKey("owner"))
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseCapabilities(inboundCapabilitiesJson().replace("route_config_hash", "owner").toByteArray())
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseCapabilities(inboundCapabilitiesJson().replace("${"BB".repeat(32)}", "${"AA".repeat(32)}").toByteArray())
        }
    }

    private fun canonicalArtifact(): String = Base64.getEncoder().encodeToString(canonicalArtifactBytes())

    private fun canonicalArtifactBytes(padding: Int = 0): ByteArray {
        val schema = ByteArray(16) { (it + 1).toByte() }
        val payload = byteArrayOf(1, 2, 3)
        val header = NoritoHeader(schema, payload.size, CRC64.compute(payload), NoritoCodec.DEFAULT_FLAGS, NoritoHeader.COMPRESSION_NONE)
        return header.encode() + ByteArray(padding) + payload
    }

    private fun capabilitiesJson(): String = """{
      "version":1,
      "registry_revision":"0x${"11".repeat(32)}",
      "native_message_submit_path":null,
      "outbound":{
        "message_bundle_path":"/v1/sccp/proofs/message/{message_id}",
        "proof_artifact_path":"/v1/sccp/artifacts/message/{message_id}",
        "proof_job_path":"/v1/sccp/jobs/message/{message_id}",
        "recent_messages_path":"/v1/sccp/messages/recent",
        "manifest_path":"/v1/sccp/manifests"
      },
      "message_payload_kinds":["transfer"],
      "codecs":[{"id":2,"key":"evm_address20","description":"canonical EVM address bytes"}],
      "inbound_lanes":[]
    }"""

    private fun inboundCapabilitiesJson(): String = capabilitiesJson().replace(
        "\"inbound_lanes\":[]",
        """"inbound_lanes":[{
          "source_profile":"bsc-mainnet","target_profile":"sora-taira",
          "source_domain":2,"target_domain":0,"source_identity_hash":"0x${"51".repeat(32)}",
          "source_identity":{
            "lane":{
              "source":{"network":"bsc_mainnet","profile":null},
              "target":{"network":"sora_taira","profile":null}
            },
            "emitter":{"emitter":"evm","identity":{
              "address":"${"11".repeat(20)}","runtime_code_hash":"${"AA".repeat(32)}",
              "route_config_hash":"${"BB".repeat(32)}"
            }}
          },
          "admission_enabled":false,"native_admission":null,"native_proof_builder":null
        }]""",
    )

    private fun manifestJson(): String = """{
      "version":1,
      "registry_revision":"0x${"12".repeat(32)}",
      "inbound_native_lanes":[],
      "outbound_destination_routes":[{
        "source_profile":"sora-nexus","target_profile":"bsc-mainnet",
        "source_domain":0,"target_domain":2,"route_id":"nexus-bsc-xor","asset_key":"xor",
        "verifier_plan":"EvmGroth16Bn254Adapter","verifier_identity":"0x${"21".repeat(20)}",
        "verifier_code_hash":"0x${"22".repeat(32)}","verifier_key_hash":"0x${"23".repeat(32)}",
        "proof_artifact_hash":null,"proving_key_hash":null,
        "destination_binding_key":"evm:0:2:nexus-bsc-xor",
        "destination_binding_hash":"0x${"24".repeat(32)}","browser_prover":null
      }]
    }"""

    private fun recentJson(): String = """{"items":[
      {"height":9,"message_id_hex":"0x${"31".repeat(32)}","kind":"transfer",
       "source_profile":"sora-nexus","target_profile":"bsc-mainnet",
       "destination_binding_hash":"0x${"32".repeat(32)}","target_domain":2,"counterparty_domain":2,
       "asset_id":"xor","route_id":"route","recipient":"recipient","amount":"7","payload_projection":null,
       "links":{"bundle_path":"/bundle/1","artifact_path":"/artifact/1","job_path":"/job/1"}},
      {"height":8,"message_id_hex":"0x${"33".repeat(32)}","kind":"token_pause",
       "source_profile":"sora-nexus","target_profile":"bsc-mainnet",
       "destination_binding_hash":"0x${"34".repeat(32)}","target_domain":2,"counterparty_domain":2,
       "asset_id":null,"route_id":null,"recipient":null,"amount":null,"payload_projection":null,
       "links":{"bundle_path":"/bundle/2","artifact_path":"/artifact/2","job_path":"/job/2"}}
    ]}"""
}
