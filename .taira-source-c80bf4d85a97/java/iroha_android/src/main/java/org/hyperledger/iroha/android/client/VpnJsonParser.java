package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.hyperledger.iroha.android.crypto.Ed25519PublicKeyAdmission;
import org.hyperledger.iroha.android.numeric.NumericV1;

/** Minimal JSON parser for Sora VPN Torii responses. */
public final class VpnJsonParser {

  private static final int VPN_HELPER_TICKET_HEX_LENGTH = 1328;
  private static final long U32_MAX = 4_294_967_295L;
  private static final Set<String> EXIT_CLASSES =
      fields("standard", "low-latency", "high-security");
  private static final Set<String> RECEIPT_STATUSES =
      fields("disconnected", "expired", "replaced", "settled");
  private static final Set<String> RECEIPT_SOURCES = fields("torii", "relay", "wsv");
  private static final Set<String> PROFILE_FIELDS =
      fields(
          "available", "relay_endpoint", "supported_exit_classes", "default_exit_class",
          "lease_secs", "dns_push_interval_secs", "meter_family", "route_pushes",
          "excluded_routes", "dns_servers", "tunnel_addresses", "mtu_bytes",
          "display_billing_label", "fee_asset_id", "escrow_account_id", "operator_account_id",
          "lease_fee", "settlement_grace_secs", "flow_label_bits", "padding_budget_ms",
          "relay_tls_spki_sha256_hex");
  private static final Set<String> QUOTE_FIELDS =
      fields(
          "quote_id", "lease_id_hex", "session_id_hex", "payment_reference", "account_id",
          "exit_class", "relay_endpoint", "lease_secs", "quote_expires_at_ms", "fee_asset_id",
          "escrow_account_id", "operator_account_id", "lease_fee", "route_pushes",
          "excluded_routes", "dns_servers", "tunnel_addresses", "mtu_bytes", "meter_family",
          "flow_label_bits", "padding_budget_ms", "relay_tls_spki_sha256_hex",
          "metering_public_key_hex", "open_lease_instruction", "tx_instructions");
  private static final Set<String> SESSION_FIELDS =
      fields(
          "session_id", "account_id", "exit_class", "relay_endpoint", "lease_secs",
          "expires_at_ms", "connected_at_ms", "meter_family", "quote_id", "payment_reference",
          "payment_tx_hash", "fee_asset_id", "escrow_account_id", "operator_account_id",
          "lease_fee", "flow_label_bits", "padding_budget_ms", "relay_tls_spki_sha256_hex",
          "route_pushes", "excluded_routes", "dns_servers", "tunnel_addresses", "mtu_bytes",
          "helper_ticket_hex", "bytes_in", "bytes_out", "status");
  private static final Set<String> RECEIPT_FIELDS =
      fields(
          "session_id", "account_id", "exit_class", "relay_endpoint", "meter_family",
          "connected_at_ms", "disconnected_at_ms", "duration_ms", "bytes_in", "bytes_out",
          "status", "receipt_source", "quote_id", "payment_tx_hash", "fee_asset_id",
          "escrow_account_id", "operator_account_id", "lease_fee", "earned_fee", "refunded_fee",
          "lease_id_hex", "settle_lease_instruction", "tx_instructions");
  private static final Set<String> RECEIPT_LIST_FIELDS = fields("items", "total");
  private static final Set<String> TX_INSTRUCTION_FIELDS = fields("wire_id", "payload_hex");

  private VpnJsonParser() {}

