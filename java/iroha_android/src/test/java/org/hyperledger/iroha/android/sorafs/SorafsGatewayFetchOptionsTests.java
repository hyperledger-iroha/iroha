package org.hyperledger.iroha.android.sorafs;

import java.util.Map;

public final class SorafsGatewayFetchOptionsTests {

  private SorafsGatewayFetchOptionsTests() {}

  public static void main(final String[] args) {
    transportPolicyParsesLabels();
    anonymityPolicyParsesLabels();
    writeModeHintParsesLabels();
    builderSerialisesExplicitOptions();
    defaultsProvideGuardPolicy();
    gatewayFetchRequestSerialisesProviders();
    rejectsInvalidProviderIdHex();
    rejectsShortProviderIdHex();
    rejectsInvalidStreamTokenBase64();
    rejectsInvalidManifestIdHex();
    rejectsShortManifestIdHex();
    rejectsInvalidManifestEnvelopeBase64();
    rejectsInvalidManifestCidHex();
    rejectsInvalidNumericOptions();
    System.out.println("[IrohaAndroid] SoraFS gateway option tests passed.");
  }

  private static void transportPolicyParsesLabels() {
    assert TransportPolicy.SORANET_FIRST == TransportPolicy.fromLabel("soranet-first");
    assert TransportPolicy.SORANET_FIRST == TransportPolicy.fromLabel("soranet_first");
    assert TransportPolicy.SORANET_STRICT == TransportPolicy.fromLabel("soranet-strict");
    assert TransportPolicy.SORANET_STRICT == TransportPolicy.fromLabel("soranet_strict");
    assert TransportPolicy.DIRECT_ONLY == TransportPolicy.fromLabel("DIRECT-ONLY");
    assert TransportPolicy.fromLabel("unknown") == null;
  }

  private static void anonymityPolicyParsesLabels() {
    assert AnonymityPolicy.ANON_GUARD_PQ == AnonymityPolicy.fromLabel("stage-a");
    assert AnonymityPolicy.ANON_GUARD_PQ == AnonymityPolicy.fromLabel("anon_guard_pq");
    assert AnonymityPolicy.ANON_MAJORIY_PQ
        == AnonymityPolicy.fromLabel("ANON-MAJORITY-PQ");
    assert AnonymityPolicy.ANON_MAJORIY_PQ == AnonymityPolicy.fromLabel("stageb");
    assert AnonymityPolicy.ANON_STRICT_PQ == AnonymityPolicy.fromLabel("anon_strict_pq");
    assert AnonymityPolicy.ANON_STRICT_PQ == AnonymityPolicy.fromLabel("stage_c");
  }

  private static void writeModeHintParsesLabels() {
    assert WriteModeHint.READ_ONLY == WriteModeHint.fromLabel("read-only");
    assert WriteModeHint.READ_ONLY == WriteModeHint.fromLabel("read_only");
    assert WriteModeHint.UPLOAD_PQ_ONLY == WriteModeHint.fromLabel("UPLOAD-PQ-ONLY");
    assert WriteModeHint.UPLOAD_PQ_ONLY == WriteModeHint.fromLabel("upload_pq_only");
    assert WriteModeHint.fromLabel("unknown") == null;
  }

  private static void builderSerialisesExplicitOptions() {
    final GatewayFetchOptions options =
        GatewayFetchOptions.builder()
            .setManifestEnvelopeBase64("ZXhhbXBsZQ==")
            .setManifestCidHex("deadbeef")
            .setClientId("android-sdk")
            .setTelemetryRegion("ap-northeast-1")
            .setRolloutPhase("ramp")
            .setMaxPeers(3)
            .setRetryBudget(5)
            .setTransportPolicy(TransportPolicy.DIRECT_ONLY)
            .setAnonymityPolicy(AnonymityPolicy.ANON_MAJORIY_PQ)
            .setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)
            .build();

