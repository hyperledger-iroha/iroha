// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client;

import static org.hyperledger.iroha.sdk.consensus.SumeragiStatusModelsKt.SUMERAGI_DIAGNOSTICS_JSON_MAX_BYTES;
import static org.hyperledger.iroha.sdk.consensus.SumeragiStatusModelsKt.SUMERAGI_STATUS_JSON_MAX_BYTES;
import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.math.BigInteger;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.sdk.alias.AccountFaucetClaimV1;
import org.hyperledger.iroha.sdk.alias.AccountFaucetPolicyV1;
import org.hyperledger.iroha.sdk.alias.AccountFaucetPreparedTransactionV1;
import org.hyperledger.iroha.sdk.alias.AccountOnboardingCurrentStateV1;
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPlanReceiptV1;
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPlanRequestV1;
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPrepareResponseV1;
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPreparedTransactionV1;
import org.hyperledger.iroha.sdk.alias.AccountOnboardingProofRequiredPrepareResponseV1;
import org.hyperledger.iroha.sdk.alias.PreparedTransactionSubmitResponseV1;
import org.hyperledger.iroha.sdk.alias.TairaPublicResetMutationBindingV1;
import org.hyperledger.iroha.sdk.client.transport.TransportRequest;
import org.hyperledger.iroha.sdk.client.transport.TransportResponse;
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent;
import org.hyperledger.iroha.sdk.core.model.NetworkId;
import org.hyperledger.iroha.sdk.core.util.HashLiteral;
import org.hyperledger.iroha.sdk.tx.SignedTransaction;
import org.junit.jupiter.api.Test;

/** Java consumers exercise the exact Kotlin-owned Sumeragi HTTP contract. */
public final class SumeragiHttpTransportTests {
  @Test
  public void statusUsesOneExactBoundedJsonGet() {
    final byte[] body = statusJson().getBytes(StandardCharsets.UTF_8);
    final OneResponseExecutor executor = new OneResponseExecutor(jsonResponse(body));
    final HttpClientTransport transport = transport(executor);

    assertEquals(4, transport.getSumeragiStatus().join().protocolVersion);
    assertEquals(1, executor.requests);
    assertEquals("https://torii.example/api/v1/sumeragi/status", executor.last.uri.toString());
    assertEquals("GET", executor.last.method);
    assertArrayEquals(new byte[0], executor.last.getBody());
    assertEquals(Arrays.asList("application/json"), executor.last.getHeaders().get("Accept"));
    assertEquals(
        org.hyperledger.iroha.sdk.client.transport.RequestReplayPolicy.ONE_SHOT,
        executor.last.replayPolicy);
    assertTrue(executor.last.getHeaders().containsKey(OperatorRequestSigner.HEADER_SIGNATURE));
    assertEquals(
        Long.valueOf(SUMERAGI_STATUS_JSON_MAX_BYTES),
        executor.last.maximumResponseBytes);
  }

  @Test
  public void diagnosticsUsesItsSeparateLargerExactJsonGet() {
    final byte[] body = diagnosticsJson().getBytes(StandardCharsets.UTF_8);
    final OneResponseExecutor executor = new OneResponseExecutor(jsonResponse(body));
    final HttpClientTransport transport = transport(executor);

    assertEquals(1, transport.getSumeragiDiagnostics().join().getTxQueueCapacity().intValueExact());
    assertEquals("https://torii.example/api/v1/sumeragi/diagnostics", executor.last.uri.toString());
    assertEquals("GET", executor.last.method);
    assertEquals(Arrays.asList("application/json"), executor.last.getHeaders().get("Accept"));
    assertEquals(
        org.hyperledger.iroha.sdk.client.transport.RequestReplayPolicy.ONE_SHOT,
        executor.last.replayPolicy);
    assertTrue(executor.last.getHeaders().containsKey(OperatorRequestSigner.HEADER_SIGNATURE));
    assertEquals(
        Long.valueOf(SUMERAGI_DIAGNOSTICS_JSON_MAX_BYTES),
        executor.last.maximumResponseBytes);
  }

