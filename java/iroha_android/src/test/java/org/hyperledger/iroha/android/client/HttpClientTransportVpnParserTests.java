package org.hyperledger.iroha.android.client;

import static org.hyperledger.iroha.android.client.HttpClientTransportSubmissionContractTests.compatibleCapabilitiesResponse;
import static org.hyperledger.iroha.android.client.HttpClientTransportSubmissionContractTests.isCapabilitiesRequest;
import static org.hyperledger.iroha.android.client.HttpClientTransportTests.vpnProfileJson;
import static org.hyperledger.iroha.android.client.HttpClientTransportTests.vpnQuoteJson;
import static org.hyperledger.iroha.android.client.HttpClientTransportTests.vpnReceiptJson;
import static org.hyperledger.iroha.android.client.HttpClientTransportTests.vpnSessionJson;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.Signature;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;

/** Strict VPN response-schema and successful-status contract coverage. */
final class HttpClientTransportVpnParserTests {
  private static final String VPN_HELPER_TICKET_HEX = "5356504e48543100" + "00".repeat(656);
  private static final String VALID_ED25519_PUBLIC_KEY_HEX = TestEd25519Keys.publicKeyHex(0x22);

  private HttpClientTransportVpnParserTests() {}

  static void runAll() throws Exception {
    vpnProfileRequestParsesNativeLeaseFields();
    vpnSessionParserRejectsNonCanonicalHelperTicketHex();
    vpnResponseParsersRejectNonCanonicalIdsHashesAndUnknownFields();
    vpnResponseParsersRejectMissingRequiredFieldsAndSchemaBounds();
    vpnRoutesRejectWrongSuccessfulStatusCodes();
  }

