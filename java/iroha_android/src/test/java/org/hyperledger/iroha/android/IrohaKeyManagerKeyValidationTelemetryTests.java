package org.hyperledger.iroha.android;

import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.spec.ECGenParameterSpec;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;
import org.hyperledger.iroha.android.IrohaKeyManager.KeyProvider;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;
import org.hyperledger.iroha.android.crypto.SoftwareKeyProvider;
import org.hyperledger.iroha.android.telemetry.DeviceProfileProvider;
import org.hyperledger.iroha.android.telemetry.KeystoreTelemetryEmitter;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetryRecord;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;

/** Verifies invalid Ed25519 SPKI outputs are surfaced via telemetry. */
public final class IrohaKeyManagerKeyValidationTelemetryTests {

  private IrohaKeyManagerKeyValidationTelemetryTests() {}

  public static void main(final String[] args) throws Exception {
    keyValidationFailureEmitsTelemetry();
    System.out.println("[IrohaAndroid] Key manager validation telemetry tests passed.");
  }

  private static void keyValidationFailureEmitsTelemetry() throws Exception {
    final RecordingTelemetrySink sink = new RecordingTelemetrySink();
    final TelemetryOptions options =
        TelemetryOptions.builder()
            .setEnabled(true)
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setEnabled(true)
                    .setSalt(new byte[] {0x02, 0x03})
                    .setSaltVersion("v2")
                    .setRotationId("rot2")
                    .build())
            .build();
    final KeystoreTelemetryEmitter emitter =
        KeystoreTelemetryEmitter.from(options, sink, DeviceProfileProvider.disabled());

    final KeyProvider invalidProvider =
        new InvalidAlgorithmProvider(KeyProviderMetadata.trustedEnvironment("invalid-provider"));
    final IrohaKeyManager manager =
        IrohaKeyManager.fromProviders(
            List.of(invalidProvider, new SoftwareKeyProvider()), emitter);

    final String alias = "telemetry-invalid";
    manager.generateOrLoad(alias, IrohaKeyManager.KeySecurityPreference.HARDWARE_PREFERRED);

    final RecordingTelemetrySink.SignalEvent event =
        sink.lastEvent("android.keystore.key_validation.failure");
    assert event != null : "key validation telemetry event missing";
    final String expectedAlias =
        options.redaction().hashIdentifier(alias).orElseThrow();
    assert expectedAlias.equals(event.fields.get("alias_label"));
    assert "invalid-provider".equals(event.fields.get("provider"));
    assert "hardware_preferred".equals(event.fields.get("preference"));
    final String phase = String.valueOf(event.fields.get("phase"));
    assert phase.equals("load") || phase.equals("generate") : "unexpected phase: " + phase;
    final String reason = String.valueOf(event.fields.get("reason"));
    assert reason.equals("length_mismatch") || reason.equals("prefix_mismatch");
    final Number length = (Number) event.fields.get("spki_length");
    assert length.longValue() > 0 : "spki_length should be populated";
  }

  private static final class InvalidAlgorithmProvider implements KeyProvider {
    private final KeyPair invalidKeyPair;
    private final KeyProviderMetadata metadata;
    private final ConcurrentHashMap<String, Boolean> aliases = new ConcurrentHashMap<>();

    private InvalidAlgorithmProvider(final KeyProviderMetadata metadata) {
      this.metadata = metadata;
      try {
        final KeyPairGenerator generator = KeyPairGenerator.getInstance("EC");
        generator.initialize(new ECGenParameterSpec("secp256r1"));
        this.invalidKeyPair = generator.generateKeyPair();
      } catch (final Exception ex) {
        throw new IllegalStateException("Failed to create invalid keypair", ex);
      }
    }

    @Override
    public Optional<KeyPair> load(final String alias) {
      aliases.put(alias, Boolean.TRUE);
      return Optional.of(invalidKeyPair);
    }

    @Override
    public KeyPair generate(final String alias) {
      aliases.put(alias, Boolean.TRUE);
      return invalidKeyPair;
    }

    @Override
    public KeyPair generateEphemeral() {
      return invalidKeyPair;
    }

    @Override
    public boolean isHardwareBacked() {
      return metadata.hardwareBacked();
    }

    @Override
    public KeyProviderMetadata metadata() {
      return metadata;
    }

    @Override
    public String name() {
      return metadata.name();
    }
  }

  private static final class RecordingTelemetrySink implements TelemetrySink {
    private final Map<String, SignalEvent> events = new ConcurrentHashMap<>();

    @Override
    public void onRequest(final TelemetryRecord record) {}

    @Override
    public void onResponse(
        final TelemetryRecord record,
        final org.hyperledger.iroha.android.client.ClientResponse response) {}

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {}

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      events.put(signalId, new SignalEvent(signalId, fields));
    }

    private SignalEvent lastEvent(final String signalId) {
      return events.get(signalId);
    }

    private static final class SignalEvent {
      private final String id;
      private final Map<String, Object> fields;

      private SignalEvent(final String id, final Map<String, Object> fields) {
        this.id = id;
        this.fields = fields;
      }
    }
  }
}
