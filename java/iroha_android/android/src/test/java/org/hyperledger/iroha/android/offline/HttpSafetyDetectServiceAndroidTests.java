package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertEquals;

import java.net.URI;
import java.time.Duration;
import java.util.concurrent.TimeUnit;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import org.hyperledger.iroha.android.offline.attestation.HttpSafetyDetectService;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectAttestation;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectOptions;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectRequest;
import org.junit.Test;

/** Android-specific coverage for the OkHttp-backed Safety Detect service. */
public final class HttpSafetyDetectServiceAndroidTests {

  @Test
  public void defaultExecutorFetchesAttestation() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setBody(
                  """
                  {
                    "access_token": "token-android",
                    "expires_in": 60
                  }
                  """));
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setBody("{\"token\":\"attest-android\"}"));
      server.start();

      final URI oauth = server.url("/oauth/token").uri();
      final URI attest = server.url("/attest").uri();
      final SafetyDetectOptions options =
          SafetyDetectOptions.builder()
              .setEnabled(true)
              .setOauthEndpoint(oauth)
              .setAttestationEndpoint(attest)
              .setClientId("client-id")
              .setClientSecret("secret")
              .setPackageName("org.example.app")
              .setSigningDigestSha256("abcdef")
              .setRequestTimeout(Duration.ofSeconds(5))
              .build();

      final HttpSafetyDetectService service = HttpSafetyDetectService.createDefault(options);
      final SafetyDetectRequest request =
          SafetyDetectRequest.builder()
              .setCertificateIdHex("deadbeef")
              .setAppId("app-id")
              .setNonceHex("00")
              .setPackageName("org.example.app")
              .setSigningDigestSha256("abcdef")
              .build();

      final SafetyDetectAttestation attestation =
          service.fetch(request).get(2, TimeUnit.SECONDS);
      assertEquals("attest-android", attestation.token());

      assertEquals("/oauth/token", server.takeRequest().getPath());
      assertEquals("/attest", server.takeRequest().getPath());
    }
  }
}
