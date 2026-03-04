package org.hyperledger.iroha.android.crypto.keystore;

import java.security.KeyPair;
import java.util.Arrays;
import java.util.Base64;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;
import java.util.concurrent.atomic.AtomicInteger;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;
import org.hyperledger.iroha.android.crypto.SoftwareKeyProvider;
import org.hyperledger.iroha.android.crypto.keystore.AndroidKeystoreStubBackend;
import org.hyperledger.iroha.android.crypto.keystore.KeyGenerationResult;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationResult;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationVerificationException;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationVerifier;
import org.hyperledger.iroha.android.telemetry.DeviceProfileProvider;
import org.hyperledger.iroha.android.telemetry.KeystoreTelemetryEmitter;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetryRecord;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;
import org.hyperledger.iroha.android.client.ClientResponse;

public final class KeystoreKeyProviderTests {

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

  private static final byte[] STRONGBOX_CHALLENGE = hex("4145454245");

  private KeystoreKeyProviderTests() {}

  public static void main(final String[] args) throws Exception {
    hardwareProviderSatisfiesRequirement();
    metadataReflectsBackendCapabilities();
    stubBackendSurfacesUnsupported();
    verifyAttestationRoundTrip();
    verifyAttestationReturnsEmptyWhenMissing();
    verifyAttestationGeneratesOnDemand();
    attestationChallengeMismatchFails();
    attestationChallengeMismatchEvictsCache();
    generateAttestationStoresBundle();
    attestationCacheAvoidsBackendRoundTrips();
    challengeSpecificCacheSeparation();
    emptyVsNonEmptyChallengeCacheMatrix();
    attestationFailureTelemetryEmitsSignal();
    challengeUnsupportedBackendFails();
    strongBoxRequiredRejectsNonStrongBoxBackend();
    strongBoxPreferredChoosesStrongBoxWhenAvailable();
    strongBoxPreferredFallbackEmitsTelemetry();
    System.out.println("[IrohaAndroid] Keystore provider scaffolding tests passed.");
  }

  private static void hardwareProviderSatisfiesRequirement() throws Exception {
    final FakeBackend backend =
        new FakeBackend(
            KeyProviderMetadata.builder("fake-strongbox-backend")
                .setStrongBoxBacked(true)
                .setSupportsAttestationCertificates(true)
                .build());
    final KeystoreKeyProvider hardwareProvider =
        new KeystoreKeyProvider(backend, KeyGenParameters.builder().setRequireStrongBox(true).build());

    final IrohaKeyManager manager =
        IrohaKeyManager.fromProviders(
            java.util.List.of(hardwareProvider, new SoftwareKeyProvider()));

    final KeyPair keyPair =
        manager.generateOrLoad("hardware-alias", IrohaKeyManager.KeySecurityPreference.HARDWARE_REQUIRED);
    assert keyPair.getPrivate() != null : "Hardware provider should supply a private key";
    assert backend.generatedAlias("hardware-alias")
        : "Backend should have produced key material for alias";
  }

  private static void metadataReflectsBackendCapabilities() {
    final FakeBackend backend =
        new FakeBackend(
            KeyProviderMetadata.builder("fake-strongbox-backend")
                .setStrongBoxBacked(true)
                .setSupportsAttestationCertificates(true)
                .build());
    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(backend, KeyGenParameters.builder().build());

    final KeyProviderMetadata metadata = provider.metadata();
    assert metadata.hardwareBacked() : "Metadata must flag hardware support";
    assert metadata.strongBoxBacked() : "StrongBox flag should bubble through metadata";
    assert metadata.supportsAttestationCertificates()
        : "Attestation capability should be preserved";
  }

  private static void stubBackendSurfacesUnsupported() {
    final AndroidKeystoreStubBackend stub = new AndroidKeystoreStubBackend();
    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(stub, KeyGenParameters.builder().build());

    assert !provider.metadata().hardwareBacked()
        : "Stub backend must advertise lack of hardware support";
    assert provider.attestation("alias").isEmpty()
        : "Stub backend should not provide attestation certificates";
    boolean threw = false;
    try {
      provider.generate("alias");
    } catch (final KeyManagementException ex) {
      threw = true;
    }
    assert threw : "Stub backend must fail key generation outside Android runtime";
  }

