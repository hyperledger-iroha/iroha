package org.hyperledger.iroha.android.nexus;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.ExpireSpaceDirectoryManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.InstructionBuilders;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
import org.hyperledger.iroha.android.model.instructions.PublishSpaceDirectoryManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.RevokeSpaceDirectoryManifestInstruction;

/** Ensures the Space Directory manifest instructions round-trip through InstructionBox. */
public final class SpaceDirectoryInstructionBuilderTests {

  private static final String UAID =
      "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";
  private static final long DATASPACE = 11L;

  private SpaceDirectoryInstructionBuilderTests() {}

  public static void main(final String[] args) throws Exception {
    final String manifestJson = loadManifestJson();

    final PublishSpaceDirectoryManifestInstruction publish =
        PublishSpaceDirectoryManifestInstruction.builder()
            .setManifestJson(manifestJson)
            .build();
    final InstructionBox publishBox =
        InstructionBuilders.publishSpaceDirectoryManifest(publish);
    assert "PublishSpaceDirectoryManifest".equals(publishBox.arguments().get("action"))
        : "action mismatch for publish";
    assert Objects.equals(manifestJson, publishBox.arguments().get("manifest_json"))
        : "manifest_json mismatch";
    final InstructionBox publishDecoded =
        InstructionBox.fromNorito(InstructionKind.CUSTOM, publishBox.arguments());
    assert publishDecoded.payload() instanceof PublishSpaceDirectoryManifestInstruction
        : "Expected PublishSpaceDirectoryManifestInstruction payload";
    final PublishSpaceDirectoryManifestInstruction publishPayload =
        (PublishSpaceDirectoryManifestInstruction) publishDecoded.payload();
    assert Objects.equals(manifestJson, publishPayload.manifestJson())
        : "manifestJson getter mismatch";

    final String reason = "Emergency deny request #INC-2026-42";
    final RevokeSpaceDirectoryManifestInstruction revoke =
        RevokeSpaceDirectoryManifestInstruction.builder()
            .setUaid(UAID)
            .setDataspace(DATASPACE)
            .setRevokedEpoch(4610)
            .setReason(reason)
            .build();
    final InstructionBox revokeBox =
        InstructionBuilders.revokeSpaceDirectoryManifest(revoke);
    assert "RevokeSpaceDirectoryManifest".equals(revokeBox.arguments().get("action"))
        : "action mismatch for revoke";
    assert Objects.equals(Long.toString(DATASPACE), revokeBox.arguments().get("dataspace"))
        : "dataspace mismatch";
    assert reason.equals(revokeBox.arguments().get("reason")) : "reason mismatch";
    final InstructionBox revokeDecoded =
        InstructionBox.fromNorito(InstructionKind.CUSTOM, revokeBox.arguments());
    assert revokeDecoded.payload() instanceof RevokeSpaceDirectoryManifestInstruction
        : "Expected RevokeSpaceDirectoryManifestInstruction payload";
    final RevokeSpaceDirectoryManifestInstruction revokePayload =
        (RevokeSpaceDirectoryManifestInstruction) revokeDecoded.payload();
    assert DATASPACE == revokePayload.dataspace() : "dataspace getter mismatch";
    assert 4610L == revokePayload.revokedEpoch() : "revokedEpoch getter mismatch";
    assert reason.equals(revokePayload.reason()) : "Decoded reason mismatch";

    final InstructionBox expireBox =
        InstructionBuilders.expireSpaceDirectoryManifest(
            ExpireSpaceDirectoryManifestInstruction.builder()
                .setUaid(UAID)
                .setDataspace(DATASPACE)
                .setExpiredEpoch(4600)
                .build());
    assert "ExpireSpaceDirectoryManifest".equals(expireBox.arguments().get("action"))
        : "action mismatch for expire";
    assert "4600".equals(expireBox.arguments().get("expired_epoch"))
        : "expired_epoch mismatch";
    final InstructionBox expireDecoded =
        InstructionBox.fromNorito(InstructionKind.CUSTOM, expireBox.arguments());
    assert expireDecoded.payload() instanceof ExpireSpaceDirectoryManifestInstruction
        : "Expected ExpireSpaceDirectoryManifestInstruction payload";
    final ExpireSpaceDirectoryManifestInstruction expirePayload =
        (ExpireSpaceDirectoryManifestInstruction) expireDecoded.payload();
    assert 4600L == expirePayload.expiredEpoch() : "expiredEpoch getter mismatch";

    System.out.println(
        "[IrohaAndroid] SpaceDirectoryInstructionBuilderTests passed (publish/revoke/expire).");
  }

  private static String loadManifestJson() throws Exception {
    final String relative =
        "fixtures/space_directory/capability/cbdc_wholesale.manifest.json";
    Path path = null;
    final Path[] candidates =
        new Path[] {Path.of(relative), Path.of("../" + relative), Path.of("../../" + relative)};
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
    return Files.readString(path.toAbsolutePath(), StandardCharsets.UTF_8).trim();
  }
}
