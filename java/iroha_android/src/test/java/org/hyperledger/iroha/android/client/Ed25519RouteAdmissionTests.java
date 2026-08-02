package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.Locale;
import java.util.Map;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.sorafs.GatewayProvider;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;

/** Focused harness for strict Ed25519 admission at textual HTTP and SoraFS boundaries. */
public final class Ed25519RouteAdmissionTests {
  private static final String VALID_PUBLIC_KEY_HEX = TestEd25519Keys.publicKeyHex(0x22);
  private static final String IDENTITY_PUBLIC_KEY_HEX = "01" + "00".repeat(31);

  private Ed25519RouteAdmissionTests() {}

  public static void main(final String[] args) {
    canonicalOutboundTextNormalizationIsPreserved();
    gatewayProviderRejectsIdentityPoint();
    vpnOutboundRejectsIdentityPoint();
    vpnInboundRejectsIdentityPoint();
    multisigProposeRejectsIdentityPoint();
    System.out.println("[IrohaAndroid] Ed25519 route admission tests passed.");
  }

  private static void canonicalOutboundTextNormalizationIsPreserved() {
    final String nonCanonicalText =
        " 0X" + VALID_PUBLIC_KEY_HEX.toUpperCase(Locale.ROOT) + " ";
    final Map<String, Object> quote =
        HttpClientTransport.buildVpnQuoteCreatePayload("standard", nonCanonicalText);
    assert VALID_PUBLIC_KEY_HEX.equals(quote.get("metering_public_key_hex"));

    final Map<String, Object> proposal =
        HttpClientTransport.buildMultisigProposePayload(
            MultisigProposeRequest.builder()
                .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()))
                .setMultisigAccountAlias("cbdc@banka")
                .setSignerAccountId("alice")
                .addInstructionBytes(new byte[] {1})
                .setPublicKeyHex(nonCanonicalText)
                .build());
    assert VALID_PUBLIC_KEY_HEX.equals(proposal.get("public_key_hex"));
  }

  private static void gatewayProviderRejectsIdentityPoint() {
    expectIllegalArgument(
        () ->
            GatewayProvider.builder()
                .setName("primary")
                .setProviderIdHex("11".repeat(32))
                .setGatewayPublicKeyHex(IDENTITY_PUBLIC_KEY_HEX)
                .setBaseUrl("https://gateway.example/")
                .setStreamTokenBase64("c3RyZWFt")
                .build());
  }

  private static void vpnOutboundRejectsIdentityPoint() {
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildVpnQuoteCreatePayload(
                "standard", IDENTITY_PUBLIC_KEY_HEX));
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildVpnSessionCreatePayload(
                "standard", "11".repeat(32), "22".repeat(32), IDENTITY_PUBLIC_KEY_HEX));
  }

  private static void vpnInboundRejectsIdentityPoint() {
    expectIllegalState(
        () ->
            VpnJsonParser.parseQuote(
                vpnQuoteJson(IDENTITY_PUBLIC_KEY_HEX).getBytes(StandardCharsets.UTF_8)));
  }

  private static void multisigProposeRejectsIdentityPoint() {
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder()
                    .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()))
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(new byte[] {1})
                    .setPublicKeyHex(IDENTITY_PUBLIC_KEY_HEX)
                    .build()));
  }

  private static void expectIllegalArgument(final Runnable call) {
    try {
      call.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }

  private static void expectIllegalState(final Runnable call) {
    try {
      call.run();
      throw new AssertionError("expected IllegalStateException");
    } catch (final IllegalStateException expected) {
      // Expected.
    }
  }

  private static String vpnQuoteJson(final String meteringKey) {
    final String quoteId = "11".repeat(32);
    return "{"
        + "\"quote_id\":\""
        + quoteId
        + "\",\"lease_id_hex\":\""
        + quoteId
        + "\",\"session_id_hex\":\""
        + "aa".repeat(16)
        + "\",\"payment_reference\":\""
        + quoteId
        + "\",\"account_id\":\"alice\","
        + "\"exit_class\":\"low-latency\","
        + "\"relay_endpoint\":\"/dns/relay.example/udp/9443/quic\","
        + "\"lease_secs\":600,"
        + "\"quote_expires_at_ms\":1700000600000,"
        + "\"fee_asset_id\":\"xor#universal.universal\","
        + "\"escrow_account_id\":\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\","
        + "\"operator_account_id\":\"sorauﾛ1NｱｻｸYSafﾇｷヰc5ﾇﾄVxﾏ9jLZヱﾋzsKqurﾊﾘ9ｸ3eｴAｶD54TDT\","
        + "\"lease_fee\":\"1000000.25\","
        + "\"route_pushes\":[\"0.0.0.0/0\"],"
        + "\"excluded_routes\":[],"
        + "\"dns_servers\":[\"1.1.1.1\"],"
        + "\"tunnel_addresses\":[\"10.208.0.2/32\"],"
        + "\"mtu_bytes\":1280,"
        + "\"meter_family\":\"soranet.vpn.standard\","
        + "\"flow_label_bits\":24,"
        + "\"padding_budget_ms\":15,"
        + "\"relay_id_hex\":\""
        + VALID_PUBLIC_KEY_HEX
        + "\","
        + "\"descriptor_commit_hex\":\""
        + "cd".repeat(32)
        + "\","
        + "\"tls_server_name\":\"relay.example\","
        + "\"relay_tls_spki_sha256_hex\":\""
        + "ab".repeat(32)
        + "\","
        + "\"relay_certificate_sha256_hex\":\""
        + "ef".repeat(32)
        + "\","
        + "\"directory_snapshot_digest_hex\":\""
        + "42".repeat(32)
        + "\",\"metering_public_key_hex\":\""
        + meteringKey
        + "\",\"open_lease_instruction\":{"
        + "\"wire_id\":\"iroha_data_model::isi::vpn::OpenVpnLeaseEscrow\","
        + "\"payload_hex\":\"cafe\"},"
        + "\"tx_instructions\":[{"
        + "\"wire_id\":\"iroha_data_model::isi::vpn::OpenVpnLeaseEscrow\","
        + "\"payload_hex\":\"cafe\"}]"
        + "}";
  }
}
