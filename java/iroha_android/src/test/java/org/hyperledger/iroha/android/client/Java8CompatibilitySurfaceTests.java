package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

import java.io.ByteArrayInputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.client.testing.FakeHttpTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.client.transport.TransportStreamResponse;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;
import org.hyperledger.iroha.android.crypto.SoftwareKeyProvider;
import org.hyperledger.iroha.android.crypto.export.InMemoryKeyExportStore;
import org.hyperledger.iroha.android.crypto.keystore.KeyGenParameters;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyBackendTag;
import org.hyperledger.iroha.android.privacy.PrivacyConfidentialWitness;
import org.junit.Test;

public final class Java8CompatibilitySurfaceTests {

  @Test
  public void clientResponseDropsWhitespaceRejectCode() {
    final ClientResponse response =
        new ClientResponse(400, new byte[0], "rejected", null, " \t\n");

    assertFalse("blank reject codes must normalize to absent", response.rejectCode().isPresent());
  }

  @Test
  public void noritoRpcOptionsDefaultBlankMethodAndConfigCopiesObservers() {
    final NoritoRpcRequestOptions options =
        NoritoRpcRequestOptions.builder().method(" \t\n").build();
    final List<ClientObserver> observers = new ArrayList<>();
    final ClientObserver firstObserver = new ClientObserver() {};
    final ClientObserver secondObserver = new ClientObserver() {};
    observers.add(firstObserver);

    final ClientConfig config = ClientConfig.builder().setObservers(observers).build();
    observers.add(secondObserver);

    assertTrue("blank Norito RPC method must use POST", "POST".equals(options.method()));
    assertTrue("config must defensively copy observers", config.observers().size() == 1);
    assertTrue("copied observer must be preserved", config.observers().get(0) == firstObserver);
    try {
      config.observers().add(secondObserver);
      fail("config observers must be immutable");
    } catch (final UnsupportedOperationException expected) {
      // Expected immutable list behavior.
    }
  }

  @Test
  public void parserListResponsesCopyItemsAndPublicKeyCodecDropsBlankLiteral() {
    final List<IdentifierPolicySummary> identifierItems = new ArrayList<>();
    final IdentifierPolicySummary identifierSummary =
        new IdentifierPolicySummary(
            "identifier_policy",
            "owner",
            true,
            IdentifierNormalization.EXACT,
            "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29",
            "backend",
            null,
            null,
            null,
            null);
    identifierItems.add(identifierSummary);
    final IdentifierPolicyListResponse identifierResponse =
        new IdentifierPolicyListResponse(1, identifierItems);
    identifierItems.clear();

    assertTrue(
        "identifier policy response must defensively copy items",
        identifierResponse.items().size() == 1);
    assertTrue(
        "identifier policy item must be preserved",
        identifierResponse.items().get(0) == identifierSummary);
    try {
      identifierResponse.items().add(identifierSummary);
      fail("identifier policy response items must be immutable");
    } catch (final UnsupportedOperationException expected) {
      // Expected immutable list behavior.
    }

    final List<RamLfeProgramPolicySummary> ramLfeItems = new ArrayList<>();
    final RamLfeProgramPolicySummary ramLfeSummary =
        new RamLfeProgramPolicySummary(
            "program",
            "owner",
            true,
            "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29",
            "backend",
            "verification",
            null,
            null,
            null,
            null);
    ramLfeItems.add(ramLfeSummary);
    final RamLfeProgramPolicyListResponse ramLfeResponse =
        new RamLfeProgramPolicyListResponse(1, ramLfeItems);
    ramLfeItems.clear();

    assertTrue(
        "RAM-LFE policy response must defensively copy items",
        ramLfeResponse.items().size() == 1);
    assertTrue(
        "RAM-LFE policy item must be preserved", ramLfeResponse.items().get(0) == ramLfeSummary);
    try {
      ramLfeResponse.items().add(ramLfeSummary);
      fail("RAM-LFE policy response items must be immutable");
    } catch (final UnsupportedOperationException expected) {
      // Expected immutable list behavior.
    }

    assertTrue(
        "blank public key literals must return null",
        PublicKeyCodec.decodePublicKeyLiteral(" \t\n") == null);
  }