  @Test
  public void responsesAcceptParametersAndRejectMalformedContentTypesLengthsAndBodies() {
    final byte[] body = statusJson().getBytes(StandardCharsets.UTF_8);
    final byte[] diagnosticsBody = diagnosticsJson().getBytes(StandardCharsets.UTF_8);
    assertThrows(
        RuntimeException.class,
        () ->
            transport(
                    new OneResponseExecutor(
                        jsonResponse(diagnosticsBody)))
                .getSumeragiStatus()
                .join(),
        "status endpoint must reject a diagnostics-shaped payload");
    assertThrows(
        RuntimeException.class,
        () ->
            transport(new OneResponseExecutor(jsonResponse(body)))
                .getSumeragiDiagnostics()
                .join(),
        "diagnostics endpoint must reject a status-shaped payload");

    for (final Map<String, List<String>> headers :
        Arrays.<Map<String, List<String>>>asList(
            headers("Content-Type", Arrays.asList("Application/JSON; charset=utf-8")),
            headers(
                "content-type",
                Arrays.asList("application/json; charset=\"UTF-8\"; profile=exact")))) {
      final TransportResponse statusResponse =
          new TransportResponse(200, body, "", headers, null, false);
      assertEquals(
          4,
          transport(new OneResponseExecutor(statusResponse))
              .getSumeragiStatus()
              .join()
              .protocolVersion);
      final TransportResponse diagnosticsResponse =
          new TransportResponse(200, diagnosticsBody, "", headers, null, false);
      assertEquals(
          BigInteger.ONE,
          transport(new OneResponseExecutor(diagnosticsResponse))
              .getSumeragiDiagnostics()
              .join()
              .getTxQueueCapacity());
    }

    for (final Map<String, List<String>> headers :
        Arrays.<Map<String, List<String>>>asList(
            Collections.emptyMap(),
            headers("Content-Type", Arrays.asList("application/json", "application/json")),
            headers("Content-Type", Arrays.asList("application/problem+json")),
            headers("Content-Type", Arrays.asList("application/json, text/plain")),
            headers("Content-Type", Arrays.asList("application/json;")),
            headers("Content-Type", Arrays.asList("application/json; charset")),
            headers("Content-Type", Arrays.asList("application/json; profile=\"a,b\"")))) {
      final TransportResponse statusResponse =
          new TransportResponse(200, body, "", headers, null, false);
      assertThrows(
          RuntimeException.class,
          () -> transport(new OneResponseExecutor(statusResponse)).getSumeragiStatus().join());
      final TransportResponse diagnosticsResponse =
          new TransportResponse(200, diagnosticsBody, "", headers, null, false);
      assertThrows(
          RuntimeException.class,
          () ->
              transport(new OneResponseExecutor(diagnosticsResponse))
                  .getSumeragiDiagnostics()
                  .join());
    }
    for (final List<String> lengths :
        Arrays.<List<String>>asList(
            Collections.emptyList(),
            Arrays.asList("+" + body.length),
            Arrays.asList("0" + body.length),
            Arrays.asList(Integer.toString(body.length + 1)),
            Arrays.asList(Integer.toString(body.length), Integer.toString(body.length)))) {
      final TransportResponse response =
          new TransportResponse(
              200,
              body,
              "",
              headers("Content-Type", Arrays.asList("application/json"), "Content-Length", lengths),
              null,
              false);
      assertThrows(
          RuntimeException.class,
          () -> transport(new OneResponseExecutor(response)).getSumeragiStatus().join());
    }
    final byte[] oversized = new byte[(int) SUMERAGI_STATUS_JSON_MAX_BYTES + 1];
    final TransportResponse oversizedResponse =
        new TransportResponse(
            200,
            oversized,
            "",
            headers("Content-Type", Arrays.asList("application/json")),
            null,
            false);
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
          public CompletableFuture<AccountOnboardingPlanReceiptV1>
              planSponsoredAccountOnboarding(
                  final AccountOnboardingPlanRequestV1 request,
                  final String onboardingToken,
                  final String expectedAuthority,
                  final NetworkId expectedNetworkId) {
            throw new AssertionError(
                "account onboarding is not used by this interface-default test");
          }

          @Override
          public CompletableFuture<AccountOnboardingPrepareResponseV1>
              prepareSponsoredAccountOnboarding(
                  final AccountOnboardingPlanRequestV1 request,
                  final AccountOnboardingPlanReceiptV1 receipt,
                  final TairaPublicResetMutationBindingV1 binding,
                  final FeePaymentIntent feePayment,
                  final String onboardingToken,
                  final String expectedAuthority,
                  final NetworkId expectedNetworkId) {
            throw new AssertionError(
                "account onboarding is not used by this interface-default test");
          }

          @Override
          public CompletableFuture<AccountOnboardingCurrentStateV1>
              verifyAccountOnboardingCurrentState(
                  final AccountOnboardingProofRequiredPrepareResponseV1 proofRequired,
                  final AccountOnboardingPlanRequestV1 request,
                  final AccountOnboardingPlanReceiptV1 receipt,
                  final TairaPublicResetMutationBindingV1 binding,
                  final String expectedAuthority,
                  final NetworkId expectedNetworkId,
                  final ToriiCanonicalRequestAuth canonicalAuth) {
            throw new AssertionError(
                "account onboarding is not used by this interface-default test");
          }

          @Override
          public CompletableFuture<PreparedTransactionSubmitResponseV1>
              submitPreparedAccountOnboarding(
                  final AccountOnboardingPlanRequestV1 request,
                  final AccountOnboardingPreparedTransactionV1 prepared,
                  final FeePaymentIntent expectedFeePayment,
                  final String onboardingToken,
                  final String expectedAuthority,
                  final NetworkId expectedNetworkId) {
            throw new AssertionError(
                "account onboarding is not used by this interface-default test");
          }

          @Override
          public CompletableFuture<AccountFaucetPreparedTransactionV1>
              prepareAccountFaucetTransaction(
                  final AccountFaucetClaimV1 claim,
                  final TairaPublicResetMutationBindingV1 binding,
                  final FeePaymentIntent feePayment,
                  final AccountFaucetPolicyV1 policy,
                  final NetworkId expectedNetworkId) {
            throw new AssertionError(
                "account faucet is not used by this interface-default test");
          }

          @Override
          public CompletableFuture<PreparedTransactionSubmitResponseV1>
              submitPreparedAccountFaucetTransaction(
                  final AccountFaucetPreparedTransactionV1 prepared,
                  final FeePaymentIntent expectedFeePayment,
                  final AccountFaucetPolicyV1 policy,
                  final NetworkId expectedNetworkId) {
            throw new AssertionError(
                "account faucet is not used by this interface-default test");
          }

          @Override
          public CompletableFuture<ClientResponse> submitTransaction(
              final SignedTransaction transaction) {
            return CompletableFuture.completedFuture(
                new ClientResponse(202, new byte[0], "accepted", null, null));
          }
        };
    assertThrows(RuntimeException.class, () -> defaultClient.getSumeragiStatus().join());
    assertThrows(RuntimeException.class, () -> defaultClient.getSumeragiDiagnostics().join());

