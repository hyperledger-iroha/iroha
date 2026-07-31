package org.hyperledger.iroha.android.sorafs;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.RegisterPinManifestInstruction;
import org.hyperledger.iroha.android.testing.SimpleJson;

/** Adversarial coverage for the first-release register-pin consensus builder. */
public final class SorafsRegisterPinManifestBuilderTests {

  private static final String CANONICAL_MANIFEST_BASE64 =
      "TlJUMAAAduKskROcpAXus8dyJtDtlwD5AAAAAAAAAP11VCbJ+r+OAgEBLCQAAAAAAAAAAXEfIGIKrspjqahUS44Ka/oZKH+vxtnoUoM+vR660uOpRn5yCQhxAAAAAAAAAF4FBAEAAAAHBnNvcmFmcwQDc2YxBgUxLjAuMAQAAAEABAAABAAEAAAIAAT//wAACB8AAAAAAAAAJgIAAAAAAAAAERBzb3JhZnMuc2YxQDEuMC4wCwpzb3JhZnMtc2YxCAAAEAAAAAAAIM5QqarfhOV1WSCNOSAWISYv0bGIeuSQylRHDioAFT8nCNwEEAAAAAAAEQIDAAQAAAAACIBRAQAAAAAACQgAAAAAAAAAAAgAAAAAAAAAAAgAAAAAAAAAAA==";

  private SorafsRegisterPinManifestBuilderTests() {}

  public static void main(final String[] args) throws Exception {
    fixtureUsesCanonicalFirstReleaseFields();
    rejectsMalformedManifestPayload();
    rejectsOversizedManifestPayload();
    defensivelyCopiesManifestPayload();
    rejectsMissingRequiredFields();
    rejectsMalformedSuccessorDigest();
    rejectsNegativeSubmittedEpoch();
    rejectsPartialAliasBinding();
    rejectsMalformedAndOversizedAliasProof();
    rejectsLegacyAndUnknownArguments();
    roundTripsCanonicalArguments();
    System.out.println("[IrohaAndroid] SorafsRegisterPinManifestBuilderTests passed.");
  }

