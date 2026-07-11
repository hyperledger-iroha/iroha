package org.hyperledger.iroha.android.client;

import java.util.Arrays;
import java.util.Base64;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.sccp.SccpV1;
import org.hyperledger.iroha.norito.NoritoHeader;

/** Shared strict encoding checks for SCCP bridge submit DTOs. */
final class SccpSubmitEncoding {
  static final int MAX_DESTINATION_ARTIFACT_BYTES = 16 * 1024 * 1024 + 64 * 1024;
  static final int MAX_NATIVE_PROOF_BYTES = 16 * 1024 * 1024;
  static final int MAX_DETACHED_SIGNATURE_BYTES = 16 * 1024;
  static final int MAX_TRANSACTION_PAYLOAD_BYTES = 16 * 1024 * 1024;
  private static final NoritoJavaCodecAdapter TRANSACTION_CODEC = new NoritoJavaCodecAdapter();

  private SccpSubmitEncoding() {}

  static byte[] validateCanonicalNoritoBase64(
      final String value, final String field, final int maximum) {
    if (value == null || value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be canonical padded base64");
    }
    if (value.length() > maximumBase64Length(maximum)) {
      throw new IllegalArgumentException(field + " exceeds its canonical size bound");
    }
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(field + " must be valid base64", ex);
    }
    if (decoded.length == 0 || decoded.length > maximum) {
      throw new IllegalArgumentException(field + " exceeds its canonical size bound");
    }
    if (!Base64.getEncoder().encodeToString(decoded).equals(value)) {
      throw new IllegalArgumentException(field + " must be canonical padded base64");
    }
    final NoritoHeader.DecodeResult result;
    try {
      result = NoritoHeader.decode(decoded, null);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(field + " must contain a canonical Norito envelope", ex);
    }
    final NoritoHeader header = result.header();
    if (header.compression() != NoritoHeader.COMPRESSION_NONE) {
      throw new IllegalArgumentException(field + " must use uncompressed canonical Norito");
    }
    final int headerPadding =
        decoded.length - NoritoHeader.HEADER_LENGTH - header.payloadLength();
    if (headerPadding != 0 && headerPadding != 8) {
      throw new IllegalArgumentException(
          field + " must use canonical Norito header alignment padding");
    }
    if (allZero(header.schemaHash())) {
      throw new IllegalArgumentException(field + " must advertise a nonzero Norito schema");
    }
    if (!Arrays.equals(
        header.encode(), Arrays.copyOfRange(decoded, 0, NoritoHeader.HEADER_LENGTH))) {
      throw new IllegalArgumentException(field + " contains a non-canonical Norito header");
    }
    header.validateChecksum(result.payload());
    return decoded;
  }

  static String requireCanonicalAuthority(final String value, final String field) {
    final String canonical = AccountIdLiteral.requireCanonicalI105Address(value, field);
    final Integer discriminant = AccountAddress.detectI105Discriminant(canonical);
    if (discriminant == null
        || discriminant.intValue() != SccpV1.TAIRA_I105_DISCRIMINANT_V1) {
      throw new IllegalArgumentException(
          field + " must use the canonical public Taira I105 discriminant");
    }
    return canonical;
  }

  static Long normalizeOptionalCreationTimeMs(final Long value) {
    if (value != null && value <= 0) {
      throw new IllegalArgumentException("creationTimeMs must be positive");
    }
    return value;
  }

  static String normalizeOptionalSignature(final String value) {
    if (value == null) return null;
    final byte[] decoded =
        canonicalBase64(value, "signature_b64", MAX_DETACHED_SIGNATURE_BYTES);
    if (allZero(decoded)) {
      throw new IllegalArgumentException(
          "signature_b64 must contain one admitted nonzero signature payload");
    }
    return value;
  }

  static void validateDetachedSigningState(
      final String signatureB64,
      final String transactionPayloadB64,
      final Long creationTimeMs) {
    if (signatureB64 == null && transactionPayloadB64 == null) {
      return;
    }
    if (signatureB64 != null && transactionPayloadB64 != null) {
      if (creationTimeMs == null || creationTimeMs <= 0) {
        throw new IllegalArgumentException(
            "signed SCCP submission requires an explicit positive creation_time_ms");
      }
      return;
    }
    throw new IllegalArgumentException(
        "SCCP preparation requires neither signature_b64 nor transaction_payload_b64; signed submission requires both");
  }

  static String normalizeOptionalTransactionPayload(
      final String value, final Long creationTimeMs, final String expectedAuthority) {
    if (value == null) return null;
    final byte[] bytes = canonicalBase64(
        value, "transaction_payload_b64", MAX_TRANSACTION_PAYLOAD_BYTES);
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
    if (!sameCanonicalAccountId(payload.authority(), expectedAuthority)) {
      throw new IllegalArgumentException(
          "transaction payload authority does not match authority");
    }
    if (creationTimeMs != null && payload.creationTimeMs() != creationTimeMs) {
      throw new IllegalArgumentException(
          "transaction payload creation time does not match creation_time_ms");
    }
    return value;
  }

  private static boolean sameCanonicalAccountId(final String left, final String right) {
    try {
      // AccountId wire identity is domainless and excludes its I105 display discriminant.
      final byte[] leftBytes =
          AccountAddress.parseEncodedIgnoringCurveSupport(left, null).address.canonicalBytes();
      final byte[] rightBytes =
          AccountAddress.parseEncodedIgnoringCurveSupport(right, null).address.canonicalBytes();
      return Arrays.equals(leftBytes, rightBytes);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalArgumentException(
          "transaction payload authority must be canonical I105", ex);
    }
  }

  static byte[] canonicalBase64(
      final String value, final String field, final int maximum) {
    if (value == null || value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be canonical padded base64");
    }
    if (value.length() > maximumBase64Length(maximum)) {
      throw new IllegalArgumentException(field + " exceeds its canonical size bound");
    }
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(field + " must be valid base64", ex);
    }
    if (decoded.length == 0 || decoded.length > maximum) {
      throw new IllegalArgumentException(field + " exceeds its canonical size bound");
    }
    if (!Base64.getEncoder().encodeToString(decoded).equals(value)) {
      throw new IllegalArgumentException(field + " must be canonical padded base64");
    }
    return decoded;
  }

  private static int maximumBase64Length(final int maximumBytes) {
    return 4 * ((maximumBytes + 2) / 3);
  }

  private static boolean allZero(final byte[] value) {
    for (final byte item : value) {
      if (item != 0) return false;
    }
    return true;
  }
}
