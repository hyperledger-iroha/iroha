package org.hyperledger.iroha.android.offline;

import java.nio.file.Files;
import java.nio.file.Path;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;

public final class AndroidProvisionedProofTest {

  private AndroidProvisionedProofTest() {}

  public static void main(final String[] args) throws Exception {
    loadsFixtureAndNormalisesFields();
    rejectsManifestsMissingDeviceId();
    rejectsInvalidHashLiteral();
    rejectsFractionalIssuedAt();
    dropsFractionalManifestVersion();
    System.out.println("[IrohaAndroid] AndroidProvisionedProofTest passed.");
  }

  private static void loadsFixtureAndNormalisesFields() throws Exception {
    final Path fixture = fixturePath();
    final AndroidProvisionedProof proof = AndroidProvisionedProof.fromPath(fixture);
    assert "offline_provisioning_v1".equals(proof.manifestSchema()) : "schema mismatch";
    assert proof.manifestVersion() != null && proof.manifestVersion() == 1 : "version mismatch";
    assert proof.manifestIssuedAtMs() == 1_730_314_876_000L : "issued_at mismatch";
    assert proof.counter() == 7 : "counter mismatch";
    assert "retail-device-001".equals(proof.deviceId()) : "device id mismatch";
    assert proof.challengeHashLiteral()
        .equals("hash:E8A8D90BF72F280BBB4AB6D1F759521D29A08DA83CCFBB3E2EE0EDE22606FB9B#E0A4")
        : "challenge hash mismatch";
    assert proof.inspectorSignatureHex()
        .equals(
            "FEFCA2274314E692ED7244C613DA012E5D8AF7BF4B2AB236D1FF91A90F300935EC96EF0090B324C79A3F68AD513E17567242C42D50DC2377494A8071FDA49F0E")
        : "signature mismatch";
    assert proof.challengeHashBytes().length == 32 : "decoded hash length";
    assert proof.inspectorSignature().length == 64 : "decoded signature length";

    final String canonical = proof.toCanonicalJson();
    final Object parsedOriginal = JsonParser.parse(Files.readString(fixture));
    final String expected = JsonEncoder.encode(parsedOriginal);
    assert canonical.equals(expected) : "canonical JSON mismatch";
  }

  private static void rejectsManifestsMissingDeviceId() {
    final String json =
        "{"
            + "\"manifest_schema\":\"offline_provisioning_v1\"," 
            + "\"manifest_issued_at_ms\":1,"
            + "\"challenge_hash\":\"hash:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA#0E5B\"," 
            + "\"counter\":1,"
            + "\"device_manifest\":{\"android.provisioned.audit_ticket\":\"1234\"},"
            + "\"inspector_signature\":\""
            + "A".repeat(128)
            + "\"}";
    try {
      AndroidProvisionedProof.fromJson(json);
      throw new AssertionError("expected IllegalStateException");
    } catch (final IllegalStateException expected) {
      // expected
    }
  }

  private static void rejectsInvalidHashLiteral() {
    final String json =
        "{"
            + "\"manifest_schema\":\"offline_provisioning_v1\"," 
            + "\"manifest_issued_at_ms\":1,"
            + "\"challenge_hash\":\"deadbeef\"," 
            + "\"counter\":1,"
            + "\"device_manifest\":{\"android.provisioned.device_id\":\"demo\"},"
            + "\"inspector_signature\":\""
            + "A".repeat(128)
            + "\"}";
    try {
      AndroidProvisionedProof.fromJson(json);
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
  }

  private static void rejectsFractionalIssuedAt() {
    final String json =
        "{"
            + "\"manifest_schema\":\"offline_provisioning_v1\"," 
            + "\"manifest_issued_at_ms\":1.5,"
            + "\"challenge_hash\":\"hash:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA#0E5B\"," 
            + "\"counter\":1,"
            + "\"device_manifest\":{\"android.provisioned.device_id\":\"demo\"},"
            + "\"inspector_signature\":\""
            + "A".repeat(128)
            + "\"}";
    try {
      AndroidProvisionedProof.fromJson(json);
      throw new AssertionError("expected IllegalStateException");
    } catch (final IllegalStateException expected) {
      // expected
    }
  }

  private static void dropsFractionalManifestVersion() {
    final String json =
        "{"
            + "\"manifest_schema\":\"offline_provisioning_v1\"," 
            + "\"manifest_version\":1.5,"
            + "\"manifest_issued_at_ms\":1,"
            + "\"challenge_hash\":\"hash:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA#0E5B\"," 
            + "\"counter\":1,"
            + "\"device_manifest\":{\"android.provisioned.device_id\":\"demo\"},"
            + "\"inspector_signature\":\""
            + "A".repeat(128)
            + "\"}";
    final AndroidProvisionedProof proof = AndroidProvisionedProof.fromJson(json);
    assert proof.manifestVersion() == null : "expected fractional manifest_version to be dropped";
  }

  private static Path fixturePath() {
    final String[] segments = {"fixtures", "offline_provision", "kiosk-demo", "proof.json"};
    final Path[] candidates =
        new Path[] {
          Path.of(segments[0], segments[1], segments[2], segments[3]),
          Path.of("..", segments[0], segments[1], segments[2], segments[3]),
          Path.of("..", "..", segments[0], segments[1], segments[2], segments[3]),
          Path.of("..", "..", "..", segments[0], segments[1], segments[2], segments[3]),
        };
    for (final Path candidate : candidates) {
      final Path resolved = candidate.toAbsolutePath().normalize();
      if (Files.exists(resolved)) {
        return resolved;
      }
    }
    throw new IllegalStateException(
        "Fixture not found under fixtures/offline_provision/kiosk-demo/proof.json");
  }
}