  private static void vpnProfileRequestParsesNativeLeaseFields() {
    final String json =
        "{"
            + "\"available\":true,"
            + "\"relay_endpoint\":\"/dns/relay.example/udp/9443/quic\","
            + "\"supported_exit_classes\":[\"standard\",\"low-latency\",\"high-security\"],"
            + "\"default_exit_class\":\"standard\","
            + "\"lease_secs\":600,"
            + "\"dns_push_interval_secs\":60,"
            + "\"meter_family\":\"soranet.vpn.standard\","
            + "\"route_pushes\":[\"0.0.0.0/0\"],"
            + "\"excluded_routes\":[\"10.0.0.0/8\"],"
            + "\"dns_servers\":[\"1.1.1.1\"],"
            + "\"tunnel_addresses\":[\"10.208.0.2/32\"],"
            + "\"mtu_bytes\":1280,"
            + "\"display_billing_label\":\"standard XOR\","
            + "\"fee_asset_id\":\"xor#universal.universal\","
            + "\"escrow_account_id\":\"sorauEscrow\","
            + "\"operator_account_id\":\"sorauOperator\","
            + "\"lease_fee\":\"1000000.25\","
            + "\"settlement_grace_secs\":120,"
            + "\"flow_label_bits\":24,"
            + "\"padding_budget_ms\":15,"
            + "\"relay_tls_spki_sha256_hex\":\""
            + "ab".repeat(32)
            + "\""
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final VpnProfile profile = transport.getVpnProfile().join();

    assert profile.available() : "VPN profile should be available";
    assert "xor#universal.universal".equals(profile.feeAssetId()) : "VPN fee asset mismatch";
    assert "sorauEscrow".equals(profile.escrowAccountId()) : "VPN escrow account mismatch";
    assert "sorauOperator".equals(profile.operatorAccountId()) : "VPN operator account mismatch";
    assert "1000000.25".equals(profile.leaseFee()) : "VPN lease fee mismatch";
    assert profile.dnsPushIntervalSecs() == 60L : "VPN DNS push interval mismatch";
    assert profile.settlementGraceSecs() == 120L : "VPN settlement grace mismatch";
    assert "ab".repeat(32).equals(profile.relayTlsSpkiSha256Hex()) : "VPN TLS pin mismatch";
    assert "GET".equals(executor.lastRequest().method()) : "VPN profile must use GET";
    assert executor.lastRequest().uri().toString().equals("https://torii.example/v1/vpn/profile")
        : "VPN profile URI mismatch";

    expectRuntimeException(
        () ->
            VpnJsonParser.parseProfile(
                json.replace("\"dns_push_interval_secs\":60", "\"dns_push_interval_secs\":29")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN profile parser must reject DNS push intervals below 30 seconds");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseProfile(
                json.replace("\"dns_push_interval_secs\":60,", "")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN profile parser must require dns_push_interval_secs");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseProfile(
                json.replaceFirst("\\{", "{\"unexpected\":true,")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN profile parser must reject unknown fields");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseProfile(
                json.replace("ab".repeat(32), "AB".repeat(32))
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN profile parser must reject uppercase TLS pins");
  }

  private static void vpnSessionParserRejectsNonCanonicalHelperTicketHex() {
    final String sessionId = "33".repeat(32);
    final String paymentTxHash = "44".repeat(32);
    final String[] invalidValues = {
      "0x" + VPN_HELPER_TICKET_HEX,
      VPN_HELPER_TICKET_HEX.toUpperCase(Locale.ROOT),
      VPN_HELPER_TICKET_HEX.substring(0, VPN_HELPER_TICKET_HEX.length() - 2)
    };
    for (final String invalid : invalidValues) {
      final byte[] payload =
          vpnSessionJson(sessionId, paymentTxHash)
              .replace(VPN_HELPER_TICKET_HEX, invalid)
              .getBytes(StandardCharsets.UTF_8);
      expectRuntimeException(
          () -> VpnJsonParser.parseSession(payload),
          "VPN session parser must reject non-canonical helper_ticket_hex");
    }
  }

  private static void vpnResponseParsersRejectNonCanonicalIdsHashesAndUnknownFields() {
    final String identifier = "ab".repeat(32);
    final String paymentTxHash = "cd".repeat(32);
    final String meteringKey = VALID_ED25519_PUBLIC_KEY_HEX;
    final String quote = vpnQuoteJson(identifier, meteringKey);

    expectRuntimeException(
        () ->
            VpnJsonParser.parseQuote(
                quote
                    .replace(
                        "\"quote_id\":\"" + identifier + "\"",
                        "\"quote_id\":\"0x" + identifier + "\"")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN quote parser must reject prefixed quote ids");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseQuote(
                quote.replace("aa".repeat(16), "AA".repeat(16))
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN quote parser must reject uppercase session ids");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseQuote(
                quote.replaceFirst("\\{", "{\"unexpected\":true,")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN quote parser must reject unknown fields");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseQuote(
                quote
                    .replaceFirst(
                        "\"payload_hex\":\"cafe\"",
                        "\"payload_hex\":\"cafe\",\"unexpected\":true")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN transaction instruction parser must reject unknown fields");

    final String session = vpnSessionJson(identifier, paymentTxHash);
    expectRuntimeException(
        () ->
            VpnJsonParser.parseSession(
                session
                    .replace(
                        "\"session_id\":\"" + identifier + "\"",
                        "\"session_id\":\"" + identifier.toUpperCase(Locale.ROOT) + "\"")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN session parser must reject uppercase session ids");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseSession(
                session
                    .replace(
                        "\"payment_tx_hash\":\"" + paymentTxHash + "\"",
                        "\"payment_tx_hash\":\"0x" + paymentTxHash + "\"")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN session parser must reject prefixed payment hashes");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseSession(
                session.replaceFirst("\\{", "{\"unexpected\":true,")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN session parser must reject unknown fields");

    final String receipt = vpnReceiptJson(identifier, paymentTxHash, true);
    expectRuntimeException(
        () ->
            VpnJsonParser.parseReceipt(
                receipt
                    .replace(
                        "\"lease_id_hex\":\"" + identifier + "\"",
                        "\"lease_id_hex\":\"" + identifier.toUpperCase(Locale.ROOT) + "\"")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN receipt parser must reject uppercase lease ids");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseReceipt(
                receipt
                    .replace(
                        "\"payment_tx_hash\":\"" + paymentTxHash + "\"",
                        "\"payment_tx_hash\":\"0x" + paymentTxHash + "\"")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN receipt parser must reject prefixed payment hashes");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseReceipt(
                receipt.replaceFirst("\\{", "{\"unexpected\":true,")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN receipt parser must reject unknown fields");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseReceiptList(
                ("{\"items\":[" + receipt + "],\"total\":1,\"unexpected\":true}")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN receipt-list parser must reject unknown fields");
  }

  private static void vpnResponseParsersRejectMissingRequiredFieldsAndSchemaBounds() {
    final String identifier = "ab".repeat(32);
    final String paymentTxHash = "cd".repeat(32);
    final String meteringKey = VALID_ED25519_PUBLIC_KEY_HEX;
    final String profile = vpnProfileJson();
    final String quote = vpnQuoteJson(identifier, meteringKey);
    final String session = vpnSessionJson(identifier, paymentTxHash);
    final String receipt = vpnReceiptJson(identifier, paymentTxHash, true);
    final String receiptList = "{\"items\":[" + receipt + "],\"total\":1}";

    final List<Runnable> missingCases =
        List.of(
            () ->
                VpnJsonParser.parseProfile(
                    vpnJsonWithoutField(profile, "relay_tls_spki_sha256_hex")),
            () ->
                VpnJsonParser.parseQuote(vpnJsonWithoutField(quote, "open_lease_instruction")),
            () -> VpnJsonParser.parseQuote(vpnJsonWithoutField(quote, "tx_instructions")),
            () -> VpnJsonParser.parseSession(vpnJsonWithoutField(session, "route_pushes")),
            () ->
                VpnJsonParser.parseReceipt(
                    vpnJsonWithoutField(receipt, "settle_lease_instruction")),
            () -> VpnJsonParser.parseReceiptList(vpnJsonWithoutField(receiptList, "items")));
    for (final Runnable decode : missingCases) {
      expectRuntimeException(decode, "VPN response parser must reject a missing required field");
    }
    expectRuntimeException(
        () -> VpnJsonParser.parseSession(vpnJsonWithField(session, "route_pushes", null)),
        "VPN response parser must reject null required arrays");

    final Object[][] profileViolations = {
      {"supported_exit_classes", List.of("standard", "low-latency")},
      {"supported_exit_classes", List.of("standard", "standard", "high-security")},
      {"default_exit_class", "unsupported"},
      {"lease_secs", 0L},
      {"lease_secs", 4_294_967_296L},
      {"mtu_bytes", 1279L},
      {"settlement_grace_secs", 0L},
      {"flow_label_bits", 23L},
      {"padding_budget_ms", 0L}
    };
    for (final Object[] violation : profileViolations) {
      expectRuntimeException(
          () ->
              VpnJsonParser.parseProfile(
                  vpnJsonWithField(profile, (String) violation[0], violation[1])),
          "VPN profile parser must reject invalid " + violation[0]);
    }

    final Object instruction = vpnJsonObject(quote).get("open_lease_instruction");
    for (final List<Object> instructions :
        List.of(Collections.emptyList(), List.of(instruction, instruction))) {
      expectRuntimeException(
          () -> VpnJsonParser.parseQuote(vpnJsonWithField(quote, "tx_instructions", instructions)),
          "VPN quote parser must require exactly one transaction instruction");
    }
    expectRuntimeException(
        () -> VpnJsonParser.parseSession(vpnJsonWithField(session, "status", "settled")),
        "VPN session parser must require active status");
    expectRuntimeException(
        () -> VpnJsonParser.parseReceipt(vpnJsonWithField(receipt, "status", "active")),
        "VPN receipt parser must reject active status");
    expectRuntimeException(
        () -> VpnJsonParser.parseReceipt(vpnJsonWithField(receipt, "receipt_source", "operator")),
        "VPN receipt parser must reject unknown receipt sources");
    final Map<String, Object> receiptInstruction =
        Map.of("wire_id", "SettleVpnLease", "payload_hex", "abcd");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseReceipt(
                vpnJsonWithField(
                    receipt,
                    "tx_instructions",
                    List.of(receiptInstruction, receiptInstruction))),
        "VPN receipt parser must allow at most one transaction instruction");

    final Map<String, Object> receiptObject = vpnJsonObject(receipt);
    expectRuntimeException(
        () ->
            VpnJsonParser.parseReceiptList(
                vpnJsonWithField(receiptList, "items", Collections.nCopies(25, receiptObject))),
        "VPN receipt-list parser must allow at most 24 items");
    expectRuntimeException(
        () -> VpnJsonParser.parseReceiptList(vpnJsonWithField(receiptList, "total", 25L)),
        "VPN receipt-list parser must cap total at 24");
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> vpnJsonObject(final String json) {
    return new LinkedHashMap<>((Map<String, Object>) JsonParser.parse(json));
  }

  private static byte[] vpnJsonWithField(
      final String json, final String field, final Object value) {
    final Map<String, Object> root = vpnJsonObject(json);
    root.put(field, value);
    return JsonEncoder.encode(root).getBytes(StandardCharsets.UTF_8);
  }

  private static byte[] vpnJsonWithoutField(final String json, final String field) {
    final Map<String, Object> root = vpnJsonObject(json);
    root.remove(field);
    return JsonEncoder.encode(root).getBytes(StandardCharsets.UTF_8);
  }

  private static void vpnRoutesRejectWrongSuccessfulStatusCodes() throws Exception {
    final String identifier = "33".repeat(32);
    final String paymentTxHash = "44".repeat(32);
    final String meteringKey = VALID_ED25519_PUBLIC_KEY_HEX;
    final KeyPair keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    final ToriiCanonicalRequestAuth auth =
        canonicalAuth("alice", keyPair, 1_700_000_000_050L, "vpn-status-nonce");

    assertVpnWrongStatusRejected(201, vpnProfileJson(), transport -> transport.getVpnProfile().join());
    assertVpnWrongStatusRejected(
        200,
        vpnQuoteJson(identifier, meteringKey),
        transport ->
            transport
                .createVpnQuote(new VpnQuoteCreateRequest("standard", "0x" + meteringKey), auth)
                .join());
    assertVpnWrongStatusRejected(
        200,
        vpnSessionJson(identifier, paymentTxHash),
        transport ->
            transport
                .createVpnSession(
                    new VpnSessionCreateRequest(
                        "standard", identifier, "0x" + paymentTxHash, meteringKey),
                    auth)
                .join());
    assertVpnWrongStatusRejected(
        201,
        vpnSessionJson(identifier, paymentTxHash),
        transport -> transport.getVpnSession(identifier, auth).join());
    assertVpnWrongStatusRejected(
        201,
        vpnReceiptJson(identifier, paymentTxHash, false),
        transport -> transport.deleteVpnSession(identifier, auth).join());
    assertVpnWrongStatusRejected(
        200,
        vpnReceiptJson(identifier, paymentTxHash, true),
        transport ->
            transport
                .submitVpnReceipt(
                    new VpnReceiptSubmitRequest("0xCAFE", "BEEF", "0x" + identifier), auth)
                .join());
    final String receipt = vpnReceiptJson(identifier, paymentTxHash, true);
    assertVpnWrongStatusRejected(
        201,
        "{\"items\":[" + receipt + "],\"total\":1}",
        transport -> transport.listVpnReceipts(auth).join());
  }

  @FunctionalInterface
  private interface VpnTransportCall {
    void invoke(HttpClientTransport transport);
  }

  private static void assertVpnWrongStatusRejected(
      final int status, final String body, final VpnTransportCall call) {
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new StubResponseExecutor(status, body.getBytes(StandardCharsets.UTF_8)),
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());
    expectRuntimeException(
        () -> call.invoke(transport),
        "VPN route must reject unexpected successful status " + status);
  }

  private static void expectRuntimeException(final Runnable action, final String message) {
    boolean threw = false;
    try {
      action.run();
    } catch (final RuntimeException ex) {
      threw = true;
    }
    assert threw : message;
  }

  private static ToriiCanonicalRequestAuth canonicalAuth(
      final String accountId,
      final KeyPair keyPair,
      final Long timestampMs,
      final String nonce) {
    return new ToriiCanonicalRequestAuth(
        accountId,
        message -> signEd25519(keyPair, message),
        timestampMs,
        nonce);
  }

  private static byte[] signEd25519(final KeyPair keyPair, final byte[] message) {
    try {
      final Signature signer = Signature.getInstance("Ed25519");
      signer.initSign(keyPair.getPrivate());
      signer.update(message);
      return signer.sign();
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to sign canonical request fixture", ex);
    }
  }

  private static final class StubResponseExecutor implements HttpTransportExecutor {
    private final TransportResponse response;
    private TransportRequest lastRequest;

    private StubResponseExecutor(final int statusCode, final byte[] body) {
      response = new TransportResponse(statusCode, body, "accepted", Map.of());
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      lastRequest = request;
      if (isCapabilitiesRequest(request)) {
        return CompletableFuture.completedFuture(compatibleCapabilitiesResponse());
      }
      return CompletableFuture.completedFuture(response);
    }

    TransportRequest lastRequest() {
      return lastRequest;
    }
  }
}