  public static VpnProfile parseProfile(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "vpn profile response"), "vpn profile response");
    requireExactFields(root, PROFILE_FIELDS, "vpn profile response");
    return new VpnProfile(
        requiredBoolean(root.get("available"), "vpn profile response.available"),
        requiredString(root.get("relay_endpoint"), "vpn profile response.relay_endpoint"),
        exitClassList(root.get("supported_exit_classes"), "vpn profile response.supported_exit_classes"),
        exitClass(root.get("default_exit_class"), "vpn profile response.default_exit_class"),
        boundedLong(root.get("lease_secs"), "vpn profile response.lease_secs", 1L, U32_MAX),
        atLeastLong(
            root.get("dns_push_interval_secs"),
            "vpn profile response.dns_push_interval_secs",
            30L),
        requiredString(root.get("meter_family"), "vpn profile response.meter_family"),
        stringList(root.get("route_pushes"), "vpn profile response.route_pushes"),
        stringList(root.get("excluded_routes"), "vpn profile response.excluded_routes"),
        stringList(root.get("dns_servers"), "vpn profile response.dns_servers"),
        stringList(root.get("tunnel_addresses"), "vpn profile response.tunnel_addresses"),
        exactLong(root.get("mtu_bytes"), "vpn profile response.mtu_bytes", 1280L),
        requiredString(root.get("display_billing_label"), "vpn profile response.display_billing_label"),
        requiredString(root.get("fee_asset_id"), "vpn profile response.fee_asset_id"),
        requiredString(root.get("escrow_account_id"), "vpn profile response.escrow_account_id"),
        requiredString(root.get("operator_account_id"), "vpn profile response.operator_account_id"),
        quantity(root.get("lease_fee"), "vpn profile response.lease_fee"),
        atLeastLong(root.get("settlement_grace_secs"), "vpn profile response.settlement_grace_secs", 1L),
        exactInt(root.get("flow_label_bits"), "vpn profile response.flow_label_bits", 24),
        boundedInt(root.get("padding_budget_ms"), "vpn profile response.padding_budget_ms", 1, 65535),
        optionalHex32(root.get("relay_tls_spki_sha256_hex"), "relayTlsSpkiSha256Hex"));
  }

  public static VpnQuote parseQuote(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "vpn quote response"), "vpn quote response");
    requireExactFields(root, QUOTE_FIELDS, "vpn quote response");
    return new VpnQuote(
        hex32(root.get("quote_id"), "quoteId"),
        hex32(root.get("lease_id_hex"), "leaseIdHex"),
        hex16(root.get("session_id_hex"), "sessionIdHex"),
        requiredString(root.get("payment_reference"), "vpn quote response.payment_reference"),
        requiredString(root.get("account_id"), "vpn quote response.account_id"),
        exitClass(root.get("exit_class"), "vpn quote response.exit_class"),
        requiredString(root.get("relay_endpoint"), "vpn quote response.relay_endpoint"),
        boundedLong(root.get("lease_secs"), "vpn quote response.lease_secs", 1L, U32_MAX),
        atLeastLong(root.get("quote_expires_at_ms"), "vpn quote response.quote_expires_at_ms", 0L),
        requiredString(root.get("fee_asset_id"), "vpn quote response.fee_asset_id"),
        requiredString(root.get("escrow_account_id"), "vpn quote response.escrow_account_id"),
        requiredString(root.get("operator_account_id"), "vpn quote response.operator_account_id"),
        quantity(root.get("lease_fee"), "vpn quote response.lease_fee"),
        stringList(root.get("route_pushes"), "vpn quote response.route_pushes"),
        stringList(root.get("excluded_routes"), "vpn quote response.excluded_routes"),
        stringList(root.get("dns_servers"), "vpn quote response.dns_servers"),
        stringList(root.get("tunnel_addresses"), "vpn quote response.tunnel_addresses"),
        exactLong(root.get("mtu_bytes"), "vpn quote response.mtu_bytes", 1280L),
        requiredString(root.get("meter_family"), "vpn quote response.meter_family"),
        exactInt(root.get("flow_label_bits"), "vpn quote response.flow_label_bits", 24),
        boundedInt(root.get("padding_budget_ms"), "vpn quote response.padding_budget_ms", 1, 65535),
        optionalHex32(root.get("relay_tls_spki_sha256_hex"), "relayTlsSpkiSha256Hex"),
        ed25519PublicKeyHex(root.get("metering_public_key_hex"), "meteringPublicKeyHex"),
        optionalTxInstruction(root.get("open_lease_instruction"), "vpn quote response.open_lease_instruction"),
        txInstructionList(root.get("tx_instructions"), "vpn quote response.tx_instructions", 1, 1));
  }

  public static VpnSession parseSession(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "vpn session response"), "vpn session response");
    requireExactFields(root, SESSION_FIELDS, "vpn session response");
    return new VpnSession(
        hex32(root.get("session_id"), "sessionId"),
        requiredString(root.get("account_id"), "vpn session response.account_id"),
        exitClass(root.get("exit_class"), "vpn session response.exit_class"),
        requiredString(root.get("relay_endpoint"), "vpn session response.relay_endpoint"),
        boundedLong(root.get("lease_secs"), "vpn session response.lease_secs", 1L, U32_MAX),
        atLeastLong(root.get("expires_at_ms"), "vpn session response.expires_at_ms", 0L),
        atLeastLong(root.get("connected_at_ms"), "vpn session response.connected_at_ms", 0L),
        requiredString(root.get("meter_family"), "vpn session response.meter_family"),
        hex32(root.get("quote_id"), "quoteId"),
        requiredString(root.get("payment_reference"), "vpn session response.payment_reference"),
        hex32(root.get("payment_tx_hash"), "paymentTxHash"),
        requiredString(root.get("fee_asset_id"), "vpn session response.fee_asset_id"),
        requiredString(root.get("escrow_account_id"), "vpn session response.escrow_account_id"),
        requiredString(root.get("operator_account_id"), "vpn session response.operator_account_id"),
        quantity(root.get("lease_fee"), "vpn session response.lease_fee"),
        exactInt(root.get("flow_label_bits"), "vpn session response.flow_label_bits", 24),
        boundedInt(root.get("padding_budget_ms"), "vpn session response.padding_budget_ms", 1, 65535),
        optionalHex32(root.get("relay_tls_spki_sha256_hex"), "relayTlsSpkiSha256Hex"),
        stringList(root.get("route_pushes"), "vpn session response.route_pushes"),
        stringList(root.get("excluded_routes"), "vpn session response.excluded_routes"),
        stringList(root.get("dns_servers"), "vpn session response.dns_servers"),
        stringList(root.get("tunnel_addresses"), "vpn session response.tunnel_addresses"),
        exactLong(root.get("mtu_bytes"), "vpn session response.mtu_bytes", 1280L),
        helperTicketHex(root.get("helper_ticket_hex"), "helperTicketHex"),
        atLeastLong(root.get("bytes_in"), "vpn session response.bytes_in", 0L),
        atLeastLong(root.get("bytes_out"), "vpn session response.bytes_out", 0L),
        exactString(root.get("status"), "vpn session response.status", fields("active")));
  }

  public static VpnReceipt parseReceipt(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "vpn receipt response"), "vpn receipt response");
    return parseReceiptObject(root, "vpn receipt response");
  }

  public static VpnReceiptListResponse parseReceiptList(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "vpn receipt list response"), "vpn receipt list response");
    requireExactFields(root, RECEIPT_LIST_FIELDS, "vpn receipt list response");
    final List<Object> rawItems = requiredList(root.get("items"), "vpn receipt list response.items");
    final List<VpnReceipt> items = new ArrayList<>();
    for (int i = 0; i < rawItems.size(); i++) {
      items.add(
          parseReceiptObject(
              expectObject(rawItems.get(i), "vpn receipt list response.items[" + i + "]"),
              "vpn receipt list response.items[" + i + "]"));
    }
    if (items.size() > 24) {
      throw new IllegalStateException(
          "vpn receipt list response.items must contain at most 24 entries");
    }
    return new VpnReceiptListResponse(
        items, boundedLong(root.get("total"), "vpn receipt list response.total", 0L, 24L));
  }

  private static VpnReceipt parseReceiptObject(final Map<String, Object> root, final String path) {
    requireExactFields(root, RECEIPT_FIELDS, path);
    return new VpnReceipt(
        hex32(root.get("session_id"), "sessionId"),
        requiredString(root.get("account_id"), path + ".account_id"),
        exitClass(root.get("exit_class"), path + ".exit_class"),
        requiredString(root.get("relay_endpoint"), path + ".relay_endpoint"),
        requiredString(root.get("meter_family"), path + ".meter_family"),
        atLeastLong(root.get("connected_at_ms"), path + ".connected_at_ms", 0L),
        atLeastLong(root.get("disconnected_at_ms"), path + ".disconnected_at_ms", 0L),
        atLeastLong(root.get("duration_ms"), path + ".duration_ms", 0L),
        atLeastLong(root.get("bytes_in"), path + ".bytes_in", 0L),
        atLeastLong(root.get("bytes_out"), path + ".bytes_out", 0L),
        exactString(root.get("status"), path + ".status", RECEIPT_STATUSES),
        exactString(root.get("receipt_source"), path + ".receipt_source", RECEIPT_SOURCES),
        hex32(root.get("quote_id"), "quoteId"),
        hex32(root.get("payment_tx_hash"), "paymentTxHash"),
        requiredString(root.get("fee_asset_id"), path + ".fee_asset_id"),
        requiredString(root.get("escrow_account_id"), path + ".escrow_account_id"),
        requiredString(root.get("operator_account_id"), path + ".operator_account_id"),
        quantity(root.get("lease_fee"), path + ".lease_fee"),
        quantity(root.get("earned_fee"), path + ".earned_fee"),
        quantity(root.get("refunded_fee"), path + ".refunded_fee"),
        hex32(root.get("lease_id_hex"), "leaseIdHex"),
        optionalTxInstruction(root.get("settle_lease_instruction"), path + ".settle_lease_instruction"),
        txInstructionList(root.get("tx_instructions"), path + ".tx_instructions", 0, 1));
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

  private static String quantity(final Object value, final String path) {
    try {
      return NumericV1.decodeQuantityJsonValue(value).toString();
    } catch (final IllegalArgumentException error) {
      throw new IllegalStateException(
          path + " must be a canonical non-negative quantity string", error);
    }
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
    final List<Object> items = requiredList(value, path);
    for (int i = 0; i < items.size(); i++) {
      out.add(requiredString(items.get(i), path + "[" + i + "]"));
    }
    return out;
  }

  private static List<VpnTxInstruction> txInstructionList(
      final Object value, final String path, final int minimum, final int maximum) {
    final List<VpnTxInstruction> out = new ArrayList<>();
    final List<Object> items = requiredList(value, path);
    for (int i = 0; i < items.size(); i++) {
      out.add(parseTxInstruction(expectObject(items.get(i), path + "[" + i + "]"), path + "[" + i + "]"));
    }
    if (out.size() < minimum || out.size() > maximum) {
      throw new IllegalStateException(
          path + " must contain between " + minimum + " and " + maximum + " entries");
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
    requireExactFields(root, TX_INSTRUCTION_FIELDS, path);
    return new VpnTxInstruction(
        requiredString(root.get("wire_id"), path + ".wire_id"),
        canonicalEvenHex(root.get("payload_hex"), "payloadHex"));
  }

  private static String requiredString(final Object value, final String path) {
    if (!(value instanceof String)
        || ((String) value).isEmpty()
        || !((String) value).trim().equals(value)) {
      throw new IllegalStateException(
          path + " must be a non-empty string without surrounding whitespace");
    }
    return (String) value;
  }

  private static boolean requiredBoolean(final Object value, final String path) {
    if (!(value instanceof Boolean)) {
      throw new IllegalStateException(path + " must be a boolean");
    }
    return ((Boolean) value).booleanValue();
  }

  private static String exitClass(final Object value, final String path) {
    return exactString(value, path, EXIT_CLASSES);
  }

  private static List<String> exitClassList(final Object value, final String path) {
    final List<Object> items = requiredList(value, path);
    final List<String> out = new ArrayList<>();
    for (int i = 0; i < items.size(); i++) {
      out.add(exitClass(items.get(i), path + "[" + i + "]"));
    }
    if (out.size() != 3 || new HashSet<>(out).size() != 3) {
      throw new IllegalStateException(
          path + " must contain each of the three supported exit classes exactly once");
    }
    return out;
  }

  private static String exactString(
      final Object value, final String path, final Set<String> allowed) {
    final String parsed = requiredString(value, path);
    if (!allowed.contains(parsed)) {
      throw new IllegalStateException(path + " must be one of " + allowed);
    }
    return parsed;
  }

  private static long asLong(final Object value, final String path) {
    return JsonNumbers.asLong(value, path);
  }

  private static long atLeastLong(
      final Object value, final String path, final long minimum) {
    final long parsed = asLong(value, path);
    if (parsed < minimum) {
      throw new IllegalStateException(path + " must be at least " + minimum);
    }
    return parsed;
  }

  private static long boundedLong(
      final Object value, final String path, final long minimum, final long maximum) {
    final long parsed = asLong(value, path);
    if (parsed < minimum || parsed > maximum) {
      throw new IllegalStateException(
          path + " must be between " + minimum + " and " + maximum);
    }
    return parsed;
  }

  private static long exactLong(final Object value, final String path, final long expected) {
    final long parsed = asLong(value, path);
    if (parsed != expected) {
      throw new IllegalStateException(path + " must equal " + expected);
    }
    return parsed;
  }

  private static int boundedInt(
      final Object value, final String path, final int minimum, final int maximum) {
    final int parsed = asInt(value, path);
    if (parsed < minimum || parsed > maximum) {
      throw new IllegalStateException(
          path + " must be between " + minimum + " and " + maximum);
    }
    return parsed;
  }

  private static int exactInt(final Object value, final String path, final int expected) {
    final int parsed = asInt(value, path);
    if (parsed != expected) {
      throw new IllegalStateException(path + " must equal " + expected);
    }
    return parsed;
  }

  private static String helperTicketHex(final Object value, final String field) {
    if (!(value instanceof String)) {
      throw new IllegalStateException(
          field
              + " must be exactly "
              + VPN_HELPER_TICKET_HEX_LENGTH
              + " lowercase hexadecimal characters");
    }
    final String literal = (String) value;
    if (literal.length() != VPN_HELPER_TICKET_HEX_LENGTH) {
      throw new IllegalStateException(
          field
              + " must be exactly "
              + VPN_HELPER_TICKET_HEX_LENGTH
              + " lowercase hexadecimal characters");
    }
    for (int i = 0; i < literal.length(); i++) {
      final char character = literal.charAt(i);
      if (!((character >= '0' && character <= '9')
          || (character >= 'a' && character <= 'f'))) {
        throw new IllegalStateException(
            field
                + " must be exactly "
                + VPN_HELPER_TICKET_HEX_LENGTH
                + " lowercase hexadecimal characters");
      }
    }
    return literal;
  }

  private static int asInt(final Object value, final String path) {
    return JsonNumbers.asInt(value, path);
  }

  private static Set<String> fields(final String... values) {
    return Collections.unmodifiableSet(new HashSet<>(Arrays.asList(values)));
  }

  private static void requireExactFields(
      final Map<String, Object> root, final Set<String> allowed, final String path) {
    for (final String field : root.keySet()) {
      if (!allowed.contains(field)) {
        throw new IllegalStateException(path + " contains unknown field `" + field + "`");
      }
    }
    for (final String field : allowed) {
      if (!root.containsKey(field)) {
        throw new IllegalStateException(path + " is missing required field `" + field + "`");
      }
    }
  }

  private static String canonicalHex(
      final Object value, final String field, final int length) {
    if (!(value instanceof String)) {
      throw new IllegalStateException(
          field + " must be exactly " + length + " lowercase hexadecimal characters");
    }
    final String literal = (String) value;
    if (literal.length() != length) {
      throw new IllegalStateException(
          field + " must be exactly " + length + " lowercase hexadecimal characters");
    }
    for (int i = 0; i < literal.length(); i++) {
      final char character = literal.charAt(i);
      if (!((character >= '0' && character <= '9')
          || (character >= 'a' && character <= 'f'))) {
        throw new IllegalStateException(
            field + " must be exactly " + length + " lowercase hexadecimal characters");
      }
    }
    return literal;
  }

  private static String hex32(final Object value, final String field) {
    return canonicalHex(value, field, 64);
  }

  private static String ed25519PublicKeyHex(final Object value, final String field) {
    final String canonical = hex32(value, field);
    final byte[] publicKey = new byte[Ed25519PublicKeyAdmission.PUBLIC_KEY_LENGTH];
    for (int index = 0; index < publicKey.length; index++) {
      final int offset = index * 2;
      publicKey[index] =
          (byte)
              ((Character.digit(canonical.charAt(offset), 16) << 4)
                  | Character.digit(canonical.charAt(offset + 1), 16));
    }
    if (!Ed25519PublicKeyAdmission.isValid(publicKey)) {
      throw new IllegalStateException(
          field + " must encode a canonical prime-order Ed25519 public key");
    }
    return canonical;
  }

  private static String hex16(final Object value, final String field) {
    return canonicalHex(value, field, 32);
  }

  private static String optionalHex32(final Object value, final String field) {
    return value == null ? null : canonicalHex(value, field, 64);
  }

  private static String canonicalEvenHex(final Object value, final String field) {
    if (!(value instanceof String)) {
      throw new IllegalStateException(
          field + " must be non-empty even-length lowercase hexadecimal");
    }
    final String literal = (String) value;
    if (literal.isEmpty() || literal.length() % 2 != 0) {
      throw new IllegalStateException(
          field + " must be non-empty even-length lowercase hexadecimal");
    }
    for (int i = 0; i < literal.length(); i++) {
      final char character = literal.charAt(i);
      if (!((character >= '0' && character <= '9')
          || (character >= 'a' && character <= 'f'))) {
        throw new IllegalStateException(
            field + " must be non-empty even-length lowercase hexadecimal");
      }
    }
    return literal;
  }
}
