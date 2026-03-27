package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.ArrayDeque;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Queue;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionActionRequest;
import org.hyperledger.iroha.android.subscriptions.SubscriptionActionResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionCreateRequest;
import org.hyperledger.iroha.android.subscriptions.SubscriptionCreateResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionListParams;
import org.hyperledger.iroha.android.subscriptions.SubscriptionListResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionPlanCreateRequest;
import org.hyperledger.iroha.android.subscriptions.SubscriptionPlanCreateResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionPlanListParams;
import org.hyperledger.iroha.android.subscriptions.SubscriptionPlanListResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionStatus;
import org.hyperledger.iroha.android.subscriptions.SubscriptionToriiException;
import org.hyperledger.iroha.android.subscriptions.SubscriptionUsageRequest;

public final class SubscriptionToriiClientTests {

  private SubscriptionToriiClientTests() {}

  private static void assertServerSideSigningRemoved(
      final String endpoint, final Runnable action) {
    try {
      action.run();
    } catch (final UnsupportedOperationException expected) {
      assert expected.getMessage().contains(endpoint) : "missing endpoint in error message";
      assert expected.getMessage().contains("locally signed transaction")
          : "missing remediation in error message";
      return;
    }
    throw new AssertionError("Expected " + endpoint + " to reject server-side signing");
  }

  public static void main(final String[] args) {
    listPlansParsesResponse();
    createPlanBuildsBody();
    createPlanRejectsInsecureTransportForPrivateKeyBody();
    planJsonBuilderParses();
    listSubscriptionsParsesResponse();
    createSubscriptionBuildsBody();
    getSubscriptionHandlesNotFound();
    getSubscriptionParsesResponse();
    subscriptionActionsAndUsagePostBodies();
    listParamsRejectInvalidStatus();
    usageRequestRejectsInvalidDelta();
    propagatesNon2xxResponses();
    configBuildsSubscriptionToriiClient();
    configBuildsSubscriptionToriiClientDefault();
    transportBuildsSubscriptionToriiClient();
    System.out.println("[IrohaAndroid] SubscriptionToriiClientTests passed.");
  }

