package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;

/** JSON persistence codec for structured Offline Note wallet notes. */
public final class OfflineNoteWalletNoteJsonCodec {
  public static final long VERSION = 1L;

  private static final String ORIGIN_ISSUER_LOAD = "issuer_load";
  private static final String ORIGIN_P2P_OUTPUT = "p2p_output";

  private OfflineNoteWalletNoteJsonCodec() {}

  public static byte[] encode(final OfflineNoteWalletNote note) {
    Objects.requireNonNull(note, "note");
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("version", VERSION);
    payload.put("chain_id", note.chainId());
    payload.put("account_id", note.accountId());
    payload.put("asset_id", note.assetId());
    payload.put("amount", note.canonicalAmount());
    payload.put(
        "key_certificate_norito_base64",
        Base64.getEncoder().encodeToString(note.keyCertificate().noritoEncoded()));
    payload.put("note_commitment_hex", note.noteCommitmentHex());
    payload.put("note_secret_base64", Base64.getEncoder().encodeToString(note.noteSecret()));
    payload.put("origin", encodeOrigin(note.origin()));
    payload.put("bearer_audit_trail_norito_base64", encodeAuditTrail(note.bearerAuditTrail()));
    payload.put("state", note.state().name());
    payload.put("created_at_ms", note.createdAtMs());
    payload.put("updated_at_ms", note.updatedAtMs());
    if (note.spentPaymentRequestId() != null) {
      payload.put("spent_payment_request_id", note.spentPaymentRequestId());
    }
    return JsonEncoder.encode(payload).getBytes(StandardCharsets.UTF_8);
  }

  public static OfflineNoteWalletNote decode(final byte[] payload) {
    final Map<String, Object> object = parseObject(payload);
    final long version = asLong(object.get("version"), "version");
    if (version != VERSION) {
      throw new IllegalArgumentException(
          "Offline Note wallet note JSON version must be " + VERSION);
    }
    return new OfflineNoteWalletNote(
        asString(object.get("chain_id"), "chain_id"),
        asString(object.get("account_id"), "account_id"),
        asString(object.get("asset_id"), "asset_id"),
        asString(object.get("amount"), "amount"),
        OfflineNote.decodeCertificate(
            Base64.getDecoder()
                .decode(
                    asString(
                        object.get("key_certificate_norito_base64"),
                        "key_certificate_norito_base64"))),
        hexBytes(
            asString(object.get("note_commitment_hex"), "note_commitment_hex"),
            "note_commitment_hex"),
        Base64.getDecoder()
            .decode(asString(object.get("note_secret_base64"), "note_secret_base64")),
        decodeOrigin(asObject(object.get("origin"), "origin")),
        decodeAuditTrail(object.get("bearer_audit_trail_norito_base64")),
        decodeState(asString(object.get("state"), "state")),
        asLong(object.get("created_at_ms"), "created_at_ms"),
        asLong(object.get("updated_at_ms"), "updated_at_ms"),
        optionalString(object.get("spent_payment_request_id"), "spent_payment_request_id"));
  }

  private static List<String> encodeAuditTrail(final List<OfflineNote.AuditBundle> audits) {
    final List<String> encoded = new ArrayList<>(audits.size());
    for (final OfflineNote.AuditBundle audit : audits) {
      encoded.add(Base64.getEncoder().encodeToString(audit.noritoEncoded()));
    }
    return encoded;
  }

  private static List<OfflineNote.AuditBundle> decodeAuditTrail(final Object value) {
    if (value == null) {
      return new ArrayList<>();
    }
    final List<Object> raw = asList(value, "bearer_audit_trail_norito_base64");
    final List<OfflineNote.AuditBundle> audits = new ArrayList<>(raw.size());
    for (int index = 0; index < raw.size(); index++) {
      audits.add(
          OfflineNote.decodeAudit(
              Base64.getDecoder()
                  .decode(
                      asString(
                          raw.get(index),
                          "bearer_audit_trail_norito_base64[" + index + "]"))));
    }
    return audits;
  }

