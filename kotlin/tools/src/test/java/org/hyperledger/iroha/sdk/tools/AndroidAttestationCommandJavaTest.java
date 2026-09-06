package org.hyperledger.iroha.sdk.tools;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import static org.junit.jupiter.api.Assertions.*;
import org.hyperledger.iroha.sdk.client.JsonParser;

import java.io.ByteArrayInputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardCopyOption;
import java.security.cert.CertificateFactory;
import java.security.cert.X509Certificate;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.stream.Stream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationResult;
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationVerificationException;

/** Java consumers retain every assertion from the retired Java attestation harness suite. */
final class AndroidAttestationCommandJavaTest {
  @TempDir static Path temporaryRoot;

  private static final byte[] ROOT_CERT = decodeBase64(
      "MIIDIjCCAgqgAwIBAgIUHifREEUziVTjk5SY9EdEKBhj+LAwDQYJKoZIhvcNAQELBQAwFzEVMBMGA1UE"
          + "AwwMVGVzdCBSb290IENBMB4XDTI1MTAyNTE1Mjc0M1oXDTM1MTAyMzE1Mjc0M1owFzEVMBMGA1UEAwwM"
          + "VGVzdCBSb290IENBMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA4cr8VyFyforGk8BkefC2"
          + "jy36UydWa50h/9tCGhx+JeYpsmNE050wPQTZJ+09vTjZN9N2dO/Bh8TGd4nIW5D+swmXrsnzyt9fpMMR"
          + "PrDpmXTAvaDdD+afCgTRkEasSb7wGNh7wtgUvP5aQnTRFHEPN8VVn31ndv093Ex84PvKgQt3SYQuW+ho"
          + "zw1TZyAhjc4ydGTX3szxx1SJNtnxCBWAspaCKVXo4vCgSHUO6/JXW8BfaCckAniGqrNySk35POmmlw70"
          + "oj0zuoqoeWygwZVnGXMAvkN6gVmW/OY18cvAhZHlLfJG0P/o+i7DTpllebDM6W7ILF+YTxEXrfi2ixdw"
          + "QwIDAQABo2YwZDAdBgNVHQ4EFgQUAiNcsp2ChOMGPTVGbslvK4wPnVQwHwYDVR0jBBgwFoAUAiNcsp2C"
          + "hOMGPTVGbslvK4wPnVQwEgYDVR0TAQH/BAgwBgEB/wIBATAOBgNVHQ8BAf8EBAMCAYYwDQYJKoZIhvcN"
          + "AQELBQADggEBAH1/kr4JUjckOxPIR0XdZE73Wwr4DXqCb/InpBs+2TJJPnXONpuwNtLPtFUyV9FuJ9qM"
          + "H+M2aGu3+enncDnaw8ChAPKn9+QmjgTrZk9sPQV9zi6coIrMqD67gMwJW7HE0YDem7pNpiN1l/VvDrwe"
          + "V/2QJu7Og+rDvVc48TIhVeTEaQLURsgwi2R8U/usieuDysfPq7OJm/1eu8pE+etK5GiR9t/24qfx8V8d"
          + "DVliRz7PjoxZoDZrgpJl94nq5665BpXQ5lbsrr22EFgqxkMs1nPNIUFVxgEZUPnOzPVGPEOefnSjuKxT"
          + "AR7INRwTwVOtoGf0swuwJo3VZHgfAcaLfLM=");

  private static final byte[] STRONGBOX_CERT = decodeBase64(
      "MIIDUTCCAjmgAwIBAgIULKS+BqcxAYB6ooMNchJ4LI59fxowDQYJKoZIhvcNAQELBQAwFzEVMBMGA1UE"
          + "AwwMVGVzdCBSb290IENBMB4XDTI1MTAyNTE1MjgwMFoXDTI2MTAyNTE1MjgwMFowHzEdMBsGA1UEAwwU"
          + "VGVzdCBBdHRlc3RhdGlvbiBLZXkwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQCbQVFuKFDD"
          + "6t52BMS3ZVot+5OPrSIcXlY1xRgXJoh+yhmXjfc5UIBgjWyNuLWyaT8N6+iVUNqLsh7Nbow8ySi1vgWI"
          + "56OVhc4yLf6z2kbwTqJScHwQbphed/wLA3I0tkzu1E0zt3AqNsPlEEMiZYHe3PBbvLBrx+Ug+UsPe0uZ"
          + "UxU5l6fDd9MeWihEvnOCWX1Fi9D4IfOeNq1UiZlkzih97JhqEWx32FVyxOdM2gx/VySv6R4KGu3nVRzA"
          + "cl4Lgw2Zex81/x9TKu5Mnf+Sz+sYtPLfS+D7R5xHI/GZPZ/SHZ8g79dm0o6D/5S1B29kolGMAnnbLN3H"
          + "ym7WJm9tVf3zAgMBAAGjgYwwgYkwHQYDVR0OBBYEFAL9ObHQIHwRJ2kTOTzbnwzAM9o0MB8GA1UdIwQY"
          + "MBaAFAIjXLKdgoTjBj01Rm7JbyuMD51UMAkGA1UdEwQCMAAwCwYDVR0PBAQDAgeAMC8GCisGAQQB1nkC"
          + "AREEITAfAgEDCgECAgEECgECBAVBRUVCRQQEAQIDBDAAMAAwADANBgkqhkiG9w0BAQsFAAOCAQEAE+vf"
          + "oKnq0xblVQmxeT8IjRRqzFnIpa7Fd92xoGSydhNwV1Ox29rPOkOthq3om/r03rETj07LbArH8iyfCs5m"
          + "cSrfWC+kELgKuWEVYs7Zi20UanZsV7lnYXaqTKt8uPLh4TDRbZ6ymRi5ionLJ8vu8cfEyAVCKmn983Kr"
          + "bMgwIYzmWPMPnp+oCJ/TXOLQjTgbmcP3QmXPs7BBjdasixlvmBForI08Y5qClDZMOqBf/l5xQi4IeLr9"
          + "Q3mFG3KuAmuoZKvKN6TAvY5Hleqy9pg4gKSB7/0wK5lfX/JfkLi6erS5l8VuED6OcOZc3VbO8OrwRdlP"
          + "FxdGTgtauVtYo24deQ==");

