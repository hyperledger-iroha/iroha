package org.hyperledger.iroha.android.crypto.keystore.attestation;

import java.math.BigInteger;
import java.security.Principal;
import java.security.PublicKey;
import java.security.cert.X509Certificate;
import java.util.Collections;
import java.util.Date;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.IrohaKeyManager.KeyProvider;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;
import org.hyperledger.iroha.android.telemetry.DeviceProfile;
import org.hyperledger.iroha.android.telemetry.DeviceProfileProvider;
import org.hyperledger.iroha.android.telemetry.KeystoreTelemetryEmitter;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;
public final class IrohaKeyManagerTelemetryTests {

  public static void main(final String[] args) throws Exception {
    verifyAttestationEmitsTelemetry();
    verifyAttestationRecordsFailures();
    verifyAttestationChallengeFailuresEmitTelemetry();
    System.out.println("[IrohaAndroid] Keystore telemetry tests passed.");
  }

  private static void verifyAttestationEmitsTelemetry() throws Exception {
    final RecordingTelemetrySink sink = new RecordingTelemetrySink();
    final KeystoreTelemetryEmitter emitter =
        KeystoreTelemetryEmitter.from(options(), sink, deviceProfileProvider());
    final IrohaKeyManager manager =
        IrohaKeyManager.fromProviders(List.of(new SuccessfulAttestationProvider()), emitter);
    final AttestationVerifier verifier =
        AttestationVerifier.builder().addTrustedRoot(new StubCertificate()).build();

    final Optional<AttestationResult> result =
        manager.verifyAttestation("retail-wallet", verifier, null);
    assert result.isPresent() : "verification result expected";

    final RecordingTelemetrySink.SignalEvent event =
        sink.lastEvent("android.keystore.attestation.result");
    assert event != null : "attestation result event missing";
    final String expectedAlias =
        options().redaction().hashIdentifier("retail-wallet").orElseThrow();
    assert expectedAlias.equals(event.fields.get("alias_label"));
    assert "trusted_environment".equals(event.fields.get("security_level"));
    assert "enterprise".equals(event.fields.get("device_brand_bucket"));
    assert "trusted-provider".equals(event.fields.get("provider"));
    final String digest = (String) event.fields.get("attestation_digest");
    assert digest != null;
    assert digest.length() == 64 : "digest should be hex encoded";
  }

  private static void verifyAttestationRecordsFailures() {
    final RecordingTelemetrySink sink = new RecordingTelemetrySink();
    final KeystoreTelemetryEmitter emitter =
        KeystoreTelemetryEmitter.from(options(), sink, deviceProfileProvider());
    final IrohaKeyManager manager =
        IrohaKeyManager.fromProviders(List.of(new FailingAttestationProvider()), emitter);
    final AttestationVerifier verifier =
        AttestationVerifier.builder().addTrustedRoot(new StubCertificate()).build();

    boolean threw = false;
    try {
      manager.verifyAttestation("retail-wallet", verifier, null);
    } catch (final AttestationVerificationException expected) {
      threw = true;
    }
    assert threw : "verification should fail";

    final RecordingTelemetrySink.SignalEvent event =
        sink.lastEvent("android.keystore.attestation.failure");
    assert event != null : "failure event missing";
    assert options()
            .redaction()
            .hashIdentifier("retail-wallet")
            .orElseThrow()
            .equals(event.fields.get("alias_label"));
    assert "verification_failed".equals(event.fields.get("failure_reason"));
    assert "failing-provider".equals(event.fields.get("provider"));
  }

