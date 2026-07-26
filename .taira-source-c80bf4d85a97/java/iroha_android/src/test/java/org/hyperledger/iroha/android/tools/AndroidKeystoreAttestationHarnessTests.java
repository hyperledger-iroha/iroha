package org.hyperledger.iroha.android.tools;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardCopyOption;
import java.util.Base64;
import java.util.stream.Stream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationResult;

public final class AndroidKeystoreAttestationHarnessTests {

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

  private AndroidKeystoreAttestationHarnessTests() {}

  public static void main(final String[] args) throws Exception {
    verifiesStrongBoxBundle();
    verifiesBundleUsingTrustedRootDirectory();
    verifiesBundleUsingTrustRootDirectoryZipOnly();
    verifiesBundleUsingTrustRootZip();
    verifiesMockHuaweiFixtureBundle();
    verifiesMockOspBundleZipOnly();
    System.out.println("[IrohaAndroid] AndroidKeystoreAttestationHarnessTests passed");
  }

  private static void verifiesStrongBoxBundle() throws Exception {
    final Path tempDir = Files.createTempDirectory("attestation-harness-test");
    final Path bundleDir = tempDir.resolve("bundle");
    Files.createDirectories(bundleDir);

    // Create attestation chain (leaf + root).
    final Path chainPem = bundleDir.resolve("chain.pem");
    final String pem = toPem("CERTIFICATE", STRONGBOX_CERT) + toPem("CERTIFICATE", ROOT_CERT);
    Files.writeString(chainPem, pem, StandardCharsets.US_ASCII);

    // Alias & challenge helpers.
    Files.writeString(bundleDir.resolve("alias.txt"), "strongbox-alias", StandardCharsets.UTF_8);
    Files.writeString(
        bundleDir.resolve("challenge.hex"), "4145454245", StandardCharsets.UTF_8);

    final Path rootPem = tempDir.resolve("root.pem");
    Files.writeString(rootPem, toPem("CERTIFICATE", ROOT_CERT), StandardCharsets.US_ASCII);

    final Path output = tempDir.resolve("result.json");
    final AndroidKeystoreAttestationHarness.Result result =
        AndroidKeystoreAttestationHarness.run(
            new String[] {
              "--bundle-dir", bundleDir.toString(),
              "--trust-root", rootPem.toString(),
              "--require-strongbox",
              "--output", output.toString()
            });

    assert "strongbox-alias".equals(result.alias()) : "Alias should match bundle metadata";
    assert result.strongBoxAttestation() : "StrongBox attestation expected";
    assert result.attestationSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX;
    assert result.keymasterSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX;
    assert result.chainLength() == 2 : "Should report leaf + root";
    assert result.challengeHex().equals("4145454245") : "Challenge hex should round-trip";

    final String json = Files.readString(output, StandardCharsets.UTF_8);
    assert json.contains("\"alias\": \"strongbox-alias\"");
    assert json.contains("\"strongbox_attestation\": true");
  }

  private static void verifiesBundleUsingTrustedRootDirectory() throws Exception {
    final Path tempDir = Files.createTempDirectory("attestation-harness-dir-test");
    final Path bundleDir = tempDir.resolve("bundle-dir");
    Files.createDirectories(bundleDir);

    final Path chainPem = bundleDir.resolve("chain.pem");
    final String pem = toPem("CERTIFICATE", STRONGBOX_CERT) + toPem("CERTIFICATE", ROOT_CERT);
    Files.writeString(chainPem, pem, StandardCharsets.US_ASCII);

    Files.writeString(bundleDir.resolve("alias.txt"), "directory-alias", StandardCharsets.UTF_8);
    Files.writeString(bundleDir.resolve("challenge.hex"), "4145454245", StandardCharsets.UTF_8);

    final Path rootsDir = tempDir.resolve("roots");
    final Path nestedDir = rootsDir.resolve("vendor");
    Files.createDirectories(nestedDir);
    Files.writeString(
        nestedDir.resolve("trust_root_vendor.pem"),
        toPem("CERTIFICATE", ROOT_CERT),
        StandardCharsets.US_ASCII);

    final AndroidKeystoreAttestationHarness.Result result =
        AndroidKeystoreAttestationHarness.run(
            new String[] {
              "--bundle-dir", bundleDir.toString(),
              "--trust-root-dir", rootsDir.toString(),
              "--require-strongbox"
            });

    assert "directory-alias".equals(result.alias())
        : "Alias should match bundle metadata when using trust-root directories";
    assert result.strongBoxAttestation() : "StrongBox attestation expected when using directories";
    assert result.attestationSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX;
    assert result.keymasterSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX;
    assert result.chainLength() == 2 : "Directory-based trust root should still report leaf + root";
  }

