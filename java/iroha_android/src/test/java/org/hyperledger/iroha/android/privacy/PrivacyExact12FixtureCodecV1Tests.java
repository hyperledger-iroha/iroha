// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.privacy;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.List;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;
import org.junit.Test;

/** Real-fixture and adversarial checks for the native-independent exact-12 codec. */
public final class PrivacyExact12FixtureCodecV1Tests {
  private static final String FIXTURE_PATH =
      "fixtures/privacy/exact12_typed_fixture_bundle_v1.norito.b64";

  @Test
  public void firstReleaseIdentifiersPreserveCanonicalOrdinalsAndLabels() {
    assertEquals(0, PrivacyProtocolIdV1.ZK_ACE_PQ_AUTHORIZATION_V1.ordinal());
    assertEquals(4, PrivacyProtocolIdV1.VEGA_EXISTING_CREDENTIAL_ZK_V1.ordinal());
    assertEquals(5, PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1.ordinal());
    assertEquals(6, PrivacyProtocolIdV1.IROHA_JINDO_POLYNOMIAL_COMMITMENT_V1.ordinal());
    assertEquals(11, PrivacyProtocolIdV1.PQ_MASP_STARK_V1.ordinal());
    assertEquals(
        0, PrivacyProofSystemIdV1.STARK_FRI_POSEIDON_X7_GOLDILOCKS_6X64_V1.ordinal());
    assertEquals(
        "stark-fri-poseidon-x7-goldilocks-6x64-v1",
        PrivacyProofSystemIdV1.STARK_FRI_POSEIDON_X7_GOLDILOCKS_6X64_V1.canonicalLabel());
    assertEquals(
        0, PrivacyEngineIdV1.NATIVE_GOLDILOCKS_POSEIDON_X7_STARK_FRI_6X64_V1.ordinal());
    assertEquals(
        "native-goldilocks-poseidon-x7-stark-fri-6x64-v1",
        PrivacyEngineIdV1.NATIVE_GOLDILOCKS_POSEIDON_X7_STARK_FRI_6X64_V1.canonicalLabel());
  }

  @Test
  public void canonicalFixtureDecodesAndReencodesByteIdentically() throws IOException {
    final Fixture fixture = loadFixture();
    final PrivacyExact12FixtureBundleV1 bundle =
        PrivacyExact12FixtureCodecV1.decodeCanonicalBase64(fixture.base64);

    assertEquals(PrivacyExact12FixtureCodecV1.VERSION, bundle.version());
    assertEquals(PrivacyExact12FixtureCodecV1.ROW_COUNT, bundle.rows().size());
    final PrivacyProtocolIdV1[] protocols =
        PrivacyProtocolIdV1.values();
    for (int index = 0; index < bundle.rows().size(); index++) {
      final PrivacyExact12TypedFixtureRowV1 row = bundle.rows().get(index);
      assertEquals(protocols[index], row.protocolId());
      assertTrue(row.statementNorito().length > 0);
      assertTrue(row.envelopeNorito().length > 0);
      assertEquals(
          PrivacyExact12FixtureCodecV1.SUBMIT_PROOF_WIRE_ID, row.submitProofWireId());
      assertTrue(row.submitProofInstructionNorito().length > 0);
      assertTrue(row.transactionIntentProjectionNorito().length > 0);
      assertEquals(
          PrivacyExact12FixtureCodecV1.HASH_BYTES, row.transactionIntentDigest().length);
      assertTrue(row.unsignedTransactionPayloadNorito().length > 0);
      assertTrue(row.signedTransactionVersionedNorito().length > 0);
      assertEquals(
          PrivacyExact12FixtureCodecV1.HASH_BYTES, row.signedTransactionHash().length);
    }

    assertArrayEquals(fixture.archive, PrivacyExact12FixtureCodecV1.encodeCanonical(bundle));
    assertEquals(fixture.base64, PrivacyExact12FixtureCodecV1.encodeCanonicalBase64(bundle));
    assertEquals(
        bundle,
        PrivacyExact12FixtureCodecV1.requireCanonicalArchive(
            fixture.archive, fixture.archive));

    final PrivacyExact12TypedFixtureRowV1 first = bundle.rows().get(0);
    final byte originalFirstByte = first.statementNorito()[0];
    final byte[] statementCopy = first.statementNorito();
    statementCopy[0] ^= (byte) 0xFF;
    assertEquals(originalFirstByte, first.statementNorito()[0]);
  }

