package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;

/**
 * Unsigned verifying-key registry transaction prepared by Torii.
 *
 * <p>SDK {@link org.hyperledger.iroha.android.crypto.Signer} implementations already apply Iroha's
 * prehash, so pass {@link #transactionPayloadBytes()} to {@code Signer.sign}. Use {@link
 * #signingMessageBytes()} only with a raw signing primitive that signs an already-prehashed
 * message. Attach the signature to the decoded payload and submit the resulting signed transaction
 * through the standard ingress.
 */
public final class VerifyingKeyTransactionDraft {
  private static final int MAX_TRANSACTION_PAYLOAD_BYTES = 16 * 1024 * 1024;
  private static final int SIGNING_MESSAGE_BYTES = 32;
  private static final Set<String> FIELDS =
      Set.of("submitted", "transaction_payload_b64", "signing_message_b64");

  private final boolean submitted;
  private final String transactionPayloadB64;
  private final String signingMessageB64;

  private VerifyingKeyTransactionDraft(
      final boolean submitted,
      final String transactionPayloadB64,
      final String signingMessageB64) {
    this.submitted = submitted;
    this.transactionPayloadB64 = transactionPayloadB64;
    this.signingMessageB64 = signingMessageB64;
  }

  public boolean submitted() {
    return submitted;
  }

  public String transactionPayloadB64() {
    return transactionPayloadB64;
  }

  public String signingMessageB64() {
    return signingMessageB64;
  }

  /** Decodes the canonical transaction payload returned by Torii. */
  public byte[] transactionPayloadBytes() {
    return Base64.getDecoder().decode(transactionPayloadB64);
  }

  /** Decodes the exact 32-byte message for a raw signer that does not apply Iroha's prehash. */
  public byte[] signingMessageBytes() {
    return Base64.getDecoder().decode(signingMessageB64);
  }

  static VerifyingKeyTransactionDraft parseRegister(
      final byte[] bytes,
      final String expectedChainId,
      final Map<String, Object> request) {
    return parse(
        bytes, expectedChainId, request, VerifyingKeyDraftBinding.Operation.REGISTER);
  }

  static VerifyingKeyTransactionDraft parseUpdate(
      final byte[] bytes,
      final String expectedChainId,
      final Map<String, Object> request) {
    return parse(
        bytes, expectedChainId, request, VerifyingKeyDraftBinding.Operation.UPDATE);
  }

  private static VerifyingKeyTransactionDraft parse(
      final byte[] bytes,
      final String expectedChainId,
      final Map<String, Object> request,
      final VerifyingKeyDraftBinding.Operation operation) {
    final Map<String, Object> value = parseObject(bytes);
    for (final String field : value.keySet()) {
      if (!FIELDS.contains(field)) {
        throw new IllegalArgumentException(
            "verifying-key draft contains unknown or retired field `" + field + "`");
      }
    }
    for (final String field : FIELDS) {
      if (!value.containsKey(field)) {
        throw new IllegalArgumentException(
            "verifying-key draft is missing required field `" + field + "`");
      }
    }
    if (!(value.get("submitted") instanceof Boolean)) {
      throw new IllegalArgumentException("submitted must be a boolean");
    }
    final boolean submitted = (Boolean) value.get("submitted");
    if (submitted) {
      throw new IllegalArgumentException(
          "verifying-key draft must be unsigned and unsubmitted");
    }
    final String transactionPayloadB64 = exactString(value, "transaction_payload_b64");
    final String signingMessageB64 = exactString(value, "signing_message_b64");
    final byte[] transactionPayload =
        decodeCanonicalBase64(
            transactionPayloadB64,
            "transaction_payload_b64",
            MAX_TRANSACTION_PAYLOAD_BYTES,
            null);
    final byte[] signingMessage =
        decodeCanonicalBase64(
            signingMessageB64,
            "signing_message_b64",
            SIGNING_MESSAGE_BYTES,
            SIGNING_MESSAGE_BYTES);
    try {
      NoritoJavaCodecAdapter.validateCanonicalTransactionPayload(transactionPayload);
    } catch (final Exception ex) {
      throw new IllegalArgumentException(
          "transaction_payload_b64 must contain one canonical transaction payload", ex);
    }
    VerifyingKeyDraftBinding.validate(
        transactionPayload, expectedChainId, request, operation);
    if (!Arrays.equals(signingMessage, IrohaHash.prehash(transactionPayload))) {
      throw new IllegalArgumentException(
          "signing_message_b64 must be the exact transaction-payload prehash");
    }
    return new VerifyingKeyTransactionDraft(
        false, transactionPayloadB64, signingMessageB64);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> parseObject(final byte[] bytes) {
    if (bytes == null || bytes.length == 0) {
      throw new IllegalArgumentException("verifying-key draft returned an empty payload");
    }
    final String text = new String(bytes, StandardCharsets.UTF_8);
    if (!Arrays.equals(bytes, text.getBytes(StandardCharsets.UTF_8))) {
      throw new IllegalArgumentException("verifying-key draft must be UTF-8 JSON");
    }
    final Object value = JsonParser.parse(text);
    if (!(value instanceof Map<?, ?>)) {
      throw new IllegalArgumentException("verifying-key draft must be a JSON object");
    }
    for (final Object key : ((Map<?, ?>) value).keySet()) {
      if (!(key instanceof String)) {
        throw new IllegalArgumentException("verifying-key draft field names must be strings");
      }
    }
    return (Map<String, Object>) value;
  }

  private static String exactString(final Map<String, Object> value, final String field) {
    if (!(value.get(field) instanceof String)) {
      throw new IllegalArgumentException(field + " must be a string");
    }
    final String text = (String) value.get(field);
    if (text.isEmpty() || !text.equals(text.trim())) {
      throw new IllegalArgumentException(field + " must be canonical non-empty text");
    }
    return text;
  }

  private static byte[] decodeCanonicalBase64(
      final String value,
      final String field,
      final int maximumBytes,
      final Integer exactBytes) {
    if (value.length() > 4 * ((maximumBytes + 2) / 3)) {
      throw new IllegalArgumentException(field + " exceeds its size bound");
    }
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(field + " must be canonical padded base64", ex);
    }
    if (decoded.length == 0 || !Base64.getEncoder().encodeToString(decoded).equals(value)) {
      throw new IllegalArgumentException(
          field + " must be canonical non-empty padded base64");
    }
    if (exactBytes != null && decoded.length != exactBytes.intValue()) {
      throw new IllegalArgumentException(
          field + " must contain exactly " + exactBytes + " bytes");
    }
    return decoded;
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) {
      return true;
    }
    if (!(other instanceof VerifyingKeyTransactionDraft)) {
      return false;
    }
    final VerifyingKeyTransactionDraft that = (VerifyingKeyTransactionDraft) other;
    return submitted == that.submitted
        && transactionPayloadB64.equals(that.transactionPayloadB64)
        && signingMessageB64.equals(that.signingMessageB64);
  }

  @Override
  public int hashCode() {
    return Objects.hash(submitted, transactionPayloadB64, signingMessageB64);
  }
}
