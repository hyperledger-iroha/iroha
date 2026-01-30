package org.hyperledger.iroha.android.sorafs;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.RegisterPinManifestInstruction;
import org.hyperledger.iroha.android.testing.SimpleJson;

/**
 * Ensures {@link RegisterPinManifestInstruction} mirrors the tracked SoraFS fixture and
 * serializes into the expected argument schema.
 */
public final class SorafsRegisterPinManifestBuilderTests {

  private SorafsRegisterPinManifestBuilderTests() {}

  public static void main(final String[] args) throws Exception {
    rejectsNegativeSubmittedEpoch();
    rejectsNegativeRetentionEpoch();
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> instruction = asMap(fixture.get("instruction"), "instruction");

    final RegisterPinManifestInstruction payload = buildInstruction(instruction);
    final InstructionBox box = payload.toInstructionBox();

    assert Objects.equals(
            box.arguments().get("digest_hex"), requireString(instruction, "digest_hex"))
        : "digest_hex mismatch";
    assert Objects.equals(
            box.arguments().get("policy.storage_class"),
            requireString(asMap(instruction.get("policy"), "policy"), "storage_class"))
        : "policy.storage_class mismatch";

    System.out.println(
        "[IrohaAndroid] SorafsRegisterPinManifestBuilderTests passed (" + box.arguments().size()
            + " arguments).");
  }

  private static void rejectsNegativeSubmittedEpoch() {
    boolean threw = false;
    try {
      RegisterPinManifestInstruction.builder()
          .setDigestHex("a0".repeat(32))
          .setChunkDigestSha3Hex("b0".repeat(32))
          .setSubmittedEpoch(-1);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected negative submitted epoch to throw";
  }

  private static void rejectsNegativeRetentionEpoch() {
    boolean threw = false;
    try {
      RegisterPinManifestInstruction.PinPolicy.builder()
          .setMinReplicas(1)
          .setStorageClass("hot")
          .setRetentionEpoch(-1);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected negative retention epoch to throw";
  }

  private static RegisterPinManifestInstruction buildInstruction(
      final Map<String, Object> instruction) {
    final Map<String, Object> policyMap = asMap(instruction.get("policy"), "policy");
    final RegisterPinManifestInstruction.PinPolicy policy =
        RegisterPinManifestInstruction.PinPolicy.builder()
            .setMinReplicas((int) requireNumber(policyMap.get("min_replicas")))
            .setStorageClass(requireString(policyMap, "storage_class"))
            .setRetentionEpoch(requireNumber(policyMap.get("retention_epoch")))
            .build();

    final Map<String, Object> chunker = asMap(instruction.get("chunker"), "chunker");
    final RegisterPinManifestInstruction.ChunkerProfile chunkerProfile =
        RegisterPinManifestInstruction.ChunkerProfile.builder()
            .setProfileId((int) requireNumber(chunker.get("profile_id")))
            .setNamespace(requireString(chunker, "namespace"))
            .setName(requireString(chunker, "name"))
            .setSemver(requireString(chunker, "semver"))
            .setHandle((String) chunker.get("handle"))
            .setMultihashCode(requireNumber(chunker.get("multihash_code")))
            .build();

    final RegisterPinManifestInstruction.Builder builder =
        RegisterPinManifestInstruction.builder()
            .setDigestHex(requireString(instruction, "digest_hex"))
            .setChunkDigestSha3Hex(requireString(instruction, "chunk_digest_sha3_256_hex"))
            .setSubmittedEpoch(requireNumber(instruction.get("submitted_epoch")))
            .setPinPolicy(policy)
            .setChunkerProfile(chunkerProfile);

    final Object aliasValue = instruction.get("alias");
    if (aliasValue instanceof Map<?, ?> aliasMapRaw) {
      final Map<String, Object> aliasMap = asMap(aliasMapRaw, "alias");
      builder.setAliasBinding(
          RegisterPinManifestInstruction.AliasBinding.builder()
              .setName(requireString(aliasMap, "name"))
              .setNamespace(requireString(aliasMap, "namespace"))
              .setProofHex(requireString(aliasMap, "proof_hex"))
              .build());
    }

    return builder.build();
  }

  private static Map<String, Object> loadFixture() throws Exception {
    final String relative =
        "docs/source/sdk/android/generated/fixtures/sorafs_register_pin_manifest_multi_peer_parity_v1.json";
    Path path = null;
    final Path[] candidates =
        new Path[] {
          Path.of(relative),
          Path.of("../" + relative),
          Path.of("../../" + relative),
          Path.of("../../../" + relative),
          Path.of("../../../../" + relative)
        };
    for (final Path candidate : candidates) {
      if (Files.exists(candidate)) {
        path = candidate;
        break;
      }
    }
    if (path == null) {
      throw new IllegalStateException(
          "Fixture not found: " + Path.of(relative).toAbsolutePath());
    }
    final String json = Files.readString(path.toAbsolutePath(), StandardCharsets.UTF_8);
    return asMap(SimpleJson.parse(json), "root");
  }

  private static Map<String, Object> asMap(final Object value, final String field) {
    if (!(value instanceof Map<?, ?> raw)) {
      throw new IllegalStateException("Expected object for " + field);
    }
    final Map<String, Object> copy = new LinkedHashMap<>();
    raw.forEach((key, v) -> copy.put(Objects.toString(key), v));
    return copy;
  }

  private static String requireString(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    if (!(value instanceof String str) || str.isBlank()) {
      throw new IllegalStateException("Field '" + key + "' must be a non-empty string");
    }
    return str;
  }

  private static long requireNumber(final Object value) {
    if (value instanceof Number number) {
      return number.longValue();
    }
    throw new IllegalStateException("Expected numeric value, found: " + value);
  }
}
