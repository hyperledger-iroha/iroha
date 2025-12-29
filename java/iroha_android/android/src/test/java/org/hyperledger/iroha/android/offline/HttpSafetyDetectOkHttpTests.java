package org.hyperledger.iroha.android.offline;

import java.time.Duration;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutorFactory;
import org.hyperledger.iroha.android.offline.attestation.HttpSafetyDetectService;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectAttestation;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectOptions;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectRequest;

/** End-to-end coverage for {@link HttpSafetyDetectService} using OkHttp transports. */
public final class HttpSafetyDetectOkHttpTests {

  private HttpSafetyDetectOkHttpTests() {}

  public static void main(final String[] args) throws Exception {
    fetchesAttestationViaOkHttp();
    System.out.println("[IrohaAndroid] HttpSafetyDetectOkHttpTests passed.");
  }

  private static void fetchesAttestationViaOkHttp() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setBody("{\"access_token\":\"oauth-token\",\"expires_in\":120}"));
      server.enqueue(new MockResponse().setResponseCode(200).setBody("{\"token\":\"attested\"}"));
      server.start();

      final SafetyDetectOptions options =
          SafetyDetectOptions.builder()
              .setEnabled(true)
              .setOauthEndpoint(server.url("/oauth/token").uri())
              .setAttestationEndpoint(server.url("/attest").uri())
              .setClientId("client-id")
              .setClientSecret("secret")
              .setPackageName("org.example.app")
              .setSigningDigestSha256("cafebabe")
              .setRequestTimeout(Duration.ofSeconds(2))
              .build();
      final HttpSafetyDetectService service =
          new HttpSafetyDetectService(OkHttpTransportExecutorFactory.createDefault(), options);

      final SafetyDetectRequest request =
          SafetyDetectRequest.builder()
              .setCertificateIdHex("deadbeef")
              .setAppId("app-id")
              .setNonceHex("0011")
              .setPackageName("org.example.app")
              .setSigningDigestSha256("cafebabe")
              .build();

      final SafetyDetectAttestation attestation = service.fetch(request).join();
      assert "attested".equals(attestation.token()) : "attestation token should match response";

      final RecordedRequest oauth = server.takeRequest();
      assert "/oauth/token".equals(oauth.getPath()) : "oauth path mismatch";
      assert "POST".equals(oauth.getMethod()) : "oauth should use POST";
      final String oauthBody = oauth.getBody().readUtf8();
      assert oauthBody.contains("grant_type=client_credentials") : "missing grant_type";
      assert oauthBody.contains("client_id=client-id") : "missing client_id";
      assert oauthBody.contains("client_secret=secret") : "missing client_secret";

      final RecordedRequest attest = server.takeRequest();
      assert "/attest".equals(attest.getPath()) : "attestation path mismatch";
      assert "Bearer oauth-token".equals(attest.getHeader("Authorization"))
          : "attestation should carry bearer token";
    }
  }
}