  private static void verifyAttestationChallengeFailuresEmitTelemetry() {
    final RecordingTelemetrySink sink = new RecordingTelemetrySink();
    final KeystoreTelemetryEmitter emitter =
        KeystoreTelemetryEmitter.from(options(), sink, deviceProfileProvider());
    final IrohaKeyManager manager =
        IrohaKeyManager.fromProviders(List.of(new ChallengeMismatchProvider()), emitter);
    final AttestationVerifier verifier =
        AttestationVerifier.builder().addTrustedRoot(new StubCertificate()).build();

    boolean threw = false;
    try {
      manager.verifyAttestation("retail-wallet", verifier, new byte[] {0x01});
    } catch (final AttestationVerificationException expected) {
      threw = true;
    }
    assert threw : "verification should fail when challenge does not match expected value";

    final RecordingTelemetrySink.SignalEvent event =
        sink.lastEvent("android.keystore.attestation.failure");
    assert event != null : "challenge failure event missing";
    assert options()
            .redaction()
            .hashIdentifier("retail-wallet")
            .orElseThrow()
            .equals(event.fields.get("alias_label"));
    assert "challenge_mismatch".equals(event.fields.get("failure_reason"));
    assert "mismatch-provider".equals(event.fields.get("provider"));
  }

  private static TelemetryOptions options() {
    return TelemetryOptions.builder()
        .setTelemetryRedaction(
            TelemetryOptions.Redaction.builder()
                .setSaltHex("0102030405060708")
                .setSaltVersion("2026Q2")
                .setRotationId("rot-telemetry")
                .build())
        .build();
  }

  private static DeviceProfileProvider deviceProfileProvider() {
    return () -> Optional.of(DeviceProfile.of("enterprise"));
  }

  private static final class SuccessfulAttestationProvider implements KeyProvider {
    @Override
    public Optional<java.security.KeyPair> load(final String alias) {
      return Optional.empty();
    }

    @Override
    public java.security.KeyPair generate(final String alias) {
      throw new UnsupportedOperationException();
    }

    @Override
    public java.security.KeyPair generateEphemeral() {
      throw new UnsupportedOperationException();
    }

    @Override
    public boolean isHardwareBacked() {
      return true;
    }

    @Override
    public Optional<AttestationResult> verifyAttestation(
        final String alias, final AttestationVerifier verifier, final byte[] expectedChallenge)
        throws AttestationVerificationException {
      final AttestationResult result =
          new AttestationResult(
              alias,
              List.of(new StubCertificate()),
              AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT,
              AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT,
              new byte[] {0x01},
              new byte[] {0x02},
              true,
              true,
              false);
      return Optional.of(result);
    }

    @Override
    public KeyProviderMetadata metadata() {
      return KeyProviderMetadata.trustedEnvironment("trusted-provider");
    }

    @Override
    public String name() {
      return "trusted-provider";
    }
  }

  private static final class FailingAttestationProvider implements KeyProvider {
    @Override
    public Optional<java.security.KeyPair> load(final String alias) {
      return Optional.empty();
    }

    @Override
    public java.security.KeyPair generate(final String alias) {
      throw new UnsupportedOperationException();
    }

    @Override
    public java.security.KeyPair generateEphemeral() {
      throw new UnsupportedOperationException();
    }

    @Override
    public boolean isHardwareBacked() {
      return true;
    }

    @Override
    public Optional<AttestationResult> verifyAttestation(
        final String alias, final AttestationVerifier verifier, final byte[] expectedChallenge)
        throws AttestationVerificationException {
      throw new AttestationVerificationException("verification_failed");
    }

    @Override
    public KeyProviderMetadata metadata() {
      return KeyProviderMetadata.trustedEnvironment("failing-provider");
    }

    @Override
    public String name() {
      return "failing-provider";
    }
  }

  private static final class ChallengeMismatchProvider implements KeyProvider {
    private static final byte[] PROVIDER_CHALLENGE = new byte[] {0x02};

    @Override
    public Optional<java.security.KeyPair> load(final String alias) {
      return Optional.empty();
    }

    @Override
    public java.security.KeyPair generate(final String alias) {
      throw new UnsupportedOperationException();
    }

    @Override
    public java.security.KeyPair generateEphemeral() {
      throw new UnsupportedOperationException();
    }

    @Override
    public boolean isHardwareBacked() {
      return true;
    }

    @Override
    public Optional<AttestationResult> verifyAttestation(
        final String alias, final AttestationVerifier verifier, final byte[] expectedChallenge)
        throws AttestationVerificationException {
      if (expectedChallenge == null
          || !java.security.MessageDigest.isEqual(expectedChallenge, PROVIDER_CHALLENGE)) {
        throw new AttestationVerificationException("challenge_mismatch");
      }
      final AttestationResult result =
          new AttestationResult(
              alias,
              List.of(new StubCertificate()),
              AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT,
              AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT,
              PROVIDER_CHALLENGE.clone(),
              new byte[] {0x02},
              true,
              true,
              false);
      return Optional.of(result);
    }