  @Test
  public void keyManagementJava8SurfaceCopiesInputsAndRejectsBlankText() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final List<IrohaKeyManager.KeyProvider> providers = new ArrayList<>();
    providers.add(provider);

    final IrohaKeyManager manager = IrohaKeyManager.fromProviders(providers);
    providers.clear();

    assertTrue(
        "key manager must defensively copy providers", manager.providerMetadata().size() == 1);
    assertFalse("software provider must drop blank aliases", provider.load(" \t\n").isPresent());
    try {
      provider.generate(" \t\n");
      fail("software provider must reject blank generated aliases");
    } catch (final Exception expected) {
      // Expected blank alias rejection.
    }
    try {
      manager.generateOrLoad(" \t\n", IrohaKeyManager.KeySecurityPreference.SOFTWARE_ONLY);
      fail("key manager must reject blank generated aliases");
    } catch (final IllegalArgumentException expected) {
      // Expected blank alias rejection.
    }

    final InMemoryKeyExportStore store = new InMemoryKeyExportStore();
    assertFalse("export store must drop blank aliases", store.load(" \t\n").isPresent());
    try {
      store.store("alias", " \t\n");
      fail("export store must reject blank bundles");
    } catch (final Exception expected) {
      // Expected blank bundle rejection.
    }

