package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoHeader;

/** Adversarial client tests for exact SCCP discovery, submission, and detached signing. */
public final class SccpClientExactTests {
  private SccpClientExactTests() {}

  public static void main(final String[] args) throws Exception {
    endpointsAcceptOnlyTheirCanonicalArtifact();
    retiredFieldsAndSecretsFailClosed();
    artifactAliasesAndCorruptionFail();
    discoveryAndReadbackAreStrict();
    detachedSigningResponseUsesTransactionPayload();
    System.out.println("[IrohaAndroid] exact SCCP client tests passed.");
  }

  private static void endpointsAcceptOnlyTheirCanonicalArtifact() {
    final String artifact = canonicalArtifact();
    final BridgeProofSubmitRequest proof =
        BridgeProofSubmitRequest.builder().authority("alice").messageBundleB64(artifact).build();
    assert proof.toJsonMap().keySet().equals(java.util.Set.of("authority", "message_bundle_b64"));
    HttpClientTransport.preflightSccpBridgeSubmitJson(proof.toJsonBytes(), "/v1/bridge/proofs/submit");
    final BridgeMessageSubmitRequest message = new BridgeMessageSubmitRequest("alice", artifact);
    HttpClientTransport.preflightSccpBridgeSubmitJson(message.toJsonBytes(), "/v1/bridge/messages");
    expectFailure(() -> HttpClientTransport.preflightSccpBridgeSubmitJson(message.toJsonBytes(), "/v1/bridge/proofs/submit"));
    expectFailure(() -> HttpClientTransport.preflightSccpBridgeSubmitJson(proof.toJsonBytes(), "/v1/bridge/messages"));
    expectFailure(() -> BridgeProofSubmitRequest.builder().authority("alice").messageBundleB64(artifact).publicKeyHex("ed").build());
    expectFailure(() -> BridgeProofSubmitRequest.builder().authority("alice").messageBundleB64(artifact).creationTimeMs(0L).build());
  }

  private static void retiredFieldsAndSecretsFailClosed() {
    final String artifact = canonicalArtifact();
    for (final String field : List.of(
        "private_key", "native_proof_b64", "burn_bundle", "message_bundle",
        "expected_destination_binding_hash_hex", "source_profile", "target_profile",
        "source_domain", "target_domain", "native_height")) {
      final Map<String, Object> body = new LinkedHashMap<>();
      body.put("authority", "alice"); body.put("message_bundle_b64", artifact);
      body.put(field, field.endsWith("_b64") ? artifact : "retired");
      expectFailure(() -> HttpClientTransport.preflightSccpBridgeSubmitJson(JsonEncoder.encode(body).getBytes(StandardCharsets.UTF_8), "/v1/bridge/proofs/submit"));
    }
    for (final String field : List.of(
        "private_key", "message_bundle_b64", "message_bundle", "burn_bundle", "receipt_lane",
        "settlement", "settlement_contract", "contract", "entrypoint", "payload", "mint",
        "asset_id", "amount", "recipient", "native_height")) {
      final Map<String, Object> body = new LinkedHashMap<>();
      body.put("authority", "alice"); body.put("native_proof_b64", artifact);
      body.put(field, field.endsWith("_b64") ? artifact : "retired");
      expectFailure(() -> HttpClientTransport.preflightSccpBridgeSubmitJson(JsonEncoder.encode(body).getBytes(StandardCharsets.UTF_8), "/v1/bridge/messages"));
    }
    for (final String body : List.of("[]", "null", "{", "42")) {
      expectFailure(() -> HttpClientTransport.preflightSccpBridgeSubmitJson(body.getBytes(StandardCharsets.UTF_8), "/v1/bridge/proofs/submit"));
    }
  }

  private static void artifactAliasesAndCorruptionFail() {
    final byte[] canonical = canonicalArtifactBytes();
    final String encoded = Base64.getEncoder().encodeToString(canonical);
    expectFailure(() -> BridgeProofSubmitRequest.builder().authority("alice").messageBundleB64(encoded.replace("=", "")).build());
    final byte[] corrupt = canonical.clone(); corrupt[corrupt.length - 1]++;
    expectFailure(() -> new BridgeMessageSubmitRequest("alice", Base64.getEncoder().encodeToString(corrupt)));
    expectFailure(() -> BridgeProofSubmitRequest.builder().authority("alice").messageBundleB64(Base64.getEncoder().encodeToString(Arrays.copyOf(canonical, canonical.length + 1))).build());
    final byte[] zeroSchema = canonical.clone(); Arrays.fill(zeroSchema, 6, 22, (byte) 0);
    expectFailure(() -> new BridgeMessageSubmitRequest("alice", Base64.getEncoder().encodeToString(zeroSchema)));
    new BridgeMessageSubmitRequest(
        "alice", Base64.getEncoder().encodeToString(canonicalArtifactBytes(8)));
  }