    final Map<String, Object> json = options.toJson();
    assert "ZXhhbXBsZQ==".equals(json.get("manifest_envelope_b64"));
    assert "deadbeef".equals(json.get("manifest_cid_hex"));
    assert "android-sdk".equals(json.get("client_id"));
    assert "ap-northeast-1".equals(json.get("telemetry_region"));
    assert "ramp".equals(json.get("rollout_phase"));
    assert Integer.valueOf(3).equals(json.get("max_peers"));
    assert Integer.valueOf(5).equals(json.get("retry_budget"));
    assert "direct-only".equals(json.get("transport_policy"));
    assert "anon-majority-pq".equals(json.get("anonymity_policy"));
    assert "upload_pq_only".equals(json.get("write_mode_hint"));
    assert json.size() == 10 : "expected ten entries in JSON map";
  }

  private static void defaultsProvideGuardPolicy() {
    final GatewayFetchOptions options = GatewayFetchOptions.builder().build();
    final Map<String, Object> json = options.toJson();
    assert "soranet-first".equals(json.get("transport_policy"));
    assert "anon-guard-pq".equals(json.get("anonymity_policy"));
    assert !json.containsKey("write_mode_hint");
  }

  private static void gatewayFetchRequestSerialisesProviders() {
    final GatewayProvider provider =
        GatewayProvider.builder()
            .setName("primary")
            .setProviderIdHex("01".repeat(32))
            .setBaseUrl("https://gateway.example/fetch")
            .setStreamTokenBase64("c3RyZWFt")
            .build();
    final GatewayFetchRequest request =
        GatewayFetchRequest.builder()
            .setManifestIdHex("deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef")
            .setChunkerHandle("sorafs.sf1@1.0.0")
            .setOptions(
                GatewayFetchOptions.builder()
                    .setTelemetryRegion("ap-southeast-1")
                    .setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)
                    .setWriteModeHint(WriteModeHint.READ_ONLY)
                    .build())
            .addProvider(provider)
            .build();

    final Map<String, Object> json = request.toJson();
    assert "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
            .equals(json.get("manifest_id_hex"));
    assert "sorafs.sf1@1.0.0".equals(json.get("chunker_handle"));
    @SuppressWarnings("unchecked")
    final Map<String, Object> options = (Map<String, Object>) json.get("options");
    assert "ap-southeast-1".equals(options.get("telemetry_region"));
    assert "anon-guard-pq".equals(options.get("anonymity_policy"));
    assert !options.containsKey("write_mode_hint");
    @SuppressWarnings("unchecked")
    final Iterable<Map<String, Object>> providers =
        (Iterable<Map<String, Object>>) json.get("providers");
    final Map<String, Object> providerJson = providers.iterator().next();
    assert "primary".equals(providerJson.get("name"));
    assert "01".repeat(32).equals(providerJson.get("provider_id_hex"));
    assert "https://gateway.example/fetch".equals(providerJson.get("base_url"));
    assert "c3RyZWFt".equals(providerJson.get("stream_token_b64"));

    final String jsonString = request.toJsonString();
    assert jsonString.contains(
        "\"manifest_id_hex\":\"deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef\"");
    assert jsonString.contains("\"telemetry_region\":\"ap-southeast-1\"");
    assert jsonString.contains("\"providers\"");
  }

  private static void rejectsInvalidProviderIdHex() {
    assertThrows(
        () -> GatewayProvider.builder().setName("primary").setProviderIdHex("zz"),
        "expected invalid providerIdHex to throw");
  }

  private static void rejectsShortProviderIdHex() {
    assertThrows(
        () -> GatewayProvider.builder().setName("primary").setProviderIdHex("aa".repeat(16)),
        "expected short providerIdHex to throw");
  }

  private static void rejectsInvalidStreamTokenBase64() {
    assertThrows(
        () ->
            GatewayProvider.builder()
                .setName("primary")
                .setProviderIdHex("01".repeat(32))
                .setBaseUrl("https://gateway.example")
                .setStreamTokenBase64("not!base64")
                .build(),
        "expected invalid base64 stream token to throw");
  }

  private static void rejectsInvalidManifestIdHex() {
    assertThrows(
        () -> GatewayFetchRequest.builder().setManifestIdHex("not-hex"),
        "expected invalid manifestIdHex to throw");
  }

  private static void rejectsShortManifestIdHex() {
    assertThrows(
        () -> GatewayFetchRequest.builder().setManifestIdHex("aa".repeat(16)),
        "expected short manifestIdHex to throw");
  }

  private static void rejectsInvalidManifestEnvelopeBase64() {
    assertThrows(
        () -> GatewayFetchOptions.builder().setManifestEnvelopeBase64("not!base64"),
        "expected invalid manifest envelope to throw");
  }

  private static void rejectsInvalidManifestCidHex() {
    assertThrows(
        () -> GatewayFetchOptions.builder().setManifestCidHex("not-hex"),
        "expected invalid manifest cid to throw");
  }

  private static void rejectsInvalidNumericOptions() {
    assertThrows(
        () -> GatewayFetchOptions.builder().setMaxPeers(0),
        "expected maxPeers <= 0 to throw");
    assertThrows(
        () -> GatewayFetchOptions.builder().setRetryBudget(-1),
        "expected negative retryBudget to throw");
  }

  private static void assertThrows(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final IllegalArgumentException expected) {
      return;
    }
    throw new AssertionError(message);
  }
}
