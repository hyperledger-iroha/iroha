package org.hyperledger.iroha.android.client;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.List;
import java.util.Set;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.sccp.SccpReplayV1;
import org.hyperledger.iroha.android.sccp.SccpV1;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;

/** Shared strict encoding checks for SCCP bridge submit DTOs. */
final class SccpSubmitEncoding {
  static final int MAX_GROTH16_ARTIFACT_BYTES = 16 * 1024 * 1024 + 64 * 1024;
  static final int MAX_DESTINATION_ARTIFACT_BYTES = MAX_GROTH16_ARTIFACT_BYTES + 64 * 1024;
  static final int MAX_DESTINATION_ARTIFACT_BASE64_BYTES = 22_544_384;
  static final int MAX_NATIVE_PROOF_BYTES = 16 * 1024 * 1024;
  static final int MAX_REPLAY_WITNESS_BYTES = 16 * 1024;
  static final int MAX_TRANSACTION_PAYLOAD_BYTES = 16 * 1024 * 1024;
  static final String DESTINATION_ARTIFACT_SCHEMA_NAME =
      "iroha_data_model::bridge::BridgeSccpDestinationProofV1";
  static final String NATIVE_INBOUND_PROOF_SCHEMA_NAME =
      "iroha_sccp::native_admission::SccpNativeInboundMessageProofV1";
  static final String REPLAY_WITNESS_SCHEMA_NAME =
      "iroha_data_model::bridge::sccp_replay::SccpSparseMerkleWitnessV1";
  static final Set<String> PROOF_REQUEST_SCHEMA_NAMES =
      Set.of(
          "iroha_sccp::SccpGroth16Bn254ProofRequestV1",
          "iroha_sccp::SccpTonGroth16Bls12381ProofRequestV1");
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

  static byte[] validateCanonicalReplayWitnessBase64(
      final String value, final String field) {
    final byte[] archive =
        validateCanonicalNoritoBase64(
            value, field, MAX_REPLAY_WITNESS_BYTES, REPLAY_WITNESS_SCHEMA_NAME);
    validateCanonicalReplayWitnessArchive(archive, field);
    return archive;
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

  private static void validateCanonicalReplayWitnessArchive(
      final byte[] archive, final String field) {
    final CompactCursor cursor =
        new CompactCursor(NoritoHeader.decode(archive, null).payload());
    final byte[] expectedRoot = requireFixed32(cursor.field(field + ".expected_shard_root"), field);
    final byte[] priorRecordDigest =
        requireFixed32(cursor.field(field + ".prior_record_digest"), field);
    final byte[] siblingBitmap =
        requireFixed32(cursor.field(field + ".sibling_bitmap"), field);
    final CompactCursor siblingSequence =
        new CompactCursor(cursor.field(field + ".siblings"));
    if (!cursor.finished()) {
      throw new IllegalArgumentException(field + " contains trailing fields");
    }
    final long siblingCount = siblingSequence.u64(field + ".siblings.count");
    if (siblingCount > SccpReplayV1.DEPTH) {
      throw new IllegalArgumentException(field + " contains too many siblings");
    }
    final List<byte[]> siblings = new ArrayList<>((int) siblingCount);
    for (int index = 0; index < (int) siblingCount; index++) {
      siblings.add(requireFixed32(siblingSequence.field(field + ".sibling"), field));
    }
    if (!siblingSequence.finished()) {
      throw new IllegalArgumentException(field + " sibling sequence contains trailing bytes");
    }
    if (!allZero(priorRecordDigest)) {
      throw new IllegalArgumentException(
          field + " must prove non-membership with an all-zero prior record digest");
    }
    SccpReplayV1.rootFromWitness(
        repeatedByte(1, 32),
        null,
        new SccpReplayV1.Witness(
            expectedRoot, priorRecordDigest, siblingBitmap, siblings));
  }

  private static byte[] requireFixed32(final byte[] value, final String field) {
    if (value.length != 32) {
      throw new IllegalArgumentException(field + " contains a malformed fixed byte array");
    }
    return value;
  }

  private static byte[] repeatedByte(final int value, final int length) {
    final byte[] result = new byte[length];
    Arrays.fill(result, (byte) value);
    return result;
  }

  private static final class CompactCursor {
    private final byte[] input;
    private int offset;

    private CompactCursor(final byte[] input) {
      this.input = input.clone();
    }

    private byte[] field(final String field) {
      final long length = compactLength(field);
      if (length > Integer.MAX_VALUE) {
        throw new IllegalArgumentException(field + " length exceeds the runtime bound");
      }
      return exact((int) length, field);
    }

    private long u64(final String field) {
      final byte[] bytes = exact(8, field);
      if ((bytes[7] & 0x80) != 0) {
        throw new IllegalArgumentException(field + " exceeds the signed runtime bound");
      }
      long value = 0;
      for (int index = 0; index < bytes.length; index++) {
        value |= (long) (bytes[index] & 0xff) << (index * 8);
      }
      return value;
    }

    private boolean finished() {
      return offset == input.length;
    }

    private long compactLength(final String field) {
      long result = 0;
      int shift = 0;
      while (true) {
        final int value = exact(1, field)[0] & 0xff;
        final int chunk = value & 0x7f;
        if (shift == 63 && chunk > 1) {
          throw new IllegalArgumentException(field + " compact length exceeds u64");
        }
        result |= (long) chunk << shift;
        if ((value & 0x80) == 0) {
          if (shift > 0 && chunk == 0) {
            throw new IllegalArgumentException(field + " compact length is overlong");
          }
          return result;
        }
        shift += 7;
        if (shift >= 64) {
          throw new IllegalArgumentException(field + " compact length exceeds u64");
        }
      }
    }

    private byte[] exact(final int length, final String field) {
      if (length < 0 || offset > input.length - length) {
        throw new IllegalArgumentException(field + " is truncated");
      }
      final byte[] result = Arrays.copyOfRange(input, offset, offset + length);
      offset += length;
      return result;
    }
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
