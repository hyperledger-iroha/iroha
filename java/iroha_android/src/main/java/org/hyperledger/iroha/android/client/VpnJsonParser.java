package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/** Minimal JSON parser for Sora VPN Torii responses. */
public final class VpnJsonParser {

  private VpnJsonParser() {}

  public static VpnProfile parseProfile(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "vpn profile response"), "vpn profile response");
    return new VpnProfile(
        Boolean.TRUE.equals(root.get("available")),
        requiredString(root.get("relay_endpoint"), "vpn profile response.relay_endpoint"),
        stringList(root.get("supported_exit_classes"), "vpn profile response.supported_exit_classes"),
        requiredString(root.get("default_exit_class"), "vpn profile response.default_exit_class"),
        asLong(root.get("lease_secs"), "vpn profile response.lease_secs"),
        asLong(root.get("dns_push_interval_secs"), "vpn profile response.dns_push_interval_secs"),
        requiredString(root.get("meter_family"), "vpn profile response.meter_family"),
        stringList(root.get("route_pushes"), "vpn profile response.route_pushes"),
        stringList(root.get("excluded_routes"), "vpn profile response.excluded_routes"),
        stringList(root.get("dns_servers"), "vpn profile response.dns_servers"),
        stringList(root.get("tunnel_addresses"), "vpn profile response.tunnel_addresses"),
        asLong(root.get("mtu_bytes"), "vpn profile response.mtu_bytes"),
        requiredString(root.get("display_billing_label"), "vpn profile response.display_billing_label"),
        requiredString(root.get("fee_asset_id"), "vpn profile response.fee_asset_id"),
        requiredString(root.get("escrow_account_id"), "vpn profile response.escrow_account_id"),
        requiredString(root.get("operator_account_id"), "vpn profile response.operator_account_id"),
        asLong(root.get("lease_fee_nanos"), "vpn profile response.lease_fee_nanos"),
        asLong(root.get("settlement_grace_secs"), "vpn profile response.settlement_grace_secs"),
        asInt(root.get("flow_label_bits"), "vpn profile response.flow_label_bits"),
        asInt(root.get("padding_budget_ms"), "vpn profile response.padding_budget_ms"),
        optionalHex32(root.get("relay_tls_spki_sha256_hex"), "relayTlsSpkiSha256Hex"));
  }

  public static VpnQuote parseQuote(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "vpn quote response"), "vpn quote response");
    return new VpnQuote(
        hex32(root.get("quote_id"), "quoteId"),
        hex32(root.get("lease_id_hex"), "leaseIdHex"),
        evenHex(root.get("session_id_hex"), "sessionIdHex"),
        requiredString(root.get("payment_reference"), "vpn quote response.payment_reference"),
        requiredString(root.get("account_id"), "vpn quote response.account_id"),
        requiredString(root.get("exit_class"), "vpn quote response.exit_class"),
        requiredString(root.get("relay_endpoint"), "vpn quote response.relay_endpoint"),
        asLong(root.get("lease_secs"), "vpn quote response.lease_secs"),
        asLong(root.get("quote_expires_at_ms"), "vpn quote response.quote_expires_at_ms"),
        requiredString(root.get("fee_asset_id"), "vpn quote response.fee_asset_id"),
        requiredString(root.get("escrow_account_id"), "vpn quote response.escrow_account_id"),
        requiredString(root.get("operator_account_id"), "vpn quote response.operator_account_id"),
        asLong(root.get("lease_fee_nanos"), "vpn quote response.lease_fee_nanos"),
        stringList(root.get("route_pushes"), "vpn quote response.route_pushes"),
        stringList(root.get("excluded_routes"), "vpn quote response.excluded_routes"),
        stringList(root.get("dns_servers"), "vpn quote response.dns_servers"),
        stringList(root.get("tunnel_addresses"), "vpn quote response.tunnel_addresses"),
        asLong(root.get("mtu_bytes"), "vpn quote response.mtu_bytes"),
        requiredString(root.get("meter_family"), "vpn quote response.meter_family"),
        asInt(root.get("flow_label_bits"), "vpn quote response.flow_label_bits"),
        asInt(root.get("padding_budget_ms"), "vpn quote response.padding_budget_ms"),
        optionalHex32(root.get("relay_tls_spki_sha256_hex"), "relayTlsSpkiSha256Hex"),
        hex32(root.get("metering_public_key_hex"), "meteringPublicKeyHex"),
        optionalTxInstruction(root.get("open_lease_instruction"), "vpn quote response.open_lease_instruction"),
        txInstructionList(root.get("tx_instructions"), "vpn quote response.tx_instructions"));
  }

  public static VpnSession parseSession(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "vpn session response"), "vpn session response");
    return new VpnSession(
        hex32(root.get("session_id"), "sessionId"),
        requiredString(root.get("account_id"), "vpn session response.account_id"),
        requiredString(root.get("exit_class"), "vpn session response.exit_class"),
        requiredString(root.get("relay_endpoint"), "vpn session response.relay_endpoint"),
        asLong(root.get("lease_secs"), "vpn session response.lease_secs"),
        asLong(root.get("expires_at_ms"), "vpn session response.expires_at_ms"),
        asLong(root.get("connected_at_ms"), "vpn session response.connected_at_ms"),
        requiredString(root.get("meter_family"), "vpn session response.meter_family"),
        hex32(root.get("quote_id"), "quoteId"),
        requiredString(root.get("payment_reference"), "vpn session response.payment_reference"),
        hex32(root.get("payment_tx_hash"), "paymentTxHash"),
        requiredString(root.get("fee_asset_id"), "vpn session response.fee_asset_id"),
        requiredString(root.get("escrow_account_id"), "vpn session response.escrow_account_id"),
        requiredString(root.get("operator_account_id"), "vpn session response.operator_account_id"),
        asLong(root.get("lease_fee_nanos"), "vpn session response.lease_fee_nanos"),
        asInt(root.get("flow_label_bits"), "vpn session response.flow_label_bits"),
        asInt(root.get("padding_budget_ms"), "vpn session response.padding_budget_ms"),
        optionalHex32(root.get("relay_tls_spki_sha256_hex"), "relayTlsSpkiSha256Hex"),
        stringList(root.get("route_pushes"), "vpn session response.route_pushes"),
        stringList(root.get("excluded_routes"), "vpn session response.excluded_routes"),
        stringList(root.get("dns_servers"), "vpn session response.dns_servers"),
        stringList(root.get("tunnel_addresses"), "vpn session response.tunnel_addresses"),
        asLong(root.get("mtu_bytes"), "vpn session response.mtu_bytes"),
        evenHex(root.get("helper_ticket_hex"), "helperTicketHex"),
        asLong(root.get("bytes_in"), "vpn session response.bytes_in"),
        asLong(root.get("bytes_out"), "vpn session response.bytes_out"),
        requiredString(root.get("status"), "vpn session response.status"));
  }

  public static VpnReceipt parseReceipt(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "vpn receipt response"), "vpn receipt response");
    return parseReceiptObject(root, "vpn receipt response");
  }

  public static VpnReceiptListResponse parseReceiptList(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "vpn receipt list response"), "vpn receipt list response");
    final List<Object> rawItems = requiredList(root.get("items"), "vpn receipt list response.items");
    final List<VpnReceipt> items = new ArrayList<>();
    for (int i = 0; i < rawItems.size(); i++) {
      items.add(
          parseReceiptObject(
              expectObject(rawItems.get(i), "vpn receipt list response.items[" + i + "]"),
              "vpn receipt list response.items[" + i + "]"));
    }
    return new VpnReceiptListResponse(
        items, asLong(root.get("total"), "vpn receipt list response.total"));
  }

  private static VpnReceipt parseReceiptObject(final Map<String, Object> root, final String path) {
    return new VpnReceipt(
        hex32(root.get("session_id"), "sessionId"),
        requiredString(root.get("account_id"), path + ".account_id"),
        requiredString(root.get("exit_class"), path + ".exit_class"),
        requiredString(root.get("relay_endpoint"), path + ".relay_endpoint"),
        requiredString(root.get("meter_family"), path + ".meter_family"),
        asLong(root.get("connected_at_ms"), path + ".connected_at_ms"),
        asLong(root.get("disconnected_at_ms"), path + ".disconnected_at_ms"),
        asLong(root.get("duration_ms"), path + ".duration_ms"),
        asLong(root.get("bytes_in"), path + ".bytes_in"),
        asLong(root.get("bytes_out"), path + ".bytes_out"),
        requiredString(root.get("status"), path + ".status"),
        requiredString(root.get("receipt_source"), path + ".receipt_source"),
        hex32(root.get("quote_id"), "quoteId"),
        hex32(root.get("payment_tx_hash"), "paymentTxHash"),
        requiredString(root.get("fee_asset_id"), path + ".fee_asset_id"),
        requiredString(root.get("escrow_account_id"), path + ".escrow_account_id"),
        requiredString(root.get("operator_account_id"), path + ".operator_account_id"),
        asLong(root.get("lease_fee_nanos"), path + ".lease_fee_nanos"),
        asLong(root.get("earned_fee_nanos"), path + ".earned_fee_nanos"),
        asLong(root.get("refunded_fee_nanos"), path + ".refunded_fee_nanos"),
        root.containsKey("lease_id_hex") && root.get("lease_id_hex") != null
            ? hex32(root.get("lease_id_hex"), "leaseIdHex")
            : "",
        optionalTxInstruction(root.get("settle_lease_instruction"), path + ".settle_lease_instruction"),
        txInstructionList(root.get("tx_instructions"), path + ".tx_instructions"));
  }

  private static Object parse(final byte[] payload, final String context) {
    if (payload == null || payload.length == 0) {
      throw new IllegalStateException(context + " returned an empty payload");
    }
    final String json = new String(payload, StandardCharsets.UTF_8).trim();
    if (json.isEmpty()) {
      throw new IllegalStateException(context + " returned a blank payload");
    }
    return JsonParser.parse(json);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> expectObject(final Object value, final String path) {
    if (!(value instanceof Map<?, ?>)) {
      throw new IllegalStateException(path + " must be a JSON object");
    }
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> requiredList(final Object value, final String path) {
    if (!(value instanceof List<?>)) {
      throw new IllegalStateException(path + " must be an array");
    }
    return (List<Object>) value;
  }

  private static List<String> stringList(final Object value, final String path) {
    final List<String> out = new ArrayList<>();
    if (value == null) {
      return out;
    }
    final List<Object> items = requiredList(value, path);
    for (int i = 0; i < items.size(); i++) {
      out.add(requiredString(items.get(i), path + "[" + i + "]"));
    }
    return out;
  }

  private static List<VpnTxInstruction> txInstructionList(final Object value, final String path) {
    final List<VpnTxInstruction> out = new ArrayList<>();
    if (value == null) {
      return out;
    }
    final List<Object> items = requiredList(value, path);
    for (int i = 0; i < items.size(); i++) {
      out.add(parseTxInstruction(expectObject(items.get(i), path + "[" + i + "]"), path + "[" + i + "]"));
    }
    return out;
  }

  private static VpnTxInstruction optionalTxInstruction(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    return parseTxInstruction(expectObject(value, path), path);
  }

  private static VpnTxInstruction parseTxInstruction(
      final Map<String, Object> root, final String path) {
    return new VpnTxInstruction(
        requiredString(root.get("wire_id"), path + ".wire_id"),
        evenHex(root.get("payload_hex"), "payloadHex"));
  }

  private static String requiredString(final Object value, final String path) {
    final String string = optionalString(value);
    if (string == null || string.isBlank()) {
      throw new IllegalStateException(path + " must be a non-empty string");
    }
    return string.trim();
  }

  private static String optionalString(final Object value) {
    if (value == null) {
      return null;
    }
    final String string = value instanceof String ? (String) value : String.valueOf(value);
    final String trimmed = string.trim();
    return trimmed.isEmpty() ? null : trimmed;
  }

  private static long asLong(final Object value, final String path) {
    if (!(value instanceof Number number)) {
      throw new IllegalStateException(path + " must be a number");
    }
    if (number instanceof Float || number instanceof Double) {
      throw new IllegalStateException(path + " must be an integer");
    }
    return number.longValue();
  }

  private static int asInt(final Object value, final String path) {
    final long parsed = asLong(value, path);
    if (parsed < Integer.MIN_VALUE || parsed > Integer.MAX_VALUE) {
      throw new IllegalStateException(path + " must fit in signed 32-bit range");
    }
    return (int) parsed;
  }

  private static String hex32(final Object value, final String field) {
    return HttpClientTransport.normalizeHex32(requiredString(value, field), field);
  }

  private static String optionalHex32(final Object value, final String field) {
    final String literal = optionalString(value);
    return literal == null ? null : HttpClientTransport.normalizeHex32(literal, field);
  }

  private static String evenHex(final Object value, final String field) {
    return HttpClientTransport.normalizeEvenLengthHex(requiredString(value, field), field);
  }
}
