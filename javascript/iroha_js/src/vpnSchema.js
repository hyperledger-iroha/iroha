import { Buffer } from "buffer";
import { assertValidEd25519PublicKey } from "./ed25519Strict.js";
import { createValidationError, ValidationErrorCode } from "./validationError.js";

const VPN_HELPER_TICKET_BYTES = 788;
const VPN_HELPER_TICKET_HEX_LENGTH = VPN_HELPER_TICKET_BYTES * 2;
const VPN_RELAY_MLDSA65_PUBLIC_KEY_HEX_LENGTH = 1_952 * 2;
const VPN_EXIT_CLASSES = new Set(["standard", "low-latency", "high-security"]);
const VPN_SESSION_STATUSES = new Set(["active"]);
const VPN_RECEIPT_STATUSES = new Set([
  "disconnected",
  "expired",
  "replaced",
  "settlement_pending",
  "settled",
]);
const VPN_RECEIPT_SOURCES = new Set(["torii", "relay", "wsv"]);
const VPN_LEASE_SECONDS_MAX = 0xffff_ffff;
const VPN_QUOTE_CREATE_REQUEST_KEYS = new Set([
  "exitClass",
  "exit_class",
  "meteringPublicKeyHex",
  "metering_public_key_hex",
]);
const VPN_SESSION_CREATE_REQUEST_KEYS = new Set([
  "exitClass",
  "exit_class",
  "quoteId",
  "quote_id",
  "paymentTxHash",
  "payment_tx_hash",
  "meteringPublicKeyHex",
  "metering_public_key_hex",
]);
const VPN_RECEIPT_SUBMIT_REQUEST_KEYS = new Set([
  "relayReceiptHex",
  "relay_receipt_hex",
  "clientVoucherHex",
  "client_voucher_hex",
  "leaseIdHex",
  "lease_id_hex",
]);
const VPN_TX_INSTRUCTION_RESPONSE_FIELDS = new Set(["wire_id", "payload_hex"]);
const VPN_PROFILE_RESPONSE_FIELDS = new Set([
  "available",
  "relay_endpoint",
  "supported_exit_classes",
  "default_exit_class",
  "lease_secs",
  "dns_push_interval_secs",
  "meter_family",
  "route_pushes",
  "excluded_routes",
  "dns_servers",
  "tunnel_addresses",
  "mtu_bytes",
  "display_billing_label",
  "operator_account_id",
  "lease_fee",
  "settlement_grace_secs",
  "flow_label_bits",
  "padding_budget_ms",
  "relay_id_hex",
  "relay_mldsa65_public_key_hex",
  "descriptor_commit_hex",
  "tls_server_name",
  "relay_tls_spki_sha256_hex",
  "relay_certificate_sha256_hex",
  "directory_snapshot_digest_hex",
]);
const VPN_QUOTE_RESPONSE_FIELDS = new Set([
  "quote_id",
  "lease_id_hex",
  "session_id_hex",
  "payment_reference",
  "account_id",
  "exit_class",
  "relay_endpoint",
  "lease_secs",
  "quote_expires_at_ms",
  "fee_asset_id",
  "escrow_account_id",
  "operator_account_id",
  "lease_fee",
  "route_pushes",
  "excluded_routes",
  "dns_servers",
  "tunnel_addresses",
  "mtu_bytes",
  "meter_family",
  "flow_label_bits",
  "padding_budget_ms",
  "relay_id_hex",
  "relay_mldsa65_public_key_hex",
  "descriptor_commit_hex",
  "tls_server_name",
  "relay_tls_spki_sha256_hex",
  "relay_certificate_sha256_hex",
  "directory_snapshot_digest_hex",
  "metering_public_key_hex",
  "open_lease_instruction",
]);
const VPN_SESSION_RESPONSE_FIELDS = new Set([
  "session_id",
  "account_id",
  "exit_class",
  "relay_endpoint",
  "lease_secs",
  "expires_at_ms",
  "connected_at_ms",
  "meter_family",
  "quote_id",
  "payment_reference",
  "payment_tx_hash",
  "fee_asset_id",
  "escrow_account_id",
  "operator_account_id",
  "lease_fee",
  "flow_label_bits",
  "padding_budget_ms",
  "relay_id_hex",
  "relay_mldsa65_public_key_hex",
  "descriptor_commit_hex",
  "tls_server_name",
  "relay_tls_spki_sha256_hex",
  "relay_certificate_sha256_hex",
  "directory_snapshot_digest_hex",
  "route_pushes",
  "excluded_routes",
  "dns_servers",
  "tunnel_addresses",
  "mtu_bytes",
  "helper_ticket_hex",
  "bytes_in",
  "bytes_out",
  "status",
]);
const VPN_RECEIPT_RESPONSE_FIELDS = new Set([
  "session_id",
  "account_id",
  "exit_class",
  "relay_endpoint",
  "meter_family",
  "connected_at_ms",
  "disconnected_at_ms",
  "duration_ms",
  "bytes_in",
  "bytes_out",
  "status",
  "receipt_source",
  "quote_id",
  "payment_tx_hash",
  "fee_asset_id",
  "escrow_account_id",
  "operator_account_id",
  "lease_fee",
  "earned_fee",
  "refunded_fee",
  "lease_id_hex",
  "settle_lease_instruction",
]);
const VPN_RECEIPT_LIST_RESPONSE_FIELDS = new Set(["items", "total"]);

