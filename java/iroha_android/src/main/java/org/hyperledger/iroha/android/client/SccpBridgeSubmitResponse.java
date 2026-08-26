package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
import java.util.Map;
import java.util.Set;
import java.util.regex.Pattern;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.sccp.SccpNetworkV1;
import org.hyperledger.iroha.android.sccp.SccpV1;

/** Unified strict detached-signing response returned by both SCCP submit endpoints. */
public final class SccpBridgeSubmitResponse {
  private static final Pattern HASH = Pattern.compile("[0-9a-f]{64}");
  private static final Set<String> CLOSED_BACKENDS =
      Set.of(
          "evm-groth16-bn254-v1",
          "tron-groth16-bn254-v1",
          "bridge/sccp/native/ethereum-beacon-v1",
          "bridge/sccp/native/bsc-parlia-v1",
          "bridge/sccp/native/tron-dpos-v1");
  private static final NoritoJavaCodecAdapter TRANSACTION_CODEC =
      new NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
  private static final Set<String> FIELDS =
      Set.of("submitted", "payload_kind", "message_id_hex", "backend", "counterparty_domain", "counterparty_chain", "route_configuration_hash_hex", "range_start_height", "range_end_height", "creation_time_ms", "tx_hash_hex", "transaction_payload_b64", "signing_message_b64");

  public final boolean submitted;
  public final SccpModels.PayloadKindV1 payloadKind;
  public final String messageIdHex;
  public final String backend;
  public final int counterpartyDomain;
  public final String counterpartyChain;
  public final String routeConfigurationHashHex;
  public final long rangeStartHeight;
  public final long rangeEndHeight;
  public final long creationTimeMs;
  public final String txHashHex;
  public final String transactionPayloadB64;
  public final String signingMessageB64;

  private SccpBridgeSubmitResponse(
      final boolean submitted, final SccpModels.PayloadKindV1 payloadKind, final String messageIdHex,
      final String backend, final int counterpartyDomain, final String counterpartyChain,
      final String routeConfigurationHashHex, final long rangeStartHeight, final long rangeEndHeight,
      final long creationTimeMs, final String txHashHex, final String transactionPayloadB64,
      final String signingMessageB64) {
    this.submitted = submitted; this.payloadKind = payloadKind; this.messageIdHex = messageIdHex;
    this.backend = backend; this.counterpartyDomain = counterpartyDomain;
    this.counterpartyChain = counterpartyChain;
    this.routeConfigurationHashHex = routeConfigurationHashHex;
    this.rangeStartHeight = rangeStartHeight; this.rangeEndHeight = rangeEndHeight;
    this.creationTimeMs = creationTimeMs; this.txHashHex = txHashHex;
    this.transactionPayloadB64 = transactionPayloadB64; this.signingMessageB64 = signingMessageB64;
  }

