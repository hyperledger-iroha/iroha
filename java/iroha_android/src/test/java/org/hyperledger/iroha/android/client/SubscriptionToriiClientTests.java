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

  public static void main(final String[] args) {
    listPlansParsesResponse();
    createPlanBuildsBody();
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
                    .provider("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
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
    assert query.contains("provider=6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
        : "provider query missing";
    assert query.contains("limit=10") : "limit query missing";
    assert query.contains("offset=5") : "offset query missing";
    assert "application/json".equals(firstHeader(executor.lastRequest, "Accept"))
        : "accept header mismatch";
  }

  private static void createPlanBuildsBody() {
    final RecordingExecutor executor = new RecordingExecutor();
    executor.enqueue(
        200,
        """
        { "ok": true, "plan_id": "aws_compute#commerce", "tx_hash_hex": "deadbeef" }
        """);
    final SubscriptionToriiClient client =
        SubscriptionToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final Map<String, Object> plan = new LinkedHashMap<>();
    plan.put("kind", "fixed");
    plan.put("price", "120");
    plan.put("period", "month");
    final SubscriptionPlanCreateResponse response =
        client
            .createSubscriptionPlan(
                SubscriptionPlanCreateRequest.builder()
                    .authority("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
                    .privateKey("deadbeef")
                    .planId("aws_compute#commerce")
                    .plan(plan)
                    .build())
            .join();
    assert response.ok() : "plan create should be ok";
    assert "aws_compute#commerce".equals(response.planId()) : "plan id mismatch";
    final Map<String, Object> body = parseBody(executor.lastBody);
    assert "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV".equals(body.get("authority")) : "authority mismatch";
    assert "deadbeef".equals(body.get("private_key")) : "private key mismatch";
    assert "aws_compute#commerce".equals(body.get("plan_id")) : "plan id missing";
    assert body.get("plan") instanceof Map : "plan missing";
  }

  private static void planJsonBuilderParses() {
    final SubscriptionPlanCreateRequest request =
        SubscriptionPlanCreateRequest.builder()
            .authority("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
            .privateKey("deadbeef")
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
                    .ownedBy("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
                    .provider("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
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
    assert query.contains("owned_by=6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
        : "owned_by query missing";
    assert query.contains("provider=6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
        : "provider query missing";
    assert query.contains("status=active") : "status query missing";
    assert query.contains("limit=10") : "limit query missing";
    assert query.contains("offset=0") : "offset query missing";
  }

  private static void createSubscriptionBuildsBody() {
    final RecordingExecutor executor = new RecordingExecutor();
    executor.enqueue(
        200,
        """
        {
          "ok": true,
          "subscription_id": "sub-1$subscriptions",
          "billing_trigger_id": "sub-1$subscriptions#billing",
          "usage_trigger_id": "sub-1$subscriptions#usage",
          "first_charge_ms": 1700000000000,
          "tx_hash_hex": "deadbeef"
        }
        """);
    final SubscriptionToriiClient client =
        SubscriptionToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final SubscriptionCreateResponse response =
        client
            .createSubscription(
                SubscriptionCreateRequest.builder()
                    .authority("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
                    .privateKey("deadbeef")
                    .subscriptionId("sub-1$subscriptions")
                    .planId("aws_compute#commerce")
                    .billingTriggerId("sub-1$subscriptions#billing")
                    .usageTriggerId("sub-1$subscriptions#usage")
                    .firstChargeMs(1_700_000_000_000L)
                    .grantUsageToProvider(true)
                    .build())
            .join();
    assert response.ok() : "subscription create should be ok";
    assert response.firstChargeMs() == 1_700_000_000_000L : "first_charge_ms mismatch";
    final Map<String, Object> body = parseBody(executor.lastBody);
    assert "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV".equals(body.get("authority")) : "authority mismatch";
    assert "deadbeef".equals(body.get("private_key")) : "private key mismatch";
    assert "sub-1$subscriptions".equals(body.get("subscription_id"))
        : "subscription id mismatch";
    assert "aws_compute#commerce".equals(body.get("plan_id")) : "plan id mismatch";
    assert body.get("billing_trigger_id").equals("sub-1$subscriptions#billing")
        : "billing trigger missing";
    assert body.get("usage_trigger_id").equals("sub-1$subscriptions#usage")
        : "usage trigger missing";
    assert ((Number) body.get("first_charge_ms")).longValue() == 1_700_000_000_000L
        : "first_charge_ms body mismatch";
    assert body.get("grant_usage_to_provider").equals(Boolean.TRUE)
        : "grant_usage_to_provider missing";
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
    final RecordingExecutor executor = new RecordingExecutor();
    final String actionResponse =
        "{\"ok\":true,\"subscription_id\":\"sub-1$subscriptions\",\"tx_hash_hex\":\"deadbeef\"}";
    executor.enqueue(200, actionResponse);
    executor.enqueue(200, actionResponse);
    executor.enqueue(200, actionResponse);
    executor.enqueue(200, actionResponse);
    executor.enqueue(200, actionResponse);
    executor.enqueue(200, actionResponse);
    final SubscriptionToriiClient client =
        SubscriptionToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final String subscriptionId = "sub-1$subscriptions";
    final SubscriptionActionRequest baseRequest =
        SubscriptionActionRequest.builder()
            .authority("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
            .privateKey("deadbeef")
            .build();
    final SubscriptionActionRequest chargeRequest =
        SubscriptionActionRequest.builder()
            .authority("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
            .privateKey("deadbeef")
            .chargeAtMs(1_700_000_000_000L)
            .build();
    final SubscriptionActionResponse pauseResponse =
        client.pauseSubscription(subscriptionId, baseRequest).join();
    assert pauseResponse.ok() : "pause should be ok";
    client.resumeSubscription(subscriptionId, chargeRequest).join();
    client.cancelSubscription(subscriptionId, baseRequest).join();
    client.keepSubscription(subscriptionId, baseRequest).join();
    client.chargeSubscriptionNow(subscriptionId, chargeRequest).join();
    client
        .recordSubscriptionUsage(
            subscriptionId,
            SubscriptionUsageRequest.builder()
                .authority("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
                .privateKey("deadbeef")
                .unitKey("compute_ms")
                .delta("3600000")
                .usageTriggerId("sub-1$subscriptions#usage")
                .build())
        .join();
    final String encodedId = urlEncode(subscriptionId);
    final Map<String, Object> pauseBody =
        parseBody(executor.bodyFor("/v1/subscriptions/" + encodedId + "/pause"));
    assert pauseBody.get("authority").equals("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV") : "pause authority missing";
    assert pauseBody.get("private_key").equals("deadbeef") : "pause private key missing";
    assert !pauseBody.containsKey("charge_at_ms") : "pause should not set charge_at_ms";
    final Map<String, Object> resumeBody =
        parseBody(executor.bodyFor("/v1/subscriptions/" + encodedId + "/resume"));
    assert resumeBody.containsKey("charge_at_ms") : "resume charge_at_ms missing";
    final Map<String, Object> cancelBody =
        parseBody(executor.bodyFor("/v1/subscriptions/" + encodedId + "/cancel"));
    assert cancelBody.get("authority").equals("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV") : "cancel authority missing";
    final Map<String, Object> keepBody =
        parseBody(executor.bodyFor("/v1/subscriptions/" + encodedId + "/keep"));
    assert keepBody.get("authority").equals("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV") : "keep authority missing";
    final Map<String, Object> chargeBody =
        parseBody(executor.bodyFor("/v1/subscriptions/" + encodedId + "/charge-now"));
    assert chargeBody.containsKey("charge_at_ms") : "charge-now charge_at_ms missing";
    final Map<String, Object> usageBody =
        parseBody(executor.bodyFor("/v1/subscriptions/" + encodedId + "/usage"));
    assert usageBody.get("unit_key").equals("compute_ms") : "usage unit_key missing";
    assert usageBody.get("delta").equals("3600000") : "usage delta missing";
    assert usageBody.get("usage_trigger_id").equals("sub-1$subscriptions#usage")
        : "usage trigger missing";
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
          .authority("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
          .privateKey("deadbeef")
          .unitKey("compute_ms")
          .delta("-1")
          .build();
      throw new AssertionError("expected negative delta to throw");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
    try {
      SubscriptionUsageRequest.builder()
          .authority("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
          .privateKey("deadbeef")
          .unitKey("compute_ms")
          .delta("invalid")
          .build();
      throw new AssertionError("expected invalid delta to throw");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
    final SubscriptionUsageRequest request =
        SubscriptionUsageRequest.builder()
            .authority("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
            .privateKey("deadbeef")
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
