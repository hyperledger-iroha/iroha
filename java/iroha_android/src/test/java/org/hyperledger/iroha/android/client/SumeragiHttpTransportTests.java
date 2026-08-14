// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.consensus.SumeragiStatusModels;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.util.HashLiteral;
import org.junit.Test;

/** Exact, bounded Sumeragi HTTP surface tests. */
public final class SumeragiHttpTransportTests {
  @Test
  public void statusUsesOneExactBoundedJsonGet() {
    final byte[] body = statusJson().getBytes(StandardCharsets.UTF_8);
    final OneResponseExecutor executor = new OneResponseExecutor(jsonResponse(body));
    final HttpClientTransport transport = transport(executor);

    assertEquals(4, transport.getSumeragiStatus().join().protocolVersion());
    assertEquals(1, executor.requests);
    assertEquals("https://torii.example/api/v1/sumeragi/status", executor.last.uri().toString());
    assertEquals("GET", executor.last.method());
    assertArrayEquals(new byte[0], executor.last.body());
    assertEquals(List.of("application/json"), executor.last.headers().get("Accept"));
    assertEquals(
        org.hyperledger.iroha.android.client.transport.RequestReplayPolicy.ONE_SHOT,
        executor.last.replayPolicy());
    assertTrue(executor.last.headers().containsKey(OperatorRequestSigner.HEADER_SIGNATURE));
    assertEquals(
        Long.valueOf(SumeragiStatusModels.STATUS_JSON_MAX_BYTES),
        executor.last.maximumResponseBytes());
  }

  @Test
  public void diagnosticsUsesItsSeparateLargerExactJsonGet() {
    final byte[] body = diagnosticsJson().getBytes(StandardCharsets.UTF_8);
    final OneResponseExecutor executor = new OneResponseExecutor(jsonResponse(body));
    final HttpClientTransport transport = transport(executor);

    assertEquals(1, transport.getSumeragiDiagnostics().join().txQueueCapacity().intValueExact());
    assertEquals("https://torii.example/api/v1/sumeragi/diagnostics", executor.last.uri().toString());
    assertEquals("GET", executor.last.method());
    assertEquals(List.of("application/json"), executor.last.headers().get("Accept"));
    assertEquals(
        org.hyperledger.iroha.android.client.transport.RequestReplayPolicy.ONE_SHOT,
        executor.last.replayPolicy());
    assertTrue(executor.last.headers().containsKey(OperatorRequestSigner.HEADER_SIGNATURE));
    assertEquals(
        Long.valueOf(SumeragiStatusModels.DIAGNOSTICS_JSON_MAX_BYTES),
        executor.last.maximumResponseBytes());
  }

  @Test
  public void responsesRequireExactContentTypeCanonicalLengthAndBoundedBody() {
    final byte[] body = statusJson().getBytes(StandardCharsets.UTF_8);
    for (final Map<String, List<String>> headers :
        List.<Map<String, List<String>>>of(
            Collections.emptyMap(),
            Map.of("Content-Type", List.of("application/json; charset=utf-8")),
            Map.of("Content-Type", List.of("application/json", "application/json")))) {
      final TransportResponse response = new TransportResponse(200, body, "", headers);
      assertThrows(RuntimeException.class, () -> transport(new OneResponseExecutor(response)).getSumeragiStatus().join());
      assertThrows(RuntimeException.class, () -> transport(new OneResponseExecutor(response)).getSumeragiDiagnostics().join());
    }
    for (final List<String> lengths :
        List.<List<String>>of(
            Collections.emptyList(),
            List.of("+" + body.length),
            List.of("0" + body.length),
            List.of(Integer.toString(body.length + 1)),
            List.of(Integer.toString(body.length), Integer.toString(body.length)))) {
      final TransportResponse response =
          new TransportResponse(
              200,
              body,
              "",
              Map.of("Content-Type", List.of("application/json"), "Content-Length", lengths));
      assertThrows(RuntimeException.class, () -> transport(new OneResponseExecutor(response)).getSumeragiStatus().join());
    }
    final byte[] oversized = new byte[(int) SumeragiStatusModels.STATUS_JSON_MAX_BYTES + 1];
    final TransportResponse oversizedResponse =
        new TransportResponse(
            200, oversized, "", Map.of("Content-Type", List.of("application/json")));
    assertThrows(
        RuntimeException.class,
        () -> transport(new OneResponseExecutor(oversizedResponse)).getSumeragiStatus().join());
  }