  private static void discoveryAndReadbackAreStrict() {
    final SccpModels.Capabilities capabilities = SccpJsonParser.parseCapabilities(capabilitiesJson().getBytes(StandardCharsets.UTF_8));
    assert capabilities.version == 1;
    assert capabilities.inboundLanes.isEmpty();
    final SccpModels.ProofManifestSet manifests = SccpJsonParser.parseProofManifests(manifestJson().getBytes(StandardCharsets.UTF_8));
    assert manifests.outboundDestinationRoutes.get(0).sourceProfile.equals("sora-nexus");
    final SccpModels.RecentMessages recent = SccpJsonParser.parseRecentMessages(recentJson().getBytes(StandardCharsets.UTF_8));
    assert recent.items.size() == 2 && recent.items.get(0).height == 9;
    expectFailure(() -> SccpJsonParser.parseCapabilities(capabilitiesJson().replace("\"version\":1", "\"version\":1,\"counterparties\":[]").getBytes(StandardCharsets.UTF_8)));
    expectFailure(() -> SccpJsonParser.parseCapabilities(capabilitiesJson().replace("evm_address20", "evm_hex").getBytes(StandardCharsets.UTF_8)));
    expectFailure(() -> SccpJsonParser.parseCapabilities(capabilitiesJson().replace("\"transfer\"", "\"burn\"").getBytes(StandardCharsets.UTF_8)));
    expectFailure(() -> SccpJsonParser.parseProofManifests(manifestJson().replace("bsc-mainnet", "BSC-MAINNET").getBytes(StandardCharsets.UTF_8)));
    expectFailure(() -> SccpJsonParser.parseProofManifests(manifestJson().replace("\"target_domain\":2", "\"target_domain\":1").getBytes(StandardCharsets.UTF_8)));
    expectFailure(() -> SccpJsonParser.parseRecentMessages(recentJson().replace("\"height\":9", "\"height\":7").getBytes(StandardCharsets.UTF_8)));
    final SccpModels.ExactInboundLaneCapability inbound =
        SccpJsonParser.parseCapabilities(inboundCapabilitiesJson().getBytes(StandardCharsets.UTF_8))
            .inboundLanes
            .get(0);
    assert inbound.sourceIdentity.emitter.identity.containsKey("route_config_hash");
    assert !inbound.sourceIdentity.emitter.identity.containsKey("owner");
    expectFailure(
        () ->
            SccpJsonParser.parseCapabilities(
                inboundCapabilitiesJson()
                    .replace("route_config_hash", "owner")
                    .getBytes(StandardCharsets.UTF_8)));
    expectFailure(
        () ->
            SccpJsonParser.parseCapabilities(
                inboundCapabilitiesJson()
                    .replace("BB".repeat(32), "AA".repeat(32))
                    .getBytes(StandardCharsets.UTF_8)));
  }

  private static void detachedSigningResponseUsesTransactionPayload() throws Exception {
    final byte[] transactionBytes =
        new NoritoJavaCodecAdapter()
            .encodeTransaction(
                TransactionPayload.builder().setCreationTimeMs(10).build());
    final String transaction = Base64.getEncoder().encodeToString(transactionBytes);
    final String signing = Base64.getEncoder().encodeToString(IrohaHash.prehash(transactionBytes));
    final String response =
        "{\"submitted\":false,\"payload_kind\":\"transfer\",\"message_id_hex\":\"" + "40".repeat(32) + "\","
            + "\"backend\":\"bridge/sccp/outbound-v1\",\"counterparty_domain\":2,"
            + "\"counterparty_chain\":\"bsc-mainnet\",\"manifest_hash_hex\":\"" + "41".repeat(32) + "\","
            + "\"range_start_height\":9,\"range_end_height\":9,\"creation_time_ms\":10,\"tx_hash_hex\":null,"
            + "\"transaction_payload_b64\":\"" + transaction + "\",\"signing_message_b64\":\"" + signing + "\"}";
    final SccpBridgeSubmitResponse parsed = SccpBridgeSubmitResponse.parse(response.getBytes(StandardCharsets.UTF_8));
    assert !parsed.submitted && transaction.equals(parsed.transactionPayloadB64);
    for (final String retired : List.of("transaction_scaffold_b64", "signed_transaction_b64")) {
      final String malformed = response.replace("\"signing_message_b64\"", "\"" + retired + "\":\"" + transaction + "\",\"signing_message_b64\"");
      expectFailure(() -> SccpBridgeSubmitResponse.parse(malformed.getBytes(StandardCharsets.UTF_8)));
    }
    for (final String retired : List.of("ok", "proof_kind", "message_kind")) {
      final String malformed = response.replace("\"submitted\"", "\"" + retired + "\":true,\"submitted\"");
      expectFailure(() -> SccpBridgeSubmitResponse.parse(malformed.getBytes(StandardCharsets.UTF_8)));
    }
    expectFailure(
        () ->
            SccpBridgeSubmitResponse.parse(
                response
                    .replace("\"counterparty_chain\":\"bsc-mainnet\"", "\"counterparty_chain\":\"ethereum-mainnet\"")
                    .getBytes(StandardCharsets.UTF_8)));
    expectFailure(
        () ->
            SccpBridgeSubmitResponse.parse(
                response.replace("\"tx_hash_hex\":null,", "").getBytes(StandardCharsets.UTF_8)));
  }

