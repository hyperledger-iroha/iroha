package org.hyperledger.iroha.android.sorafs;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;

import java.time.Duration;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;
import okhttp3.OkHttpClient;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutor;
import org.junit.Test;

/** OkHttp-backed regression for SoraFS gateway fetch/summary helpers. */
public final class SorafsGatewayClientOkHttpTests {

  @Test
  public void fetchSummaryUsesOkHttpTransportAndParsesResponse() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      final String summaryJson = JsonWriter.encode(buildSummaryPayload());
      server.enqueue(new MockResponse().setResponseCode(200).setBody(summaryJson));
      server.start();

      final SorafsGatewayClient client =
          SorafsGatewayClient.builder()
              .setExecutor(new OkHttpTransportExecutor(new OkHttpClient()))
              .setBaseUri(server.url("/").uri())
              .setTimeout(Duration.ofSeconds(2))
              .setDefaultHeaders(Map.of("X-Test", "ok"))
              .build();

      final GatewayProvider provider =
          GatewayProvider.builder()
              .setName("p1")
              .setProviderIdHex("aa11".repeat(16))
              .setBaseUrl("https://provider.example")
              .setStreamTokenBase64("c3RyZWFt")
              .build();
      final GatewayFetchRequest request =
          GatewayFetchRequest.builder()
              .setManifestIdHex("feedbeef".repeat(8))
              .setChunkerHandle("chunker-1")
              .setOptions(
                  GatewayFetchOptions.builder()
                      .setClientId("client-1")
                      .setRetryBudget(2)
                      .build())
              .addProvider(provider)
              .build();

      final GatewayFetchSummary summary = client.fetchSummary(request).get(2, TimeUnit.SECONDS);
      assertEquals("feedbeef".repeat(8), summary.manifestIdHex());
      assertEquals("chunker-1", summary.chunkerHandle());
      assertEquals("client-1", summary.clientId());
      assertEquals(2, summary.chunkCount());
      assertEquals(1, summary.providerReports().size());
      assertEquals("p1", summary.providerReports().get(0).provider());

      final RecordedRequest recorded = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull(recorded);
      assertEquals("/v1/sorafs/gateway/fetch", recorded.getPath());
      assertEquals("POST", recorded.getMethod());
      assertEquals("application/json", recorded.getHeader("Content-Type"));
      assertEquals("application/json", recorded.getHeader("Accept"));
      assertEquals("ok", recorded.getHeader("X-Test"));
      assertEquals(request.toJsonString(), recorded.getBody().readUtf8());
    }
  }

  private static Map<String, Object> buildSummaryPayload() {
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("manifest_id_hex", "feedbeef".repeat(8));
    payload.put("chunker_handle", "chunker-1");
    payload.put("client_id", "client-1");
    payload.put("chunk_count", 2L);
    payload.put("content_length", 128L);
    payload.put("assembled_bytes", 64L);
    payload.put("anonymity_policy", "anon-guard-pq");
    payload.put("anonymity_status", "ok");
    payload.put("anonymity_reason", "none");
    payload.put("anonymity_soranet_selected", 1L);
    payload.put("anonymity_pq_selected", 1L);
    payload.put("anonymity_classical_selected", 0L);
    payload.put("anonymity_classical_ratio", 0.0d);
    payload.put("anonymity_pq_ratio", 1.0d);
    payload.put("anonymity_candidate_ratio", 1.0d);
    payload.put("anonymity_deficit_ratio", 0.0d);
    payload.put("anonymity_supply_delta", 0.0d);
    payload.put("anonymity_brownout", false);
    payload.put("anonymity_brownout_effective", false);
    payload.put("anonymity_uses_classical", false);
    payload.put(
        "provider_reports",
        List.of(
            Map.of(
                "provider", "p1",
                "successes", 1L,
                "failures", 0L,
                "disabled", false)));
    payload.put(
        "chunk_receipts",
        List.of(Map.of("chunk_index", 0, "provider", "p1", "attempts", 1L)));
    return payload;
  }
}
