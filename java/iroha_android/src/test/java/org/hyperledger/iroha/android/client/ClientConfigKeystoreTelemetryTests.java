package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.net.URI;
import java.security.KeyPair;
import java.security.Principal;
import java.security.PublicKey;
import java.security.cert.X509Certificate;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Date;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.atomic.AtomicReference;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.IrohaKeyManager.KeyProvider;
import org.hyperledger.iroha.android.client.ClientConfig.ExportOptions;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;
import org.hyperledger.iroha.android.crypto.keystore.KeyAttestation;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationResult;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationVerificationException;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationVerifier;
import org.hyperledger.iroha.android.telemetry.DeviceProfile;
import org.hyperledger.iroha.android.telemetry.DeviceProfileProvider;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetryRecord;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;

/** Verifies ClientConfig wires keystore telemetry into export options. */
public final class ClientConfigKeystoreTelemetryTests {

  public static void main(final String[] args) throws Exception {
    exportOptionsKeyManagerEmitsTelemetry();
    exportOptionsKeyManagerEmitsFailureTelemetry();
    System.out.println("[IrohaAndroid] ClientConfig keystore telemetry tests passed.");
  }

  private static void exportOptionsKeyManagerEmitsTelemetry() throws Exception {
    final RecordingTelemetrySink sink = new RecordingTelemetrySink();
    final ClientConfig config =
        configWithExportOptions(sink, new SuccessfulAttestationProvider());
    final IrohaKeyManager keyManager = config.exportOptions().keyManager();

    final AttestationVerifier verifier =
        AttestationVerifier.builder().addTrustedRoot(new StubCertificate()).build();
    final Optional<AttestationResult> result =
        keyManager.verifyAttestation("retail-wallet", verifier, null);
    assert result.isPresent() : "verification result expected";

    final RecordingTelemetrySink.SignalEvent event =
        sink.lastEvent("android.keystore.attestation.result");
    assert event != null : "attestation result event missing";
    assert hashedAlias().equals(event.fields.get("alias_label"));
    assert "trusted-provider".equals(event.fields.get("provider"));
    assert "trusted_environment".equals(event.fields.get("security_level"));
    assert "enterprise".equals(event.fields.get("device_brand_bucket"));
    final String digest = (String) event.fields.get("attestation_digest");
    assert digest != null && digest.length() == 64 : "digest must be hex encoded";
  }

  private static void exportOptionsKeyManagerEmitsFailureTelemetry() throws Exception {
    final RecordingTelemetrySink sink = new RecordingTelemetrySink();
    final ClientConfig config =
        configWithExportOptions(sink, new FailingAttestationProvider());
    final IrohaKeyManager keyManager = config.exportOptions().keyManager();

    final AttestationVerifier verifier =
        AttestationVerifier.builder().addTrustedRoot(new StubCertificate()).build();

    boolean threw = false;
    try {
      keyManager.verifyAttestation("retail-wallet", verifier, null);
    } catch (final AttestationVerificationException expected) {
      threw = true;
    }
    assert threw : "verification should fail";

    final RecordingTelemetrySink.SignalEvent event =
        sink.lastEvent("android.keystore.attestation.failure");
    assert event != null : "attestation failure event missing";
    assert hashedAlias().equals(event.fields.get("alias_label"));
    assert "verification_failed".equals(event.fields.get("failure_reason"));
    assert "failing-provider".equals(event.fields.get("provider"));
  }

  private static ClientConfig configWithExportOptions(
      final RecordingTelemetrySink sink, final KeyProvider provider) throws Exception {
    final IrohaKeyManager keyManager = IrohaKeyManager.fromProviders(List.of(provider));
    final ExportOptions exportOptions =
        ExportOptions.builder().setKeyManager(keyManager).build();
    return ClientConfig.builder()
        .setBaseUri(URI.create("http://127.0.0.1:8080"))
        .setRequestTimeout(Duration.ofSeconds(5))
        .setTelemetryOptions(telemetryOptions())
        .setTelemetrySink(sink)
        .setDeviceProfileProvider(deviceProfileProvider())
        .setExportOptions(exportOptions)
        .build();
  }

