package org.hyperledger.iroha.android.sorafs;

import java.util.Base64;
import java.util.Map;

public final class SorafsGatewayFetchOptionsTests {

  private static final String MANIFEST_ID_HEX =
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
  private static final String PROVIDER_ID_HEX =
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
  private static final String GATEWAY_PUBLIC_KEY_HEX =
      "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
  private static final String MANIFEST_CID_HEX =
      "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";
  private static final String CHUNKER_HANDLE = "sorafs.sf1@1.0.0";
  private static final String STREAM_TOKEN_BASE64 = "c3RyZWFtLXRva2Vu";

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
    rejectsInvalidGatewayPublicKeyHex();
    rejectsInvalidStreamTokenBase64();
    rejectsInvalidManifestIdHex();
    rejectsShortManifestIdHex();
    rejectsInvalidManifestEnvelopeBase64();
    rejectsInvalidManifestCidHex();
    rejectsInvalidNumericOptions();
    rejectsNonCanonicalProviderIds();
    rejectsNonCanonicalBase64Inputs();
    rejectsNonExactProviderText();
    rejectsOversizedProviderInputsAndProviderLists();
    rejectsNonCanonicalManifestIdsAndCids();
    rejectsNonExactOptionalText();
    rejectsNonCanonicalRolloutPhases();
    rejectsNonCanonicalChunkerHandles();
    canonicalValuesArePreservedWithoutRewriting();
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
            .setManifestCidHex(MANIFEST_CID_HEX)
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
    assert MANIFEST_CID_HEX.equals(json.get("manifest_cid_hex"));
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
            .setGatewayPublicKeyHex(GATEWAY_PUBLIC_KEY_HEX)
            .setBaseUrl("https://gateway.example/")
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
    assert GATEWAY_PUBLIC_KEY_HEX.equals(providerJson.get("gateway_public_key_hex"));
    assert "https://gateway.example/".equals(providerJson.get("base_url"));
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

  private static void rejectsInvalidGatewayPublicKeyHex() {
    assertThrows(
        () -> GatewayProvider.builder().setName("primary").setGatewayPublicKeyHex("zz"),
        "expected invalid gatewayPublicKeyHex to throw");
    assertThrows(
        () -> GatewayProvider.builder().setName("primary").setGatewayPublicKeyHex("aa".repeat(16)),
        "expected short gatewayPublicKeyHex to throw");
  }