  private static void verifyAttestationRoundTrip() throws Exception {
    final FakeBackend backend =
        new FakeBackend(
            KeyProviderMetadata.builder("fake-strongbox-backend")
                .setStrongBoxBacked(true)
                .setSupportsAttestationCertificates(true)
                .build());
    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(backend, KeyGenParameters.builder().build());

    backend.setAttestation(
        "alias",
        KeyAttestation.builder()
            .setAlias("alias")
            .addCertificate(STRONGBOX_CERT)
            .addCertificate(ROOT_CERT)
            .build());

    final AttestationVerifier verifier =
        AttestationVerifier.builder().addTrustedRoot(ROOT_CERT).requireStrongBox(true).build();
    final Optional<AttestationResult> result =
        provider.verifyAttestation("alias", verifier, STRONGBOX_CHALLENGE);
    assert result.isPresent() : "Attestation must verify when present";
    assert result.get().isStrongBoxAttestation() : "Expected StrongBox attestation";
    assert Arrays.equals(STRONGBOX_CHALLENGE, result.get().attestationChallenge())
        : "Challenge should match expected value";
  }

  private static void verifyAttestationReturnsEmptyWhenMissing() throws Exception {
    final FakeBackend backend =
        new FakeBackend(KeyProviderMetadata.builder("fake-strongbox-backend").build());
    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(backend, KeyGenParameters.builder().build());
    final AttestationVerifier verifier =
        AttestationVerifier.builder().addTrustedRoot(ROOT_CERT).build();
    final Optional<AttestationResult> result =
        provider.verifyAttestation("alias", verifier, STRONGBOX_CHALLENGE);
    assert result.isEmpty() : "Absent attestation should return empty optional";
  }

  private static void verifyAttestationGeneratesOnDemand() throws Exception {
    final FakeBackend backend =
        new FakeBackend(
            KeyProviderMetadata.builder("fake-strongbox-backend")
                .setStrongBoxBacked(true)
                .setSupportsAttestationCertificates(true)
                .build());
    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(backend, KeyGenParameters.builder().build());
    provider.generate("generated-alias");

    final AttestationVerifier verifier =
        AttestationVerifier.builder().addTrustedRoot(ROOT_CERT).requireStrongBox(true).build();
    final Optional<AttestationResult> result =
        provider.verifyAttestation("generated-alias", verifier, STRONGBOX_CHALLENGE);
    assert result.isPresent() : "Attestation should be generated when missing";
    assert backend.attestationGenerations() == 1
        : "Challenge verification should generate a fresh attestation";
    assert backend.attestationReads() == 0
        : "Generation path should not require a separate attestation read";

    final Optional<AttestationResult> cached =
        provider.verifyAttestation("generated-alias", verifier, STRONGBOX_CHALLENGE);
    assert cached.isPresent() : "Cached attestation should verify";
    assert backend.attestationGenerations() == 1
        : "Cached challenge-specific attestation should prevent regeneration";
  }

  private static void attestationChallengeMismatchFails() throws Exception {
    final FakeBackend backend =
        new FakeBackend(
            KeyProviderMetadata.builder("fake-strongbox-backend")
                .setStrongBoxBacked(true)
                .setSupportsAttestationCertificates(true)
                .build());
    backend.setAttestation(
        "alias",
        KeyAttestation.builder()
            .setAlias("alias")
            .addCertificate(STRONGBOX_CERT)
            .addCertificate(ROOT_CERT)
            .build());

    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(backend, KeyGenParameters.builder().build());
    final AttestationVerifier verifier =
        AttestationVerifier.builder().addTrustedRoot(ROOT_CERT).requireStrongBox(true).build();
    boolean threw = false;
    try {
      provider.verifyAttestation("alias", verifier, hex("DEADBEEF"));
    } catch (final AttestationVerificationException ex) {
      threw = true;
    }
    assert threw : "Verification must fail when challenge does not match";
  }