  private static void verifiesBundleUsingTrustRootDirectoryZipOnly() throws Exception {
    final Path tempDir = Files.createTempDirectory("attestation-harness-dir-zip-test");
    final Path bundleDir = tempDir.resolve("bundle-dir-zip");
    Files.createDirectories(bundleDir);

    final Path chainPem = bundleDir.resolve("chain.pem");
    final String pem = toPem("CERTIFICATE", STRONGBOX_CERT) + toPem("CERTIFICATE", ROOT_CERT);
    Files.writeString(chainPem, pem, StandardCharsets.US_ASCII);

    Files.writeString(bundleDir.resolve("alias.txt"), "directory-zip-alias", StandardCharsets.UTF_8);
    Files.writeString(bundleDir.resolve("challenge.hex"), "4145454245", StandardCharsets.UTF_8);

    final Path rootsDir = tempDir.resolve("roots-zip-only");
    Files.createDirectories(rootsDir);
    final Path bundleZip = rootsDir.resolve("alt_vendor_roots.zip");
    try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(bundleZip))) {
      zip.putNextEntry(new ZipEntry("nested/trust_root_alt.der"));
      zip.write(ROOT_CERT);
      zip.closeEntry();
    }

    final AndroidKeystoreAttestationHarness.Result result =
        AndroidKeystoreAttestationHarness.run(
            new String[] {
              "--bundle-dir", bundleDir.toString(),
              "--trust-root-dir", rootsDir.toString(),
              "--require-strongbox"
            });

    assert "directory-zip-alias".equals(result.alias())
        : "Alias should match bundle metadata when scanning trust-root ZIP directories";
    assert result.strongBoxAttestation()
        : "Directory ZIP bundles should keep StrongBox classification";
    assert result.attestationSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX;
    assert result.chainLength() == 2 : "ZIP directories should deliver the same chain depth";
    assert result.challengeHex().equals("4145454245")
        : "Challenge should remain tied to bundle contents when roots are discovered via ZIPs";
  }

  private static void verifiesBundleUsingTrustRootZip() throws Exception {
    final Path tempDir = Files.createTempDirectory("attestation-harness-zip-test");
    final Path bundleDir = tempDir.resolve("bundle-zip");
    Files.createDirectories(bundleDir);

    final Path chainPem = bundleDir.resolve("chain.pem");
    final String pem = toPem("CERTIFICATE", STRONGBOX_CERT) + toPem("CERTIFICATE", ROOT_CERT);
    Files.writeString(chainPem, pem, StandardCharsets.US_ASCII);

    Files.writeString(bundleDir.resolve("alias.txt"), "zip-alias", StandardCharsets.UTF_8);
    Files.writeString(bundleDir.resolve("challenge.hex"), "4145454245", StandardCharsets.UTF_8);

    final Path bundleZip = tempDir.resolve("roots.zip");
    try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(bundleZip))) {
      zip.putNextEntry(new ZipEntry("trust_root_vendor.pem"));
      zip.write(toPem("CERTIFICATE", ROOT_CERT).getBytes(StandardCharsets.US_ASCII));
      zip.closeEntry();
    }

    final AndroidKeystoreAttestationHarness.Result result =
        AndroidKeystoreAttestationHarness.run(
            new String[] {
              "--bundle-dir", bundleDir.toString(),
              "--trust-root-bundle", bundleZip.toString(),
              "--require-strongbox"
            });

    assert "zip-alias".equals(result.alias()) : "Alias should reuse bundle metadata when zipped";
    assert result.chainLength() == 2 : "Zip bundles should not change chain depth";
    assert result.attestationSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX;
  }

  private static void verifiesMockHuaweiFixtureBundle() throws Exception {
    final Path bundleDir = fixtureBundle("mock_huawei");
    final AndroidKeystoreAttestationHarness.Result result =
        AndroidKeystoreAttestationHarness.run(
            new String[] {
              "--bundle-dir", bundleDir.toString(),
              "--require-strongbox"
            });

    assert "mock-huawei-strongbox".equals(result.alias())
        : "Fixture alias should match mock_huawei bundle";
    assert result.strongBoxAttestation()
        : "Mock Huawei bundle encodes a StrongBox attestation";
    assert result.attestationSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX;
    assert result.keymasterSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX;
    assert result.chainLength() == 2 : "Fixture chains contain leaf + root entries";
    assert result.challengeHex().equals(readChallengeHex(bundleDir))
        : "Challenge value should mirror challenge.hex contents";
  }

  private static void verifiesMockOspBundleZipOnly() throws Exception {
    final Path tempDir = copyFixtureBundle("mock_osp");
    final Path pem = tempDir.resolve("trust_root_osp.pem");
    Files.deleteIfExists(pem);

    final AndroidKeystoreAttestationHarness.Result result =
        AndroidKeystoreAttestationHarness.run(
            new String[] {
              "--bundle-dir", tempDir.toString(),
            });

    assert "mock-osp-keymint".equals(result.alias())
        : "Fixture alias should match mock_osp bundle";
    assert result.strongBoxAttestation()
        : "Mock OSP bundle encodes a StrongBox/KeyMint attestation";
    assert result.attestationSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX;
    assert result.keymasterSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX;
    assert result.chainLength() == 2 : "Fixture chains contain leaf + root entries";
    assert result.challengeHex().equals(readChallengeHex(tempDir))
        : "Challenge value should mirror challenge.hex contents";
  }

  private static byte[] decodeBase64(final String value) {
    return Base64.getDecoder().decode(value);
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
    return Files.readString(bundleDir.resolve("challenge.hex"), StandardCharsets.UTF_8).trim();
  }

  private static Path fixtureBundle(final String name) {
    return REPO_ROOT.resolve("fixtures/android/attestation").resolve(name);
  }

  private static Path copyFixtureBundle(final String name) throws Exception {
    final Path source = fixtureBundle(name);
    final Path target = Files.createTempDirectory("attestation-fixture-" + name + "-");
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
}