  private static void rejectsInvalidStreamTokenBase64() {
    assertThrows(
        () ->
            GatewayProvider.builder()
                .setName("primary")
                .setProviderIdHex("01".repeat(32))
                .setGatewayPublicKeyHex(GATEWAY_PUBLIC_KEY_HEX)
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

  private static void rejectsNonCanonicalProviderIds() {
    for (final String value : nonCanonicalHex32()) {
      assertThrows(
          () ->
              GatewayProvider.builder()
                  .setName("primary")
                  .setProviderIdHex(value)
                  .setGatewayPublicKeyHex(GATEWAY_PUBLIC_KEY_HEX)
                  .setBaseUrl("https://gateway.example")
                  .setStreamTokenBase64(STREAM_TOKEN_BASE64)
                  .build(),
          "expected noncanonical provider id to throw: " + printable(value));
    }
  }

  private static void rejectsNonCanonicalBase64Inputs() {
    for (final String value : nonCanonicalBase64()) {
      assertThrows(
          () ->
              GatewayProvider.builder()
                  .setName("primary")
                  .setProviderIdHex(PROVIDER_ID_HEX)
                  .setGatewayPublicKeyHex(GATEWAY_PUBLIC_KEY_HEX)
                  .setBaseUrl("https://gateway.example")
                  .setStreamTokenBase64(value)
                  .build(),
          "expected noncanonical stream token to throw: " + printable(value));
      assertThrows(
          () -> GatewayFetchOptions.builder().setManifestEnvelopeBase64(value),
          "expected noncanonical manifest envelope to throw: " + printable(value));
    }
  }

  private static void rejectsNonExactProviderText() {
    for (final String value : nonExactText()) {
      assertThrows(
          () -> GatewayProvider.builder().setName(value),
          "expected non-exact provider name to throw: " + printable(value));
    }
    for (final String value : nonExactUrls()) {
      assertThrows(
          () -> GatewayProvider.builder().setBaseUrl(value),
          "expected non-exact provider URL to throw: " + printable(value));
    }
  }

  private static void rejectsOversizedProviderInputsAndProviderLists() {
    GatewayProvider.builder()
        .setName("primary")
        .setProviderIdHex(PROVIDER_ID_HEX)
        .setGatewayPublicKeyHex(GATEWAY_PUBLIC_KEY_HEX)
        .setBaseUrl("https://gateway.example/")
        .setStreamTokenBase64(Base64.getEncoder().encodeToString(new byte[2 * 1024]))
        .build();
    assertThrows(
        () -> GatewayProvider.builder().setName("a".repeat(129)),
        "expected oversized provider name to throw");
    assertThrows(
        () -> GatewayProvider.builder().setStreamTokenBase64("A".repeat(4 * 1024 + 1)),
        "expected oversized stream token to throw before decoding");
    assertThrows(
        () ->
            GatewayProvider.builder()
                .setStreamTokenBase64(
                    Base64.getEncoder().encodeToString(new byte[2 * 1024 + 1])),
        "expected decoded stream token above the wire ceiling to throw");

    final GatewayProvider provider =
        GatewayProvider.builder()
            .setName("primary")
            .setProviderIdHex(PROVIDER_ID_HEX)
            .setGatewayPublicKeyHex(GATEWAY_PUBLIC_KEY_HEX)
            .setBaseUrl("https://gateway.example/")
            .setStreamTokenBase64(STREAM_TOKEN_BASE64)
            .build();
    final GatewayFetchRequest.Builder request =
        GatewayFetchRequest.builder().setManifestIdHex(MANIFEST_ID_HEX);
    for (int i = 0; i < 256; i++) {
      request.addProvider(provider);
    }
    assertIllegalState(
        () -> request.addProvider(provider),
        "expected the 257th provider to be rejected");
  }

  private static void rejectsNonCanonicalManifestIdsAndCids() {
    for (final String value : nonCanonicalHex32()) {
      assertThrows(
          () -> GatewayFetchRequest.builder().setManifestIdHex(value),
          "expected noncanonical manifest id to throw: " + printable(value));
      assertThrows(
          () -> GatewayFetchOptions.builder().setManifestCidHex(value),
          "expected noncanonical manifest CID to throw: " + printable(value));
    }
  }

  private static void rejectsNonExactOptionalText() {
    for (final String value : nonExactText()) {
      assertThrows(
          () -> GatewayFetchOptions.builder().setClientId(value),
          "expected non-exact client id to throw: " + printable(value));
      assertThrows(
          () -> GatewayFetchOptions.builder().setTelemetryRegion(value),
          "expected non-exact telemetry region to throw: " + printable(value));
    }
  }

  private static void rejectsNonCanonicalRolloutPhases() {
    final String[] values = {
      "", " ", " ramp", "ramp ", "RAMP", "stage-b", "stage_b", "ga", "stable", "unknown"
    };
    for (final String value : values) {
      assertThrows(
          () -> GatewayFetchOptions.builder().setRolloutPhase(value),
          "expected noncanonical rollout phase to throw: " + printable(value));
    }
  }

  private static void rejectsNonCanonicalChunkerHandles() {
    for (final String value : nonCanonicalChunkerHandles()) {
      assertThrows(
          () -> GatewayFetchRequest.builder().setChunkerHandle(value),
          "expected noncanonical chunker handle to throw: " + printable(value));
    }
  }

  private static void canonicalValuesArePreservedWithoutRewriting() {
    final GatewayProvider provider =
        GatewayProvider.builder()
            .setName("primary")
            .setProviderIdHex(PROVIDER_ID_HEX)
            .setGatewayPublicKeyHex(GATEWAY_PUBLIC_KEY_HEX)
            .setBaseUrl("https://gateway.example/")
            .setStreamTokenBase64(STREAM_TOKEN_BASE64)
            .build();
    final GatewayFetchOptions options =
        GatewayFetchOptions.builder()
            .setManifestEnvelopeBase64("YQ==")
            .setManifestCidHex(MANIFEST_CID_HEX)
            .setClientId("android-sdk")
            .setTelemetryRegion("ap-northeast-1")
            .setRolloutPhase("ramp")
            .build();
    final GatewayFetchRequest request =
        GatewayFetchRequest.builder()
            .setManifestIdHex(MANIFEST_ID_HEX)
            .setChunkerHandle(CHUNKER_HANDLE)
            .setOptions(options)
            .addProvider(provider)
            .build();

    assert PROVIDER_ID_HEX.equals(provider.providerIdHex());
    assert GATEWAY_PUBLIC_KEY_HEX.equals(provider.gatewayPublicKeyHex());
    assert STREAM_TOKEN_BASE64.equals(provider.streamTokenBase64());
    assert "YQ==".equals(options.manifestEnvelopeBase64());
    assert MANIFEST_CID_HEX.equals(options.manifestCidHex());
    assert "android-sdk".equals(options.clientId());
    assert "ap-northeast-1".equals(options.telemetryRegion());
    assert "ramp".equals(options.rolloutPhase());
    assert MANIFEST_ID_HEX.equals(request.manifestIdHex());
    assert CHUNKER_HANDLE.equals(request.chunkerHandle());
  }

  private static String[] nonCanonicalHex32() {
    return new String[] {
      MANIFEST_ID_HEX.toUpperCase(java.util.Locale.ROOT),
      "0x" + MANIFEST_ID_HEX,
      "0X" + MANIFEST_ID_HEX,
      " " + MANIFEST_ID_HEX,
      MANIFEST_ID_HEX + " ",
      MANIFEST_ID_HEX + "\n",
      MANIFEST_ID_HEX.substring(0, 63),
      MANIFEST_ID_HEX + "00",
      MANIFEST_ID_HEX.substring(0, 62) + "gg",
      "00".repeat(32)
    };
  }

  private static String[] nonCanonicalBase64() {
    return new String[] {
      "", " ", "YQ", " YQ==", "YQ== ", "Y Q==", "YQ==\n", "YQ===", "YR==", "_w==",
      "-w==", "not!base64"
    };
  }

  private static String[] nonExactText() {
    return new String[] {
      "", " ", " primary", "primary ", "\u00a0primary", "primary\u2003", "primary\nbeta",
      "primary\u0000beta"
    };
  }

  private static String[] nonExactUrls() {
    return new String[] {
      "",
      " ",
      " https://gateway.example",
      "https://gateway.example ",
      "\u00a0https://gateway.example",
      "https://gateway.example\u2003",
      "https://gateway.example/\npath",
      "http://gateway.example/",
      "https://user@gateway.example/",
      "https://gateway.example:443/",
      "https://gateway.example:444/",
      "https://Gateway.Example/",
      "https://gateway.example/path",
      "https://gateway.example/?query=1",
      "https://gateway.example/#fragment",
      "https://localhost/",
      "https://127.0.0.1/",
      "https://10.0.0.1/",
      "https://169.254.169.254/",
      "https://192.0.2.1/",
      "https://198.51.100.1/",
      "https://203.0.113.1/",
      "https://[::1]/",
      "https://[fc00::1]/",
      "https://[fe80::1]/",
      "https://[2001:db8::1]/",
      "https://[::ffff:127.0.0.1]/"
    };
  }

  private static String[] nonCanonicalChunkerHandles() {
    return new String[] {
      "",
      " ",
      " " + CHUNKER_HANDLE,
      CHUNKER_HANDLE + " ",
      "Sorafs.sf1@1.0.0",
      "sorafs.SF1@1.0.0",
      "sorafs/sf1@1.0.0",
      "sorafs-sf1",
      "sorafs.sf1",
      ".sf1@1.0.0",
      "sorafs.@1.0.0",
      "sorafs.sf1@",
      "sorafs.sf1@1.0",
      "sorafs.sf1@1.0.0.0",
      "sorafs.sf1@01.0.0",
      "sorafs.sf1@1.00.0",
      "sorafs.sf1@1.0.00",
      "sorafs.sf1@+1.0.0",
      "sorafs.sf1@1.0.0-beta",
      "sorafs..sf1@1.0.0",
      "sorafs.sf1@1.0.0@extra",
      "sorafs.sf1@\u0661.0.0",
      "sorafs.sf1@1.0.0\n"
    };
  }

  private static String printable(final String value) {
    return value.replace("\n", "\\n").replace("\u0000", "\\0");
  }

  private static void assertThrows(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final IllegalArgumentException expected) {
      return;
    }
    throw new AssertionError(message);
  }

  private static void assertIllegalState(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final IllegalStateException expected) {
      return;
    }
    throw new AssertionError(message);
  }
}