  private static void attestationChallengeMismatchEvictsCache() throws Exception {
    final FakeBackend backend =
        new FakeBackend(
            KeyProviderMetadata.builder("fake-strongbox-backend")
                .setStrongBoxBacked(true)
                .setSupportsAttestationCertificates(true)
                .build());
    backend.setAttestation(
        "alias",
        KeyAttestation.builder()
            .setAlias("alias")
            .addCertificate(STRONGBOX_CERT)
            .addCertificate(ROOT_CERT)
            .build());

    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(backend, KeyGenParameters.builder().build());
    final AttestationVerifier verifier =
        AttestationVerifier.builder().addTrustedRoot(ROOT_CERT).requireStrongBox(true).build();

    boolean threw = false;
    try {
      provider.verifyAttestation("alias", verifier, hex("DEADBEEF"));
    } catch (final AttestationVerificationException ex) {
      threw = true;
    }
    assert threw : "Verification must fail when cached attestation does not match the challenge";
    assert backend.attestationGenerations() == 1
        : "Verification should generate fresh attestation for the supplied challenge";
    assert backend.attestationReads() == 1 : "Fallback attestation should be fetched when present";

    backend.setAttestation(
        "alias",
        KeyAttestation.builder()
            .setAlias("alias")
            .addCertificate(STRONGBOX_CERT)
            .addCertificate(ROOT_CERT)
            .build());

    final Optional<AttestationResult> refreshed =
        provider.verifyAttestation("alias", verifier, STRONGBOX_CHALLENGE);
    assert refreshed.isPresent() : "Verification should succeed after cache eviction";
    assert backend.attestationGenerations() == 2
        : "Fresh attestation should be generated after mismatch eviction";
    assert backend.attestationReads() == 2
        : "Cache should be cleared after mismatch so verification rereads attestation";
  }

  private static void generateAttestationStoresBundle() throws Exception {
    final FakeBackend backend =
        new FakeBackend(
            KeyProviderMetadata.builder("fake-strongbox-backend")
                .setStrongBoxBacked(true)
                .setSupportsAttestationCertificates(true)
                .build());
    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(backend, KeyGenParameters.builder().setRequireStrongBox(true).build());

    provider.generate("alias");
    final Optional<KeyAttestation> generated =
        provider.generateAttestation("alias", STRONGBOX_CHALLENGE);
    assert generated.isPresent() : "Expected generated attestation bundle";
    assert backend.attestation("alias").isPresent() : "Backend should store generated attestation";
  }

  private static void attestationCacheAvoidsBackendRoundTrips() throws Exception {
    final FakeBackend backend =
        new FakeBackend(
            KeyProviderMetadata.builder("fake-strongbox-backend")
                .setStrongBoxBacked(true)
                .setSupportsAttestationCertificates(true)
                .build());
    backend.setAttestation(
        "cached-alias",
        KeyAttestation.builder()
            .setAlias("cached-alias")
            .addCertificate(STRONGBOX_CERT)
            .addCertificate(ROOT_CERT)
            .build());

    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(backend, KeyGenParameters.builder().build());

    final Optional<KeyAttestation> first = provider.attestation("cached-alias");
    final Optional<KeyAttestation> second = provider.attestation("cached-alias");

    assert first.isPresent() : "Expected attestation to be present";
    assert second.isPresent() : "Cache should return attestation on repeat calls";
    assert backend.attestationReads() == 1
        : "Backend attestation lookup should occur only once when cached";
  }

  private static void challengeSpecificCacheSeparation() throws Exception {
    final FakeBackend backend =
        new FakeBackend(
            KeyProviderMetadata.builder("fake-strongbox-backend")
                .setStrongBoxBacked(true)
                .setSupportsAttestationCertificates(true)
                .build());
    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(backend, KeyGenParameters.builder().build());

    provider.generate("challenge-alias");
    provider.generateAttestation("challenge-alias", STRONGBOX_CHALLENGE);
    provider.generateAttestation("challenge-alias", STRONGBOX_CHALLENGE);
    provider.generateAttestation("challenge-alias", hex("DEADBEEF"));

    assert backend.attestationGenerations() == 3
        : "Attestation should be regenerated for every request when a challenge is supplied";
    assert backend.attestationReads() == 0
        : "Cache hits should not require direct backend attestation lookups";
  }

  private static void emptyVsNonEmptyChallengeCacheMatrix() throws Exception {
    final FakeBackend backend =
        new FakeBackend(
            KeyProviderMetadata.builder("fake-strongbox-backend")
                .setStrongBoxBacked(true)
                .setSupportsAttestationCertificates(true)
                .build());
    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(backend, KeyGenParameters.builder().build());

    provider.generate("matrix-alias");
    provider.generateAttestation("matrix-alias", null);
    provider.generateAttestation("matrix-alias", null);
    assert backend.attestationGenerations() == 1
        : "Empty challenge attestation should reuse the cached bundle";

    provider.generateAttestation("matrix-alias", STRONGBOX_CHALLENGE);
    provider.generateAttestation("matrix-alias", STRONGBOX_CHALLENGE);
    provider.generateAttestation("matrix-alias", hex("CAFEBABE"));
    assert backend.attestationGenerations() == 4
        : "Supplying a challenge must regenerate attestation material per request";
  }