  private static final byte[] STRONGBOX_CHALLENGE = decodeHex("4145454245");
  private static final String EVALUATION_TIME_MILLIS = "1764547200000";

  @Test
  void verifiesStrongBoxBundle() throws Exception {
    final Path tempDir = Files.createTempDirectory(temporaryRoot, "attestation-harness-test");
    final Path bundleDir = tempDir.resolve("bundle");
    Files.createDirectories(bundleDir);

    // Create attestation chain (leaf + root).
    final Path chainPem = bundleDir.resolve("chain.pem");
    final String pem = toPem("CERTIFICATE", STRONGBOX_CERT) + toPem("CERTIFICATE", ROOT_CERT);
    writeString(chainPem, pem, StandardCharsets.US_ASCII);

    // Alias & challenge helpers.
    writeString(bundleDir.resolve("alias.txt"), "strongbox-alias", StandardCharsets.UTF_8);
    writeString(
        bundleDir.resolve("challenge.hex"), "4145454245", StandardCharsets.UTF_8);

    final Path rootPem = tempDir.resolve("root.pem");
    writeString(rootPem, toPem("CERTIFICATE", ROOT_CERT), StandardCharsets.US_ASCII);

    final Path output = tempDir.resolve("result.json");
    final AndroidAttestationCommand.Result result =
        AndroidAttestationCommand.run(
            withGovernedRevocation(new String[] {
              "--bundle-dir", bundleDir.toString(),
              "--trust-root", rootPem.toString(),
              "--require-strongbox",
              "--output", output.toString()
            }));

    assertTrue("strongbox-alias".equals(result.getAlias()), "Alias should match the separately supplied fixture expectation");
    assertTrue(result.getStrongBoxAttestation(), "StrongBox attestation expected");
    assertTrue(result.getAttestationSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX);
    assertTrue(result.getKeymasterSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX);
    assertTrue(result.getChainLength() == 2, "Should report leaf + root");
    assertTrue(result.getChallengeHex().equals("4145454245"), "Challenge hex should round-trip");

    final String json = readString(output, StandardCharsets.UTF_8);
    assertEquals("strongbox-alias", ((java.util.Map<?, ?>) JsonParser.parse(json)).get("alias"));
    assertEquals(Boolean.TRUE, ((java.util.Map<?, ?>) JsonParser.parse(json)).get("strongbox_attestation"));
  }

  @Test
  void verifiesBundleUsingTrustedRootDirectory() throws Exception {
    final Path tempDir = Files.createTempDirectory(temporaryRoot, "attestation-harness-dir-test");
    final Path bundleDir = tempDir.resolve("bundle-dir");
    Files.createDirectories(bundleDir);

    final Path chainPem = bundleDir.resolve("chain.pem");
    final String pem = toPem("CERTIFICATE", STRONGBOX_CERT) + toPem("CERTIFICATE", ROOT_CERT);
    writeString(chainPem, pem, StandardCharsets.US_ASCII);

    writeString(bundleDir.resolve("alias.txt"), "directory-alias", StandardCharsets.UTF_8);
    writeString(bundleDir.resolve("challenge.hex"), "4145454245", StandardCharsets.UTF_8);

    final Path rootsDir = tempDir.resolve("roots");
    final Path nestedDir = rootsDir.resolve("vendor");
    Files.createDirectories(nestedDir);
    writeString(
        nestedDir.resolve("trust_root_vendor.pem"),
        toPem("CERTIFICATE", ROOT_CERT),
        StandardCharsets.US_ASCII);

    final AndroidAttestationCommand.Result result =
        AndroidAttestationCommand.run(
            withGovernedRevocation(new String[] {
              "--bundle-dir", bundleDir.toString(),
              "--trust-root-dir", rootsDir.toString(),
              "--require-strongbox"
            }));

    assertTrue("directory-alias".equals(result.getAlias()), "Alias should match the separately supplied fixture expectation when using trust-root directories");
    assertTrue(result.getStrongBoxAttestation(), "StrongBox attestation expected when using directories");
    assertTrue(result.getAttestationSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX);
    assertTrue(result.getKeymasterSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX);
    assertTrue(result.getChainLength() == 2, "Directory-based trust root should still report leaf + root");
  }