  private static TelemetryOptions telemetryOptions() {
    return TelemetryOptions.builder()
        .setTelemetryRedaction(
            TelemetryOptions.Redaction.builder()
                .setSaltHex("0102030405060708090a0b0c0d0e0f00")
                .setSaltVersion("2026Q2")
                .setRotationId("rot-android")
                .build())
        .build();
  }

  private static DeviceProfileProvider deviceProfileProvider() {
    return () -> Optional.of(DeviceProfile.of("enterprise"));
  }

  private static String hashedAlias() {
    return telemetryOptions()
        .redaction()
        .hashIdentifier("retail-wallet")
        .orElseThrow();
  }

  private static final class SuccessfulAttestationProvider implements KeyProvider {
    @Override
    public Optional<KeyPair> load(final String alias) {
      return Optional.empty();
    }

    @Override
    public KeyPair generate(final String alias) {
      throw new UnsupportedOperationException();
    }

    @Override
    public KeyPair generateEphemeral() {
      throw new UnsupportedOperationException();
    }

    @Override
    public boolean isHardwareBacked() {
      return true;
    }

    @Override
    public Optional<KeyAttestation> generateAttestation(final String alias, final byte[] challenge) {
      return Optional.empty();
    }

    @Override
    public Optional<AttestationResult> verifyAttestation(
        final String alias, final AttestationVerifier verifier, final byte[] expectedChallenge) {
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
    public Optional<KeyPair> load(final String alias) {
      return Optional.empty();
    }

    @Override
    public KeyPair generate(final String alias) {
      throw new UnsupportedOperationException();
    }

    @Override
    public KeyPair generateEphemeral() {
      throw new UnsupportedOperationException();
    }

    @Override
    public boolean isHardwareBacked() {
      return true;
    }

    @Override
    public Optional<KeyAttestation> generateAttestation(final String alias, final byte[] challenge) {
      return Optional.empty();
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

  private static final class StubCertificate extends X509Certificate {
    private static final long serialVersionUID = 1L;
    private final AtomicReference<byte[]> encoded = new AtomicReference<>(new byte[] {0x00});

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
      return () -> "CN=Stub";
    }

    @Override
    public Date getNotBefore() {
      return new Date(0L);
    }

    @Override
    public Date getNotAfter() {
      return new Date(System.currentTimeMillis() + 60_000L);
    }

    @Override
    public byte[] getTBSCertificate() {
      return new byte[] {0x01};
    }

    @Override
    public byte[] getSignature() {
      return new byte[] {0x02};
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
      return new byte[0];
    }

    @Override
    public boolean[] getIssuerUniqueID() {
      return new boolean[0];
    }

    @Override
    public boolean[] getSubjectUniqueID() {
      return new boolean[0];
    }

    @Override
    public boolean[] getKeyUsage() {
      return new boolean[0];
    }

    @Override
    public int getBasicConstraints() {
      return -1;
    }

    @Override
    public byte[] getEncoded() {
      return encoded.get().clone();
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
    public byte[] getExtensionValue(final String oid) {
      return null;
    }

    @Override
    public boolean hasUnsupportedCriticalExtension() {
      return false;
    }

    @Override
    public java.util.Set<String> getCriticalExtensionOIDs() {
      return null;
    }

    @Override
    public java.util.Set<String> getNonCriticalExtensionOIDs() {
      return null;
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
          return new byte[0];
        }
      };
    }
  }

  private static final class RecordingTelemetrySink implements TelemetrySink {
    private final List<SignalEvent> events = new ArrayList<>();

    @Override
    public void onRequest(final TelemetryRecord record) {}

    @Override
    public void onResponse(final TelemetryRecord record, final ClientResponse response) {}

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {}

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      events.add(new SignalEvent(signalId, fields));
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

    static final class SignalEvent {
      final String signalId;
      final Map<String, Object> fields;

      SignalEvent(final String signalId, final Map<String, Object> fields) {
        this.signalId = signalId;
        this.fields = Map.copyOf(fields);
      }
    }
  }
}