  private static void attestationFailureTelemetryEmitsSignal() throws Exception {
    final FakeBackend backend =
        new FakeBackend(
            KeyProviderMetadata.builder("fake-strongbox-backend")
                .setStrongBoxBacked(true)
                .setSupportsAttestationCertificates(true)
                .build());
    backend.setAttestation(
        "failure-alias",
        KeyAttestation.builder()
            .setAlias("failure-alias")
            .addCertificate(STRONGBOX_CERT)
            .addCertificate(ROOT_CERT)
            .build());

    final RecordingTelemetrySink sink = new RecordingTelemetrySink();
    final TelemetryOptions options =
        TelemetryOptions.builder()
            .setEnabled(true)
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setEnabled(true)
                    .setSalt(new byte[] {0x01, 0x02, 0x03})
                    .setSaltVersion("test-salt")
                    .setRotationId("test-rotation")
                    .build())
            .build();
    final KeystoreTelemetryEmitter emitter =
        KeystoreTelemetryEmitter.from(options, sink, DeviceProfileProvider.disabled());
    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(backend, KeyGenParameters.builder().build());
    final IrohaKeyManager manager =
        IrohaKeyManager.fromProviders(java.util.List.of(provider), emitter);

    final AttestationVerifier verifier =
        AttestationVerifier.builder().addTrustedRoot(ROOT_CERT).requireStrongBox(true).build();
    boolean threw = false;
    try {
      manager.verifyAttestation("failure-alias", verifier, hex("BADC0DE0"));
    } catch (final AttestationVerificationException expected) {
      threw = true;
    }
    assert threw : "Verification must fail on challenge mismatch";
    assert "android.keystore.attestation.failure".equals(sink.lastSignalId())
        : "Failure telemetry must be emitted when verification fails";
    final Map<String, Object> fields = sink.lastFields();
    assert fields != null : "Telemetry fields should be recorded";
    assert "fake-strongbox-backend".equals(fields.get("provider"))
        : "Provider label should be preserved in telemetry";
    assert fields.get("failure_reason") != null
        : "Failure reason should be surfaced for diagnostics";
  }

  private static void challengeUnsupportedBackendFails() throws Exception {
    final ChallengeUnsupportedBackend backend =
        new ChallengeUnsupportedBackend(
            KeyProviderMetadata.builder("fake-tee-backend")
                .setSupportsAttestationCertificates(true)
                .build());
    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(backend, KeyGenParameters.builder().build());
    provider.generate("alias");
    boolean threw = false;
    try {
      provider.generateAttestation("alias", STRONGBOX_CHALLENGE);
    } catch (final KeyManagementException expected) {
      threw = true;
    }
    assert threw : "Challenge attestation should fail when backend cannot honor challenge";
    assert backend.attestationGenerations() == 1
        : "Backend should be invoked before surfacing the error";
  }

  private static void strongBoxPreferredSignalsPreference() throws Exception {
    final FakeBackend backend =
        new FakeBackend(
            KeyProviderMetadata.builder("fake-strongbox-backend")
                .setStrongBoxBacked(true)
                .setSupportsAttestationCertificates(true)
                .build());
    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(backend, KeyGenParameters.builder().build())
            .withPreference(IrohaKeyManager.KeySecurityPreference.STRONGBOX_PREFERRED);
    provider.generate("preferred");
    final KeyGenParameters params = backend.lastParameters();
    assert params != null : "Backend should receive parameters";
    assert params.preferStrongBox() : "Preference should signal StrongBox preference";
    assert params.allowStrongBoxFallback()
        : "Preferred should allow fallback when StrongBox unavailable";
  }

  private static void strongBoxRequiredRejectsNonStrongBoxBackend() throws Exception {
    final FakeBackend teeBackend =
        new FakeBackend(KeyProviderMetadata.trustedEnvironment("fake-tee-backend"));
    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(teeBackend, KeyGenParameters.builder().build());
    final IrohaKeyManager manager =
        IrohaKeyManager.fromProviders(List.of(provider), KeystoreTelemetryEmitter.noop());

    boolean threw = false;
    try {
      manager.generateOrLoad(
          "sb-required-missing", IrohaKeyManager.KeySecurityPreference.STRONGBOX_REQUIRED);
    } catch (final KeyManagementException ex) {
      threw = true;
    }
    assert threw : "StrongBox-required preference must fail when backend lacks StrongBox";
    assert !teeBackend.generatedAlias("sb-required-missing")
        : "Key generation should not proceed on non-StrongBox backend";
  }

  private static void strongBoxPreferredChoosesStrongBoxWhenAvailable() throws Exception {
    final FakeBackend teeBackend =
        new FakeBackend(KeyProviderMetadata.trustedEnvironment("fake-tee-backend"));
    final FakeBackend strongBoxBackend =
        new FakeBackend(
            KeyProviderMetadata.builder("fake-strongbox-backend")
                .setStrongBoxBacked(true)
                .setSupportsAttestationCertificates(true)
                .build());
    final KeystoreKeyProvider teeProvider =
        new KeystoreKeyProvider(teeBackend, KeyGenParameters.builder().build());
    final KeystoreKeyProvider strongBoxProvider =
        new KeystoreKeyProvider(strongBoxBackend, KeyGenParameters.builder().build());
    final IrohaKeyManager manager =
        IrohaKeyManager.fromProviders(
            List.of(teeProvider, strongBoxProvider), KeystoreTelemetryEmitter.noop());

    manager.generateOrLoad(
        "sb-preferred-choice", IrohaKeyManager.KeySecurityPreference.STRONGBOX_PREFERRED);
    assert strongBoxBackend.generatedAlias("sb-preferred-choice")
        : "StrongBox backend should satisfy preferred request first";
    assert !teeBackend.generatedAlias("sb-preferred-choice")
        : "TEE backend should not be used when StrongBox is available";
  }

  private static void strongBoxPreferredFallbackEmitsTelemetry() throws Exception {
    final FakeBackend teeBackend =
        new FakeBackend(KeyProviderMetadata.trustedEnvironment("fake-tee-backend"));
    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(teeBackend, KeyGenParameters.builder().build());
    final RecordingTelemetrySink sink = new RecordingTelemetrySink();
    final TelemetryOptions.Redaction redaction =
        TelemetryOptions.Redaction.builder()
            .setEnabled(true)
            .setSalt(new byte[] {0x01, 0x02})
            .setSaltVersion("test-salt")
            .setRotationId("test-rotation")
            .build();
    final TelemetryOptions options =
        TelemetryOptions.builder().setEnabled(true).setTelemetryRedaction(redaction).build();
    final KeystoreTelemetryEmitter emitter =
        KeystoreTelemetryEmitter.from(options, sink, DeviceProfileProvider.disabled());
    final IrohaKeyManager manager = IrohaKeyManager.fromProviders(List.of(provider), emitter);

    manager.generateOrLoad(
        "sb-fallback", IrohaKeyManager.KeySecurityPreference.STRONGBOX_PREFERRED);

    assert "android.keystore.keygen".equals(sink.lastSignalId())
        : "Key generation telemetry should be emitted";
    final Map<String, Object> fields = sink.lastFields();
    assert fields != null : "Telemetry fields must be recorded";
    assert Boolean.TRUE.equals(fields.get("fallback"))
        : "Fallback flag should be true when StrongBox preference falls back to TEE";
    assert "hardware".equals(fields.get("route"))
        : "Route should reflect hardware (TEE) when StrongBox is unavailable";
    assert "strongbox_preferred".equals(fields.get("preference"))
        : "Preference label should be lowercased in telemetry";
  }

  private static final class FakeBackend implements KeystoreBackend {
    private final ConcurrentMap<String, KeyPair> keys = new ConcurrentHashMap<>();
    private final ConcurrentMap<String, KeyAttestation> attestations = new ConcurrentHashMap<>();
    private final KeyProviderMetadata metadata;
    private final SoftwareKeyProvider delegate = new SoftwareKeyProvider();
    private final AtomicInteger attestationReads = new AtomicInteger();
    private final AtomicInteger attestationGenerations = new AtomicInteger();
    private volatile KeyGenParameters lastParameters;

    private FakeBackend(final KeyProviderMetadata metadata) {
      this.metadata = metadata;
    }

    @Override
    public Optional<KeyPair> load(final String alias) {
      return Optional.ofNullable(keys.get(alias));
    }

    @Override
    public KeyGenerationResult generate(final String alias, final KeyGenParameters parameters)
        throws KeyManagementException {
      final KeyPair keyPair = delegate.generate(alias);
      keys.put(alias, keyPair);
      lastParameters = parameters;
      final boolean strongBox =
          metadata.strongBoxBacked() && (parameters.requireStrongBox() || parameters.preferStrongBox());
      return new KeyGenerationResult(keyPair, strongBox);
    }

    @Override
    public KeyPair generateEphemeral(final KeyGenParameters parameters) throws KeyManagementException {
      return delegate.generateEphemeral();
    }

    @Override
    public KeyProviderMetadata metadata() {
      return metadata;
    }

    @Override
    public String name() {
      return metadata.name();
    }

    @Override
    public Optional<KeyAttestation> attestation(final String alias) {
      attestationReads.incrementAndGet();
      return Optional.ofNullable(attestations.get(alias));
    }

    @Override
    public Optional<KeyAttestation> generateAttestation(final String alias, final byte[] challenge) {
      attestationGenerations.incrementAndGet();
      if (!keys.containsKey(alias)) {
        return Optional.empty();
      }
      final KeyAttestation attestation =
          KeyAttestation.builder()
              .setAlias(alias)
              .addCertificate(STRONGBOX_CERT)
              .addCertificate(ROOT_CERT)
              .build();
      attestations.put(alias, attestation);
      return Optional.of(attestation);
    }

    boolean generatedAlias(final String alias) {
      return keys.containsKey(alias);
    }

    void setAttestation(final String alias, final KeyAttestation attestation) {
      attestations.put(alias, attestation);
    }

    int attestationReads() {
      return attestationReads.get();
    }

    int attestationGenerations() {
      return attestationGenerations.get();
    }

    KeyGenParameters lastParameters() {
      return lastParameters;
    }
  }

  private static final class ChallengeUnsupportedBackend implements KeystoreBackend {
    private final SoftwareKeyProvider delegate = new SoftwareKeyProvider();
    private final ConcurrentMap<String, KeyPair> keys = new ConcurrentHashMap<>();
    private final KeyProviderMetadata metadata;
    private final AtomicInteger attestationGenerations = new AtomicInteger();

    private ChallengeUnsupportedBackend(final KeyProviderMetadata metadata) {
      this.metadata = metadata;
    }

    @Override
    public Optional<KeyPair> load(final String alias) {
      return Optional.ofNullable(keys.get(alias));
    }

    @Override
    public KeyGenerationResult generate(final String alias, final KeyGenParameters parameters)
        throws KeyManagementException {
      final KeyPair keyPair = delegate.generate(alias);
      keys.put(alias, keyPair);
      final boolean strongBox =
          metadata.strongBoxBacked() && (parameters.requireStrongBox() || parameters.preferStrongBox());
      return new KeyGenerationResult(keyPair, strongBox);
    }

    @Override
    public KeyPair generateEphemeral(final KeyGenParameters parameters) throws KeyManagementException {
      return delegate.generateEphemeral();
    }

    @Override
    public KeyProviderMetadata metadata() {
      return metadata;
    }

    @Override
    public String name() {
      return metadata.name();
    }

    @Override
    public Optional<KeyAttestation> generateAttestation(final String alias, final byte[] challenge) {
      attestationGenerations.incrementAndGet();
      return Optional.empty();
    }

    int attestationGenerations() {
      return attestationGenerations.get();
    }
  }

  private static final class RecordingTelemetrySink implements TelemetrySink {
    private String lastSignalId;
    private Map<String, Object> lastFields;

    @Override
    public void onRequest(final TelemetryRecord record) {
      // No-op for tests.
    }

    @Override
    public void onResponse(final TelemetryRecord record, final ClientResponse response) {
      // No-op for tests.
    }

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {
      // No-op for tests.
    }

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      this.lastSignalId = signalId;
      this.lastFields = fields;
    }

    String lastSignalId() {
      return lastSignalId;
    }

    Map<String, Object> lastFields() {
      return lastFields;
    }
  }

  private static byte[] decodeBase64(final String value) {
    return Base64.getDecoder().decode(value);
  }

  private static byte[] hex(final String value) {
    final int length = value.length();
    final byte[] result = new byte[length / 2];
    for (int i = 0; i < length; i += 2) {
      result[i / 2] = (byte) Integer.parseInt(value.substring(i, i + 2), 16);
    }
    return result;
  }
}
