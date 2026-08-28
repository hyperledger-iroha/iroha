package org.hyperledger.iroha.android.client;

import java.util.Arrays;
import java.util.Base64;
import java.util.Objects;
import java.util.Set;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.sccp.SccpV1;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;

/** Shared strict encoding checks for SCCP bridge submit DTOs. */
final class SccpSubmitEncoding {
  static final int MAX_GROTH16_ARTIFACT_BYTES = 16 * 1024 * 1024 + 64 * 1024;
  static final int MAX_DESTINATION_ARTIFACT_BYTES = MAX_GROTH16_ARTIFACT_BYTES + 64 * 1024;
  static final int MAX_DESTINATION_ARTIFACT_BASE64_BYTES = 22_544_384;
  static final int MAX_NATIVE_PROOF_BYTES = 16 * 1024 * 1024;
  static final int MAX_DETACHED_SIGNATURE_BYTES = 16 * 1024;
  static final int MAX_TRANSACTION_PAYLOAD_BYTES = 16 * 1024 * 1024;
  static final String DESTINATION_ARTIFACT_SCHEMA_NAME =
      "iroha_data_model::bridge::BridgeSccpDestinationProofV1";
  static final String NATIVE_INBOUND_PROOF_SCHEMA_NAME =
      "iroha_sccp::native_admission::SccpNativeInboundMessageProofV1";
  static final Set<String> PROOF_REQUEST_SCHEMA_NAMES =
      Set.of(
          "iroha_sccp::SccpGroth16Bn254ProofRequestV1",
          "iroha_sccp::SccpTonGroth16Bls12381ProofRequestV1");
  private static final NoritoJavaCodecAdapter TRANSACTION_CODEC =
      new NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1);

  private SccpSubmitEncoding() {}

  static byte[] validateCanonicalNoritoBase64(
      final String value,
      final String field,
      final int maximum,
      final String expectedSchemaName) {
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
    return validateCanonicalNoritoBytes(
        decoded, field, maximum, Set.of(expectedSchemaName));
  }

  static byte[] validateCanonicalProofRequestNorito(
      final byte[] value, final String field) {
    return validateCanonicalNoritoBytes(
        value, field, MAX_GROTH16_ARTIFACT_BYTES, PROOF_REQUEST_SCHEMA_NAMES);
  }

  private static byte[] validateCanonicalNoritoBytes(
      final byte[] decoded,
      final String field,
      final int maximum,
      final Set<String> expectedSchemaNames) {
    if (decoded == null || decoded.length == 0 || decoded.length > maximum) {
      throw new IllegalArgumentException(field + " exceeds its canonical size bound");
    }
    final NoritoHeader.DecodeResult result;
    try {
      result = NoritoHeader.decode(decoded, null);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(field + " must contain a canonical Norito envelope", ex);
    }
    final NoritoHeader header = result.header();
    if (expectedSchemaNames.stream()
        .map(SchemaHash::hash16)
        .noneMatch(hash -> Arrays.equals(hash, header.schemaHash()))) {
      throw new IllegalArgumentException(
          field + " schema hash does not match the closed SCCP type set");
    }
    if (header.compression() != NoritoHeader.COMPRESSION_NONE) {
      throw new IllegalArgumentException(field + " must use uncompressed canonical Norito");
    }
    final int headerPadding =
        decoded.length - NoritoHeader.HEADER_LENGTH - header.payloadLength();
    if (headerPadding != 0) {
      throw new IllegalArgumentException(
          field + " must use the exact zero-padded SCCP Norito alignment");
    }
    if (!Arrays.equals(
        header.encode(), Arrays.copyOfRange(decoded, 0, NoritoHeader.HEADER_LENGTH))) {
      throw new IllegalArgumentException(field + " contains a non-canonical Norito header");
    }
    header.validateChecksum(result.payload());
    return decoded.clone();
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
      final String value,
      final Long creationTimeMs,
      final String expectedAuthority,
      final FeePaymentIntent expectedFeePayment) {
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
    if (!sameSccpFeePayerAndGasBound(expectedFeePayment, payload.feePayment())) {
      throw new IllegalArgumentException(
          "transaction payload changed the requested payer, sponsor revision, or gas bound");
    }
    if (payload.admissionIntent() != TransactionAdmissionIntent.QUEUE_PLAN_SYNCED) {
      throw new IllegalArgumentException(
          "transaction payload admission intent must be QueuePlanSynced");
    }
    if (creationTimeMs != null && payload.creationTimeMs() != creationTimeMs) {
      throw new IllegalArgumentException(
          "transaction payload creation time does not match creation_time_ms");
    }
    return value;
  }

  private static boolean sameSccpFeePayerAndGasBound(
      final FeePaymentIntent expected, final FeePaymentIntent actual) {
    if (!Objects.equals(expected.gasLimit(), actual.gasLimit())) return false;
    if (expected instanceof FeePaymentIntent.Authority
        && actual instanceof FeePaymentIntent.Authority) {
      return true;
    }
    if (expected instanceof FeePaymentIntent.Sponsor
        && actual instanceof FeePaymentIntent.Sponsor) {
      final FeePaymentIntent.Sponsor left = (FeePaymentIntent.Sponsor) expected;
      final FeePaymentIntent.Sponsor right = (FeePaymentIntent.Sponsor) actual;
      return left.programRevision() == right.programRevision()
          && left.programId().name().equals(right.programId().name())
          && sameCanonicalAccountId(
              left.programId().sponsor(), right.programId().sponsor());
    }
    return false;
  }

  private static boolean sameCanonicalAccountId(final String left, final String right) {
    try {
      // AccountId wire identity is domainless and excludes its I105 display discriminant.
      final byte[] leftBytes =
          AccountAddress.parseEncodedIgnoringCurveSupport(left, null).canonicalBytes();
      final byte[] rightBytes =
          AccountAddress.parseEncodedIgnoringCurveSupport(right, null).canonicalBytes();
      return Arrays.equals(leftBytes, rightBytes);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalArgumentException(
          "transaction payload account must be canonical I105", ex);
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