  private static void listPlansParsesResponse() {
    final RecordingExecutor executor = new RecordingExecutor();
    executor.enqueue(
        200,
        """
        {
          "total": 1,
          "items": [
            {
              "plan_id": "aws_compute#commerce",
              "plan": {
                "kind": "usage",
                "price": "120",
                "bill_for": { "kind": "previous_period", "value": null }
              }
            }
          ]
        }
        """);
    final SubscriptionToriiClient client =
        SubscriptionToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .addHeader("X-Test", "1")
            .build();
    final SubscriptionPlanListResponse response =
        client
            .listSubscriptionPlans(
                SubscriptionPlanListParams.builder()
                    .provider("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
                    .limit(10L)
                    .offset(5L)
                    .build())
            .join();
    assert response.total() == 1 : "plan total mismatch";
    assert response.items().size() == 1 : "plan item count mismatch";
    final Map<String, Object> plan = response.items().get(0).plan();
    assert "usage".equals(plan.get("kind")) : "plan kind mismatch";
    final Object billFor = plan.get("bill_for");
    assert billFor instanceof Map : "bill_for is not a map";
    @SuppressWarnings("unchecked")
    final Map<String, Object> billForMap = (Map<String, Object>) billFor;
    assert billForMap.containsKey("value") : "bill_for value missing";
    assert billForMap.get("value") == null : "bill_for value should be null";
    final String query = executor.lastRequest.uri().getRawQuery();
    assert query.contains("provider=aws%40commerce") : "provider query missing";
    assert query.contains("limit=10") : "limit query missing";
    assert query.contains("offset=5") : "offset query missing";
    assert "application/json".equals(firstHeader(executor.lastRequest, "Accept"))
        : "accept header mismatch";
  }

  private static void createPlanBuildsBody() {
    final SubscriptionToriiClient client =
        SubscriptionToriiClient.builder()
            .executor(new RecordingExecutor())
            .baseUri(URI.create("https://example.com"))
            .build();
    final Map<String, Object> plan = new LinkedHashMap<>();
    plan.put("kind", "fixed");
    plan.put("price", "120");
    plan.put("period", "month");
    assertServerSideSigningRemoved(
        "/v1/subscriptions/plans",
        () ->
            client
                .createSubscriptionPlan(
                    SubscriptionPlanCreateRequest.builder()
                        .authority("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
                        .planId("aws_compute#commerce")
                        .plan(plan)
                        .build())
                .join());
  }

  private static void createPlanRejectsInsecureTransportForPrivateKeyBody() {
    final SubscriptionToriiClient client =
        SubscriptionToriiClient.builder()
            .executor(new RecordingExecutor())
            .baseUri(URI.create("http://example.com"))
            .build();
    final Map<String, Object> plan = new LinkedHashMap<>();
    plan.put("kind", "fixed");
    assertServerSideSigningRemoved(
        "/v1/subscriptions/plans",
        () ->
            client
                .createSubscriptionPlan(
                    SubscriptionPlanCreateRequest.builder()
                        .authority("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
                        .planId("aws_compute#commerce")
                        .plan(plan)
                        .build())
                .join());
  }

  private static void planJsonBuilderParses() {
    final SubscriptionPlanCreateRequest request =
        SubscriptionPlanCreateRequest.builder()
            .authority("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
            .planId("aws_compute#commerce")
            .planJson("{\"kind\":\"fixed\",\"bill_for\":{\"value\":null}}")
            .build();
    final Map<String, Object> plan = request.plan();
    assert plan.get("kind").equals("fixed") : "plan json parse failed";
    assert plan.get("bill_for") instanceof Map : "bill_for missing";
    @SuppressWarnings("unchecked")
    final Map<String, Object> billFor = (Map<String, Object>) plan.get("bill_for");
    assert billFor.containsKey("value") : "bill_for value missing";
    assert billFor.get("value") == null : "bill_for value should be null";
  }

  private static void listSubscriptionsParsesResponse() {
    final RecordingExecutor executor = new RecordingExecutor();
    executor.enqueue(
        200,
        """
        {
          "total": 1,
          "items": [
            {
              "subscription_id": "sub-1$subscriptions",
              "subscription": { "status": "active", "next_charge_ms": 1700000000000 },
              "invoice": { "bill_for": { "kind": "previous_period", "value": null }, "amount": "120" },
              "plan": { "kind": "fixed", "price": "120", "period": "month" }
            }
          ]
        }
        """);
    final SubscriptionToriiClient client =
        SubscriptionToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final SubscriptionListResponse response =
        client
            .listSubscriptions(
                SubscriptionListParams.builder()
                    .ownedBy("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
                    .provider("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
                    .status(SubscriptionStatus.ACTIVE)
                    .limit(10L)
                    .offset(0L)
                    .build())
            .join();
    assert response.total() == 1 : "subscription total mismatch";
    final SubscriptionListResponse.SubscriptionRecord record = response.items().get(0);
    assert "sub-1$subscriptions".equals(record.subscriptionId()) : "subscription id mismatch";
    assert "active".equals(record.subscription().get("status")) : "status mismatch";
    final Object invoice = record.invoice();
    assert invoice instanceof Map : "invoice missing";
    @SuppressWarnings("unchecked")
    final Map<String, Object> invoiceMap = (Map<String, Object>) invoice;
    final Object billFor = invoiceMap.get("bill_for");
    assert billFor instanceof Map : "invoice bill_for missing";
    @SuppressWarnings("unchecked")
    final Map<String, Object> billForMap = (Map<String, Object>) billFor;
    assert billForMap.containsKey("value") : "invoice bill_for value missing";
    assert billForMap.get("value") == null : "invoice bill_for value should be null";
    final String query = executor.lastRequest.uri().getRawQuery();
    assert query.contains("owned_by=alice%40wonderland") : "owned_by query missing";
    assert query.contains("provider=aws%40commerce") : "provider query missing";
    assert query.contains("status=active") : "status query missing";
    assert query.contains("limit=10") : "limit query missing";
    assert query.contains("offset=0") : "offset query missing";
  }

  private static void createSubscriptionBuildsBody() {
    final SubscriptionToriiClient client =
        SubscriptionToriiClient.builder()
            .executor(new RecordingExecutor())
            .baseUri(URI.create("https://example.com"))
            .build();
    assertServerSideSigningRemoved(
        "/v1/subscriptions",
        () ->
            client
                .createSubscription(
                    SubscriptionCreateRequest.builder()
                        .authority("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
                        .subscriptionId("sub-1$subscriptions")
                        .planId("aws_compute#commerce")
                        .billingTriggerId("sub-1$subscriptions#billing")
                        .usageTriggerId("sub-1$subscriptions#usage")
                        .firstChargeMs(1_700_000_000_000L)
                        .grantUsageToProvider(true)
                        .build())
                .join());
  }

  private static void getSubscriptionHandlesNotFound() {
    final RecordingExecutor executor = new RecordingExecutor();
    executor.enqueue(404, "{}");
    final SubscriptionToriiClient client =
        SubscriptionToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final SubscriptionListResponse.SubscriptionRecord record =
        client.getSubscription("sub-1$subscriptions").join();
    assert record == null : "expected null for 404";
    assert "GET".equals(executor.lastRequest.method()) : "expected GET";
    final String rawPath = executor.lastRequest.uri().getRawPath();
    assert rawPath.contains("sub-1%24subscriptions") : "subscription id not encoded";
  }

  private static void getSubscriptionParsesResponse() {
    final RecordingExecutor executor = new RecordingExecutor();
    executor.enqueue(
        200,
        """
        {
          "subscription_id": "sub-1$subscriptions",
          "subscription": { "status": "active" },
          "invoice": null,
          "plan": { "kind": "fixed" }
        }
        """);
    final SubscriptionToriiClient client =
        SubscriptionToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final SubscriptionListResponse.SubscriptionRecord record =
        client.getSubscription("sub-1$subscriptions").join();
    assert record != null : "expected record";
    assert "sub-1$subscriptions".equals(record.subscriptionId()) : "subscription id mismatch";
    assert record.plan() != null : "plan missing";
  }

  private static void subscriptionActionsAndUsagePostBodies() {
    final SubscriptionToriiClient client =
        SubscriptionToriiClient.builder()
            .executor(new RecordingExecutor())
            .baseUri(URI.create("https://example.com"))
            .build();
    final String subscriptionId = "sub-1$subscriptions";
    final SubscriptionActionRequest baseRequest =
        SubscriptionActionRequest.builder()
            .authority("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
            .build();
    final SubscriptionActionRequest chargeRequest =
        SubscriptionActionRequest.builder()
            .authority("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
            .chargeAtMs(1_700_000_000_000L)
            .build();
    assertServerSideSigningRemoved(
        "/v1/subscriptions/{subscription_id}/pause",
        () -> client.pauseSubscription(subscriptionId, baseRequest).join());
    assertServerSideSigningRemoved(
        "/v1/subscriptions/{subscription_id}/resume",
        () -> client.resumeSubscription(subscriptionId, chargeRequest).join());
    assertServerSideSigningRemoved(
        "/v1/subscriptions/{subscription_id}/cancel",
        () -> client.cancelSubscription(subscriptionId, baseRequest).join());
    assertServerSideSigningRemoved(
        "/v1/subscriptions/{subscription_id}/keep",
        () -> client.keepSubscription(subscriptionId, baseRequest).join());
    assertServerSideSigningRemoved(
        "/v1/subscriptions/{subscription_id}/charge-now",
        () -> client.chargeSubscriptionNow(subscriptionId, chargeRequest).join());
    assertServerSideSigningRemoved(
        "/v1/subscriptions/{subscription_id}/usage",
        () ->
            client
                .recordSubscriptionUsage(
                    subscriptionId,
                    SubscriptionUsageRequest.builder()
                        .authority("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
                        .unitKey("compute_ms")
                        .delta("3600000")
                        .usageTriggerId("sub-1$subscriptions#usage")
                        .build())
                .join());
  }

  private static void listParamsRejectInvalidStatus() {
    try {
      SubscriptionListParams.builder().status("unknown").build();
      throw new AssertionError("expected invalid status to throw");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
  }

  private static void usageRequestRejectsInvalidDelta() {
    try {
      SubscriptionUsageRequest.builder()
          .authority("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
          .unitKey("compute_ms")
          .delta("-1")
          .build();
      throw new AssertionError("expected negative delta to throw");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
    try {
      SubscriptionUsageRequest.builder()
          .authority("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
          .unitKey("compute_ms")
          .delta("invalid")
          .build();
      throw new AssertionError("expected invalid delta to throw");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
    final SubscriptionUsageRequest request =
        SubscriptionUsageRequest.builder()
            .authority("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
            .unitKey("compute_ms")
            .delta("12.5")
            .build();
    assert "12.5".equals(request.delta()) : "delta should preserve numeric literal";
  }

  private static void propagatesNon2xxResponses() {
    final RecordingExecutor executor = new RecordingExecutor();
    executor.enqueue(500, "{\"error\":\"boom\"}");
    final RecordingObserver observer = new RecordingObserver();
    final SubscriptionToriiClient client =
        SubscriptionToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .addObserver(observer)
            .build();
    try {
      client.listSubscriptionPlans(null).join();
    } catch (final java.util.concurrent.CompletionException ex) {
      assert ex.getCause() instanceof SubscriptionToriiException
          : "expected SubscriptionToriiException";
      assert observer.requestCount == 1 : "observer should see request";
      assert observer.responseCount == 0 : "observer should not see response";
      assert observer.failureCount == 1 : "observer should see failure";
      return;
    }
    throw new AssertionError("expected non-2xx response to throw");
  }

  private static void configBuildsSubscriptionToriiClient() {
    final RecordingExecutor executor = new RecordingExecutor();
    executor.enqueue(200, "{\"total\":0,\"items\":[]}");
    final RecordingObserver observer = new RecordingObserver();
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://example.com"))
            .putDefaultHeader("Authorization", "Bearer token")
            .addObserver(observer)
            .build();
    final SubscriptionToriiClient client = config.toSubscriptionToriiClient(executor);
    client.listSubscriptionPlans(null).join();
    assert executor.lastRequest.uri().toString().endsWith("/v1/subscriptions/plans")
        : "expected plans endpoint";
    assert "Bearer token".equals(firstHeader(executor.lastRequest, "Authorization"))
        : "config headers missing";
    assert observer.requestCount == 1 : "observer should see request";
    assert observer.responseCount == 1 : "observer should see response";
    assert observer.failureCount == 0 : "observer should not see failure";
  }

  private static void transportBuildsSubscriptionToriiClient() {
    final RecordingExecutor executor = new RecordingExecutor();
    executor.enqueue(200, "{\"total\":0,\"items\":[]}");
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://example.com"))
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);
    transport.subscriptionToriiClient().listSubscriptionPlans(null).join();
    assert executor.lastRequest != null : "transport client should use executor";
  }

  private static void configBuildsSubscriptionToriiClientDefault() {
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://example.com"))
            .build();
    final SubscriptionToriiClient client = config.toSubscriptionToriiClient();
    assert client != null : "default subscription client should not be null";
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> parseBody(final String body) {
    final Object parsed = JsonParser.parse(body);
    if (!(parsed instanceof Map)) {
      throw new IllegalStateException("expected JSON object");
    }
    return (Map<String, Object>) parsed;
  }

  private static String firstHeader(final TransportRequest request, final String name) {
    for (final var entry : request.headers().entrySet()) {
      if (entry.getKey().equalsIgnoreCase(name)) {
        final List<String> values = entry.getValue();
        if (!values.isEmpty()) {
          return values.get(0);
        }
      }
    }
    return "";
  }

  private static String urlEncode(final String value) {
    try {
      return java.net.URLEncoder.encode(value, StandardCharsets.UTF_8.name());
    } catch (final java.io.UnsupportedEncodingException ex) {
      throw new IllegalStateException("UTF-8 not supported", ex);
    }
  }

  private static final class RecordingExecutor implements HttpTransportExecutor {
    private final Queue<TransportResponse> responses = new ArrayDeque<>();
    private final Map<String, String> bodiesByPath = new LinkedHashMap<>();
    private TransportRequest lastRequest;
    private String lastBody = "";

    private void enqueue(final int status, final String body) {
      responses.add(
          new TransportResponse(
              status, body.getBytes(StandardCharsets.UTF_8), "", java.util.Map.of()));
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      lastRequest = request;
      lastBody = new String(request.body(), StandardCharsets.UTF_8);
      bodiesByPath.put(request.uri().getRawPath(), lastBody);
      final TransportResponse response = responses.poll();
      if (response == null) {
        throw new IllegalStateException("Missing stub response");
      }
      return CompletableFuture.completedFuture(response);
    }

    private String bodyFor(final String path) {
      final String body = bodiesByPath.get(path);
      if (body == null) {
        throw new IllegalStateException("Missing body for path " + path);
      }
      return body;
    }
  }

  private static final class RecordingObserver implements ClientObserver {
    private int requestCount;
    private int responseCount;
    private int failureCount;

    @Override
    public void onRequest(final TransportRequest request) {
      requestCount++;
    }

    @Override
    public void onResponse(final TransportRequest request, final ClientResponse response) {
      responseCount++;
    }

    @Override
    public void onFailure(final TransportRequest request, final Throwable error) {
      failureCount++;
    }
  }
}
