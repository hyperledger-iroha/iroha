package org.hyperledger.iroha.android;

import java.security.KeyPair;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;
import org.hyperledger.iroha.android.crypto.SoftwareKeyProvider;
import org.hyperledger.iroha.android.crypto.keystore.KeyAttestation;
import org.hyperledger.iroha.android.crypto.keystore.KeyGenParameters;
import org.hyperledger.iroha.android.crypto.keystore.KeyGenerationResult;
import org.hyperledger.iroha.android.crypto.keystore.KeystoreBackend;
import org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProvider;
import org.hyperledger.iroha.android.telemetry.DeviceProfileProvider;
import org.hyperledger.iroha.android.telemetry.KeystoreTelemetryEmitter;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;

/**
 * Verifies that attestation failures (e.g., challenge unsupported) are emitted via telemetry.
 */
public final class IrohaKeyManagerAttestationTelemetryTests {

  private IrohaKeyManagerAttestationTelemetryTests() {}

  public static void main(final String[] args) throws Exception {
    attestationFailureEmitsTelemetry();
    System.out.println("[IrohaAndroid] Key manager attestation telemetry tests passed.");
  }

  private static void attestationFailureEmitsTelemetry() throws Exception {
    final RecordingTelemetrySink sink = new RecordingTelemetrySink();
    final TelemetryOptions options =
        TelemetryOptions.builder()
            .setEnabled(true)
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setEnabled(true)
                    .setSalt(new byte[] {0x01, 0x02})
                    .setSaltVersion("v1")
                    .setRotationId("rot1")
                    .build())
            .build();
    final KeystoreTelemetryEmitter emitter =
        KeystoreTelemetryEmitter.from(options, sink, DeviceProfileProvider.disabled());

    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(
            new ChallengeUnsupportedBackend(), KeyGenParameters.builder().build());
    final IrohaKeyManager manager =
        IrohaKeyManager.fromProviders(java.util.List.of(provider), emitter);

    boolean threw = false;
    try {
      manager.generateAttestation("alias", new byte[] {0x01, 0x02});
    } catch (final KeyManagementException expected) {
      threw = true;
    }
    assert threw : "Expected attestation challenge to fail";
    assert "android.keystore.attestation.failure".equals(sink.lastSignalId)
        : "Telemetry should record attestation failure";
    final Object reason = sink.lastFields.get("failure_reason");
    assert reason != null && reason.toString().toLowerCase().contains("challenge")
        : "Failure reason should mention challenge";
  }

  private static final class ChallengeUnsupportedBackend implements KeystoreBackend {
    private final SoftwareKeyProvider delegate = new SoftwareKeyProvider();
    private final Map<String, KeyPair> keys = new ConcurrentHashMap<>();
    private final KeyProviderMetadata metadata =
        KeyProviderMetadata.builder("challenge-unsupported")
            .setHardwareBacked(true)
            .setSupportsAttestationCertificates(true)
            .build();

    @Override
    public Optional<KeyPair> load(final String alias) {
      return Optional.ofNullable(keys.get(alias));
    }

    @Override
    public KeyGenerationResult generate(final String alias, final KeyGenParameters parameters)
        throws KeyManagementException {
      final KeyPair pair = delegate.generate(alias);
      keys.put(alias, pair);
      return new KeyGenerationResult(pair, false);
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
      return Optional.empty();
    }
  }

  private static final class RecordingTelemetrySink implements TelemetrySink {
    private String lastSignalId;
    private Map<String, Object> lastFields = Map.of();

    @Override
    public void onRequest(final org.hyperledger.iroha.android.telemetry.TelemetryRecord record) {
      // unused
    }

    @Override
    public void onResponse(
        final org.hyperledger.iroha.android.telemetry.TelemetryRecord record,
        final org.hyperledger.iroha.android.client.ClientResponse response) {
      // unused
    }

    @Override
    public void onFailure(
        final org.hyperledger.iroha.android.telemetry.TelemetryRecord record, final Throwable error) {
      // unused
    }

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      this.lastSignalId = signalId;
      this.lastFields = fields;
    }
  }
}
