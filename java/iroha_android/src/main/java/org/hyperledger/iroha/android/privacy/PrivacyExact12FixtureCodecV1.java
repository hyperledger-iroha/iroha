// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.privacy;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Strict native-independent codec for the canonical exact-12 outer fixture bundle. */
public final class PrivacyExact12FixtureCodecV1 {
  public static final String SCHEMA_NAME =
      "iroha.privacy.exact12-typed-fixture-bundle.v1";
  public static final String SUBMIT_PROOF_WIRE_ID = "iroha.privacy.submit_proof.v1";
  public static final String CANONICAL_ARCHIVE_SHA256_HEX =
      "1fe944a149ffab36a1f3ea04af029c07446d586ead7ae479bbdacf0e02d99397";
  public static final int VERSION = 1;
  public static final int ROW_COUNT = 12;
  public static final int HASH_BYTES = 32;
  public static final int MAX_ARCHIVE_BYTES = 2 * 1024 * 1024;
  public static final long MAX_AGGREGATE_NESTED_BYTES = 2L * 1024L * 1024L;
  public static final int MAX_STATEMENT_BYTES = 256 * 1024;
  public static final int MAX_ENVELOPE_BYTES = 512 * 1024;
  public static final int MAX_INSTRUCTION_BYTES = 512 * 1024;
  public static final int MAX_INTENT_PROJECTION_BYTES = 512 * 1024;
  public static final int MAX_UNSIGNED_TRANSACTION_BYTES = 768 * 1024;
  public static final int MAX_SIGNED_TRANSACTION_BYTES = 1024 * 1024;

  private static final long MAX_ROW_ENCODED_BYTES = 2L * 1024L * 1024L;
  private static final long MAX_WIRE_ID_ENCODED_BYTES = 128L;
  private static final long FIXED_HASH_ENCODED_BYTES = HASH_BYTES;
  private static final int HEADER_PAYLOAD_LENGTH_OFFSET = 23;
  private static final int HEADER_COMPRESSION_OFFSET = 22;
  private static final int HEADER_FLAGS_OFFSET = NoritoHeader.HEADER_LENGTH - 1;
  private static final TypeAdapter<Long> UINT32_ADAPTER = NoritoAdapters.uint(32);
  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<byte[]> RAW_BYTES_ADAPTER =
      NoritoAdapters.rawByteVecAdapter();
  private static final TypeAdapter<byte[]> FIXED_HASH_ADAPTER =
      NoritoAdapters.fixedBytes(HASH_BYTES);
  private static final BundleAdapter BUNDLE_ADAPTER = new BundleAdapter();

  private PrivacyExact12FixtureCodecV1() {}

