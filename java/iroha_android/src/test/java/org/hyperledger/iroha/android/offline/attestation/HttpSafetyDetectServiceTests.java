package org.hyperledger.iroha.android.offline.attestation;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Deque;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

public final class HttpSafetyDetectServiceTests {

  private HttpSafetyDetectServiceTests() {}

  public static void main(final String[] args) {
    runsAttestationFlow();
    cachesTokens();
    handlesErrors();
    rejectsFractionalExpiry();
    System.out.println("[IrohaAndroid] HttpSafetyDetectServiceTests passed.");
  }

  private static void runsAttestationFlow() {
    final FakeExecutor executor = new FakeExecutor();
    executor.enqueueJson(200, "{\"access_token\":\"token-123\",\"expires_in\":3600}");
    executor.enqueueJson(200, "{\"token\":\"jws-token\"}");
    final SafetyDetectOptions options =
        SafetyDetectOptions.builder()
            .setClientId("client")
            .setClientSecret("secret")
            .setPackageName("pkg")
            .setSigningDigestSha256("abcd")
            .build();
    final HttpSafetyDetectService service = new HttpSafetyDetectService(executor, options);
    final SafetyDetectRequest request =
        SafetyDetectRequest.builder()
            .setCertificateIdHex("deadbeef")
            .setAppId("app")
            .setNonceHex("00ff")
            .setPackageName("pkg")
            .setSigningDigestSha256("abcd")
            .build();
    final SafetyDetectAttestation attestation = service.fetch(request).join();
    assert "jws-token".equals(attestation.token());
    assert attestation.fetchedAtMs() > 0;
    final List<TransportRequest> calls = executor.requests();
    assert calls.size() == 2;
    final String attestationBody =
        new String(executor.requestBodies().get(1), StandardCharsets.UTF_8);
    assert attestationBody.contains("\"app_id\":\"app\"");
    assert attestationBody.contains(
        Base64.getEncoder().encodeToString(new byte[] {0, (byte) 0xFF}));
  }

  private static void cachesTokens() {
    final FakeExecutor executor = new FakeExecutor();
    executor.enqueueJson(200, "{\"access_token\":\"token-abc\",\"expires_in\":3600}");
    executor.enqueueJson(200, "{\"token\":\"first\"}");
    executor.enqueueJson(200, "{\"token\":\"second\"}");
    final SafetyDetectOptions options =
        SafetyDetectOptions.builder()
            .setClientId("client")
            .setClientSecret("secret")
            .setPackageName("pkg")
            .setSigningDigestSha256("abcd")
            .build();
    final HttpSafetyDetectService service = new HttpSafetyDetectService(executor, options);
    final SafetyDetectRequest request =
        SafetyDetectRequest.builder()
            .setCertificateIdHex("deadbeef")
            .setAppId("app")
            .setNonceHex("00ff")
            .setPackageName("pkg")
            .setSigningDigestSha256("abcd")
            .build();
    final SafetyDetectAttestation first = service.fetch(request).join();
    assert "first".equals(first.token());
    final SafetyDetectAttestation second = service.fetch(request).join();
    assert "second".equals(second.token());
    assert executor.requests().size() == 3 : "token request should be cached";
  }

  private static void handlesErrors() {
    final FakeExecutor executor = new FakeExecutor();
    executor.enqueueJson(500, "{\"error\":\"boom\"}");
    final SafetyDetectOptions options =
        SafetyDetectOptions.builder()
            .setClientId("client")
            .setClientSecret("secret")
            .setPackageName("pkg")
            .setSigningDigestSha256("abcd")
            .build();
    final HttpSafetyDetectService service = new HttpSafetyDetectService(executor, options);
    final SafetyDetectRequest request =
        SafetyDetectRequest.builder()
            .setCertificateIdHex("deadbeef")
            .setAppId("app")
            .setNonceHex("00ff")
            .setPackageName("pkg")
            .setSigningDigestSha256("abcd")
            .build();
    boolean thrown = false;
    try {
      service.fetch(request).join();
    } catch (Exception ex) {
      thrown = true;
    }
    assert thrown : "expected exception when HTTP request fails";
  }

  private static void rejectsFractionalExpiry() {
    final FakeExecutor executor = new FakeExecutor();
    executor.enqueueJson(200, "{\"access_token\":\"token-123\",\"expires_in\":3600.5}");
    final SafetyDetectOptions options =
        SafetyDetectOptions.builder()
            .setClientId("client")
            .setClientSecret("secret")
            .setPackageName("pkg")
            .setSigningDigestSha256("abcd")
            .build();
    final HttpSafetyDetectService service = new HttpSafetyDetectService(executor, options);
    final SafetyDetectRequest request =
        SafetyDetectRequest.builder()
            .setCertificateIdHex("deadbeef")
            .setAppId("app")
            .setNonceHex("00ff")
            .setPackageName("pkg")
            .setSigningDigestSha256("abcd")
            .build();
    boolean thrown = false;
    try {
      service.fetch(request).join();
    } catch (Exception ex) {
      thrown = true;
    }
    assert thrown : "expected fractional expiry to be rejected";
  }

  private static final class FakeExecutor implements HttpTransportExecutor {
    private final Deque<TransportResponse> responses = new ArrayDeque<>();
    private final List<TransportRequest> requests = new ArrayList<>();
    private final List<byte[]> bodies = new ArrayList<>();

    void enqueueJson(final int status, final String json) {
      responses.add(new TransportResponse(status, json.getBytes(StandardCharsets.UTF_8), "", Map.of()));
    }

    List<TransportRequest> requests() {
      return requests;
    }

    List<byte[]> requestBodies() {
      return bodies;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requests.add(request);
      bodies.add(request.body());
      final TransportResponse response = responses.removeFirst();
      return CompletableFuture.completedFuture(response);
    }
  }
}
