package org.hyperledger.iroha.android.model.instructions;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.numeric.NumericV1;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;
import org.junit.Test;

/** Strict native V1 parity tests for {@link CancelAssetLockInstruction}. */
public final class CancelAssetLockInstructionTests {

  private static final String FIXTURE_LOCK_ID =
      "sorafs-appeal-cancel-asset-lock-v1";
  private static final String FIXTURE_ESCROW_ID =
      "hash:73CCD4E0DD69AD434DB75056B600AA4F74C8FC5556B11BDC799DFDB7EA29851F#434B";
  private static final String[] REQUIRED_FIXTURE_NAMES = {
    "cancel_asset_lock_v1.json",
    "cancel_asset_lock_v1.to",
    "negative/cancel_asset_lock_legacy_missing_expected_v1.json",
    "negative/cancel_asset_lock_legacy_missing_expected_v1.to",
    "negative/cancel_asset_lock_nested_escrow_id_v1.to",
    "negative/cancel_asset_lock_noncanonical_quantity_v1.json",
    "negative/cancel_asset_lock_zero_expected_v1.json",
    "negative/cancel_asset_lock_zero_expected_v1.to"
  };
  private static final byte[] CANONICAL_PAYLOAD =
      hexToBytes(
          "4e5254300000b5c8a665a7de80e2eef75ccb287078fa002d00000000000000"
              + "d5f0a9bf0af707a1022073ccd4e0dd69ad434db75056b600aa4f74c8fc5556b11bdc"
              + "799dfdb7ea29851f0b0501000000140400000000");
  private static final byte[] RETIRED_NESTED_ESCROW_ID_PAYLOAD =
      hexToBytes(
          "4e5254300000b5c8a665a7de80e2eef75ccb287078fa002e00000000000000"
              + "0e55fb7ed463b87302212073ccd4e0dd69ad434db75056b600aa4f74c8fc5556b11b"
              + "dc799dfdb7ea29851f0b0501000000140400000000");

  @Test
  public void builderDerivesNativeEscrowIdAndEmitsOnlyV1Fields() {
    final CancelAssetLockInstruction instruction =
        CancelAssetLockInstruction.builder()
            .setLockId(FIXTURE_LOCK_ID)
            .setExpectedRemainingAmount("20")
            .build();

    assertEquals(FIXTURE_ESCROW_ID, instruction.escrowId());
    assertEquals("20", instruction.expectedRemainingAmount().toString());
    assertEquals(2, instruction.toArguments().size());
    assertEquals(FIXTURE_ESCROW_ID, instruction.toArguments().get("escrow_id"));
    assertEquals(
        "20", instruction.toArguments().get("expected_remaining_amount"));
    expectUnsupportedOperation(
        () -> instruction.toArguments().put("amount", "20"),
        "canonical fields were mutable");

    final InstructionBox box = instruction.toInstructionBox();
    assertEquals(CancelAssetLockInstruction.WIRE_NAME, box.name());
    assertTrue(box.payload() instanceof InstructionBox.WirePayload);
    final InstructionBox.WirePayload wire =
        (InstructionBox.WirePayload) box.payload();
    assertArrayEquals(CANONICAL_PAYLOAD, wire.payloadBytes());
  }

  @Test
  public void canonicalNativeFieldsRejectRetiredAndAmbiguousSurfaces() {
    final Map<String, String> valid = new LinkedHashMap<>();
    valid.put("escrow_id", FIXTURE_ESCROW_ID);
    valid.put("expected_remaining_amount", "20");
    assertEquals(
        CancelAssetLockInstruction.builder()
            .setLockId(FIXTURE_LOCK_ID)
            .setExpectedRemainingAmount("20")
            .build(),
        CancelAssetLockInstruction.fromCanonicalFields(valid));

    final Map<String, String> missing = new LinkedHashMap<>();
    missing.put("escrow_id", FIXTURE_ESCROW_ID);
    expectIllegalArgument(
        () -> CancelAssetLockInstruction.fromCanonicalFields(missing),
        "accepted the retired one-field shape");

    final Map<String, String> alias = new LinkedHashMap<>();
    alias.put("escrow_id", FIXTURE_ESCROW_ID);
    alias.put("expectedRemainingAmount", "20");
    expectIllegalArgument(
        () -> CancelAssetLockInstruction.fromCanonicalFields(alias),
        "accepted a field alias");

    final Map<String, String> extra = new LinkedHashMap<>(valid);
    extra.put("amount", "20");
    expectIllegalArgument(
        () -> CancelAssetLockInstruction.fromCanonicalFields(extra),
        "accepted an extra field");
    expectIllegalArgument(
        () ->
            CancelAssetLockInstruction.fromEscrowId(
                FIXTURE_ESCROW_ID.toLowerCase(java.util.Locale.ROOT), "20"),
        "accepted a noncanonical escrow literal");
    final String rawEscrowId = FIXTURE_ESCROW_ID.substring(5, 69);
    for (final String escrowAlias :
        new String[] {
          rawEscrowId,
          "0x" + rawEscrowId,
          "[\"" + FIXTURE_ESCROW_ID + "\"]",
          "{\"value\":\"" + FIXTURE_ESCROW_ID + "\"}",
          FIXTURE_ESCROW_ID.substring(0, FIXTURE_ESCROW_ID.length() - 4) + "0000"
        }) {
      expectIllegalArgument(
          () -> CancelAssetLockInstruction.fromEscrowId(escrowAlias, "20"),
          "accepted escrow alias '" + escrowAlias + "'");
    }
  }

