package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;

/** JSON persistence codec for structured Offline Note V2 wallet notes. */
public final class OfflineNoteV2WalletNoteJsonCodec {
  public static final long VERSION = 1L;

  private static final String ORIGIN_ISSUER_LOAD = "issuer_load";
  private static final String ORIGIN_P2P_OUTPUT = "p2p_output";

  private OfflineNoteV2WalletNoteJsonCodec() {}

  public static byte[] encode(final OfflineNoteV2WalletNote note) {
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
    payload.put("state", note.state().name());
    payload.put("created_at_ms", note.createdAtMs());
    payload.put("updated_at_ms", note.updatedAtMs());
    return JsonEncoder.encode(payload).getBytes(StandardCharsets.UTF_8);
  }

  public static OfflineNoteV2WalletNote decode(final byte[] payload) {
    final Map<String, Object> object = parseObject(payload);
    final long version = asLong(object.get("version"), "version");
    if (version != VERSION) {
      throw new IllegalArgumentException(
          "Offline Note V2 wallet note JSON version must be " + VERSION);
    }
    return new OfflineNoteV2WalletNote(
        asString(object.get("chain_id"), "chain_id"),
        asString(object.get("account_id"), "account_id"),
        asString(object.get("asset_id"), "asset_id"),
        asString(object.get("amount"), "amount"),
        OfflineNoteV2.decodeCertificate(
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
        OfflineNoteV2WalletNoteState.valueOf(asString(object.get("state"), "state")),
        asLong(object.get("created_at_ms"), "created_at_ms"),
        asLong(object.get("updated_at_ms"), "updated_at_ms"));
  }

  private static Map<String, Object> encodeOrigin(final OfflineNoteV2.CommitmentOriginV2 origin) {
    final Map<String, Object> payload = new LinkedHashMap<>();
    if (origin instanceof OfflineNoteV2.CommitmentOriginV2.IssuerLoad) {
      final OfflineNoteV2.CommitmentOriginV2.IssuerLoad issuerLoad =
          (OfflineNoteV2.CommitmentOriginV2.IssuerLoad) origin;
      payload.put("type", ORIGIN_ISSUER_LOAD);
      payload.put("operation_id", issuerLoad.operationId());
      payload.put("lineage_id", issuerLoad.lineageId());
      payload.put("local_revision", issuerLoad.localRevision());
      return payload;
    }
    if (origin instanceof OfflineNoteV2.CommitmentOriginV2.P2pOutput) {
      final OfflineNoteV2.CommitmentOriginV2.P2pOutput output =
          (OfflineNoteV2.CommitmentOriginV2.P2pOutput) origin;
      payload.put("type", ORIGIN_P2P_OUTPUT);
      payload.put("payment_request_id", output.paymentRequestId());
      payload.put("output_index", output.outputIndex());
      return payload;
    }
    throw new IllegalArgumentException("unsupported Offline Note V2 commitment origin");
  }

  private static OfflineNoteV2.CommitmentOriginV2 decodeOrigin(final Map<String, Object> payload) {
    final String type = asString(payload.get("type"), "origin.type");
    if (ORIGIN_ISSUER_LOAD.equals(type)) {
      return new OfflineNoteV2.CommitmentOriginV2.IssuerLoad(
          asString(payload.get("operation_id"), "origin.operation_id"),
          asString(payload.get("lineage_id"), "origin.lineage_id"),
          asLong(payload.get("local_revision"), "origin.local_revision"));
    }
    if (ORIGIN_P2P_OUTPUT.equals(type)) {
      return new OfflineNoteV2.CommitmentOriginV2.P2pOutput(
          asString(payload.get("payment_request_id"), "origin.payment_request_id"),
          Math.toIntExact(asLong(payload.get("output_index"), "origin.output_index")));
    }
    throw new IllegalArgumentException("unknown Offline Note V2 commitment origin type: " + type);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> parseObject(final byte[] payload) {
    final Object parsed =
        JsonParser.parse(
            new String(Objects.requireNonNull(payload, "payload"), StandardCharsets.UTF_8));
    if (!(parsed instanceof Map<?, ?>)) {
      throw new IllegalArgumentException("Offline Note V2 wallet note JSON root must be an object");
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
