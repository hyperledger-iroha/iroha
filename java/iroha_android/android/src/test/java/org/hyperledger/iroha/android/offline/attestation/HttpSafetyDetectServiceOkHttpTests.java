package org.hyperledger.iroha.android.offline.attestation;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;

import java.time.Duration;
import java.util.List;
import java.util.concurrent.TimeUnit;
import okhttp3.OkHttpClient;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutor;
import org.junit.Test;

/** Verifies Safety Detect flows run over the OkHttp transport and reuse cached OAuth tokens. */
public final class HttpSafetyDetectServiceOkHttpTests {

  @Test
  public void okHttpExecutorCachesOauthAcrossAttestations() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setBody("{\"access_token\":\"token-1\",\"expires_in\":3600}"));
      server.enqueue(new MockResponse().setResponseCode(200).setBody("{\"token\":\"att-1\"}"));
      server.enqueue(new MockResponse().setResponseCode(200).setBody("{\"token\":\"att-2\"}"));
      server.start();

      final SafetyDetectOptions options =
          SafetyDetectOptions.builder()
              .setOauthEndpoint(server.url("/oauth").uri())
              .setAttestationEndpoint(server.url("/attest").uri())
              .setClientId("client-id")
              .setClientSecret("client-secret")
              .setPackageName("com.example.app")
              .setSigningDigestSha256("deadbeef")
              .setRequestTimeout(Duration.ofSeconds(2))
              .build();
      final HttpSafetyDetectService service =
          new HttpSafetyDetectService(
              new OkHttpTransportExecutor(new OkHttpClient()), options);

      final SafetyDetectRequest request =
          SafetyDetectRequest.builder()
              .setCertificateIdHex("aa01")
              .setAppId("app-123")
              .setNonceHex("a1b2c3d4")
              .setPackageName(options.packageName())
              .setSigningDigestSha256(options.signingDigestSha256())
              .setRequiredEvaluations(List.of("basic"))
              .build();

      final SafetyDetectAttestation first = service.fetch(request).get(2, TimeUnit.SECONDS);
      final SafetyDetectAttestation second = service.fetch(request).get(2, TimeUnit.SECONDS);
      assertEquals("att-1", first.token());
      assertEquals("att-2", second.token());

      final RecordedRequest oauth = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull(oauth);
      assertEquals("/oauth", oauth.getPath());
      assertEquals("application/x-www-form-urlencoded", oauth.getHeader("Content-Type"));
      final String oauthBody = oauth.getBody().readUtf8();
      // Grant flow should include credentials and client grant type.
      assertTrue(oauthBody.contains("grant_type=client_credentials"));
      assertTrue(oauthBody.contains("client_id=client-id"));
      assertTrue(oauthBody.contains("client_secret=client-secret"));

      final RecordedRequest attestation1 = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull(attestation1);
      assertEquals("/attest", attestation1.getPath());
      assertEquals("Bearer token-1", attestation1.getHeader("Authorization"));
      assertAttestationPayload(attestation1.getBody().readUtf8(), request);

      final RecordedRequest attestation2 = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull(attestation2);
      assertEquals("/attest", attestation2.getPath());
      assertEquals("Bearer token-1", attestation2.getHeader("Authorization"));

      // No additional OAuth refresh should have been issued.
      assertNull(server.takeRequest(100, TimeUnit.MILLISECONDS));
    }
  }

  private static void assertAttestationPayload(
      final String json, final SafetyDetectRequest request) {
    final Object parsed = JsonParser.parse(json);
    if (!(parsed instanceof java.util.Map<?, ?> map)) {
      throw new AssertionError("expected attestation payload to be a JSON object");
    }
    final java.util.Map<?, ?> payload = map;
    assertEquals(request.appId(), payload.get("app_id"));
    assertEquals(request.packageName(), payload.get("package_name"));
    assertEquals(request.signingDigestSha256(), payload.get("sign_cert_sha256"));
    assertEquals(request.certificateIdHex(), payload.get("certificate_id_hex"));
    assertEquals(request.maxTokenAgeMs(), payload.get("max_token_age_ms"));
  }
}