  private static Map<String, Object> encodeOrigin(final OfflineNote.CommitmentOrigin origin) {
    final Map<String, Object> payload = new LinkedHashMap<>();
    if (origin instanceof OfflineNote.CommitmentOrigin.IssuerLoad) {
      final OfflineNote.CommitmentOrigin.IssuerLoad issuerLoad =
          (OfflineNote.CommitmentOrigin.IssuerLoad) origin;
      payload.put("type", ORIGIN_ISSUER_LOAD);
      payload.put("operation_id", issuerLoad.operationId());
      payload.put("lineage_id", issuerLoad.lineageId());
      payload.put("local_revision", issuerLoad.localRevision());
      return payload;
    }
    if (origin instanceof OfflineNote.CommitmentOrigin.P2pOutput) {
      final OfflineNote.CommitmentOrigin.P2pOutput output =
          (OfflineNote.CommitmentOrigin.P2pOutput) origin;
      payload.put("type", ORIGIN_P2P_OUTPUT);
      payload.put("payment_request_id", output.paymentRequestId());
      payload.put("output_index", output.outputIndex());
      return payload;
    }
    throw new IllegalArgumentException("unsupported Offline Note commitment origin");
  }

  private static OfflineNote.CommitmentOrigin decodeOrigin(final Map<String, Object> payload) {
    final String type = asString(payload.get("type"), "origin.type");
    if (ORIGIN_ISSUER_LOAD.equals(type)) {
      return new OfflineNote.CommitmentOrigin.IssuerLoad(
          asString(payload.get("operation_id"), "origin.operation_id"),
          asString(payload.get("lineage_id"), "origin.lineage_id"),
          asLong(payload.get("local_revision"), "origin.local_revision"));
    }
    if (ORIGIN_P2P_OUTPUT.equals(type)) {
      return new OfflineNote.CommitmentOrigin.P2pOutput(
          asString(payload.get("payment_request_id"), "origin.payment_request_id"),
          Math.toIntExact(asLong(payload.get("output_index"), "origin.output_index")));
    }
    throw new IllegalArgumentException("unknown Offline Note commitment origin type: " + type);
  }

  private static OfflineNoteWalletNoteState decodeState(final String state) {
    if ("SPEND_PENDING".equals(state) || "spendPending".equals(state)) {
      return OfflineNoteWalletNoteState.SPENT;
    }
    if ("CHANGE_PENDING".equals(state) || "changePending".equals(state)) {
      return OfflineNoteWalletNoteState.SPENDABLE;
    }
    return OfflineNoteWalletNoteState.valueOf(state);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> parseObject(final byte[] payload) {
    final Object parsed =
        JsonParser.parse(
            new String(Objects.requireNonNull(payload, "payload"), StandardCharsets.UTF_8));
    if (!(parsed instanceof Map<?, ?>)) {
      throw new IllegalArgumentException("Offline Note wallet note JSON root must be an object");
    }
    return (Map<String, Object>) parsed;
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> asObject(final Object value, final String field) {
    if (!(value instanceof Map<?, ?>)) {
      throw new IllegalArgumentException(field + " must be an object");
    }
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> asList(final Object value, final String field) {
    if (!(value instanceof List<?>)) {
      throw new IllegalArgumentException(field + " must be an array");
    }
    return (List<Object>) value;
  }

  private static String asString(final Object value, final String field) {
    if (!(value instanceof String)) {
      throw new IllegalArgumentException(field + " must be a non-empty string");
    }
    final String string = (String) value;
    if (string.trim().isEmpty()) {
      throw new IllegalArgumentException(field + " must be a non-empty string");
    }
    return string;
  }

  private static String optionalString(final Object value, final String field) {
    if (value == null) {
      return null;
    }
    return asString(value, field);
  }

  private static long asLong(final Object value, final String field) {
    if (value instanceof Number) {
      return ((Number) value).longValue();
    }
    if (value instanceof String) {
      return Long.parseLong((String) value);
    }
    throw new IllegalArgumentException(field + " must be an integer");
  }

  private static byte[] hexBytes(final String value, final String field) {
    final String normalized = value.toLowerCase(Locale.ROOT);
    if ((normalized.length() & 1) != 0) {
      throw new IllegalArgumentException(field + " must have an even hex length");
    }
    final byte[] out = new byte[normalized.length() / 2];
    for (int i = 0; i < out.length; i++) {
      final int hi = Character.digit(normalized.charAt(i * 2), 16);
      final int lo = Character.digit(normalized.charAt(i * 2 + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalArgumentException(field + " must be hex");
      }
      out[i] = (byte) ((hi << 4) | lo);
    }
    return out;
  }
}
