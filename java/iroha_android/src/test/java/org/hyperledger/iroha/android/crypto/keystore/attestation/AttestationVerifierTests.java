package org.hyperledger.iroha.android.crypto.keystore.attestation;

import java.util.Arrays;
import java.util.Base64;
import org.hyperledger.iroha.android.crypto.keystore.KeyAttestation;

public final class AttestationVerifierTests {

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

  private static final byte[] TEE_CERT = decodeBase64(
      "MIIDRjCCAi6gAwIBAgIULKS+BqcxAYB6ooMNchJ4LI59fxwwDQYJKoZIhvcNAQELBQAwFzEVMBMGA1UE"
          + "AwwMVGVzdCBSb290IENBMB4XDTI1MTAyNTE1MzIwMVoXDTI2MTAyNTE1MzIwMVowFzEVMBMGA1UEAwwM"
          + "VGVzdCBURUUgS2V5MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAkumd8MpObKGwkipgAFyn"
          + "GluNSxEekQYCXrmAiEaze9/vHzFbi4Mp6CeUlK3oykMwi76TnAYhfMoCkYcWoynfsa62FhzONt8LIKCT"
          + "JGF7XZ/olOWAqIwinshhY4GkFrQ8WCVT8RVCikn+KInxGY8ssgHQEhKJKIm7Gd8bErJITCG42DbGeli8"
          + "eCbNkzmDYg4hk9i69A35ds0mHWU2Ir8jLSONCAAqUtRxlmBww9hs1/u8yFW+CqTy38NDv8fxN/XOlNEN"
          + "8oOyRanmvC25AojWmE+71amnBCtAbeJ0tdzLkaJjKpyp+v2852aYUbdDhJnL8exRSxWxodcQMK9PTVJ+"
          + "pQIDAQABo4GJMIGGMB0GA1UdDgQWBBSwqLItgrQ495Q3INPm4p/v5jmVXzAfBgNVHSMEGDAWgBQCI1yy"
          + "nYKE4wY9NUZuyW8rjA+dVDAJBgNVHRMEAjAAMAsGA1UdDwQEAwIHgDAsBgorBgEEAdZ5AgERBB4wHAIB"
          + "AwoBAQIBBAoBAQQEQ0hBTAQEBQYHCDAAMAAwDQYJKoZIhvcNAQELBQADggEBAH8k/Yx6842HO/mWuOV5"
          + "NjtWutJub+gF++hyqjVBDCJdZfUaN67VHxVAtGf56L5EnnN8/0/02md9ncqWsDtJ1/9soho67LZerbHv"
          + "q8BcbqLZzdbVMDrbXDTcJcseCvoeLkAo3N0KE3Bi9KzWpN0UV4uM47p/SGqTp+/GrjfSffykv0C3g2lK"
          + "wkWMEMk0L1O35ER0X9HT5vKjI3arDxFvuIMUoW50h2a7Q+haYdDyJbcy/wV5c8EGIc8kT59bKEQazUWl"
          + "Q+LzSqA7cBK0kW8lcbH+rBgWxOc0TtesJhi03mxjIF+x3fQxjMbxfoQQQgPsIcoTRylmHZLCnYzPKpPf"
          + "GZw=");

  private static final byte[] STRONGBOX_CHALLENGE = hex("4145454245");
  private static final byte[] TEE_CHALLENGE = hex("4348414C");

  private AttestationVerifierTests() {}

  public static void main(final String[] args) throws Exception {
    strongBoxAttestationPasses();
    challengeMismatchFails();
    strongBoxRequirementFailsForTee();
    teeAttestationAllowedWithoutStrongBoxRequirement();
    builderRequiresTrustAnchor();
    System.out.println("[IrohaAndroid] Attestation verifier tests passed.");
  }

  private static void strongBoxAttestationPasses() throws Exception {
    final KeyAttestation attestation =
        KeyAttestation.builder()
            .setAlias("strongbox-alias")
            .addCertificate(STRONGBOX_CERT)
            .addCertificate(ROOT_CERT)
            .build();
    final AttestationVerifier verifier =
        AttestationVerifier.builder()
            .addTrustedRoot(ROOT_CERT)
            .requireStrongBox(true)
            .build();
    final AttestationResult result = verifier.verify(attestation, STRONGBOX_CHALLENGE);
    assert result.isStrongBoxAttestation() : "Expected StrongBox security level";
    assert Arrays.equals(STRONGBOX_CHALLENGE, result.attestationChallenge())
        : "Challenge must round-trip";
    assert result.certificateChain().size() == 2 : "Certificate chain should contain leaf + root";
    assert result.attestationSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX;
    assert result.keymasterSecurityLevel() == AttestationResult.SecurityLevel.STRONG_BOX;
  }

  private static void challengeMismatchFails() throws Exception {
    final KeyAttestation attestation =
        KeyAttestation.builder()
            .setAlias("strongbox-alias")
            .addCertificate(STRONGBOX_CERT)
            .addCertificate(ROOT_CERT)
            .build();
    final AttestationVerifier verifier =
        AttestationVerifier.builder().addTrustedRoot(ROOT_CERT).requireStrongBox(true).build();
    boolean threw = false;
    try {
      verifier.verify(attestation, hex("DEADBEEF"));
    } catch (final AttestationVerificationException ex) {
      threw = true;
    }
    assert threw : "Challenge mismatch should fail verification";
  }

  private static void strongBoxRequirementFailsForTee() throws Exception {
    final KeyAttestation attestation =
        KeyAttestation.builder()
            .setAlias("tee-alias")
            .addCertificate(TEE_CERT)
            .addCertificate(ROOT_CERT)
            .build();
    final AttestationVerifier verifier =
        AttestationVerifier.builder().addTrustedRoot(ROOT_CERT).requireStrongBox(true).build();
    boolean threw = false;
    try {
      verifier.verify(attestation, TEE_CHALLENGE);
    } catch (final AttestationVerificationException ex) {
      threw = true;
    }
    assert threw : "TEE attestation must be rejected when StrongBox is required";
  }

  private static void teeAttestationAllowedWithoutStrongBoxRequirement() throws Exception {
    final KeyAttestation attestation =
        KeyAttestation.builder()
            .setAlias("tee-alias")
            .addCertificate(TEE_CERT)
            .addCertificate(ROOT_CERT)
            .build();
    final AttestationVerifier verifier =
        AttestationVerifier.builder().addTrustedRoot(ROOT_CERT).requireStrongBox(false).build();
    final AttestationResult result = verifier.verify(attestation, TEE_CHALLENGE);
    assert result.attestationSecurityLevel() == AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT
        : "Expected TEE security level";
    assert !result.isStrongBoxAttestation() : "TEE security level should not report StrongBox";
  }

  private static void builderRequiresTrustAnchor() {
    boolean threw = false;
    try {
      AttestationVerifier.builder().build();
    } catch (final IllegalStateException ex) {
      threw = true;
    }
    assert threw : "Verifier must reject configuration without trusted anchors";
  }

  private static byte[] decodeBase64(final String value) {
    return Base64.getDecoder().decode(value);
  }

  private static byte[] hex(final String hex) {
    final int length = hex.length();
    final byte[] out = new byte[length / 2];
    for (int i = 0; i < length; i += 2) {
      out[i / 2] = (byte) Integer.parseInt(hex.substring(i, i + 2), 16);
    }
    return out;
  }
}