  @Test
  public void expectedRemainingAmountIsPositiveAndCanonicallySpelled() {
    for (final String amount :
        new String[] {"", " ", "0", "-1", "+20", "020", "20.0", "1e1", "20 "}) {
      expectIllegalArgument(
          () ->
              CancelAssetLockInstruction.builder()
                  .setLockId(FIXTURE_LOCK_ID)
                  .setExpectedRemainingAmount(amount),
          "accepted '" + amount + "'");
      final Map<String, String> fields = new LinkedHashMap<>();
      fields.put("escrow_id", FIXTURE_ESCROW_ID);
      fields.put("expected_remaining_amount", amount);
      expectIllegalArgument(
          () -> CancelAssetLockInstruction.fromCanonicalFields(fields),
          "read back '" + amount + "'");
    }
    expectIllegalArgument(
        () ->
            CancelAssetLockInstruction.builder()
                .setLockId(FIXTURE_LOCK_ID)
                .setExpectedRemainingAmount(
                    NumericV1.QuantityValue.of(BigInteger.ZERO, 0)),
        "accepted zero");
    for (final String lockId :
        new String[] {
          "",
          " ",
          " " + FIXTURE_LOCK_ID,
          FIXTURE_LOCK_ID + " ",
          "\uFEFF" + FIXTURE_LOCK_ID,
          FIXTURE_LOCK_ID + "\uFEFF",
          "\uD800",
          "\uDC00",
          "lock\uD800id"
        }) {
      expectIllegalArgument(
          () ->
              CancelAssetLockInstruction.builder()
                  .setLockId(lockId),
          "accepted lock id '" + lockId + "'");
    }
  }

  @Test
  public void lockIdPreimageUsesExactUtf8ByteBound() {
    final StringBuilder exactBoundBuilder = new StringBuilder();
    for (int index = 0; index < 1_024; index++) {
      exactBoundBuilder.append("🔒");
    }
    final String exactBound = exactBoundBuilder.toString();
    assertEquals(4_096, exactBound.getBytes(StandardCharsets.UTF_8).length);
    assertEquals(4_096, CancelAssetLockInstruction.MAX_LOCK_ID_UTF8_BYTES_V1);
    CancelAssetLockInstruction.builder()
        .setLockId(exactBound)
        .setExpectedRemainingAmount("1")
        .build();

    final String overBound = exactBound + "a";
    assertEquals(4_097, overBound.getBytes(StandardCharsets.UTF_8).length);
    expectIllegalArgument(
        () ->
            CancelAssetLockInstruction.builder()
                .setLockId(overBound)
                .setExpectedRemainingAmount("1")
                .build(),
        "accepted a 4,097-byte lock id");
  }

  @Test
  public void wireDecoderIsStrictAndRoundtripsCanonicalFrame() {
    assertEquals(85, CANONICAL_PAYLOAD.length);
    final CancelAssetLockInstruction decoded =
        CancelAssetLockInstruction.fromWirePayload(CANONICAL_PAYLOAD);
    assertEquals(FIXTURE_ESCROW_ID, decoded.escrowId());
    assertEquals("20", decoded.expectedRemainingAmount().toString());
    assertArrayEquals(
        CANONICAL_PAYLOAD,
        CancelAssetLockWirePayloadEncoder.encodePayload(decoded));

    final byte[] trailing =
        Arrays.copyOf(CANONICAL_PAYLOAD, CANONICAL_PAYLOAD.length + 1);
    expectIllegalArgument(
        () -> CancelAssetLockInstruction.fromWirePayload(trailing),
        "accepted trailing bytes");
    expectIllegalArgument(
        () -> CancelAssetLockInstruction.fromWirePayload(legacyOneFieldFrame()),
        "accepted the retired one-field frame");
    assertEquals(86, RETIRED_NESTED_ESCROW_ID_PAYLOAD.length);
    assertArrayEquals(
        new byte[] {0x21, 0x20},
        Arrays.copyOfRange(RETIRED_NESTED_ESCROW_ID_PAYLOAD, 40, 42));
    expectIllegalArgument(
        () ->
            CancelAssetLockInstruction.fromWirePayload(
                RETIRED_NESTED_ESCROW_ID_PAYLOAD),
        "accepted the retired nested EscrowId frame");
  }

