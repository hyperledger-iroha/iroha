package org.hyperledger.iroha.android.nexus;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.instructions.ExpireSpaceDirectoryManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.PublishSpaceDirectoryManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.RevokeSpaceDirectoryManifestInstruction;

/** Ensures the Space Directory manifest instructions emit the expected argument schema. */
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
    final Map<String, String> publishArgs = publish.toArguments();
    assert "PublishSpaceDirectoryManifest".equals(publishArgs.get("action"))
        : "action mismatch for publish";
    assert Objects.equals(manifestJson, publishArgs.get("manifest_json"))
        : "manifest_json mismatch";
    assert Objects.equals(manifestJson, publish.manifestJson()) : "manifestJson getter mismatch";

    final String reason = "Emergency deny request #INC-2026-42";
    final RevokeSpaceDirectoryManifestInstruction revoke =
        RevokeSpaceDirectoryManifestInstruction.builder()
            .setUaid(UAID)
            .setDataspace(DATASPACE)
            .setRevokedEpoch(4610)
            .setReason(reason)
            .build();
    final Map<String, String> revokeArgs = revoke.toArguments();
    assert "RevokeSpaceDirectoryManifest".equals(revokeArgs.get("action"))
        : "action mismatch for revoke";
    assert Objects.equals(Long.toString(DATASPACE), revokeArgs.get("dataspace"))
        : "dataspace mismatch";
    assert reason.equals(revokeArgs.get("reason")) : "reason mismatch";
    assert DATASPACE == revoke.dataspace() : "dataspace getter mismatch";
    assert 4610L == revoke.revokedEpoch() : "revokedEpoch getter mismatch";
    assert reason.equals(revoke.reason()) : "Decoded reason mismatch";

    final ExpireSpaceDirectoryManifestInstruction expire =
        ExpireSpaceDirectoryManifestInstruction.builder()
            .setUaid(UAID)
            .setDataspace(DATASPACE)
            .setExpiredEpoch(4600)
            .build();
    final Map<String, String> expireArgs = expire.toArguments();
    assert "ExpireSpaceDirectoryManifest".equals(expireArgs.get("action"))
        : "action mismatch for expire";
    assert "4600".equals(expireArgs.get("expired_epoch"))
        : "expired_epoch mismatch";
    assert 4600L == expire.expiredEpoch() : "expiredEpoch getter mismatch";

    System.out.println(
        "[IrohaAndroid] SpaceDirectoryInstructionBuilderTests passed (publish/revoke/expire).");
  }

  private static String loadManifestJson() throws Exception {
    final String relative =
        "fixtures/space_directory/capability/cbdc_wholesale.manifest.json";
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
    return Files.readString(path.toAbsolutePath(), StandardCharsets.UTF_8).trim();
  }
}