  @Test
  public void canonicalBase64RejectsWhitespaceAlternateSpellingsAndOverflow()
      throws IOException {
    final String encoded = loadFixture().base64;
    for (final String malformed :
        Arrays.asList(
            encoded + "\n",
            " " + encoded,
            encoded + " ",
            encoded + "=",
            encoded.substring(0, encoded.length() - 1))) {
      assertThrows(
          IllegalArgumentException.class,
          () -> PrivacyExact12FixtureCodecV1.decodeCanonicalBase64(malformed));
    }

    assertEquals(0L, PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(0L));
    assertEquals(4L, PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(1L));
    assertEquals(4L, PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(3L));
    assertEquals(8L, PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(4L));
    assertThrows(
        IllegalArgumentException.class,
        () -> PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(-1L));
    assertThrows(
        IllegalArgumentException.class,
        () -> PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(Long.MAX_VALUE));
    final int maximumEncodedLength =
        Math.toIntExact(
            PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(
                PrivacyExact12FixtureCodecV1.MAX_ARCHIVE_BYTES));
    final String oversizedBase64 = "A".repeat(maximumEncodedLength + 1);
    assertThrows(
        IllegalArgumentException.class,
        () -> PrivacyExact12FixtureCodecV1.decodeCanonicalBase64(oversizedBase64));
  }

  @Test
  public void malformedHeadersLengthsAndTruncationAreRejected() throws IOException {
    final byte[] canonical = loadFixture().archive;
    final byte[] declaredTooLarge = canonical.clone();
    ByteBuffer.wrap(declaredTooLarge)
        .order(ByteOrder.LITTLE_ENDIAN)
        .putLong(23, PrivacyExact12FixtureCodecV1.MAX_ARCHIVE_BYTES);
    final byte[] wrongSchema = canonical.clone();
    wrongSchema[6] ^= (byte) 0x80;
    final byte[] wrongFlags = canonical.clone();
    wrongFlags[NoritoHeader.HEADER_LENGTH - 1] = 0;
    final byte[] wrongCompression = canonical.clone();
    wrongCompression[22] = (byte) NoritoHeader.COMPRESSION_ZSTD;
    final byte[] wrongChecksum = canonical.clone();
    wrongChecksum[wrongChecksum.length - 1] ^= 0x01;
    for (final byte[] malformed :
        Arrays.asList(
            Arrays.copyOf(canonical, canonical.length - 1),
            concat(canonical, new byte[] {0}),
            declaredTooLarge,
            wrongSchema,
            wrongFlags,
            wrongCompression,
            wrongChecksum,
            appendUnknownByteToFirstRow(canonical),
            new byte[NoritoHeader.HEADER_LENGTH - 1],
            new byte[PrivacyExact12FixtureCodecV1.MAX_ARCHIVE_BYTES + 1])) {
      assertThrows(
          IllegalArgumentException.class,
          () -> PrivacyExact12FixtureCodecV1.decodeCanonical(malformed));
    }
  }