  /** Decode a complete canonical archive and reject every alternate representation. */
  public static PrivacyExact12FixtureBundleV1 decodeCanonical(final byte[] archive) {
    final byte[] snapshot = Objects.requireNonNull(archive, "archive").clone();
    if (snapshot.length == 0) {
      throw new IllegalArgumentException("exact-12 fixture archive must not be empty");
    }
    if (snapshot.length > MAX_ARCHIVE_BYTES) {
      throw new IllegalArgumentException(
          "exact-12 fixture archive exceeds " + MAX_ARCHIVE_BYTES + " bytes");
    }
    if (snapshot.length < NoritoHeader.HEADER_LENGTH) {
      throw new IllegalArgumentException(
          "exact-12 fixture archive is truncated before the Norito header");
    }
    if ((snapshot[HEADER_COMPRESSION_OFFSET] & 0xFF) != NoritoHeader.COMPRESSION_NONE) {
      throw new IllegalArgumentException("exact-12 fixture must use uncompressed Norito");
    }
    if ((snapshot[HEADER_FLAGS_OFFSET] & 0xFF) != NoritoHeader.COMPACT_LEN) {
      throw new IllegalArgumentException(
          "exact-12 fixture must use only the canonical compact-length flag");
    }
    final long declaredPayloadLength =
        ByteBuffer.wrap(snapshot)
            .order(ByteOrder.LITTLE_ENDIAN)
            .getLong(HEADER_PAYLOAD_LENGTH_OFFSET);
    if (declaredPayloadLength < 0L
        || declaredPayloadLength > MAX_ARCHIVE_BYTES - NoritoHeader.HEADER_LENGTH) {
      throw new IllegalArgumentException(
          "exact-12 fixture declares an oversized Norito payload");
    }
    if (declaredPayloadLength != snapshot.length - NoritoHeader.HEADER_LENGTH) {
      throw new IllegalArgumentException(
          "exact-12 fixture payload length does not cover the complete archive");
    }

    final NoritoHeader.DecodeResult decodedHeader =
        NoritoHeader.decode(snapshot, SchemaHash.hash16(SCHEMA_NAME));
    decodedHeader.header().validateChecksum(decodedHeader.payload());
    final NoritoDecoder decoder =
        new NoritoDecoder(
            decodedHeader.payload(),
            decodedHeader.header().flags());
    final PrivacyExact12FixtureBundleV1 bundle = BUNDLE_ADAPTER.decode(decoder);
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("exact-12 fixture contains trailing payload data");
    }
    final byte[] canonical = encodeCanonical(bundle);
    if (!Arrays.equals(snapshot, canonical)) {
      throw new IllegalArgumentException("exact-12 fixture is not byte-canonical Norito");
    }
    return bundle;
  }

  /** Encode a validated bundle with the exact first-release schema and layout flags. */
  public static byte[] encodeCanonical(final PrivacyExact12FixtureBundleV1 bundle) {
    final byte[] encoded =
        NoritoCodec.encode(
            Objects.requireNonNull(bundle, "bundle"),
            SCHEMA_NAME,
            BUNDLE_ADAPTER,
            NoritoHeader.COMPACT_LEN);
    if (encoded.length > MAX_ARCHIVE_BYTES) {
      throw new IllegalArgumentException(
          "exact-12 fixture archive exceeds " + MAX_ARCHIVE_BYTES + " bytes");
    }
    if (!CANONICAL_ARCHIVE_SHA256_HEX.equals(canonicalArchiveDigestHex(encoded))) {
      throw new IllegalArgumentException(
          "exact-12 fixture differs from the pinned Rust-derived first-release KAT");
    }
    return encoded;
  }

  /** Decode standard padded Base64 without accepting whitespace or alternate spellings. */
  public static PrivacyExact12FixtureBundleV1 decodeCanonicalBase64(final String encoded) {
    Objects.requireNonNull(encoded, "encoded");
    if (encoded.isEmpty()) {
      throw new IllegalArgumentException("exact-12 fixture base64 must not be empty");
    }
    if ((long) encoded.length() > canonicalBase64EncodedLength(MAX_ARCHIVE_BYTES)) {
      throw new IllegalArgumentException("exact-12 fixture base64 exceeds the archive limit");
    }
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(encoded);
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException(
          "exact-12 fixture must use canonical standard base64", error);
    }
    if (!Base64.getEncoder().encodeToString(decoded).equals(encoded)) {
      throw new IllegalArgumentException(
          "exact-12 fixture must use canonical padded standard base64");
    }
    return decodeCanonical(decoded);
  }

  /** Encode one validated bundle as canonical padded standard Base64. */
  public static String encodeCanonicalBase64(final PrivacyExact12FixtureBundleV1 bundle) {
    return Base64.getEncoder().encodeToString(encodeCanonical(bundle));
  }

  /**
   * Decode {@code candidate} and require byte identity with an independently supplied canonical
   * fixture. This closes same-shape cross-row and cross-field substitutions without inventing
   * fixture semantics.
   */
  public static PrivacyExact12FixtureBundleV1 requireCanonicalArchive(
      final byte[] candidate, final byte[] expectedCanonicalArchive) {
    final byte[] candidateSnapshot = Objects.requireNonNull(candidate, "candidate").clone();
    final byte[] expectedSnapshot =
        Objects.requireNonNull(expectedCanonicalArchive, "expectedCanonicalArchive").clone();
    decodeCanonical(expectedSnapshot);
    final PrivacyExact12FixtureBundleV1 decoded = decodeCanonical(candidateSnapshot);
    if (!Arrays.equals(candidateSnapshot, expectedSnapshot)) {
      throw new IllegalArgumentException(
          "exact-12 fixture differs from the supplied canonical archive");
    }
    return decoded;
  }

  /** Compute the canonical Base64 size without allocating the encoded archive. */
  public static long canonicalBase64EncodedLength(final long decodedByteCount) {
    if (decodedByteCount < 0L) {
      throw new IllegalArgumentException("decodedByteCount must be non-negative");
    }
    try {
      final long groups =
          Math.addExact(decodedByteCount / 3L, decodedByteCount % 3L == 0L ? 0L : 1L);
      return Math.multiplyExact(groups, 4L);
    } catch (final ArithmeticException error) {
      throw new IllegalArgumentException(
          "canonical base64 length overflows the supported range", error);
    }
  }

  private static String canonicalArchiveDigestHex(final byte[] bytes) {
    try {
      final byte[] digest = MessageDigest.getInstance("SHA-256").digest(bytes);
      final StringBuilder encoded = new StringBuilder(digest.length * 2);
      for (final byte value : digest) {
        encoded.append(String.format("%02x", value & 0xff));
      }
      return encoded.toString();
    } catch (final NoSuchAlgorithmException error) {
      throw new IllegalStateException("SHA-256 is unavailable", error);
    }
  }

  private static final class BundleAdapter
      implements TypeAdapter<PrivacyExact12FixtureBundleV1> {
    @Override
    public void encode(
        final NoritoEncoder encoder, final PrivacyExact12FixtureBundleV1 value) {
      encodeSizedField(encoder, UINT32_ADAPTER, (long) value.version());
      encodeSizedField(encoder, new RowsAdapter(null), value.rows());
    }

    @Override
    public PrivacyExact12FixtureBundleV1 decode(final NoritoDecoder decoder) {
      final int version =
          decodeExactSizedField(decoder, UINT32_ADAPTER, 4L, "bundle version").intValue();
      final DecodeBudget budget = new DecodeBudget(MAX_AGGREGATE_NESTED_BYTES);
      final List<PrivacyExact12TypedFixtureRowV1> rows =
          decodeBoundedSizedField(
              decoder,
              new RowsAdapter(budget),
              MAX_ARCHIVE_BYTES,
              "bundle rows");
      return new PrivacyExact12FixtureBundleV1(version, rows);
    }
  }

  private static final class RowsAdapter
      implements TypeAdapter<List<PrivacyExact12TypedFixtureRowV1>> {
    private final DecodeBudget budget;

    private RowsAdapter(final DecodeBudget budget) {
      this.budget = budget;
    }

    @Override
    public void encode(
        final NoritoEncoder encoder,
        final List<PrivacyExact12TypedFixtureRowV1> value) {
      if (value.size() != ROW_COUNT) {
        throw new IllegalArgumentException("exact-12 row count must be " + ROW_COUNT);
      }
      encoder.writeLength(value.size(), false);
      final RowAdapter adapter = new RowAdapter(null);
      for (final PrivacyExact12TypedFixtureRowV1 row : value) {
        encodeSizedField(encoder, adapter, row);
      }
    }

    @Override
    public List<PrivacyExact12TypedFixtureRowV1> decode(final NoritoDecoder decoder) {
      if (decoder.readLength(false) != ROW_COUNT) {
        throw new IllegalArgumentException(
            "exact-12 fixture must declare exactly " + ROW_COUNT + " rows");
      }
      final PrivacyProtocolIdV1[] expected =
          PrivacyProtocolIdV1.values();
      final RowAdapter adapter = new RowAdapter(Objects.requireNonNull(budget, "budget"));
      final List<PrivacyExact12TypedFixtureRowV1> rows = new ArrayList<>(ROW_COUNT);
      for (int index = 0; index < ROW_COUNT; index++) {
        final PrivacyExact12TypedFixtureRowV1 row =
            decodeBoundedSizedField(
                decoder, adapter, MAX_ROW_ENCODED_BYTES, "row " + index);
        if (row.protocolId() != expected[index]) {
          throw new IllegalArgumentException(
              "exact-12 row " + index + " is out of canonical protocol order");
        }
        rows.add(row);
      }
      return rows;
    }
  }

  private static final class RowAdapter
      implements TypeAdapter<PrivacyExact12TypedFixtureRowV1> {
    private final DecodeBudget budget;

    private RowAdapter(final DecodeBudget budget) {
      this.budget = budget;
    }

    @Override
    public void encode(
        final NoritoEncoder encoder,
        final PrivacyExact12TypedFixtureRowV1 value) {
      encodeSizedField(encoder, UINT32_ADAPTER, (long) value.protocolId().ordinal());
      encodeSizedField(encoder, RAW_BYTES_ADAPTER, value.statementNorito());
      encodeSizedField(encoder, RAW_BYTES_ADAPTER, value.envelopeNorito());
      encodeSizedField(encoder, STRING_ADAPTER, value.submitProofWireId());
      encodeSizedField(
          encoder, RAW_BYTES_ADAPTER, value.submitProofInstructionNorito());
      encodeSizedField(
          encoder, RAW_BYTES_ADAPTER, value.transactionIntentProjectionNorito());
      encodeSizedField(encoder, FIXED_HASH_ADAPTER, value.transactionIntentDigest());
      encodeSizedField(
          encoder, RAW_BYTES_ADAPTER, value.unsignedTransactionPayloadNorito());
      encodeSizedField(
          encoder, RAW_BYTES_ADAPTER, value.signedTransactionVersionedNorito());
      encodeSizedField(encoder, FIXED_HASH_ADAPTER, value.signedTransactionHash());
    }

    @Override
    public PrivacyExact12TypedFixtureRowV1 decode(final NoritoDecoder decoder) {
      final DecodeBudget decodeBudget = Objects.requireNonNull(budget, "budget");
      final long protocolTag =
          decodeExactSizedField(decoder, UINT32_ADAPTER, 4L, "protocol id");
      final PrivacyProtocolIdV1[] protocols =
          PrivacyProtocolIdV1.values();
      if (protocolTag < 0L || protocolTag >= protocols.length) {
        throw new IllegalArgumentException(
            "unknown exact-12 protocol discriminant: " + protocolTag);
      }
      final byte[] statement =
          decodeRawBytesField(decoder, MAX_STATEMENT_BYTES, decodeBudget, "statement");
      final byte[] envelope =
          decodeRawBytesField(decoder, MAX_ENVELOPE_BYTES, decodeBudget, "envelope");
      final String wireId =
          decodeBoundedSizedField(
              decoder, STRING_ADAPTER, MAX_WIRE_ID_ENCODED_BYTES, "submit-proof wire id");
      if (!SUBMIT_PROOF_WIRE_ID.equals(wireId)) {
        throw new IllegalArgumentException("unknown or retired submit-proof wire id");
      }
      decodeBudget.claim(
          wireId.getBytes(StandardCharsets.UTF_8).length, "submit-proof wire id");
      final byte[] instruction =
          decodeRawBytesField(
              decoder,
              MAX_INSTRUCTION_BYTES,
              decodeBudget,
              "submit-proof instruction");
      final byte[] projection =
          decodeRawBytesField(
              decoder,
              MAX_INTENT_PROJECTION_BYTES,
              decodeBudget,
              "transaction intent projection");
      final byte[] intentDigest =
          decodeExactSizedField(
              decoder,
              FIXED_HASH_ADAPTER,
              FIXED_HASH_ENCODED_BYTES,
              "transaction intent digest");
      decodeBudget.claim(intentDigest.length, "transaction intent digest");
      final byte[] unsigned =
          decodeRawBytesField(
              decoder,
              MAX_UNSIGNED_TRANSACTION_BYTES,
              decodeBudget,
              "unsigned transaction payload");
      final byte[] signed =
          decodeRawBytesField(
              decoder,
              MAX_SIGNED_TRANSACTION_BYTES,
              decodeBudget,
              "signed transaction");
      final byte[] transactionHash =
          decodeExactSizedField(
              decoder,
              FIXED_HASH_ADAPTER,
              FIXED_HASH_ENCODED_BYTES,
              "signed transaction hash");
      decodeBudget.claim(transactionHash.length, "signed transaction hash");
      return new PrivacyExact12TypedFixtureRowV1(
          protocols[(int) protocolTag],
          statement,
          envelope,
          wireId,
          instruction,
          projection,
          intentDigest,
          unsigned,
          signed,
          transactionHash);
    }
  }

  private static final class DecodeBudget {
    private final long maximum;
    private long used;

    private DecodeBudget(final long maximum) {
      this.maximum = maximum;
    }

    private void claim(final long bytes, final String fieldName) {
      if (bytes < 0L) {
        throw new IllegalArgumentException(fieldName + " declares a negative byte count");
      }
      try {
        used = Math.addExact(used, bytes);
      } catch (final ArithmeticException error) {
        throw new IllegalArgumentException(
            "exact-12 aggregate byte count overflows", error);
      }
      if (used > maximum) {
        throw new IllegalArgumentException(
            "exact-12 aggregate nested-byte limit exceeded at " + fieldName);
      }
    }
  }

  private static byte[] decodeRawBytesField(
      final NoritoDecoder decoder,
      final int maximum,
      final DecodeBudget budget,
      final String fieldName) {
    final long encodedLength = decoder.readLength(true);
    if (encodedLength < 9L || encodedLength > maximum + 8L) {
      throw new IllegalArgumentException(
          fieldName + " field exceeds its encoded byte limit");
    }
    final NoritoDecoder child =
        new NoritoDecoder(
            decoder.readBytes((int) encodedLength), decoder.flags());
    final long declaredLength = child.readLength(false);
    if (declaredLength < 1L || declaredLength > maximum) {
      throw new IllegalArgumentException(fieldName + " byte length is invalid");
    }
    if (declaredLength != child.remaining()) {
      throw new IllegalArgumentException(
          fieldName + " declared length does not cover its complete field");
    }
    budget.claim(declaredLength, fieldName);
    return child.readBytes((int) declaredLength);
  }

  private static <T> void encodeSizedField(
      final NoritoEncoder encoder, final TypeAdapter<T> adapter, final T value) {
    final NoritoEncoder child = encoder.childEncoder();
    adapter.encode(child, value);
    final byte[] payload = child.toByteArray();
    encoder.writeLength(payload.length, true);
    encoder.writeBytes(payload);
  }

  private static <T> T decodeExactSizedField(
      final NoritoDecoder decoder,
      final TypeAdapter<T> adapter,
      final long expectedEncodedLength,
      final String fieldName) {
    final long actual = decoder.readLength(true);
    if (actual != expectedEncodedLength) {
      throw new IllegalArgumentException(
          fieldName + " must contain exactly " + expectedEncodedLength + " encoded bytes");
    }
    return decodeChild(decoder, adapter, (int) actual, fieldName);
  }

  private static <T> T decodeBoundedSizedField(
      final NoritoDecoder decoder,
      final TypeAdapter<T> adapter,
      final long maximumEncodedLength,
      final String fieldName) {
    final long length = decoder.readLength(true);
    if (length < 0L || length > maximumEncodedLength) {
      throw new IllegalArgumentException(fieldName + " exceeds its encoded byte limit");
    }
    return decodeChild(decoder, adapter, (int) length, fieldName);
  }

  private static <T> T decodeChild(
      final NoritoDecoder decoder,
      final TypeAdapter<T> adapter,
      final int length,
      final String fieldName) {
    final NoritoDecoder child =
        new NoritoDecoder(decoder.readBytes(length), decoder.flags());
    final T value = adapter.decode(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException(fieldName + " contains trailing or unknown data");
    }
    return value;
  }
}