    @Override
    public KeyProviderMetadata metadata() {
      return KeyProviderMetadata.trustedEnvironment("mismatch-provider");
    }

    @Override
    public String name() {
      return "mismatch-provider";
    }
  }

  private static final class RecordingTelemetrySink implements TelemetrySink {
    private final List<SignalEvent> events = new java.util.ArrayList<>();

    @Override
    public void onRequest(final org.hyperledger.iroha.android.telemetry.TelemetryRecord record) {}

    @Override
    public void onResponse(
        final org.hyperledger.iroha.android.telemetry.TelemetryRecord record,
        final org.hyperledger.iroha.android.client.ClientResponse response) {}

    @Override
    public void onFailure(
        final org.hyperledger.iroha.android.telemetry.TelemetryRecord record, final Throwable error) {}

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      events.add(new SignalEvent(signalId, Map.copyOf(fields)));
    }

    SignalEvent lastEvent(final String signalId) {
      for (int i = events.size() - 1; i >= 0; --i) {
        final SignalEvent event = events.get(i);
        if (event.signalId.equals(signalId)) {
          return event;
        }
      }
      return null;
    }

    private static final class SignalEvent {
      final String signalId;
      final Map<String, Object> fields;

      SignalEvent(final String signalId, final Map<String, Object> fields) {
        this.signalId = signalId;
        this.fields = fields;
      }
    }
  }

  private static final class StubCertificate extends X509Certificate {
    private static final long serialVersionUID = 1L;
    private final byte[] encoded = new byte[] {0x30, 0x03, 0x02, 0x01, 0x01};

    @Override
    public void checkValidity() {}

    @Override
    public void checkValidity(final Date date) {}

    @Override
    public int getVersion() {
      return 3;
    }

    @Override
    public BigInteger getSerialNumber() {
      return BigInteger.ONE;
    }

    @Override
    public Principal getIssuerDN() {
      return () -> "CN=Stub";
    }

    @Override
    public Principal getSubjectDN() {
      return getIssuerDN();
    }

    @Override
    public Date getNotBefore() {
      return new Date();
    }

    @Override
    public Date getNotAfter() {
      return new Date();
    }

    @Override
    public byte[] getTBSCertificate() {
      return encoded.clone();
    }

    @Override
    public byte[] getSignature() {
      return new byte[] {0x01};
    }

    @Override
    public String getSigAlgName() {
      return "NONE";
    }

    @Override
    public String getSigAlgOID() {
      return "1.2.3";
    }

    @Override
    public byte[] getSigAlgParams() {
      return new byte[] {0x00};
    }

    @Override
    public boolean[] getIssuerUniqueID() {
      return null;
    }

    @Override
    public boolean[] getSubjectUniqueID() {
      return null;
    }

    @Override
    public boolean[] getKeyUsage() {
      return null;
    }

    @Override
    public int getBasicConstraints() {
      return -1;
    }

    @Override
    public byte[] getEncoded() {
      return encoded.clone();
    }

    @Override
    public void verify(final PublicKey key) {}

    @Override
    public void verify(final PublicKey key, final String sigProvider) {}

    @Override
    public String toString() {
      return "StubCertificate";
    }

    @Override
    public PublicKey getPublicKey() {
      return new PublicKey() {
        @Override
        public String getAlgorithm() {
          return "NONE";
        }

        @Override
        public String getFormat() {
          return "RAW";
        }

        @Override
        public byte[] getEncoded() {
          return new byte[] {0x01};
        }
      };
    }

    @Override
    public boolean hasUnsupportedCriticalExtension() {
      return false;
    }

    @Override
    public Set<String> getCriticalExtensionOIDs() {
      return Collections.emptySet();
    }

    @Override
    public Set<String> getNonCriticalExtensionOIDs() {
      return Collections.emptySet();
    }

    @Override
    public byte[] getExtensionValue(final String oid) {
      return null;
    }
  }
}