  @Test
  public void hostileNestedCountsAndDeclaredLengthsAreRejectedBeforeAllocation() {
    final byte[] wrongRowCount = frame(bundlePayload(u64(11L)));
    final byte[] oversizedRows =
        frame(
            concat(
                field(u32(PrivacyExact12FixtureCodecV1.VERSION)),
                compactLength(PrivacyExact12FixtureCodecV1.MAX_ARCHIVE_BYTES + 1L)));
    final byte[] oversizedFirstRow =
        frame(
            bundlePayload(
                concat(
                    u64(PrivacyExact12FixtureCodecV1.ROW_COUNT),
                    compactLength(PrivacyExact12FixtureCodecV1.MAX_ARCHIVE_BYTES + 1L))));
    final byte[] unknownProtocol =
        frame(
            bundlePayload(
                concat(
                    u64(PrivacyExact12FixtureCodecV1.ROW_COUNT),
                    field(field(u32(PrivacyExact12FixtureCodecV1.ROW_COUNT))))));
    final byte[] truncatedFirstRow =
        frame(
            bundlePayload(
                concat(
                    u64(PrivacyExact12FixtureCodecV1.ROW_COUNT),
                    field(field(u32(0L))))));
    final byte[] oversizedStatementFrame =
        frame(
            bundlePayload(
                concat(
                    u64(PrivacyExact12FixtureCodecV1.ROW_COUNT),
                    field(
                        concat(
                            field(u32(0L)),
                            compactLength(
                                PrivacyExact12FixtureCodecV1.MAX_STATEMENT_BYTES + 9L))))));
    final byte[] oversizedStatementVector =
        frame(
            bundlePayload(
                concat(
                    u64(PrivacyExact12FixtureCodecV1.ROW_COUNT),
                    field(
                        concat(
                            field(u32(0L)),
                            field(
                                concat(
                                    u64(
                                        PrivacyExact12FixtureCodecV1.MAX_STATEMENT_BYTES
                                            + 1L),
                                    new byte[] {0})))))));
    final byte[] nonMinimalVersionLength =
        frame(
            concat(
                new byte[] {(byte) 0x84, 0},
                u32(PrivacyExact12FixtureCodecV1.VERSION),
                field(u64(PrivacyExact12FixtureCodecV1.ROW_COUNT))));
    for (final byte[] hostile :
        Arrays.asList(
            wrongRowCount,
            oversizedRows,
            oversizedFirstRow,
            unknownProtocol,
            truncatedFirstRow,
            oversizedStatementFrame,
            oversizedStatementVector,
            nonMinimalVersionLength)) {
      assertTrue(
          "hostile test archive unexpectedly allocated a large payload", hostile.length < 512);
      assertThrows(
          IllegalArgumentException.class,
          () -> PrivacyExact12FixtureCodecV1.decodeCanonical(hostile));
    }
  }

  @Test
  public void reorderAndEveryNestedCrossRowSubstitutionAreRejected() throws IOException {
    final Fixture fixture = loadFixture();
    final PrivacyExact12FixtureBundleV1 bundle =
        PrivacyExact12FixtureCodecV1.decodeCanonical(fixture.archive);

    final List<PrivacyExact12TypedFixtureRowV1> swappedRows =
        new ArrayList<>(bundle.rows());
    final PrivacyExact12TypedFixtureRowV1 first = swappedRows.get(0);
    swappedRows.set(0, swappedRows.get(1));
    swappedRows.set(1, first);
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new PrivacyExact12FixtureBundleV1(
                PrivacyExact12FixtureCodecV1.VERSION, swappedRows));

    final byte[] reorderedArchive = swapFirstTwoRowFrames(fixture.archive);
    assertFalse(Arrays.equals(reorderedArchive, fixture.archive));
    assertThrows(
        IllegalArgumentException.class,
        () -> PrivacyExact12FixtureCodecV1.decodeCanonical(reorderedArchive));