    final OneResponseExecutor executor =
        new OneResponseExecutor(jsonResponse(statusJson().getBytes(StandardCharsets.UTF_8)));
    final HttpClientTransport invalid =
        new HttpClientTransport(
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
        new HttpClientTransport(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .build());
    assertThrows(IllegalStateException.class, missing::getSumeragiStatus);
    assertEquals(0, executor.requests);

    final HttpClientTransport fallback =
        new HttpClientTransport(
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
    return new HttpClientTransport(
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
        "ed0120" + repeat("66", 32),
        message -> {
          final byte[] signature = new byte[64];
          java.util.Arrays.fill(signature, (byte) 0x55);
          return signature;
        });
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder result = new StringBuilder(value.length() * count);
    for (int index = 0; index < count; index++) {
      result.append(value);
    }
    return result.toString();
  }

  private static Map<String, List<String>> headers(
      final String name, final List<String> values) {
    return Collections.singletonMap(name, values);
  }

  private static Map<String, List<String>> headers(
      final String firstName,
      final List<String> firstValues,
      final String secondName,
      final List<String> secondValues) {
    final Map<String, List<String>> result = new LinkedHashMap<>();
    result.put(firstName, firstValues);
    result.put(secondName, secondValues);
    return result;
  }

  private static TransportResponse jsonResponse(final byte[] body) {
    return new TransportResponse(
        200,
        body,
        "ok",
        headers(
            "Content-Type", Arrays.asList("application/json"),
            "Content-Length", Arrays.asList(Integer.toString(body.length))),
        null,
        false);
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
        + "\"epoch_seed\":\"" + repeat("00", 32) + "\","
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