  @Test
  public void checkedInAppealFinanceFixturesAreMandatoryAndByteExact()
      throws Exception {
    final Path root = requireFixtureRoot();
    final Map<String, byte[]> fixtures = new LinkedHashMap<>();
    for (final String relative : REQUIRED_FIXTURE_NAMES) {
      final byte[] bytes = readMandatoryFixture(root, relative);
      assertTrue("Mandatory fixture `" + relative + "` is empty", bytes.length > 0);
      fixtures.put(relative, bytes);
    }
    assertEquals(8, fixtures.size());

    final CancelAssetLockInstruction canonicalFromJson =
        CancelAssetLockInstruction.fromCanonicalFields(
            decodeCanonicalJsonFields(fixtures.get("cancel_asset_lock_v1.json")));
    final CancelAssetLockInstruction canonicalFromNorito =
        CancelAssetLockInstruction.fromWirePayload(
            fixtures.get("cancel_asset_lock_v1.to"));
    assertEquals(canonicalFromJson, canonicalFromNorito);
    assertEquals(85, fixtures.get("cancel_asset_lock_v1.to").length);
    assertArrayEquals(
        fixtures.get("cancel_asset_lock_v1.to"),
        CancelAssetLockWirePayloadEncoder.encodePayload(canonicalFromJson));

    for (final String relative :
        new String[] {
          "negative/cancel_asset_lock_legacy_missing_expected_v1.json",
          "negative/cancel_asset_lock_noncanonical_quantity_v1.json",
          "negative/cancel_asset_lock_zero_expected_v1.json"
        }) {
      expectIllegalArgument(
          () ->
              CancelAssetLockInstruction.fromCanonicalFields(
                  decodeCanonicalJsonFields(fixtures.get(relative))),
          "accepted " + relative);
    }

    assertArrayEquals(
        RETIRED_NESTED_ESCROW_ID_PAYLOAD,
        fixtures.get("negative/cancel_asset_lock_nested_escrow_id_v1.to"));
    for (final String relative :
        new String[] {
          "negative/cancel_asset_lock_legacy_missing_expected_v1.to",
          "negative/cancel_asset_lock_nested_escrow_id_v1.to",
          "negative/cancel_asset_lock_zero_expected_v1.to"
        }) {
      expectIllegalArgument(
          () -> {
            CancelAssetLockInstruction.fromWirePayload(fixtures.get(relative));
          },
          "accepted " + relative);
    }
  }

  private static Map<String, String> decodeCanonicalJsonFields(final byte[] payload) {
    final Object parsed =
        JsonParser.parse(new String(payload, StandardCharsets.UTF_8));
    if (!(parsed instanceof Map<?, ?>)) {
      throw new IllegalArgumentException("CancelAssetLock JSON must be an object");
    }
    final Map<String, String> fields = new LinkedHashMap<>();
    for (final Map.Entry<?, ?> entry : ((Map<?, ?>) parsed).entrySet()) {
      if (!(entry.getKey() instanceof String) || !(entry.getValue() instanceof String)) {
        throw new IllegalArgumentException(
            "CancelAssetLock JSON fields and values must be strings");
      }
      fields.put((String) entry.getKey(), (String) entry.getValue());
    }
    return fields;
  }

  private static byte[] legacyOneFieldFrame() {
    return NoritoCodec.encode(
        hexToBytes(FIXTURE_ESCROW_ID.substring(5, 69)),
        CancelAssetLockInstruction.WIRE_NAME,
        new TypeAdapter<byte[]>() {
          @Override
          public void encode(final NoritoEncoder encoder, final byte[] value) {
            encoder.writeLength(
                value.length,
                (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0);
            encoder.writeBytes(value);
          }

          @Override
          public byte[] decode(final NoritoDecoder decoder) {
            throw new UnsupportedOperationException();
          }
        });
  }

  private static Path requireFixtureRoot() {
    final Path[] candidates = {
      Paths.get("../../../fixtures/sorafs_manifest/appeal_finance"),
      Paths.get("../../fixtures/sorafs_manifest/appeal_finance"),
      Paths.get("fixtures/sorafs_manifest/appeal_finance")
    };
    for (final Path candidate : candidates) {
      if (Files.isDirectory(candidate)) {
        return candidate;
      }
    }
    throw new AssertionError(
        "Missing mandatory CancelAssetLock fixture directory; searched: "
            + Arrays.toString(candidates));
  }

  private static byte[] readMandatoryFixture(
      final Path root, final String relative) throws java.io.IOException {
    final Path path = root.resolve(relative);
    assertTrue(
        "Missing mandatory CancelAssetLock fixture `"
            + relative
            + "` at "
            + path,
        Files.isRegularFile(path));
    return Files.readAllBytes(path);
  }

  private static byte[] hexToBytes(final String value) {
    final byte[] bytes = new byte[value.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] =
          (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return bytes;
  }

  private static void expectIllegalArgument(
      final Runnable action, final String message) {
    try {
      action.run();
      fail(message);
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }

  private static void expectUnsupportedOperation(
      final Runnable action, final String message) {
    try {
      action.run();
      fail(message);
    } catch (final UnsupportedOperationException expected) {
      // Expected.
    }
  }
}