  @Test
  public void malformedUtf8DefaultMethodsAndAcceptOverridesFailBeforeReturningData() {
    final byte[] malformed = new byte[] {0x7b, 0x22, (byte) 0xc3, 0x28};
    assertThrows(
        RuntimeException.class,
        () -> transport(new OneResponseExecutor(jsonResponse(malformed))).getSumeragiStatus().join());

    final IrohaClient defaultClient =
        new IrohaClient() {
          @Override
          public CompletableFuture<ClientResponse> submitTransaction(
              final SignedTransaction transaction) {
            return CompletableFuture.completedFuture(
                new ClientResponse(202, new byte[0], "accepted"));
          }
        };
    assertThrows(RuntimeException.class, () -> defaultClient.getSumeragiStatus().join());
    assertThrows(RuntimeException.class, () -> defaultClient.getSumeragiDiagnostics().join());

    final OneResponseExecutor executor =
        new OneResponseExecutor(jsonResponse(statusJson().getBytes(StandardCharsets.UTF_8)));
    final HttpClientTransport invalid =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .putDefaultHeader("aCcEpT", "application/json")
                .build());
    assertThrows(IllegalArgumentException.class, invalid::getSumeragiStatus);
    assertEquals(0, executor.requests);
  }

  @Test
  public void operatorReadsRejectMissingAndFallbackAuthenticationBeforeDispatch() {
    final OneResponseExecutor executor =
        new OneResponseExecutor(jsonResponse(statusJson().getBytes(StandardCharsets.UTF_8)));
    final HttpClientTransport missing =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .build());
    assertThrows(IllegalStateException.class, missing::getSumeragiStatus);
    assertEquals(0, executor.requests);

    final HttpClientTransport fallback =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .setOperatorSigningContext(operatorContext())
                .putDefaultHeader("Authorization", "Bearer retired")
                .build());
    assertThrows(IllegalArgumentException.class, fallback::getSumeragiStatus);
    assertEquals(0, executor.requests);
  }

  private static HttpClientTransport transport(final HttpTransportExecutor executor) {
    return HttpClientTransport.withExecutor(
        executor,
        ClientConfig.builder()
            .setBaseUri(URI.create("https://torii.example/api"))
            .setOperatorSigningContext(operatorContext())
            .build());
  }

  private static OperatorSigningContext operatorContext() {
    final byte[] network = new byte[NetworkId.BYTE_LENGTH];
    java.util.Arrays.fill(network, (byte) 0x5a);
    network[network.length - 1] |= 1;
    return new OperatorSigningContext(
        NetworkId.fromBytes(network),
        "ed0120" + "66".repeat(32),
        message -> {
          final byte[] signature = new byte[64];
          java.util.Arrays.fill(signature, (byte) 0x55);
          return signature;
        });
  }

  private static TransportResponse jsonResponse(final byte[] body) {
    return new TransportResponse(
        200,
        body,
        "ok",
        Map.of(
            "Content-Type", List.of("application/json"),
            "Content-Length", List.of(Integer.toString(body.length))));
  }

  private static String statusJson() {
    return "{"
        + "\"protocol_version\":4,"
        + "\"node_fingerprint\":\"" + hash(0x11) + "\","
        + "\"build_fingerprint\":\"" + hash(0x12) + "\","
        + "\"config_fingerprint\":\"" + hash(0x13) + "\","
        + "\"restart_required\":false,"
        + "\"height_context_id\":[\"" + hash(0x14) + "\"],"
        + "\"height\":1,\"view\":0,"
        + "\"phase\":{\"phase\":\"awaiting_proposal\",\"details\":null},"
        + "\"leader\":0,"
        + "\"locked_prepare_qc\":null,\"highest_prepare_qc\":null,"
        + "\"last_timeout_certificate\":null,"
        + "\"body_state\":{\"state\":\"missing\",\"details\":null},"
        + "\"pending_persistence_id\":null,"
        + "\"last_committed_height\":0,\"last_committed_subject\":null,"
        + "\"height_context\":{\"epoch\":0,\"epoch_end_height\":1,"
        + "\"mode\":{\"mode\":\"permissioned\",\"details\":null},"
        + "\"epoch_seed\":\"" + "00".repeat(32) + "\","
        + "\"validator_count\":4,\"quorum\":{\"min_signers\":3,\"total_power\":4}},"
        + "\"last_commit_qc\":null,"
        + "\"liveness\":{\"generation\":0,\"prepare_quorums\":[],"
        + "\"commit_quorums\":[],\"timeout_quorums\":[],\"outbound_intents\":[],"
        + "\"work\":{"
        + "\"candidate\":{\"stage\":\"idle\",\"details\":null},"
        + "\"body_recovery\":{\"stage\":\"idle\",\"details\":null},"
        + "\"body_store\":{\"stage\":\"idle\",\"details\":null},"
        + "\"validation\":{\"stage\":\"idle\",\"details\":null},"
        + "\"application\":{\"stage\":\"idle\",\"details\":null},"
        + "\"successor_height\":{\"stage\":\"idle\",\"details\":null}},"
        + "\"queues\":[],\"last_progress\":null,\"no_progress_age_ms\":0,"
        + "\"blocker\":null,\"ignore_counts\":[]}}";
  }

  private static String diagnosticsJson() {
    return "{"
        + "\"pipeline_execution\":{"
        + "\"tx_vertices_total\":0,\"tx_edges_total\":0,\"overlay_count_total\":0,"
        + "\"overlay_instr_total\":0,\"overlay_bytes_total\":0,"
        + "\"rbc_chunks_total\":0,\"rbc_bytes_total\":0,"
        + "\"detached_prepared_total\":0,\"detached_merged_total\":0,"
        + "\"detached_fallback_total\":0,"
        + "\"detached_fallback_fee_postprocessing_total\":0,"
        + "\"detached_fallback_user_executor_total\":0,"
        + "\"detached_fallback_durable_state_total\":0,"
        + "\"detached_fallback_unsupported_instruction_total\":0,"
        + "\"detached_fallback_rejected_eval_total\":0,"
        + "\"detached_fallback_overlay_error_total\":0,\"quarantine_executed_total\":0},"
        + "\"tx_queue_depth\":0,\"tx_queue_capacity\":1,"
        + "\"tx_queue_retained_bytes\":0,\"tx_queue_max_retained_bytes\":1,"
        + "\"tx_queue_saturated\":false,\"tx_queue_saturated_by_count\":false,"
        + "\"tx_queue_saturated_by_bytes\":false,\"tx_queue_saturated_by_age\":false,"
        + "\"tx_queue_oldest_queued_age_ms\":0,"
        + "\"lane_commitments\":[],\"dataspace_commitments\":[],"
        + "\"lane_settlement_commitments\":[],\"lane_relay_envelopes\":[],"
        + "\"lane_payload_ownerships\":[],\"committed_lane_blocks\":[],"
        + "\"lane_block_sessions\":[],\"lane_governance_sealed_total\":0,"
        + "\"lane_governance_sealed_aliases\":[],\"lane_governance\":[],"
        + "\"native_amx_participant_applications\":[],"
        + "\"autonomous_lane_executions\":[]}";
  }

  private static String hash(final int seed) {
    final byte[] bytes = new byte[32];
    java.util.Arrays.fill(bytes, (byte) seed);
    return HashLiteral.canonicalize(bytes);
  }

  private static final class OneResponseExecutor implements HttpTransportExecutor {
    private final TransportResponse response;
    private int requests;
    private TransportRequest last;

    private OneResponseExecutor(final TransportResponse response) {
      this.response = response;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requests++;
      last = request;
      return CompletableFuture.completedFuture(response);
    }
  }
}