    final KeyProviderMetadata metadata = KeyProviderMetadata.builder(" \t\n").build();
    assertTrue("blank provider names must fall back", "unknown-provider".equals(metadata.name()));
    final KeyGenParameters parameters = KeyGenParameters.builder().setAlgorithm(" \t\n").build();
    assertTrue(
        "blank keystore algorithms must keep default", "Ed25519".equals(parameters.algorithm()));
  }

  @Test
  public void privacyAndBfvJava8SurfaceCopiesLists() {
    final List<Long> b = new ArrayList<>();
    final List<Long> a = new ArrayList<>();
    b.add(Long.valueOf(11L));
    a.add(Long.valueOf(17L));

    final IdentifierBfvPublicParameters.PublicKey publicKey =
        new IdentifierBfvPublicParameters.PublicKey(b, a);
    b.clear();
    a.clear();

    assertTrue("BFV public key b polynomial must be copied", publicKey.b().size() == 1);
    assertTrue("BFV public key a polynomial must be copied", publicKey.a().size() == 1);
    try {
      publicKey.b().add(Long.valueOf(23L));
      fail("BFV public key polynomial lists must be immutable");
    } catch (final UnsupportedOperationException expected) {
      // Expected immutable list behavior.
    }

    final PrivacyConfidentialWitness.NoteWitnessV1 input =
        new PrivacyConfidentialWitness.NoteWitnessV1(
            "7", repeatedByte(0x22), repeatedByte(0x33), 0L);
    final PrivacyConfidentialWitness.TransferOutputWitnessV1 output =
        new PrivacyConfidentialWitness.TransferOutputWitnessV1(
            "7", repeatedByte(0x44), repeatedByte(0x55));
    final List<PrivacyConfidentialWitness.NoteWitnessV1> inputs = new ArrayList<>();
    final List<PrivacyConfidentialWitness.TransferOutputWitnessV1> outputs = new ArrayList<>();
    inputs.add(input);
    outputs.add(output);

    final PrivacyConfidentialWitness.WitnessV1 witness =
        new PrivacyConfidentialWitness.WitnessV1(
            "fc56984b-2be7-431d-840e-21514d1883f0",
            "xor#universal",
            repeatedByte(0x11),
            Collections.singletonList(repeatedByte(0x10)),
            inputs,
            outputs,
            Collections.emptyList(),
            "0",
            repeatedByte(0x66));
    inputs.clear();
    outputs.clear();

    assertTrue("privacy witness inputs must be copied", witness.inputs().size() == 1);
    assertTrue(
        "privacy witness transfer outputs must be copied", witness.transferOutputs().size() == 1);
    try {
      witness.inputs().add(input);
      fail("privacy witness input lists must be immutable");
    } catch (final UnsupportedOperationException expected) {
      // Expected immutable list behavior.
    }
  }

  @Test
  public void verifierParserAndFakeTransportJava8SurfaceRejectsDrift() throws Exception {
    assertTrue(
        "exact Kagemusha verifier profile must remain registry-admissible",
        VerifyingKeyBackendTag.isVerifierBackendRegistryLabelV1(
            "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3"));
    try {
      VerifyingKeyBackendTag.requireVerifierBackendRegistryLabelV1(" halo2/ipa", "backend");
      fail("verifier-registry labels must reject padding");
    } catch (final IllegalArgumentException expected) {
      assertTrue(expected.getMessage().contains("unsupported verifier-registry label"));
    }

    try {
      VpnJsonParser.parseProfile(bytes(vpnProfileWithRelayEndpoint("   ")));
      fail("VPN parser must reject blank required strings");
    } catch (final IllegalStateException expected) {
      assertTrue(expected.getMessage().contains("relay_endpoint"));
    }
    try {
      SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
          bytes(soracloudExecuteResponseJsonWithBlankReceiptId()));
      fail("Soracloud parser must reject blank required strings");
    } catch (final IllegalStateException expected) {
      assertTrue(expected.getMessage().contains("receipt_id"));
    }

    final FakeHttpTransportExecutor executor = new FakeHttpTransportExecutor();
    final TransportRequest request =
        TransportRequest.builder()
            .setUri(URI.create("https://example.test/default"))
            .setMethod("GET")
            .build();
    final TransportResponse response = executor.execute(request).get();
    assertTrue("fake transport default response must stay OK", response.statusCode() == 200);
    assertTrue("fake transport default headers must be empty", response.headers().isEmpty());
  }

  @Test
  public void transactionStatusExceptionsDropWhitespaceDetails() {
    final TransactionStatusException statusException =
        new TransactionStatusException("ab12", "Rejected", " \t\n", new LinkedHashMap<>());
    final TransactionStatusHttpException httpException =
        new TransactionStatusHttpException("ab12", 429, " \t\n", " \t\n");

    assertFalse(
        "blank rejection reasons must normalize to absent",
        statusException.rejectionReason().isPresent());
    assertFalse(
        "blank reject codes must normalize to absent", httpException.rejectCode().isPresent());
    assertFalse(
        "blank response bodies must normalize to absent", httpException.responseBody().isPresent());
  }

  @Test
  public void executableIvmReturnsImmutableEmptyInstructions() {
    final List<InstructionBox> instructions = Executable.ivm(new byte[] {1, 2, 3}).instructions();

    assertTrue("IVM executable must expose no instructions", instructions.isEmpty());
    try {
      instructions.add(InstructionBox.fromWirePayload("iroha.test", new byte[] {1}));
      fail("empty instruction list must be immutable");
    } catch (final UnsupportedOperationException expected) {
      // Expected immutable list behavior.
    }
  }

  @Test
  public void instructionBoxRejectsWhitespaceWireName() {
    try {
      InstructionBox.fromWirePayload(" \t\n", new byte[] {1});
      fail("blank wire names must be rejected");
    } catch (final IllegalArgumentException expected) {
      assertTrue(expected.getMessage().contains("wireName"));
    }
  }

  @Test
  public void transportWrappersCopyNullHeaderValuesAsImmutableEmptyLists() throws Exception {
    final Map<String, List<String>> headers = new LinkedHashMap<>();
    headers.put("X-Test", null);

    final TransportRequest request = TransportRequest.builder().setHeaders(headers).build();
    assertTrue(request.headers().get("X-Test").isEmpty());
    assertImmutable(request.headers().get("X-Test"));

    final TransportResponse response = new TransportResponse(202, null, null, headers);
    assertTrue(response.headers().get("X-Test").isEmpty());
    assertImmutable(response.headers().get("X-Test"));

    try (TransportStreamResponse streamResponse =
        new TransportStreamResponse(
            202, new ByteArrayInputStream(new byte[0]), null, headers, null)) {
      assertTrue(streamResponse.headers().get("X-Test").isEmpty());
      assertImmutable(streamResponse.headers().get("X-Test"));
    }
  }

  @Test
  public void transportSecurityDetectsCredentialHeaders() {
    final Map<String, String> headers = new LinkedHashMap<>();
    headers.put("Authorization", "Bearer secret");

    assertTrue(TransportSecurity.headersContainCredentials(headers));
    try {
      TransportSecurity.requireHttpRequestAllowed(
          "java8-test",
          URI.create("https://torii.example"),
          URI.create("http://torii.example"),
          headers,
          null);
      fail("credentialed HTTP request must be rejected");
    } catch (final IllegalArgumentException expected) {
      assertTrue(expected.getMessage().contains("refuses insecure transport"));
    }
  }

  private static void assertImmutable(final List<String> values) {
    try {
      values.add("mutated");
      fail("header values must be immutable");
    } catch (final UnsupportedOperationException expected) {
      // Expected immutable list behavior.
    }
  }

  private static byte[] repeatedByte(final int value) {
    final byte[] out = new byte[32];
    java.util.Arrays.fill(out, (byte) value);
    return out;
  }

  private static byte[] bytes(final String value) {
    return value.getBytes(StandardCharsets.UTF_8);
  }

  private static String vpnProfileWithRelayEndpoint(final String relayEndpoint) {
    return "{"
        + "\"available\":true,"
        + "\"relay_endpoint\":\""
        + relayEndpoint
        + "\","
        + "\"supported_exit_classes\":[\"standard\",\"low-latency\",\"high-security\"],"
        + "\"default_exit_class\":\"standard\","
        + "\"lease_secs\":600,"
        + "\"dns_push_interval_secs\":60,"
        + "\"meter_family\":\"soranet.vpn.standard\","
        + "\"route_pushes\":[\"0.0.0.0/0\"],"
        + "\"excluded_routes\":[\"10.0.0.0/8\"],"
        + "\"dns_servers\":[\"1.1.1.1\"],"
        + "\"tunnel_addresses\":[\"10.208.0.2/32\"],"
        + "\"mtu_bytes\":1280,"
        + "\"display_billing_label\":\"standard XOR\","
        + "\"fee_asset_id\":\"xor#universal.universal\","
        + "\"escrow_account_id\":\"sorauEscrow\","
        + "\"operator_account_id\":\"sorauOperator\","
        + "\"lease_fee\":\"1000000.25\","
        + "\"settlement_grace_secs\":120,"
        + "\"flow_label_bits\":24,"
        + "\"padding_budget_ms\":15,"
        + "\"relay_tls_spki_sha256_hex\":null"
        + "}";
  }

  private static String soracloudExecuteResponseJsonWithBlankReceiptId() {
    return "{"
        + "\"schema_version\":1,"
        + "\"status\":{\"status\":\"finalized\",\"service_name\":\"portal\"},"
        + "\"receipt\":{"
        + "\"schema_version\":1,"
        + "\"receipt_id\":\"   \","
        + "\"service_name\":\"portal\","
        + "\"model_id\":\"upload-1\","
        + "\"weight_version\":\"v1\","
        + "\"runtime_version\":\"soracloud.private.quantized_cpu.v1\","
        + "\"model_manifest_digest\":\"model-manifest\","
        + "\"model_bundle_root\":\"bundle-root\","
        + "\"policy_id\":\"policy-1\","
        + "\"input_artifact\":{"
        + "\"schema_version\":1,"
        + "\"sorafs_manifest_digest\":\"input-manifest\","
        + "\"artifact_hash\":\"input-artifact\","
        + "\"ciphertext_bytes\":64,"
        + "\"artifact_role\":\"input\""
        + "},"
        + "\"output_artifact\":{"
        + "\"schema_version\":1,"
        + "\"sorafs_manifest_digest\":\"output-manifest\","
        + "\"artifact_hash\":\"output-artifact\","
        + "\"ciphertext_bytes\":96,"
        + "\"artifact_role\":\"output\""
        + "},"
        + "\"input_commitment\":\"input-commitment\","
        + "\"output_commitment\":\"output-commitment\","
        + "\"request_commitment\":\"request-commitment\","
        + "\"result_commitment\":\"result-commitment\","
        + "\"emitted_sequence\":17"
        + "}"
        + "}";
  }
}