  /** Strictly decode the unified first-release response envelope. */
  public static SccpBridgeSubmitResponse parse(final byte[] bytes) {
    final Map<String, Object> value = root(bytes);
    for (final String field : value.keySet()) {
      if (!FIELDS.contains(field)) throw new IllegalArgumentException("bridge response contains unknown or retired field `" + field + "`");
    }
    for (final String field : FIELDS) {
      if (!value.containsKey(field)) {
        throw new IllegalArgumentException("bridge response is missing required field `" + field + "`");
      }
    }
    final boolean submitted = bool(value, "submitted");
    final long start = longValue(value, "range_start_height", 1);
    final long end = longValue(value, "range_end_height", start);
    final long creationTime = longValue(value, "creation_time_ms", 1);
    final String txHash = optionalTransactionHash(value, "tx_hash_hex");
    final String transactionPayload = optionalText(value, "transaction_payload_b64");
    final String signingMessage = optionalText(value, "signing_message_b64");
    if (submitted) {
      if (txHash == null || transactionPayload != null || signingMessage != null) {
        throw new IllegalArgumentException("submitted SCCP response must contain tx_hash_hex and no signing scaffold");
      }
    } else {
      if (txHash != null || transactionPayload == null || signingMessage == null) {
        throw new IllegalArgumentException("unsigned SCCP response requires transaction_payload_b64 and signing_message_b64");
      }
      final byte[] transactionBytes =
          validateCanonicalTransactionPayload(transactionPayload, creationTime);
      final byte[] signingBytes = canonicalBase64(signingMessage, "signing_message_b64", 32);
      if (!Arrays.equals(signingBytes, IrohaHash.prehash(transactionBytes))) {
        throw new IllegalArgumentException(
            "signing_message_b64 must be the exact transaction-payload prehash");
      }
    }
    final SccpModels.PayloadKindV1 kind =
        SccpModels.PayloadKindV1.fromWireKey(text(value, "payload_kind"));
    if (kind == null) throw new IllegalArgumentException("payload_kind is unknown or retired");
    final String backend = text(value, "backend");
    if (!CLOSED_BACKENDS.contains(backend)) {
      throw new IllegalArgumentException("backend must be one closed SCCP verifier label");
    }
    final int counterpartyDomain = intValue(value, "counterparty_domain", 1, 5);
    final String counterpartyChain = text(value, "counterparty_chain");
    final SccpNetworkV1 counterparty = SccpNetworkV1.fromProfileKey(counterpartyChain);
    if (counterparty == null
        || !counterparty.isExternal()
        || counterparty.domainId() != counterpartyDomain) {
      throw new IllegalArgumentException(
          "counterparty_chain and counterparty_domain must identify one exact external network");
    }
    if (!backendsForDomain(counterpartyDomain).contains(backend)) {
      throw new IllegalArgumentException(
          "backend does not match the exact counterparty family");
    }
    return new SccpBridgeSubmitResponse(
        submitted, kind, hash(value, "message_id_hex"), backend,
        counterpartyDomain, counterpartyChain,
        hash(value, "route_configuration_hash_hex"), start, end, creationTime,
        txHash, transactionPayload, signingMessage);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> root(final byte[] bytes) {
    final String text = new String(bytes, StandardCharsets.UTF_8);
    if (!Arrays.equals(bytes, text.getBytes(StandardCharsets.UTF_8))) throw new IllegalArgumentException("bridge response must be UTF-8 JSON");
    final Object parsed = JsonParser.parse(text);
    if (!(parsed instanceof Map<?, ?>)
        || ((Map<?, ?>) parsed).keySet().stream().anyMatch(key -> !(key instanceof String))) {
      throw new IllegalArgumentException("bridge response must be an object");
    }
    return (Map<String, Object>) parsed;
  }
  private static String text(final Map<String, Object> value, final String field) {
    if (!(value.get(field) instanceof String)) throw new IllegalArgumentException(field + " must be string");
    final String result = (String) value.get(field);
    if (result.isEmpty() || !result.equals(result.trim())) throw new IllegalArgumentException(field + " must be canonical text");
    return result;
  }
  private static String optionalText(final Map<String, Object> value, final String field) { return value.get(field) == null ? null : text(value, field); }
  private static boolean bool(final Map<String, Object> value, final String field) {
    if (!(value.get(field) instanceof Boolean)) throw new IllegalArgumentException(field + " must be boolean");
    return (Boolean) value.get(field);
  }
  private static long longValue(final Map<String, Object> value, final String field, final long minimum) {
    if (!(value.get(field) instanceof Number)) throw new IllegalArgumentException(field + " must be integer");
    final Number number = (Number) value.get(field); final long result = number.longValue();
    if (!number.toString().equals(Long.toString(result)) || result < minimum) throw new IllegalArgumentException(field + " is out of range");
    return result;
  }
  private static int intValue(final Map<String, Object> value, final String field, final int minimum, final int maximum) {
    final long result = longValue(value, field, minimum);
    if (result > maximum) throw new IllegalArgumentException(field + " is out of range");
    return (int) result;
  }
  private static String hash(final Map<String, Object> value, final String field) {
    final String result = text(value, field);
    if (!HASH.matcher(result).matches() || result.chars().allMatch(item -> item == '0')) throw new IllegalArgumentException(field + " must be canonical lowercase nonzero hash");
    return result;
  }
  private static String optionalHash(final Map<String, Object> value, final String field) { return value.get(field) == null ? null : hash(value, field); }
  private static String optionalTransactionHash(
      final Map<String, Object> value, final String field) {
    if (value.get(field) == null) return null;
    final String literal = text(value, field);
    if (!literal.matches("[0-9a-f]{63}[13579bdf]")) {
      throw new IllegalArgumentException(
          field + " must match [0-9a-f]{63}[13579bdf] with the Iroha HashOf marker");
    }
    return literal;
  }
  private static byte[] canonicalBase64(
      final String value, final String field, final Integer exactBytes) {
    final int maximumBytes =
        exactBytes == null ? SccpSubmitEncoding.MAX_TRANSACTION_PAYLOAD_BYTES : exactBytes;
    final int maximumLength = 4 * ((maximumBytes + 2) / 3);
    if (value.length() > maximumLength) {
      throw new IllegalArgumentException(field + " exceeds its size bound");
    }
    final byte[] decoded;
    try { decoded = Base64.getDecoder().decode(value); }
    catch (final IllegalArgumentException ex) { throw new IllegalArgumentException(field + " must be canonical base64", ex); }
    if (decoded.length == 0 || !Base64.getEncoder().encodeToString(decoded).equals(value)) throw new IllegalArgumentException(field + " must be canonical nonempty padded base64");
    if (exactBytes != null && decoded.length != exactBytes) {
      throw new IllegalArgumentException(field + " must contain exactly " + exactBytes + " bytes");
    }
    return decoded;
  }

  private static byte[] validateCanonicalTransactionPayload(
      final String value, final long creationTimeMs) {
    final byte[] bytes = canonicalBase64(value, "transaction_payload_b64", null);
    final TransactionPayload payload;
    final byte[] canonical;
    try {
      payload = TRANSACTION_CODEC.decodeTransaction(bytes);
      canonical = TRANSACTION_CODEC.encodeTransaction(payload);
    } catch (final Exception ex) {
      throw new IllegalArgumentException(
          "transaction_payload_b64 must contain one canonical transaction payload", ex);
    }
    if (!Arrays.equals(bytes, canonical)) {
      throw new IllegalArgumentException("transaction_payload_b64 is not canonical");
    }
    if (payload.creationTimeMs() != creationTimeMs) {
      throw new IllegalArgumentException(
          "transaction payload creation time does not match creation_time_ms");
    }
    if (payload.admissionIntent() != TransactionAdmissionIntent.QUEUE_PLAN_SYNCED) {
      throw new IllegalArgumentException(
          "transaction payload admission intent must be QueuePlanSynced");
    }
    return bytes;
  }

  private static Set<String> backendsForDomain(final int domain) {
    return switch (domain) {
      case 1 ->
          Set.of(
              "evm-groth16-bn254-v1", "bridge/sccp/native/ethereum-beacon-v1");
      case 2 -> Set.of("evm-groth16-bn254-v1", "bridge/sccp/native/bsc-parlia-v1");
      case 5 -> Set.of("tron-groth16-bn254-v1", "bridge/sccp/native/tron-dpos-v1");
      default -> Set.of();
    };
  }
}