    final PrivacyExact12TypedFixtureRowV1 source = bundle.rows().get(0);
    final PrivacyExact12TypedFixtureRowV1 donor = bundle.rows().get(1);
    for (int field = 0; field < 8; field++) {
      final PrivacyExact12TypedFixtureRowV1 substituted =
          copyRowFieldFrom(source, donor, field);
      final List<PrivacyExact12TypedFixtureRowV1> substitutedRows =
          new ArrayList<>(bundle.rows());
      substitutedRows.set(0, substituted);
      assertThrows(
          IllegalArgumentException.class,
          () ->
              PrivacyExact12FixtureCodecV1.encodeCanonical(
                  new PrivacyExact12FixtureBundleV1(
                      PrivacyExact12FixtureCodecV1.VERSION, substitutedRows)));
    }
  }

  @Test
  public void modelEnforcesPerFieldAndAggregateBounds() {
    assertThrows(
        IllegalArgumentException.class,
        () ->
            syntheticRow(
                PrivacyProtocolIdV1.values()[0],
                new byte[PrivacyExact12FixtureCodecV1.MAX_STATEMENT_BYTES + 1],
                new byte[] {1}));
    final byte[] largeSigned = new byte[180_000];
    Arrays.fill(largeSigned, (byte) 0x5A);
    final List<PrivacyExact12TypedFixtureRowV1> aggregateRows = new ArrayList<>();
    for (final PrivacyProtocolIdV1 protocol :
        PrivacyProtocolIdV1.values()) {
      aggregateRows.add(syntheticRow(protocol, new byte[] {1}, largeSigned));
    }
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new PrivacyExact12FixtureBundleV1(
                PrivacyExact12FixtureCodecV1.VERSION, aggregateRows));
  }

  private static Fixture loadFixture() throws IOException {
    Path cursor = Paths.get("").toAbsolutePath().normalize();
    Path fixturePath = null;
    while (cursor != null) {
      final Path candidate = cursor.resolve(FIXTURE_PATH);
      if (Files.isRegularFile(candidate)) {
        fixturePath = candidate;
        break;
      }
      cursor = cursor.getParent();
    }
    if (fixturePath == null) {
      throw new IllegalStateException("cannot locate " + FIXTURE_PATH);
    }
    final byte[] bytes = Files.readAllBytes(fixturePath);
    if (bytes.length == 0 || bytes[bytes.length - 1] != '\n') {
      throw new IllegalStateException(FIXTURE_PATH + " must end in exactly one LF");
    }
    for (int index = 0; index < bytes.length; index++) {
      if (bytes[index] == '\r' || (bytes[index] == '\n' && index != bytes.length - 1)) {
        throw new IllegalStateException(FIXTURE_PATH + " must contain one LF-only base64 line");
      }
    }
    final String encoded =
        new String(bytes, 0, bytes.length - 1, StandardCharsets.US_ASCII);
    final byte[] archive = Base64.getDecoder().decode(encoded);
    if (!Base64.getEncoder().encodeToString(archive).equals(encoded)) {
      throw new IllegalStateException(FIXTURE_PATH + " is not canonical standard base64");
    }
    if (archive.length > PrivacyExact12FixtureCodecV1.MAX_ARCHIVE_BYTES) {
      throw new IllegalStateException(FIXTURE_PATH + " exceeds the decoded archive ceiling");
    }
    return new Fixture(encoded, archive);
  }

  private static byte[] bundlePayload(final byte[] rowsPayload) {
    return concat(
        field(u32(PrivacyExact12FixtureCodecV1.VERSION)), field(rowsPayload));
  }

  private static byte[] frame(final byte[] payload) {
    final byte[] header =
        new NoritoHeader(
                SchemaHash.hash16(PrivacyExact12FixtureCodecV1.SCHEMA_NAME),
                payload.length,
                CRC64.compute(payload),
                NoritoHeader.COMPACT_LEN,
                NoritoHeader.COMPRESSION_NONE)
            .encode();
    return concat(header, payload);
  }

  private static byte[] swapFirstTwoRowFrames(final byte[] archive) {
    final byte[] payload =
        Arrays.copyOfRange(archive, NoritoHeader.HEADER_LENGTH, archive.length);
    final Frame version = readFrame(payload, 0);
    final CompactLength rowsLength = readCompactLength(payload, version.end);
    final int rowsStart = rowsLength.end;
    final int rowsEnd = Math.addExact(rowsStart, Math.toIntExact(rowsLength.value));
    if (rowsEnd != payload.length) {
      throw new IllegalStateException("fixture rows field does not consume the payload");
    }
    final byte[] rowsPayload = Arrays.copyOfRange(payload, rowsStart, rowsEnd);
    if (ByteBuffer.wrap(rowsPayload).order(ByteOrder.LITTLE_ENDIAN).getLong()
        != PrivacyExact12FixtureCodecV1.ROW_COUNT) {
      throw new IllegalStateException("fixture row count changed");
    }
    final List<Frame> frames = new ArrayList<>(PrivacyExact12FixtureCodecV1.ROW_COUNT);
    int rowCursor = Long.BYTES;
    for (int index = 0; index < PrivacyExact12FixtureCodecV1.ROW_COUNT; index++) {
      final Frame row = readFrame(rowsPayload, rowCursor);
      frames.add(row);
      rowCursor = row.end;
    }
    if (rowCursor != rowsPayload.length) {
      throw new IllegalStateException("fixture rows contain trailing data");
    }

    final ByteArrayOutputStream reorderedRows =
        new ByteArrayOutputStream(rowsPayload.length);
    reorderedRows.write(rowsPayload, 0, Long.BYTES);
    final int[] order = new int[PrivacyExact12FixtureCodecV1.ROW_COUNT];
    order[0] = 1;
    order[1] = 0;
    for (int index = 2; index < order.length; index++) {
      order[index] = index;
    }
    for (final int index : order) {
      final Frame row = frames.get(index);
      reorderedRows.write(rowsPayload, row.start, row.end - row.start);
    }
    final ByteArrayOutputStream modifiedPayload =
        new ByteArrayOutputStream(payload.length);
    modifiedPayload.write(payload, 0, version.end);
    modifiedPayload.write(payload, version.end, rowsStart - version.end);
    modifiedPayload.writeBytes(reorderedRows.toByteArray());
    return frame(modifiedPayload.toByteArray());
  }

  private static byte[] appendUnknownByteToFirstRow(final byte[] archive) {
    final byte[] payload =
        Arrays.copyOfRange(archive, NoritoHeader.HEADER_LENGTH, archive.length);
    final Frame version = readFrame(payload, 0);
    final CompactLength rowsLength = readCompactLength(payload, version.end);
    final int rowsStart = rowsLength.end;
    final int rowsEnd = Math.addExact(rowsStart, Math.toIntExact(rowsLength.value));
    if (rowsEnd != payload.length) {
      throw new IllegalStateException("fixture rows field does not consume the payload");
    }
    final byte[] rowsPayload = Arrays.copyOfRange(payload, rowsStart, rowsEnd);
    final CompactLength firstRowLength = readCompactLength(rowsPayload, Long.BYTES);
    final int firstRowEnd =
        Math.addExact(firstRowLength.end, Math.toIntExact(firstRowLength.value));
    if (firstRowEnd > rowsPayload.length) {
      throw new IllegalStateException("fixture first row is truncated");
    }
    final byte[] firstRowPayload =
        Arrays.copyOfRange(rowsPayload, firstRowLength.end, firstRowEnd);
    final byte[] modifiedRows =
        concat(
            Arrays.copyOfRange(rowsPayload, 0, Long.BYTES),
            field(concat(firstRowPayload, new byte[] {0})),
            Arrays.copyOfRange(rowsPayload, firstRowEnd, rowsPayload.length));
    final byte[] modifiedPayload =
        concat(Arrays.copyOfRange(payload, 0, version.end), field(modifiedRows));
    return frame(modifiedPayload);
  }

  private static Frame readFrame(final byte[] bytes, final int offset) {
    final CompactLength length = readCompactLength(bytes, offset);
    final int end = Math.addExact(length.end, Math.toIntExact(length.value));
    if (end > bytes.length) {
      throw new IllegalStateException("fixture frame is truncated");
    }
    return new Frame(offset, end);
  }

  private static CompactLength readCompactLength(final byte[] bytes, final int offset) {
    long value = 0L;
    int shift = 0;
    int cursor = offset;
    while (true) {
      if (cursor >= bytes.length || shift > 63) {
        throw new IllegalStateException("invalid compact fixture length");
      }
      final int octet = bytes[cursor++] & 0xFF;
      value |= (long) (octet & 0x7F) << shift;
      if ((octet & 0x80) == 0) {
        return new CompactLength(value, cursor);
      }
      shift += 7;
    }
  }

  private static byte[] compactLength(final long value) {
    final NoritoEncoder encoder = new NoritoEncoder(NoritoHeader.COMPACT_LEN);
    encoder.writeLength(value, true);
    return encoder.toByteArray();
  }

  private static byte[] field(final byte[] payload) {
    return concat(compactLength(payload.length), payload);
  }

  private static byte[] u32(final long value) {
    return ByteBuffer.allocate(Integer.BYTES)
        .order(ByteOrder.LITTLE_ENDIAN)
        .putInt((int) value)
        .array();
  }

  private static byte[] u64(final long value) {
    return ByteBuffer.allocate(Long.BYTES)
        .order(ByteOrder.LITTLE_ENDIAN)
        .putLong(value)
        .array();
  }

  private static PrivacyExact12TypedFixtureRowV1 copyRowFieldFrom(
      final PrivacyExact12TypedFixtureRowV1 row,
      final PrivacyExact12TypedFixtureRowV1 donor,
      final int field) {
    return new PrivacyExact12TypedFixtureRowV1(
        row.protocolId(),
        field == 0 ? donor.statementNorito() : row.statementNorito(),
        field == 1 ? donor.envelopeNorito() : row.envelopeNorito(),
        row.submitProofWireId(),
        field == 2 ? donor.submitProofInstructionNorito() : row.submitProofInstructionNorito(),
        field == 3
            ? donor.transactionIntentProjectionNorito()
            : row.transactionIntentProjectionNorito(),
        field == 4 ? donor.transactionIntentDigest() : row.transactionIntentDigest(),
        field == 5
            ? donor.unsignedTransactionPayloadNorito()
            : row.unsignedTransactionPayloadNorito(),
        field == 6
            ? donor.signedTransactionVersionedNorito()
            : row.signedTransactionVersionedNorito(),
        field == 7 ? donor.signedTransactionHash() : row.signedTransactionHash());
  }

  private static PrivacyExact12TypedFixtureRowV1 syntheticRow(
      final PrivacyProtocolIdV1 protocol,
      final byte[] statement,
      final byte[] signed) {
    return new PrivacyExact12TypedFixtureRowV1(
        protocol,
        statement,
        new byte[] {2},
        PrivacyExact12FixtureCodecV1.SUBMIT_PROOF_WIRE_ID,
        new byte[] {3},
        new byte[] {4},
        fill(5, PrivacyExact12FixtureCodecV1.HASH_BYTES),
        new byte[] {6},
        signed,
        fill(7, PrivacyExact12FixtureCodecV1.HASH_BYTES));
  }

  private static byte[] fill(final int value, final int count) {
    final byte[] result = new byte[count];
    Arrays.fill(result, (byte) value);
    return result;
  }

  private static byte[] concat(final byte[]... parts) {
    int length = 0;
    for (final byte[] part : parts) {
      length = Math.addExact(length, part.length);
    }
    final byte[] result = new byte[length];
    int offset = 0;
    for (final byte[] part : parts) {
      System.arraycopy(part, 0, result, offset, part.length);
      offset += part.length;
    }
    return result;
  }

  private static final class Fixture {
    private final String base64;
    private final byte[] archive;

    private Fixture(final String base64, final byte[] archive) {
      this.base64 = base64;
      this.archive = archive;
    }
  }

  private static final class Frame {
    private final int start;
    private final int end;

    private Frame(final int start, final int end) {
      this.start = start;
      this.end = end;
    }
  }

  private static final class CompactLength {
    private final long value;
    private final int end;

    private CompactLength(final long value, final int end) {
      this.value = value;
      this.end = end;
    }
  }
}