  private static void fixtureUsesCanonicalFirstReleaseFields() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> instruction = asMap(fixture.get("instruction"), "instruction");
    final RegisterPinManifestInstruction payload =
        RegisterPinManifestInstruction.builder()
            .setManifestPayloadBase64(requireString(instruction, "manifest_payload_base64"))
            .setSubmittedEpoch(requireNumber(instruction.get("submitted_epoch")))
            .build();
    final InstructionBox box = payload.toInstructionBox();
    assert box.arguments().size() == 3 : "unexpected retired consensus fields";
    assert Objects.equals(
        box.arguments().get("manifest_payload_base64"),
        requireString(instruction, "manifest_payload_base64"));
    assert !box.arguments().containsKey("chunk_digest_sha3_256_hex");
    assert !box.arguments().containsKey("digest_hex");
    assert !box.arguments().containsKey("chunker.profile_id");
    assert !box.arguments().containsKey("content_length");
    assert !box.arguments().containsKey("policy.min_replicas");
  }

  private static void rejectsMalformedManifestPayload() {
    for (final String payload : new String[] {"", "%%%", "AQ"}) {
      expectIllegalArgument(
          () -> RegisterPinManifestInstruction.builder().setManifestPayloadBase64(payload),
          "malformed or noncanonical manifest payload must fail");
    }
    expectIllegalArgument(
        () -> RegisterPinManifestInstruction.builder().setManifestPayload(new byte[0]),
        "empty raw manifest payload must fail");
  }

  private static void rejectsOversizedManifestPayload() {
    final byte[] oversized = new byte[512 * 1024 + 1];
    Arrays.fill(oversized, (byte) 1);
    expectIllegalArgument(
        () -> RegisterPinManifestInstruction.builder().setManifestPayload(oversized),
        "oversized raw manifest payload must fail");
    final String oversizedBase64 = Base64.getEncoder().encodeToString(oversized);
    expectIllegalArgument(
        () -> RegisterPinManifestInstruction.builder().setManifestPayloadBase64(oversizedBase64),
        "oversized base64 manifest payload must fail");
  }

  private static void defensivelyCopiesManifestPayload() {
    final byte[] source = Base64.getDecoder().decode(CANONICAL_MANIFEST_BASE64);
    final byte[] expected = Arrays.copyOf(source, source.length);
    final RegisterPinManifestInstruction instruction =
        RegisterPinManifestInstruction.builder()
            .setManifestPayload(source)
            .setSubmittedEpoch(1)
            .build();
    Arrays.fill(source, (byte) 0);
    assert Arrays.equals(expected, instruction.manifestPayloadBytes());
    final byte[] returned = instruction.manifestPayloadBytes();
    Arrays.fill(returned, (byte) 0);
    assert Arrays.equals(expected, instruction.manifestPayloadBytes());
  }

  private static void rejectsMissingRequiredFields() {
    expectIllegalState(
        () ->
            RegisterPinManifestInstruction.builder()
                .setSubmittedEpoch(1)
                .build(),
        "missing manifest payload must fail");
    expectIllegalState(
        () ->
            RegisterPinManifestInstruction.builder()
                .setManifestPayloadBase64(CANONICAL_MANIFEST_BASE64)
                .build(),
        "missing submitted epoch must fail");
  }

  private static void rejectsMalformedSuccessorDigest() {
    for (final String digest :
        new String[] {
          repeat("00", 32),
          repeat("c1", 31),
          repeat("C1", 32),
          "0x" + repeat("c1", 32)
        }) {
      expectIllegalArgument(
          () -> RegisterPinManifestInstruction.builder().setSuccessorOfHex(digest),
          "successor digest must be canonical nonzero 32-byte hex");
    }
  }

  private static void rejectsNegativeSubmittedEpoch() {
    expectIllegalArgument(
        () -> RegisterPinManifestInstruction.builder().setSubmittedEpoch(-1),
        "negative submitted epoch must fail");
    final Map<String, String> arguments = baseArguments();
    arguments.put("submitted_epoch", "NaN");
    expectIllegalArgument(
        () -> RegisterPinManifestInstruction.fromArguments(arguments),
        "nonnumeric submitted epoch must fail");
  }

  private static void rejectsPartialAliasBinding() {
    final Map<String, String> arguments = baseArguments();
    arguments.put("alias.name", "docs");
    expectIllegalArgument(
        () -> RegisterPinManifestInstruction.fromArguments(arguments),
        "partial alias binding must fail");
  }

  private static void rejectsMalformedAndOversizedAliasProof() {
    expectIllegalArgument(
        () ->
            RegisterPinManifestInstruction.AliasBinding.builder()
                .setName("docs")
                .setNamespace("sora")
                .setProofHex("A1"),
        "uppercase alias proof must fail");
    expectIllegalArgument(
        () ->
            RegisterPinManifestInstruction.AliasBinding.builder()
                .setName(" docs")
                .setNamespace("sora")
                .setProofHex("a1"),
        "padded alias name must fail");
    for (final String name : new String[] {"Docs", "main site", "máin", repeat("a", 129)}) {
      expectIllegalArgument(
          () -> RegisterPinManifestInstruction.AliasBinding.builder().setName(name),
          "noncanonical alias name must fail");
    }
    final String oversized = repeat("aa", 1024 * 1024 + 1);
    expectIllegalArgument(
        () ->
            RegisterPinManifestInstruction.AliasBinding.builder()
                .setName("docs")
                .setNamespace("sora")
                .setProofHex(oversized),
        "oversized alias proof must fail");
  }

  private static void rejectsLegacyAndUnknownArguments() {
    final Map<String, String> legacy = baseArguments();
    legacy.put("digest_hex", repeat("a0", 32));
    expectIllegalArgument(
        () -> RegisterPinManifestInstruction.fromArguments(legacy),
        "legacy digest field must fail");

    final Map<String, String> retiredChunkDigest = baseArguments();
    retiredChunkDigest.put("chunk_digest_sha3_256_hex", repeat("b0", 32));
    expectIllegalArgument(
        () -> RegisterPinManifestInstruction.fromArguments(retiredChunkDigest),
        "retired out-of-band chunk digest must fail");

    final Map<String, String> wrongAction = baseArguments();
    wrongAction.put("action", "ApprovePinManifest");
    expectIllegalArgument(
        () -> RegisterPinManifestInstruction.fromArguments(wrongAction),
        "wrong action must fail");

    for (final String key :
        new String[] {"action", "manifest_payload_base64", "submitted_epoch"}) {
      final Map<String, String> missing = baseArguments();
      missing.remove(key);
      expectIllegalArgument(
          () -> RegisterPinManifestInstruction.fromArguments(missing),
          "missing required argument must fail");
    }
  }

  private static void roundTripsCanonicalArguments() {
    final RegisterPinManifestInstruction.AliasBinding alias =
        RegisterPinManifestInstruction.AliasBinding.builder()
            .setName("docs")
            .setNamespace("sora")
            .setProofHex("a1b2")
            .build();
    final RegisterPinManifestInstruction instruction =
        baseBuilder().setSuccessorOfHex(repeat("c1", 32)).setAliasBinding(alias).build();
    assert instruction.equals(
        RegisterPinManifestInstruction.fromArguments(instruction.toArguments()));
  }

  private static RegisterPinManifestInstruction.Builder baseBuilder() {
    return RegisterPinManifestInstruction.builder()
        .setManifestPayloadBase64(CANONICAL_MANIFEST_BASE64)
        .setSubmittedEpoch(1);
  }

  private static Map<String, String> baseArguments() {
    return new LinkedHashMap<>(baseBuilder().build().toArguments());
  }

  private static void expectIllegalArgument(final Runnable action, final String message) {
    boolean threw = false;
    try {
      action.run();
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : message;
  }

  private static void expectIllegalState(final Runnable action, final String message) {
    boolean threw = false;
    try {
      action.run();
    } catch (final IllegalStateException ex) {
      threw = true;
    }
    assert threw : message;
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder result = new StringBuilder(value.length() * count);
    for (int index = 0; index < count; index++) {
      result.append(value);
    }
    return result.toString();
  }

  private static Map<String, Object> loadFixture() throws Exception {
    final String relative =
        "specs/sdk/android/generated/fixtures/sorafs_register_pin_manifest_multi_peer_parity_v1.json";
    Path path = null;
    final Path[] candidates =
        new Path[] {
          Paths.get(relative),
          Paths.get("../" + relative),
          Paths.get("../../" + relative),
          Paths.get("../../../" + relative),
          Paths.get("../../../../" + relative)
        };
    for (final Path candidate : candidates) {
      if (Files.exists(candidate)) {
        path = candidate;
        break;
      }
    }
    if (path == null) {
      throw new IllegalStateException("Fixture not found: " + Paths.get(relative).toAbsolutePath());
    }
    final String json =
        new String(Files.readAllBytes(path.toAbsolutePath()), StandardCharsets.UTF_8);
    return asMap(SimpleJson.parse(json), "root");
  }

  private static Map<String, Object> asMap(final Object value, final String field) {
    if (!(value instanceof Map)) {
      throw new IllegalStateException("Expected object for " + field);
    }
    final Map<?, ?> raw = (Map<?, ?>) value;
    final Map<String, Object> copy = new LinkedHashMap<>();
    for (final Map.Entry<?, ?> entry : raw.entrySet()) {
      copy.put(Objects.toString(entry.getKey()), entry.getValue());
    }
    return copy;
  }

  private static String requireString(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    if (!(value instanceof String) || ((String) value).isEmpty()) {
      throw new IllegalStateException("Field '" + key + "' must be a non-empty string");
    }
    return (String) value;
  }

  private static long requireNumber(final Object value) {
    if (value instanceof Number) {
      return ((Number) value).longValue();
    }
    throw new IllegalStateException("Expected numeric value, found: " + value);
  }
}