  @Test
  void verifiesBundleUsingTrustRootDirectoryZipOnly() throws Exception {
    final Path tempDir = Files.createTempDirectory(temporaryRoot, "attestation-harness-dir-zip-test");
    final Path bundleDir = tempDir.resolve("bundle-dir-zip");
    Files.createDirectories(bundleDir);

    final Path chainPem = bundleDir.resolve("chain.pem");
    final String pem = toPem("CERTIFICATE", STRONGBOX_CERT) + toPem("CERTIFICATE", ROOT_CERT);
    writeString(chainPem, pem, StandardCharsets.US_ASCII);

    writeString(bundleDir.resolve("alias.txt"), "directory-zip-alias", StandardCharsets.UTF_8);
    writeString(bundleDir.resolve("challenge.hex"), "4145454245", StandardCharsets.UTF_8);

    final Path rootsDir = tempDir.resolve("roots-zip-only");
    Files.createDirectories(rootsDir);
    final Path bundleZip = rootsDir.resolve("alt_vendor_roots.zip");
    try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(bundleZip))) {
      zip.putNextEntry(new ZipEntry("nested/trust_root_alt.der"));
      zip.write(ROOT_CERT);
      zip.closeEntry();
    }

    final AndroidAttestationCommand.Result result =
        AndroidAttestationCommand.run(
            withGovernedRevocation(new String[] {
              "--bundle-dir", bundleDir.toString(),
              "--trust-root-dir", rootsDir.toString(),
              "--require-strongbox"
            }));

    assertTrue("directory-zip-alias".equals(result.getAlias()), "Alias should match the separately supplied fixture expectation when scanning trust-root ZIP directories");
    assertTrue(result.getStrongBoxAttestation(), "Directory ZIP bundles should keep StrongBox classification");
    assertTrue(result.getAttestationSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX);
    assertTrue(result.getChainLength() == 2, "ZIP directories should deliver the same chain depth");
    assertTrue(result.getChallengeHex().equals("4145454245"), "Challenge should remain tied to bundle contents when roots are discovered via ZIPs");
  }

  @Test
  void verifiesBundleUsingTrustRootZip() throws Exception {
    final Path tempDir = Files.createTempDirectory(temporaryRoot, "attestation-harness-zip-test");
    final Path bundleDir = tempDir.resolve("bundle-zip");
    Files.createDirectories(bundleDir);

    final Path chainPem = bundleDir.resolve("chain.pem");
    final String pem = toPem("CERTIFICATE", STRONGBOX_CERT) + toPem("CERTIFICATE", ROOT_CERT);
    writeString(chainPem, pem, StandardCharsets.US_ASCII);

    writeString(bundleDir.resolve("alias.txt"), "zip-alias", StandardCharsets.UTF_8);
    writeString(bundleDir.resolve("challenge.hex"), "4145454245", StandardCharsets.UTF_8);

    final Path bundleZip = tempDir.resolve("roots.zip");
    try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(bundleZip))) {
      zip.putNextEntry(new ZipEntry("trust_root_vendor.pem"));
      zip.write(toPem("CERTIFICATE", ROOT_CERT).getBytes(StandardCharsets.US_ASCII));
      zip.closeEntry();
    }

    final AndroidAttestationCommand.Result result =
        AndroidAttestationCommand.run(
            withGovernedRevocation(new String[] {
              "--bundle-dir", bundleDir.toString(),
              "--trust-root-bundle", bundleZip.toString(),
              "--require-strongbox"
            }));

    assertTrue("zip-alias".equals(result.getAlias()), "Alias should match the separately supplied fixture expectation when zipped");
    assertTrue(result.getChainLength() == 2, "Zip bundles should not change chain depth");
    assertTrue(result.getAttestationSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX);
  }

  @Test
  void verifiesMockHuaweiFixtureBundle() throws Exception {
    final Path bundleDir = fixtureBundle("mock_huawei");
    final AndroidAttestationCommand.Result result =
        AndroidAttestationCommand.run(
            withGovernedRevocation(new String[] {
              "--bundle-dir", bundleDir.toString(),
              "--require-strongbox"
            }));

    assertTrue("mock-huawei-strongbox".equals(result.getAlias()), "Fixture alias should match mock_huawei bundle");
    assertTrue(result.getStrongBoxAttestation(), "Mock Huawei bundle encodes a StrongBox attestation");
    assertTrue(result.getAttestationSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX);
    assertTrue(result.getKeymasterSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX);
    assertTrue(result.getChainLength() == 2, "Fixture chains contain leaf + root entries");
    assertArrayEquals(decodeHex(readChallengeHex(bundleDir)), decodeHex(result.getChallengeHex()), "Challenge bytes should match the fixture");
  }

  @Test
  void verifiesMockOspBundleZipOnly() throws Exception {
    final Path tempDir = copyFixtureBundle("mock_osp");
    final Path pem = tempDir.resolve("trust_root_osp.pem");
    Files.deleteIfExists(pem);

    final AndroidAttestationCommand.Result result =
        AndroidAttestationCommand.run(
            withGovernedRevocation(new String[] {
              "--bundle-dir", tempDir.toString(),
              "--trust-root-bundle", tempDir.resolve("trust_root_bundle_osp.zip").toString(),
            }));

    assertTrue("mock-osp-keymint".equals(result.getAlias()), "Fixture alias should match mock_osp bundle");
    assertTrue(result.getStrongBoxAttestation(), "Mock OSP bundle encodes a StrongBox/KeyMint attestation");
    assertTrue(result.getAttestationSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX);
    assertTrue(result.getKeymasterSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX);
    assertTrue(result.getChainLength() == 2, "Fixture chains contain leaf + root entries");
    assertArrayEquals(decodeHex(readChallengeHex(tempDir)), decodeHex(result.getChallengeHex()), "Challenge bytes should match the fixture");
  }

  @Test
  void rejectsMissingChallenge() throws Exception {
    final Path bundleDir = createStrongBoxBundle(false);
    assertHarnessFails(
        () ->
            AndroidAttestationCommand.run(
                withGovernedRevocation(
                    new String[] {"--bundle-dir", bundleDir.toString()})));
  }

  @Test
  void rejectsBundleControlledTrustAndIdentity() throws Exception {
    final Path bundleDir = createStrongBoxBundle(true);
    assertHarnessFails(
        () ->
            AndroidAttestationCommand.run(
                withGovernedRevocationOnly(
                    new String[] {"--bundle-dir", bundleDir.toString()})));
  }

  @Test
  void rejectsMismatchedLeafKeyCommitment() throws Exception {
    final Path bundleDir = createStrongBoxBundle(true);
    assertHarnessFails(
        () ->
            AndroidAttestationCommand.run(
                withGovernedRevocation(
                    new String[] {
                      "--bundle-dir", bundleDir.toString(),
                      "--expected-leaf-spki-sha256", repeat("22", 32)
                    })));
  }

  @Test
  void rejectsMissingRevocationSnapshot() throws Exception {
    String[] valid = withGovernedRevocation(new String[] {
        "--bundle-dir", createStrongBoxBundle(true).toString()
    });
    assertThrows(IllegalArgumentException.class, () -> AndroidAttestationCommand.run(
        removeOption(valid, "--revocation-snapshot")));
  }

  @Test
  void rejectsStaleRevocationSnapshot() throws Exception {
    final Path bundleDir = createStrongBoxBundle(true);
    assertHarnessFails(
        () ->
            AndroidAttestationCommand.run(
                withGovernedRevocation(
                    new String[] {"--bundle-dir", bundleDir.toString()},
                    "1764547199000",
                    "1",
                    EVALUATION_TIME_MILLIS)));
  }

  @Test
  void rejectsRevokedLeafCertificate() throws Exception {
    final Path bundleDir = createStrongBoxBundle(true);
    final X509Certificate leaf =
        (X509Certificate)
            CertificateFactory.getInstance("X.509")
                .generateCertificate(new ByteArrayInputStream(STRONGBOX_CERT));
    assertHarnessFails(
        () ->
            AndroidAttestationCommand.run(
                withGovernedRevocation(
                    new String[] {"--bundle-dir", bundleDir.toString()},
                    EVALUATION_TIME_MILLIS,
                    "86400",
                    EVALUATION_TIME_MILLIS,
                    Collections.singletonList(leaf.getSerialNumber().toString(16)))));
  }

  @Test
  void trustedSnapshotAndChallengeSubstitutionsAreRejected() throws Exception {
    String[] valid = withGovernedRevocation(new String[] {
        "--bundle-dir", createStrongBoxBundle(true).toString()
    });
    assertThrows(IllegalArgumentException.class, () -> AndroidAttestationCommand.run(
        replaceOption(valid, "--revocation-snapshot-sha256", repeat("22", 32))));
    assertThrows(AttestationVerificationException.class, () -> AndroidAttestationCommand.run(
        replaceOption(valid, "--challenge-hex", "00")));
  }

  @Test
  void evaluationTimeUsesTheHalfOpenSnapshotBoundary() throws Exception {
    String[] valid = withGovernedRevocation(new String[] {
        "--bundle-dir", createStrongBoxBundle(true).toString()
    });
    long start = Long.parseLong(EVALUATION_TIME_MILLIS);
    assertThrows(AttestationVerificationException.class, () -> AndroidAttestationCommand.run(
        replaceOption(valid, "--evaluation-time-ms", Long.toString(start - 1))));
    AndroidAttestationCommand.Result result = AndroidAttestationCommand.run(
        replaceOption(valid, "--evaluation-time-ms", Long.toString(start + 86400000L - 1)));
    assertEquals(start + 86400000L - 1, result.getEvaluationTimeMillis());
    assertThrows(AttestationVerificationException.class, () -> AndroidAttestationCommand.run(
        replaceOption(valid, "--evaluation-time-ms", Long.toString(start + 86400000L))));
  }

  @Test
  void explicitIdentityWinsOverEvidenceMetadataAndJsonBindsTrustedInputs() throws Exception {
    Path bundle = createStrongBoxBundle(true);
    Path output = bundle.getParent().resolve(bundle.getFileName() + "-result.json");
    String[] valid = withGovernedRevocation(new String[] {
        "--bundle-dir", bundle.toString(), "--alias", "trusted-\"alias", "--output", output.toString()
    });
    AndroidAttestationCommand.Result result = AndroidAttestationCommand.run(valid);
    assertEquals("trusted-\"alias", result.getAlias());
    String json = readString(output, StandardCharsets.UTF_8);
    assertEquals(result.toJson(), json);
    java.util.Map<?, ?> decoded = (java.util.Map<?, ?>) JsonParser.parse(json);
    assertEquals("iroha.android.attestation.verification.v1", decoded.get("schema"));
    assertEquals(result.getAlias(), decoded.get("alias"));
    assertEquals(result.getRevocationSnapshotSha256(), decoded.get("revocation_snapshot_sha256"));
    assertEquals(result.getLeafSpkiSha256(), decoded.get("leaf_spki_sha256"));
  }

  @Test
  void singletonAndMutuallyExclusiveInputsHaveNoOverridePath() throws Exception {
    Path bundle = createStrongBoxBundle(true);
    String[] valid = withGovernedRevocation(new String[] {"--bundle-dir", bundle.toString()});
    assertThrows(IllegalArgumentException.class, () -> AndroidAttestationCommand.run(
        appendOptions(valid, "--alias", "replacement")));
    assertThrows(IllegalArgumentException.class, () -> AndroidAttestationCommand.run(
        appendOptions(valid, "--chain", bundle.resolve("chain.pem").toString())));
    assertThrows(IllegalArgumentException.class, () -> AndroidAttestationCommand.run(
        appendOptions(valid, "--challenge-file", bundle.resolve("challenge.hex").toString())));
  }

  @Test
  void explicitChainAndChallengeFileUseTheCanonicalVerifier() throws Exception {
    Path bundle = createStrongBoxBundle(true);
    String[] valid = withGovernedRevocation(new String[] {
        "--chain", bundle.resolve("chain.pem").toString(),
        "--challenge-file", bundle.resolve("challenge.hex").toString()
    });
    assertEquals("4145454245", AndroidAttestationCommand.run(valid).getChallengeHex());
  }

  @Test
  void orderedDirectoryChainNeedsNoPemBundle() throws Exception {
    Path bundle = createStrongBoxBundle(true);
    String[] valid = withGovernedRevocation(new String[] {"--bundle-dir", bundle.toString()});
    Files.delete(bundle.resolve("chain.pem"));
    Files.delete(bundle.resolve("trust_root_fixture.pem"));
    Files.write(bundle.resolve("00-leaf.der"), STRONGBOX_CERT);
    Files.write(bundle.resolve("01-root.der"), ROOT_CERT);
    assertEquals(2, AndroidAttestationCommand.run(valid).getChainLength());
  }

  @Test
  void outputCannotOverwriteAnInputOrItsHardLink() throws Exception {
    Path bundle = createStrongBoxBundle(true);
    Path chain = bundle.resolve("chain.pem");
    byte[] original = Files.readAllBytes(chain);
    String[] valid = withGovernedRevocation(new String[] {"--bundle-dir", bundle.toString()});
    assertThrows(IllegalArgumentException.class, () -> AndroidAttestationCommand.run(
        appendOptions(valid, "--output", chain.toString())));
    Path link = bundle.resolve("same-inode.pem");
    Files.createLink(link, chain);
    assertThrows(IllegalArgumentException.class, () -> AndroidAttestationCommand.run(
        appendOptions(valid, "--output", link.toString())));
    assertArrayEquals(original, Files.readAllBytes(chain));
  }

  @Test
  void oversizedSnapshotAndIgnoredZipEntriesFailBeforeVerification() throws Exception {
    Path bundle = createStrongBoxBundle(true);
    String[] valid = withGovernedRevocation(new String[] {"--bundle-dir", bundle.toString()});
    Path snapshot = bundle.resolve("oversized-snapshot.txt");
    Files.write(snapshot, new byte[512 * 1024 + 1]);
    IllegalArgumentException snapshotError = assertThrows(IllegalArgumentException.class,
        () -> AndroidAttestationCommand.run(replaceOption(valid, "--revocation-snapshot", snapshot.toString())));
    assertTrue(snapshotError.getMessage().contains("byte bounds"));
    Path zipFile = bundle.resolve("oversized.zip");
    try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(zipFile))) {
      zip.putNextEntry(new ZipEntry("ignored-metadata.txt"));
      zip.write(new byte[1024 * 1024 + 1]);
      zip.closeEntry();
    }
    IllegalArgumentException zipError = assertThrows(IllegalArgumentException.class,
        () -> AndroidAttestationCommand.run(appendOptions(valid, "--trust-root-bundle", zipFile.toString())));
    assertTrue(zipError.getMessage().contains("byte bounds"));
  }

  @Test
  void chainLengthAndSymlinkInputsAreRejected() throws Exception {
    Path bundle = createStrongBoxBundle(true);
    String[] valid = withGovernedRevocation(new String[] {"--bundle-dir", bundle.toString()});
    Path original = bundle.resolve("original.pem");
    Files.move(bundle.resolve("chain.pem"), original);
    Files.createSymbolicLink(bundle.resolve("chain.pem"), original);
    assertThrows(IllegalArgumentException.class, () -> AndroidAttestationCommand.run(valid));
    Files.delete(bundle.resolve("chain.pem"));
    writeString(bundle.resolve("chain.pem"), repeat(toPem("CERTIFICATE", ROOT_CERT), 17), StandardCharsets.US_ASCII);
    IllegalArgumentException failure = assertThrows(IllegalArgumentException.class,
        () -> AndroidAttestationCommand.run(valid));
    assertTrue(failure.getMessage().contains("1..16 certificates"));
  }

  @Test
  void evidenceCannotFillMissingTrustedInputs() throws Exception {
    String[] valid = withGovernedRevocation(new String[] {
        "--bundle-dir", createStrongBoxBundle(true).toString()
    });
    for (String required : new String[] {
        "--alias", "--challenge-hex", "--trust-root", "--expected-leaf-spki-sha256"
    }) {
      assertThrows(IllegalArgumentException.class,
          () -> AndroidAttestationCommand.run(removeOption(valid, required)), required);
    }
  }

  private static String[] removeOption(String[] original, String option) {
    java.util.List<String> result = new java.util.ArrayList<>();
    boolean removed = false;
    for (int index = 0; index < original.length; index++) {
      if (original[index].equals(option)) {
        index++;
        removed = true;
      } else result.add(original[index]);
    }
    assertTrue(removed, "Missing fixture option " + option);
    return result.toArray(new String[0]);
  }

  private static String[] replaceOption(String[] original, String option, String value) {
    String[] result = original.clone();
    for (int index = 0; index + 1 < result.length; index++) {
      if (result[index].equals(option)) {
        result[index + 1] = value;
        return result;
      }
    }
    throw new AssertionError("Missing fixture option " + option);
  }

  private static String[] appendOptions(String[] original, String... options) {
    String[] result = Arrays.copyOf(original, original.length + options.length);
    System.arraycopy(options, 0, result, original.length, options.length);
    return result;
  }

  private static Path createStrongBoxBundle(final boolean includeChallenge) throws Exception {
    final Path bundleDir = Files.createTempDirectory(temporaryRoot, "attestation-negative-test");
    writeString(
        bundleDir.resolve("chain.pem"),
        toPem("CERTIFICATE", STRONGBOX_CERT) + toPem("CERTIFICATE", ROOT_CERT),
        StandardCharsets.US_ASCII);
    writeString(bundleDir.resolve("alias.txt"), "strongbox-alias", StandardCharsets.UTF_8);
    if (includeChallenge) {
      writeString(
          bundleDir.resolve("challenge.hex"), "4145454245", StandardCharsets.UTF_8);
    }
    writeString(
        bundleDir.resolve("trust_root_fixture.pem"),
        toPem("CERTIFICATE", ROOT_CERT),
        StandardCharsets.US_ASCII);
    return bundleDir;
  }

  private static void assertHarnessFails(final HarnessCall call) throws Exception {
    boolean threw = false;
    try {
      call.run();
    } catch (final IllegalArgumentException | AttestationVerificationException expected) {
      threw = true;
    }
    assertTrue(threw, "Attestation command must fail closed");
  }

  @FunctionalInterface
  private interface HarnessCall {
    void run() throws Exception;
  }

  private static byte[] decodeBase64(final String value) {
    return Base64.getDecoder().decode(value);
  }

  private static String[] withGovernedRevocation(final String[] arguments) throws Exception {
    return withGovernedRevocation(
        arguments, EVALUATION_TIME_MILLIS, "86400", EVALUATION_TIME_MILLIS);
  }

  private static String[] withGovernedRevocation(
      final String[] arguments,
      final String responseDateMillis,
      final String cacheMaxAgeSeconds,
      final String evaluationTimeMillis) throws Exception {
    return withGovernedRevocation(
        arguments,
        responseDateMillis,
        cacheMaxAgeSeconds,
        evaluationTimeMillis,
        Collections.emptyList());
  }

  private static String[] withGovernedRevocation(
      final String[] arguments,
      final String responseDateMillis,
      final String cacheMaxAgeSeconds,
      final String evaluationTimeMillis,
      final java.util.List<String> serials) throws Exception {
    final byte[] snapshot =
        canonicalSnapshot(
            Long.parseLong(responseDateMillis),
            null,
            Long.parseLong(cacheMaxAgeSeconds),
            serials,
            Collections.emptyList());
    final Path snapshotPath = Files.createTempFile(temporaryRoot, "android-revocation-snapshot", ".txt");
    Files.write(snapshotPath, snapshot);
    final java.util.List<String> governed = new java.util.ArrayList<>(Arrays.asList(
          "--revocation-snapshot", snapshotPath.toString(),
          "--revocation-snapshot-sha256",
              hexLower(
                  sha256(snapshot)),
          "--evaluation-time-ms", evaluationTimeMillis
        ));
    appendTrustedIdentityArguments(arguments, governed);
    final String[] combined = Arrays.copyOf(arguments, arguments.length + governed.size());
    System.arraycopy(governed.toArray(new String[0]), 0, combined, arguments.length, governed.size());
    return combined;
  }

  private static String[] withGovernedRevocationOnly(final String[] arguments) throws Exception {
    final byte[] snapshot =
        canonicalSnapshot(
            Long.parseLong(EVALUATION_TIME_MILLIS),
            null,
            86400L,
            Collections.emptyList(),
            Collections.emptyList());
    final Path snapshotPath = Files.createTempFile(temporaryRoot, "android-revocation-snapshot", ".txt");
    Files.write(snapshotPath, snapshot);
    final String[] governed =
        new String[] {
          "--revocation-snapshot", snapshotPath.toString(),
          "--revocation-snapshot-sha256",
              hexLower(
                  sha256(snapshot)),
          "--evaluation-time-ms", EVALUATION_TIME_MILLIS
        };
    final String[] combined = Arrays.copyOf(arguments, arguments.length + governed.length);
    System.arraycopy(governed, 0, combined, arguments.length, governed.length);
    return combined;
  }

  private static void appendTrustedIdentityArguments(
      final String[] arguments, final java.util.List<String> governed) throws Exception {
    final Path bundleDir = optionPath(arguments, "--bundle-dir");
    final Path chainPath = optionPath(arguments, "--chain");
    final Path effectiveChain =
        chainPath != null ? chainPath : bundleDir == null ? null : bundleDir.resolve("chain.pem");
    if (effectiveChain == null) {
      return;
    }
    final java.util.List<X509Certificate> certificates = readTestCertificates(effectiveChain);
    if (!hasOption(arguments, "--expected-leaf-spki-sha256")) {
      governed.add("--expected-leaf-spki-sha256");
      governed.add(hexLower(sha256(certificates.get(0).getPublicKey().getEncoded())));
    }
    if (!hasOption(arguments, "--trust-root")
        && !hasOption(arguments, "--trust-root-dir")
        && !hasOption(arguments, "--trust-root-bundle")) {
      final Path externalRoot = Files.createTempFile(temporaryRoot, "trusted-attestation-root", ".pem");
      writeString(
          externalRoot,
          toPem("CERTIFICATE", certificates.get(certificates.size() - 1).getEncoded()),
          StandardCharsets.US_ASCII);
      governed.add("--trust-root");
      governed.add(externalRoot.toString());
    }
    if (!hasOption(arguments, "--alias")) {
      final Path aliasPath = bundleDir == null ? null : bundleDir.resolve("alias.txt");
      governed.add("--alias");
      governed.add(
          aliasPath != null && Files.isRegularFile(aliasPath)
              ? readString(aliasPath, StandardCharsets.UTF_8).trim()
              : "test-attestation-alias");
    }
    if (!hasOption(arguments, "--challenge-hex") && !hasOption(arguments, "--challenge-file")) {
      final Path challengePath = bundleDir == null ? null : bundleDir.resolve("challenge.hex");
      if (challengePath != null && Files.isRegularFile(challengePath)) {
        governed.add("--challenge-hex");
        governed.add(readString(challengePath, StandardCharsets.UTF_8).trim());
      }
    }
  }

  private static Path optionPath(final String[] arguments, final String option) {
    for (int index = 0; index + 1 < arguments.length; index++) {
      if (option.equals(arguments[index])) {
        return Paths.get(arguments[index + 1]);
      }
    }
    return null;
  }

  private static boolean hasOption(final String[] arguments, final String option) {
    return optionPath(arguments, option) != null;
  }

  private static java.util.List<X509Certificate> readTestCertificates(final Path path)
      throws Exception {
    try (java.io.InputStream input = Files.newInputStream(path)) {
      final java.util.Collection<? extends java.security.cert.Certificate> decoded =
          CertificateFactory.getInstance("X.509").generateCertificates(input);
      final java.util.List<X509Certificate> certificates = new java.util.ArrayList<>();
      for (final java.security.cert.Certificate certificate : decoded) {
        certificates.add((X509Certificate) certificate);
      }
      return certificates;
    }
  }

  private static byte[] sha256(final byte[] bytes) throws Exception {
    return MessageDigest.getInstance("SHA-256").digest(bytes);
  }

  private static String hexLower(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte current : bytes) {
      builder.append(String.format(java.util.Locale.ROOT, "%02x", current));
    }
    return builder.toString();
  }

  private static byte[] decodeHex(final String hex) {
    final int length = hex.length();
    final byte[] out = new byte[length / 2];
    for (int i = 0; i < length; i += 2) {
      out[i / 2] = (byte) Integer.parseInt(hex.substring(i, i + 2), 16);
    }
    return out;
  }

  private static String toPem(final String type, final byte[] der) {
    final String base64 = Base64.getEncoder().encodeToString(der);
    final StringBuilder builder = new StringBuilder();
    builder.append("-----BEGIN ").append(type).append("-----\n");
    for (int i = 0; i < base64.length(); i += 64) {
      final int end = Math.min(i + 64, base64.length());
      builder.append(base64, i, end).append('\n');
    }
    builder.append("-----END ").append(type).append("-----\n");
    return builder.toString();
  }

  private static String readChallengeHex(final Path bundleDir) throws Exception {
    return readString(bundleDir.resolve("challenge.hex"), StandardCharsets.UTF_8).trim();
  }

  private static Path fixtureBundle(final String name) {
    return REPO_ROOT.resolve("fixtures/android/attestation").resolve(name);
  }

  private static Path copyFixtureBundle(final String name) throws Exception {
    final Path source = fixtureBundle(name);
    final Path target = Files.createTempDirectory(temporaryRoot, "attestation-fixture-" + name + "-");
    try (Stream<Path> stream = Files.walk(source)) {
      for (Path path : (Iterable<Path>) stream::iterator) {
        final Path relative = source.relativize(path);
        final Path destination = target.resolve(relative);
        if (Files.isDirectory(path)) {
          Files.createDirectories(destination);
        } else {
          Files.createDirectories(destination.getParent());
          Files.copy(path, destination, StandardCopyOption.REPLACE_EXISTING);
        }
      }
    }
    return target;
  }

  private static final Path REPO_ROOT = locateRepoRoot();

  private static Path locateRepoRoot() {
    Path dir = Paths.get("").toAbsolutePath().normalize();
    for (int depth = 0; depth < 5 && dir != null; depth++, dir = dir.getParent()) {
      if (Files.exists(dir.resolve("fixtures/android/attestation"))) {
        return dir;
      }
    }
    return Paths.get("").toAbsolutePath().normalize();
  }

  private static void writeString(Path path, String value, java.nio.charset.Charset charset)
      throws java.io.IOException {
    Files.write(path, value.getBytes(charset));
  }

  private static String readString(Path path, java.nio.charset.Charset charset)
      throws java.io.IOException {
    return new String(Files.readAllBytes(path), charset);
  }

  private static String repeat(String value, int count) {
    StringBuilder result = new StringBuilder();
    for (int index = 0; index < count; index++) result.append(value);
    return result.toString();
  }

  private static byte[] canonicalSnapshot(
      long responseDate, Long lastModified, long maxAge, java.util.List<String> serials,
      java.util.List<byte[]> tbsDigests) {
    java.util.List<String> sortedSerials = new java.util.ArrayList<>(serials);
    Collections.sort(sortedSerials);
    java.util.List<String> sortedTbs = new java.util.ArrayList<>();
    for (byte[] digest : tbsDigests) sortedTbs.add(hexLower(digest));
    Collections.sort(sortedTbs);
    StringBuilder snapshot = new StringBuilder();
    snapshot.append(org.hyperledger.iroha.sdk.crypto.keystore.attestation
        .AndroidAttestationRevocationPolicyV1.SNAPSHOT_DOMAIN).append('\n');
    snapshot.append("payload_sha256=").append(repeat("11", 32)).append('\n');
    snapshot.append("response_date_ms=").append(responseDate).append('\n');
    snapshot.append("last_modified_ms=").append(lastModified == null ? "-" : lastModified).append('\n');
    snapshot.append("cache_max_age_seconds=").append(maxAge).append('\n');
    snapshot.append("serial_count=").append(sortedSerials.size()).append('\n');
    for (String serial : sortedSerials) snapshot.append("serial=").append(serial).append('\n');
    snapshot.append("tbs_sha256_count=").append(sortedTbs.size()).append('\n');
    for (String digest : sortedTbs) snapshot.append("tbs_sha256=").append(digest).append('\n');
    return snapshot.toString().getBytes(StandardCharsets.US_ASCII);
  }
}
