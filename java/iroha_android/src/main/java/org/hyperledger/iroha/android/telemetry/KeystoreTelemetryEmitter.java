package org.hyperledger.iroha.android.telemetry;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.crypto.KeyGenerationOutcome;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationResult;

/** Emits Android keystore attestation telemetry events. */
public final class KeystoreTelemetryEmitter {

  private static final KeystoreTelemetryEmitter NOOP =
      new KeystoreTelemetryEmitter(null, TelemetryOptions.Redaction.disabled(), DeviceProfileProvider.disabled());

  private final TelemetrySink sink;
  private final TelemetryOptions.Redaction redaction;
  private final DeviceProfileProvider deviceProfileProvider;

  private KeystoreTelemetryEmitter(
      final TelemetrySink sink,
      final TelemetryOptions.Redaction redaction,
      final DeviceProfileProvider deviceProfileProvider) {
    this.sink = sink;
    this.redaction = redaction;
    this.deviceProfileProvider = deviceProfileProvider;
  }

  /** Returns a no-op emitter. */
  public static KeystoreTelemetryEmitter noop() {
    return NOOP;
  }

  /**
   * Creates an emitter backed by {@link TelemetryOptions} and {@link TelemetrySink}. Returns a
   * no-op instance when telemetry is disabled or the sink is missing.
   */
  public static KeystoreTelemetryEmitter from(
      final TelemetryOptions options,
      final TelemetrySink sink,
      final DeviceProfileProvider provider) {
    if (options == null || sink == null || !options.enabled()) {
      return noop();
    }
    final DeviceProfileProvider safeProvider =
        provider == null ? DeviceProfileProvider.disabled() : provider;
    return new KeystoreTelemetryEmitter(sink, options.redaction(), safeProvider);
  }

  /** Records a successful attestation verification/generation. */
  public void recordResult(
      final String alias, final KeyProviderMetadata metadata, final AttestationResult result) {
    if (sink == null || redaction == null || result == null) {
      return;
    }
    final Optional<String> aliasLabel = aliasLabel(alias);
    if (aliasLabel.isEmpty()) {
      return;
    }
    final String securityLevel = result.attestationSecurityLevel().name().toLowerCase(Locale.ROOT);
    final String digest = leafCertificateDigest(result);
    final String brandBucket = deviceBrandBucket();
    sink.emitSignal(
        "android.keystore.attestation.result",
        Map.of(
            "alias_label", aliasLabel.get(),
            "security_level", securityLevel,
            "attestation_digest", digest,
            "device_brand_bucket", brandBucket,
            "provider", metadata == null ? "unknown" : metadata.name()));
  }

  /**
   * Records a key generation event, including the preference requested and the hardware route used.
   */
  public void recordKeyGeneration(
      final String alias,
      final IrohaKeyManager.KeySecurityPreference preference,
      final KeyProviderMetadata metadata,
      final KeyGenerationOutcome.Route route,
      final boolean fallback) {
    if (sink == null || redaction == null) {
      return;
    }
    final Optional<String> aliasLabel = aliasLabel(alias);
    if (aliasLabel.isEmpty()) {
      return;
    }
    final String preferenceLabel =
        preference == null ? "unknown" : preference.name().toLowerCase(Locale.ROOT);
    final String routeLabel = route == null ? "unknown" : route.name().toLowerCase(Locale.ROOT);
    sink.emitSignal(
        "android.keystore.keygen",
        Map.of(
            "alias_label", aliasLabel.get(),
            "preference", preferenceLabel,
            "route", routeLabel,
            "fallback", fallback,
            "provider", metadata == null ? "unknown" : metadata.name(),
            "device_brand_bucket", deviceBrandBucket()));
  }

  /**
   * Records a failure to validate Ed25519 SPKI outputs from a key provider.
   */
  public void recordKeyValidationFailure(
      final String alias,
      final IrohaKeyManager.KeySecurityPreference preference,
      final KeyProviderMetadata metadata,
      final String phase,
      final String reason,
      final int spkiLength,
      final int expectedLength,
      final String spkiPrefixHex) {
    if (sink == null || redaction == null) {
      return;
    }
    final Optional<String> aliasLabel = aliasLabel(alias);
    if (aliasLabel.isEmpty()) {
      return;
    }
    final String preferenceLabel =
        preference == null ? "unknown" : preference.name().toLowerCase(Locale.ROOT);
    final String phaseLabel = phase == null || phase.isBlank() ? "unknown" : phase;
    final String failureReason = reason == null || reason.isBlank() ? "unknown" : reason;
    final String prefix = spkiPrefixHex == null || spkiPrefixHex.isBlank() ? "unknown" : spkiPrefixHex;
    sink.emitSignal(
        "android.keystore.key_validation.failure",
        Map.of(
            "alias_label", aliasLabel.get(),
            "preference", preferenceLabel,
            "provider", metadata == null ? "unknown" : metadata.name(),
            "phase", phaseLabel,
            "reason", failureReason,
            "spki_length", spkiLength,
            "expected_spki_length", expectedLength,
            "spki_prefix", prefix,
            "device_brand_bucket", deviceBrandBucket()));
  }

  /** Records an attestation failure surfaced by {@link AttestationVerificationException}. */
  public void recordFailure(
      final String alias, final KeyProviderMetadata metadata, final String reason) {
    if (sink == null || redaction == null) {
      return;
    }
    final Optional<String> aliasLabel = aliasLabel(alias);
    if (aliasLabel.isEmpty()) {
      return;
    }
    final String failureReason = reason == null || reason.isBlank() ? "unknown" : reason;
    sink.emitSignal(
        "android.keystore.attestation.failure",
        Map.of(
            "alias_label", aliasLabel.get(),
            "failure_reason", failureReason,
            "provider", metadata == null ? "unknown" : metadata.name()));
  }


  private Optional<String> aliasLabel(final String alias) {
    if (alias == null || alias.isBlank()) {
      return Optional.empty();
    }
    if (!redaction.enabled()) {
      return Optional.empty();
    }
    return redaction.hashIdentifier(alias);
  }

  private String deviceBrandBucket() {
    return deviceProfileProvider
        .snapshot()
        .map(DeviceProfile::bucket)
        .orElse("unknown");
  }

  private static String leafCertificateDigest(final AttestationResult result) {
    try {
      final MessageDigest digest = MessageDigest.getInstance("SHA-256");
      final byte[] encoded = result.leafCertificate().getEncoded();
      final byte[] hash = digest.digest(encoded);
      return bytesToHex(hash);
    } catch (final NoSuchAlgorithmException | java.security.cert.CertificateEncodingException ex) {
      return "error";
    }
  }

  private static String bytesToHex(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte value : bytes) {
      builder.append(String.format(Locale.ROOT, "%02x", value));
    }
    return builder.toString();
  }
}