  private static String canonicalArtifact() { return Base64.getEncoder().encodeToString(canonicalArtifactBytes()); }
  private static byte[] canonicalArtifactBytes() {
    return canonicalArtifactBytes(0);
  }

  private static byte[] canonicalArtifactBytes(final int padding) {
    final byte[] schema = new byte[16]; for (int i = 0; i < schema.length; i++) schema[i] = (byte) (i + 1);
    final byte[] payload = {1, 2, 3};
    final NoritoHeader header = new NoritoHeader(schema, payload.length, CRC64.compute(payload), NoritoHeader.COMPACT_LEN, NoritoHeader.COMPRESSION_NONE);
    final byte[] encoded = new byte[NoritoHeader.HEADER_LENGTH + padding + payload.length];
    System.arraycopy(header.encode(), 0, encoded, 0, NoritoHeader.HEADER_LENGTH);
    System.arraycopy(payload, 0, encoded, NoritoHeader.HEADER_LENGTH + padding, payload.length);
    return encoded;
  }

  private static String capabilitiesJson() {
    return "{\"version\":1,\"registry_revision\":\"0x" + "11".repeat(32) + "\",\"native_message_submit_path\":null,"
        + "\"outbound\":{\"message_bundle_path\":\"/v1/sccp/proofs/message/{message_id}\",\"proof_artifact_path\":\"/v1/sccp/artifacts/message/{message_id}\",\"proof_job_path\":\"/v1/sccp/jobs/message/{message_id}\",\"recent_messages_path\":\"/v1/sccp/messages/recent\",\"manifest_path\":\"/v1/sccp/manifests\"},"
        + "\"message_payload_kinds\":[\"transfer\"],\"codecs\":[{\"id\":2,\"key\":\"evm_address20\",\"description\":\"canonical EVM address bytes\"}],\"inbound_lanes\":[]}";
  }
  private static String manifestJson() {
    return "{\"version\":1,\"registry_revision\":\"0x" + "12".repeat(32) + "\",\"inbound_native_lanes\":[],\"outbound_destination_routes\":[{"
        + "\"source_profile\":\"sora-nexus\",\"target_profile\":\"bsc-mainnet\",\"source_domain\":0,\"target_domain\":2,\"route_id\":\"nexus-bsc-xor\",\"asset_key\":\"xor\","
        + "\"verifier_plan\":\"EvmGroth16Bn254Adapter\",\"verifier_identity\":\"0x" + "21".repeat(20) + "\",\"verifier_code_hash\":\"0x" + "22".repeat(32) + "\","
        + "\"verifier_key_hash\":null,\"proof_artifact_hash\":null,\"proving_key_hash\":null,\"destination_binding_key\":\"evm:0:2:route\",\"destination_binding_hash\":\"0x" + "24".repeat(32) + "\",\"browser_prover\":null}]}";
  }
  private static String inboundCapabilitiesJson() {
    final String inbound =
        "\"inbound_lanes\":[{"
            + "\"source_profile\":\"bsc-mainnet\",\"target_profile\":\"sora-taira\","
            + "\"source_domain\":2,\"target_domain\":0,\"source_identity_hash\":\"0x" + "51".repeat(32) + "\","
            + "\"source_identity\":{\"lane\":{"
            + "\"source\":{\"network\":\"bsc_mainnet\",\"profile\":null},"
            + "\"target\":{\"network\":\"sora_taira\",\"profile\":null}},"
            + "\"emitter\":{\"emitter\":\"evm\",\"identity\":{"
            + "\"address\":\"" + "11".repeat(20) + "\","
            + "\"runtime_code_hash\":\"" + "AA".repeat(32) + "\","
            + "\"route_config_hash\":\"" + "BB".repeat(32) + "\"}}},"
            + "\"admission_enabled\":false,\"native_admission\":null,\"native_proof_builder\":null}]";
    return capabilitiesJson().replace("\"inbound_lanes\":[]", inbound);
  }
  private static String recentJson() {
    return "{\"items\":[" + recentItem(9, "31") + "," + recentItem(8, "33") + "]}";
  }
  private static String recentItem(final int height, final String hashByte) {
    return "{\"height\":" + height + ",\"message_id_hex\":\"0x" + hashByte.repeat(32) + "\",\"kind\":\"transfer\",\"source_profile\":\"sora-nexus\",\"target_profile\":\"bsc-mainnet\",\"destination_binding_hash\":\"0x" + "32".repeat(32) + "\",\"target_domain\":2,\"counterparty_domain\":2,\"asset_id\":null,\"route_id\":null,\"recipient\":null,\"amount\":null,\"payload_projection\":null,\"links\":{\"bundle_path\":\"/bundle/1\",\"artifact_path\":\"/artifact/1\",\"job_path\":\"/job/1\"}}";
  }
  private static void expectFailure(final Runnable action) { try { action.run(); throw new AssertionError("expected rejection"); } catch (final IllegalArgumentException expected) { /* expected */ } }
}
