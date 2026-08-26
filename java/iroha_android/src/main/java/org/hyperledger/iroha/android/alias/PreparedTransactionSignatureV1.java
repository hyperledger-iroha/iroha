package org.hyperledger.iroha.android.alias;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import org.hyperledger.iroha.android.crypto.IrohaHash;

/** Stable cross-SDK signature transcript for Taira prepared transactions. */
public final class PreparedTransactionSignatureV1 {
  public static final String TRANSCRIPT_SCHEMA =
      "iroha.taira.prepared-signature-transcript.v1";
  private static final byte[] DOMAIN =
      "iroha:taira:prepared-transaction:v1\0".getBytes(StandardCharsets.UTF_8);

  private PreparedTransactionSignatureV1() {}

  /** Returns the exact transcript authenticated by an onboarding prepared envelope. */
  public static byte[] onboardingPrepared(
      final AccountOnboardingPreparedTransactionV1 envelope) {
    final ByteArrayOutputStream transcript =
        base(envelope.schema(), envelope.operation(), envelope.binding());
    field(transcript, "semantic_hash_hex", envelope.semanticHashHex());
    field(transcript, "account_id", envelope.accountId());
    field(transcript, "alias", envelope.alias());
    field(transcript, "disposition", envelope.disposition().wireValue());
    field(transcript, "transaction_hash_hex", envelope.transactionHashHex());
    field(
        transcript,
        "signed_transaction_wire_sha256",
        envelope.signedTransactionWireSha256());
    field(
        transcript,
        "signed_transaction_wire",
        decodeLowerHex(envelope.signedTransactionWireHex()));
    return transcript.toByteArray();
  }

  /** Returns the exact transcript authenticated by a nonterminal onboarding proof requirement. */
  public static byte[] onboardingProofRequired(
      final AccountOnboardingProofRequiredPrepareResponseV1 result) {
    final ByteArrayOutputStream transcript =
        base(result.schema(), result.operation(), result.binding());
    field(transcript, "outcome", result.outcome());
    field(transcript, "proof_kind", result.proofKind());
    field(transcript, "semantic_hash_hex", result.semanticHashHex());
    field(transcript, "account_id", result.accountId());
    field(transcript, "alias", result.alias());
    field(transcript, "disposition", result.disposition().wireValue());
    return transcript.toByteArray();
  }

  /** Returns the Iroha BLAKE2b-256 digest signed by the prepared-result authority. */
  public static byte[] digest(final byte[] transcript) {
    return IrohaHash.prehash(transcript);
  }

  private static ByteArrayOutputStream base(
      final String envelopeSchema,
      final String operation,
      final TairaPublicResetMutationBindingV1 binding) {
    final ByteArrayOutputStream transcript = new ByteArrayOutputStream();
    frame(transcript, DOMAIN);
    field(transcript, "transcript_schema", TRANSCRIPT_SCHEMA);
    field(transcript, "envelope_schema", envelopeSchema);
    field(transcript, "operation", operation);
    field(transcript, "binding.schema", binding.schema());
    field(transcript, "binding.authorization_sha256", binding.authorizationSha256());
    field(transcript, "binding.authorization_nonce", binding.authorizationNonce());
    field(transcript, "binding.kind", binding.kind());
    field(transcript, "binding.phase", binding.phase());
    field(transcript, "binding.idempotency_key", binding.idempotencyKey());
    field(
        transcript,
        "binding.execution_expires_at_unix_ms",
        Long.toString(binding.executionExpiresAtUnixMs()));
    return transcript;
  }

  private static void field(
      final ByteArrayOutputStream output, final String label, final String value) {
    field(output, label, value.getBytes(StandardCharsets.UTF_8));
  }

  private static void field(
      final ByteArrayOutputStream output, final String label, final byte[] value) {
    frame(output, label.getBytes(StandardCharsets.UTF_8));
    frame(output, value);
  }

  private static void frame(final ByteArrayOutputStream output, final byte[] value) {
    final long length = value.length;
    for (int shift = 56; shift >= 0; shift -= 8) {
      output.write((int) ((length >>> shift) & 0xffL));
    }
    output.write(value, 0, value.length);
  }

  static byte[] decodeLowerHex(final String value) {
    TairaPublicResetMutationBindingV1.requireLowerHex(value, "value");
    final byte[] result = new byte[value.length() / 2];
    for (int index = 0; index < result.length; index++) {
      result[index] =
          (byte)
              ((Character.digit(value.charAt(index * 2), 16) << 4)
                  | Character.digit(value.charAt(index * 2 + 1), 16));
    }
    return result;
  }

  static String hexLower(final byte[] bytes) {
    final char[] digits = "0123456789abcdef".toCharArray();
    final char[] output = new char[bytes.length * 2];
    for (int index = 0; index < bytes.length; index++) {
      final int value = bytes[index] & 0xff;
      output[index * 2] = digits[value >>> 4];
      output[index * 2 + 1] = digits[value & 0xf];
    }
    return new String(output);
  }
}