export function createVpnSchema({
  requireExactLowerHex32String,
  requireExactNonEmptyString,
}) {
  function normalizeVpnTrustTuple(record, context, { allowEmpty = false } = {}) {
    return {
      relayIdHex: requireVpnRelayId(record.relay_id_hex, `${context}.relay_id_hex`, {
        allowEmpty,
      }),
      relayMldsa65PublicKeyHex: requireVpnMldsa65PublicKey(
        record.relay_mldsa65_public_key_hex,
        `${context}.relay_mldsa65_public_key_hex`,
        { allowEmpty },
      ),
      descriptorCommitHex: requireVpnTrustDigest(
        record.descriptor_commit_hex,
        `${context}.descriptor_commit_hex`,
        { allowEmpty },
      ),
      tlsServerName: requireVpnTlsServerName(
        record.tls_server_name,
        `${context}.tls_server_name`,
        { allowEmpty },
      ),
      relayTlsSpkiSha256Hex: requireVpnTrustDigest(
        record.relay_tls_spki_sha256_hex,
        `${context}.relay_tls_spki_sha256_hex`,
        { allowEmpty },
      ),
      relayCertificateSha256Hex: requireVpnTrustDigest(
        record.relay_certificate_sha256_hex,
        `${context}.relay_certificate_sha256_hex`,
        { allowEmpty },
      ),
      directorySnapshotDigestHex: requireVpnTrustDigest(
        record.directory_snapshot_digest_hex,
        `${context}.directory_snapshot_digest_hex`,
        { allowEmpty },
      ),
    };
  }

  function requireVpnRelayId(value, context, { allowEmpty = false } = {}) {
    if (allowEmpty && value === "") return "";
    const literal = requireExactLowerHex32String(value, context);
    try {
      assertValidEd25519PublicKey(Buffer.from(literal, "hex"));
    } catch (error) {
      throw createValidationError(
        ValidationErrorCode.INVALID_HEX,
        `${context} must encode a canonical prime-order Ed25519 public key`,
        context,
        { cause: error },
      );
    }
    return literal;
  }

  function requireVpnMldsa65PublicKey(value, context, { allowEmpty = false } = {}) {
    if (allowEmpty && value === "") return "";
    if (
      typeof value !== "string" ||
      value.length !== VPN_RELAY_MLDSA65_PUBLIC_KEY_HEX_LENGTH ||
      /[^0-9a-f]/u.test(value)
    ) {
      throw createValidationError(
        ValidationErrorCode.INVALID_HEX,
        `${context} must be exactly ${VPN_RELAY_MLDSA65_PUBLIC_KEY_HEX_LENGTH} lowercase hexadecimal characters`,
        context,
      );
    }
    if (/^0+$/u.test(value)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_HEX,
        `${context} must not be the all-zero ML-DSA-65 key`,
        context,
      );
    }
    return value;
  }

  function requireVpnTrustDigest(value, context, { allowEmpty = false } = {}) {
    if (allowEmpty && value === "") return "";
    const literal = requireExactLowerHex32String(value, context);
    if (/^0+$/u.test(literal)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_HEX,
        `${context} must not be the all-zero digest`,
        context,
      );
    }
    return literal;
  }

  function requireVpnTlsServerName(value, context, { allowEmpty = false } = {}) {
    if (allowEmpty && value === "") return "";
    const literal = requireExactNonEmptyString(value, context);
    const labels = literal.split(".");
    if (
      literal.length > 253 ||
      literal !== literal.toLowerCase() ||
      labels.some(
        (label) =>
          label.length === 0 ||
          label.length > 63 ||
          !/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/u.test(label),
      )
    ) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context} must be a canonical lowercase DNS name`,
        context,
      );
    }
    return literal;
  }

  function requireVpnRelayEndpoint(value, context, { allowEmpty = false } = {}) {
    if (allowEmpty && value === "") return "";
    const literal = requireExactNonEmptyString(value, context);
    const parts = literal.split("/");
    if (
      parts.length !== 6 ||
      parts[0] !== "" ||
      !new Set(["ip4", "ip6", "dns", "dns4", "dns6"]).has(parts[1]) ||
      parts[3] !== "udp" ||
      parts[5] !== "quic"
    ) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context} must use /{ip4|ip6|dns|dns4|dns6}/host/udp/port/quic`,
        context,
      );
    }
    const [, protocol, host, , port] = parts;
    if (protocol === "ip4") {
      const octets = host.split(".");
      if (
        octets.length !== 4 ||
        octets.some((octet) => {
          const parsed = Number(octet);
          return !/^(?:0|[1-9][0-9]{0,2})$/u.test(octet) || parsed > 255;
        })
      ) {
        throw createValidationError(
          ValidationErrorCode.INVALID_STRING,
          `${context} must contain a canonical IPv4 address`,
          context,
        );
      }
    } else if (protocol === "ip6") {
      let canonicalHost = null;
      try {
        canonicalHost = new URL(`https://[${host}]/`).hostname;
      } catch {
        // Handled by the canonical comparison below.
      }
      if (canonicalHost !== `[${host}]` || host !== host.toLowerCase()) {
        throw createValidationError(
          ValidationErrorCode.INVALID_STRING,
          `${context} must contain a canonical lowercase IPv6 address`,
          context,
        );
      }
    } else {
      requireVpnTlsServerName(host, `${context} host`);
    }
    const parsedPort = Number(port);
    if (
      !/^[1-9][0-9]{0,4}$/u.test(port) ||
      !Number.isSafeInteger(parsedPort) ||
      parsedPort > 65535
    ) {
      throw createValidationError(
        ValidationErrorCode.INVALID_NUMERIC,
        `${context} must contain a canonical non-zero UDP port`,
        context,
      );
    }
    return literal;
  }

  function requireVpnEnum(value, allowed, context) {
    const literal = requireExactNonEmptyString(value, context);
    if (!allowed.has(literal)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context} must be one of: ${[...allowed].join(", ")}`,
        context,
      );
    }
    return literal;
  }

  function requireVpnProfileExitClasses(value, context) {
    if (!Array.isArray(value)) {
      throw new TypeError(`${context} must be an array`);
    }
    const exits = value.map((entry, index) =>
      requireVpnEnum(entry, VPN_EXIT_CLASSES, `${context}[${index}]`),
    );
    if (exits.length !== 3 || new Set(exits).size !== 3) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context} must contain exactly three unique exit classes`,
        context,
      );
    }
    return exits;
  }

  return {
    VPN_HELPER_TICKET_BYTES,
    VPN_HELPER_TICKET_HEX_LENGTH,
    VPN_EXIT_CLASSES,
    VPN_SESSION_STATUSES,
    VPN_RECEIPT_STATUSES,
    VPN_RECEIPT_SOURCES,
    VPN_LEASE_SECONDS_MAX,
    VPN_QUOTE_CREATE_REQUEST_KEYS,
    VPN_SESSION_CREATE_REQUEST_KEYS,
    VPN_RECEIPT_SUBMIT_REQUEST_KEYS,
    VPN_TX_INSTRUCTION_RESPONSE_FIELDS,
    VPN_PROFILE_RESPONSE_FIELDS,
    VPN_QUOTE_RESPONSE_FIELDS,
    VPN_SESSION_RESPONSE_FIELDS,
    VPN_RECEIPT_RESPONSE_FIELDS,
    VPN_RECEIPT_LIST_RESPONSE_FIELDS,
    normalizeVpnTrustTuple,
    requireVpnRelayEndpoint,
    requireVpnEnum,
    requireVpnProfileExitClasses,
  };
}
