package org.hyperledger.iroha.android;

import java.security.KeyPair;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.crypto.Ed25519Signer;
import org.hyperledger.iroha.android.crypto.MlDsaPrivateKey;
import org.hyperledger.iroha.android.crypto.MlDsaPublicKey;
import org.hyperledger.iroha.android.crypto.MlDsaSigner;
import org.hyperledger.iroha.android.crypto.NativeSignerBridge;
import org.hyperledger.iroha.android.crypto.SoftwareKeyProvider;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;
import org.hyperledger.iroha.android.crypto.SigningAlgorithm;
import org.hyperledger.iroha.android.crypto.Signer;
import org.hyperledger.iroha.android.crypto.export.KeyExportBundle;
import org.hyperledger.iroha.android.crypto.export.KeyExportException;
import org.hyperledger.iroha.android.crypto.export.KeyExportStore;
import org.hyperledger.iroha.android.crypto.export.KeyPassphraseProvider;
import org.hyperledger.iroha.android.crypto.keystore.KeyAttestation;
import org.hyperledger.iroha.android.crypto.keystore.KeyGenParameters;
import org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProvider;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationResult;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationVerificationException;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationVerifier;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.android.telemetry.KeystoreTelemetryEmitter;

/**
 * Coordinates key generation and lookup for Iroha Android clients.
 *
 * <p>The manager accepts one or more {@link KeyProvider} implementations and
 * routes requests according to the supplied {@link KeySecurityPreference}. When
 * a hardware-backed provider is unavailable, the manager falls back to software
 * providers so developers can continue testing on emulators and desktop JVMs.
 *
 * <p>Future revisions will supply Android Keystore/StrongBox backed providers.
 */
public final class IrohaKeyManager {

  private static final byte[] ED25519_SPKI_PREFIX =
      new byte[] {
        0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00
      };
  private static final int ED25519_SPKI_SIZE = 44;

  private final List<KeyProvider> providers;
  private final KeystoreTelemetryEmitter keystoreTelemetry;
  private final SigningAlgorithm signingAlgorithm;

  private IrohaKeyManager(
      final List<KeyProvider> providers,
      final KeystoreTelemetryEmitter keystoreTelemetry,
      final SigningAlgorithm signingAlgorithm) {
    if (providers.isEmpty()) {
      throw new IllegalArgumentException("At least one KeyProvider is required");
    }
    this.providers = List.copyOf(providers);
    this.keystoreTelemetry =
        keystoreTelemetry == null ? KeystoreTelemetryEmitter.noop() : keystoreTelemetry;
    this.signingAlgorithm =
        signingAlgorithm == null ? SigningAlgorithm.ED25519 : signingAlgorithm;
  }

  private IrohaKeyManager(final List<KeyProvider> providers) {
    this(providers, KeystoreTelemetryEmitter.noop(), SigningAlgorithm.ED25519);
  }

  /** Creates a manager that uses the provided providers in priority order. */
  public static IrohaKeyManager fromProviders(final List<KeyProvider> providers) {
    return fromProviders(providers, SigningAlgorithm.ED25519);
  }

  /** Creates a manager that uses the provided providers in priority order. */
  public static IrohaKeyManager fromProviders(
      final List<KeyProvider> providers, final SigningAlgorithm signingAlgorithm) {
    return new IrohaKeyManager(providers, KeystoreTelemetryEmitter.noop(), signingAlgorithm);
  }

  /** Creates a manager with explicit keystore telemetry configuration. */
  public static IrohaKeyManager fromProviders(
      final List<KeyProvider> providers, final KeystoreTelemetryEmitter telemetry) {
    return new IrohaKeyManager(providers, telemetry, SigningAlgorithm.ED25519);
  }

  /** Creates a manager with explicit keystore telemetry configuration. */
  public static IrohaKeyManager fromProviders(
      final List<KeyProvider> providers,
      final KeystoreTelemetryEmitter telemetry,
      final SigningAlgorithm signingAlgorithm) {
    return new IrohaKeyManager(providers, telemetry, signingAlgorithm);
  }

  /** Creates a manager with a software fallback provider only (desktop/emulator friendly). */
  public static IrohaKeyManager withSoftwareFallback() {
    return withSoftwareFallback(SigningAlgorithm.ED25519);
  }

  /** Creates a manager with a software fallback provider only (desktop/emulator friendly). */
  public static IrohaKeyManager withSoftwareFallback(final SigningAlgorithm signingAlgorithm) {
    return new IrohaKeyManager(
        List.of(new SoftwareKeyProvider(signingAlgorithm)),
        KeystoreTelemetryEmitter.noop(),
        signingAlgorithm);
  }

  /**
   * Creates a manager backed by an exportable software provider that persists deterministic key
   * exports using {@code exportStore}.
   */
  public static IrohaKeyManager withExportableSoftwareKeys(
      final KeyExportStore exportStore, final KeyPassphraseProvider passphraseProvider) {
    return withExportableSoftwareKeys(
        exportStore, passphraseProvider, SigningAlgorithm.ED25519);
  }

  /**
   * Creates a manager backed by an exportable software provider that persists deterministic key
   * exports using {@code exportStore}.
   */
  public static IrohaKeyManager withExportableSoftwareKeys(
      final KeyExportStore exportStore,
      final KeyPassphraseProvider passphraseProvider,
      final SigningAlgorithm signingAlgorithm) {
    Objects.requireNonNull(exportStore, "exportStore");
    Objects.requireNonNull(passphraseProvider, "passphraseProvider");
    return new IrohaKeyManager(
        List.of(
            new SoftwareKeyProvider(
                SoftwareKeyProvider.ProviderPolicy.BOUNCY_CASTLE_REQUIRED,
                exportStore,
                passphraseProvider,
                signingAlgorithm)),
        KeystoreTelemetryEmitter.noop(),
        signingAlgorithm);
  }

  /**
   * Creates a manager that attempts to use hardware-backed keystore providers (when available) and
   * falls back to the software provider for emulators/desktop JVMs.
   */
  public static IrohaKeyManager withDefaultProviders() {
    return withDefaultProviders(KeyGenParameters.builder().build());
  }

  /** Creates a manager that uses the supplied app-level signing algorithm. */
  public static IrohaKeyManager withDefaultProviders(final SigningAlgorithm signingAlgorithm) {
    return withDefaultProviders(
        KeyGenParameters.builder().setSigningAlgorithm(signingAlgorithm).build());
  }

  /**
   * Creates a manager that attempts to use hardware-backed keystore providers with the supplied
   * generation parameters and falls back to a software provider.
   */
  public static IrohaKeyManager withDefaultProviders(final KeyGenParameters keyGenParameters) {
    final SigningAlgorithm signingAlgorithm = keyGenParameters.signingAlgorithm();
    final List<KeyProvider> providers = new ArrayList<>();
    if (signingAlgorithm.supportsHardwareBackedKeys()) {
      KeystoreKeyProvider.maybeCreate(keyGenParameters).ifPresent(providers::add);
    }
    providers.add(new SoftwareKeyProvider(signingAlgorithm));
    return new IrohaKeyManager(providers, KeystoreTelemetryEmitter.noop(), signingAlgorithm);
  }

  /**
   * Creates a manager that attempts to use hardware-backed keystore providers with telemetry and
   * falls back to a software provider.
   */
  public static IrohaKeyManager withDefaultProviders(
      final KeyGenParameters keyGenParameters, final KeystoreTelemetryEmitter telemetry) {
    final SigningAlgorithm signingAlgorithm = keyGenParameters.signingAlgorithm();
    final List<KeyProvider> providers = new ArrayList<>();
    if (signingAlgorithm.supportsHardwareBackedKeys()) {
      KeystoreKeyProvider.maybeCreate(keyGenParameters).ifPresent(providers::add);
    }
    providers.add(new SoftwareKeyProvider(signingAlgorithm));
    return new IrohaKeyManager(providers, telemetry, signingAlgorithm);
  }

  /** Returns a copy of this manager that emits keystore telemetry through {@code telemetry}. */
  public IrohaKeyManager withTelemetry(final KeystoreTelemetryEmitter telemetry) {
    return new IrohaKeyManager(this.providers, telemetry, signingAlgorithm);
  }

  /**
   * Generates or loads a key associated with {@code alias} honouring the requested security
   * preference.
   *
   * @throws KeyManagementException when no provider can satisfy the request
   */
  public KeyPair generateOrLoad(final String alias, final KeySecurityPreference preference)
      throws KeyManagementException {
    Objects.requireNonNull(alias, "alias");
    Objects.requireNonNull(preference, "preference");
    if (alias.isBlank()) {
      throw new IllegalArgumentException("alias must not be blank");
    }
    enforceAlgorithmPreference(preference);

    final List<KeyProvider> ordered = orderedProviders(preference);
    KeyManagementException lastError = null;
    for (final KeyProvider provider : ordered) {
      try {
        final Optional<KeyPair> existing = provider.load(alias);
        if (existing.isPresent()) {
          ensureExpectedKeyPair(
              alias, preference, existing.get(), provider.metadata(), "load");
          return existing.get();
        }
      } catch (final KeyManagementException e) {
        lastError = e;
      }
    }

    for (final KeyProvider provider : ordered) {
      try {
        final org.hyperledger.iroha.android.crypto.KeyGenerationOutcome outcome =
            provider.generateWithOutcome(alias, preference);
        ensureExpectedKeyPair(
            alias, preference, outcome.keyPair(), provider.metadata(), "generate");
        enforcePreference(preference, provider.metadata(), outcome);
        recordKeyGenerationTelemetry(alias, preference, provider.metadata(), outcome);
        return outcome.keyPair();
      } catch (final KeyManagementException e) {
        lastError = e;
      }
    }
    if (lastError != null) {
      throw lastError;
    }
    throw new KeyManagementException("No key providers available for alias=" + alias);
  }

  private void enforcePreference(
      final KeySecurityPreference preference,
      final KeyProviderMetadata metadata,
      final org.hyperledger.iroha.android.crypto.KeyGenerationOutcome outcome)
      throws KeyManagementException {
    if (preference == KeySecurityPreference.STRONGBOX_REQUIRED
        && outcome.route()
            != org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.STRONGBOX) {
      throw new KeyManagementException(
          "StrongBox required but provider "
              + metadata.name()
              + " generated key using "
              + outcome.route());
    }
    if (preference == KeySecurityPreference.HARDWARE_REQUIRED
        && outcome.route()
            == org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.SOFTWARE) {
      throw new KeyManagementException(
          "Hardware-backed key required but provider " + metadata.name() + " generated software key");
    }
  }

  private void enforceAlgorithmPreference(final KeySecurityPreference preference)
      throws KeyManagementException {
    if (signingAlgorithm.supportsHardwareBackedKeys()) {
      return;
    }
    if (preference != KeySecurityPreference.SOFTWARE_ONLY) {
      throw new KeyManagementException(
          signingAlgorithm.providerName() + " signing keys currently support SOFTWARE_ONLY");
    }
  }

  private void recordKeyGenerationTelemetry(
      final String alias,
      final KeySecurityPreference preference,
      final KeyProviderMetadata metadata,
      final org.hyperledger.iroha.android.crypto.KeyGenerationOutcome outcome) {
    final boolean fallback =
        (preference == KeySecurityPreference.STRONGBOX_REQUIRED
                || preference == KeySecurityPreference.STRONGBOX_PREFERRED)
            && outcome.route()
                != org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.STRONGBOX;
    keystoreTelemetry.recordKeyGeneration(alias, preference, metadata, outcome.route(), fallback);
  }

  private void ensureExpectedKeyPair(
      final String alias,
      final KeySecurityPreference preference,
      final KeyPair keyPair,
      final KeyProviderMetadata metadata,
      final String phase)
      throws KeyManagementException {
    final KeyMaterialValidation validation = validateKeyPair(signingAlgorithm, keyPair);
    if (!validation.valid) {
      recordKeyValidationFailure(alias, preference, metadata, phase, validation);
      final String provider = metadata == null ? "unknown" : metadata.name();
      throw new KeyManagementException(
          "Provider "
              + provider
              + " returned unexpected "
              + signingAlgorithm.providerName()
              + " key material ("
              + validation.detail()
              + ")");
    }
  }

  private void recordKeyValidationFailure(
      final String alias,
      final KeySecurityPreference preference,
      final KeyProviderMetadata metadata,
      final String phase,
      final KeyMaterialValidation validation) {
    keystoreTelemetry.recordKeyValidationFailure(
        alias,
        preference,
        metadata,
        phase,
        validation.reason,
        validation.length,
        validation.expectedLength,
        validation.prefixHex);
  }

  private static KeyMaterialValidation validateKeyPair(
      final SigningAlgorithm signingAlgorithm, final KeyPair keyPair) {
    return switch (signingAlgorithm) {
      case ED25519 -> validateEd25519KeyPair(keyPair);
      case ML_DSA -> validateMlDsaKeyPair(keyPair);
    };
  }

  private static KeyMaterialValidation validateEd25519KeyPair(final KeyPair keyPair) {
    if (keyPair == null || keyPair.getPublic() == null) {
      return KeyMaterialValidation.invalid(0, ED25519_SPKI_SIZE, "", "public_key_missing");
    }
    return validateEd25519Spki(keyPair.getPublic().getEncoded());
  }

  private static KeyMaterialValidation validateEd25519Spki(final byte[] encoded) {
    if (encoded == null || encoded.length == 0) {
      return KeyMaterialValidation.invalid(0, ED25519_SPKI_SIZE, "", "spki_missing");
    }
    final int length = encoded.length;
    final int prefixLen = Math.min(ED25519_SPKI_PREFIX.length, length);
    final String prefixHex = toHex(encoded, prefixLen);
    if (length != ED25519_SPKI_SIZE) {
      return KeyMaterialValidation.invalid(length, ED25519_SPKI_SIZE, prefixHex, "length_mismatch");
    }
    for (int i = 0; i < ED25519_SPKI_PREFIX.length; i++) {
      if (encoded[i] != ED25519_SPKI_PREFIX[i]) {
        return KeyMaterialValidation.invalid(length, ED25519_SPKI_SIZE, prefixHex, "prefix_mismatch");
      }
    }
    return KeyMaterialValidation.valid(length, ED25519_SPKI_SIZE, prefixHex);
  }

  private static KeyMaterialValidation validateMlDsaKeyPair(final KeyPair keyPair) {
    if (keyPair == null || !(keyPair.getPublic() instanceof MlDsaPublicKey)) {
      return KeyMaterialValidation.invalid(0, 0, "", "mldsa_public_key_missing");
    }
    if (!(keyPair.getPrivate() instanceof MlDsaPrivateKey)) {
      final byte[] publicBytes = keyPair.getPublic().getEncoded();
      final int length = publicBytes == null ? 0 : publicBytes.length;
      return KeyMaterialValidation.invalid(
          length,
          length,
          toHex(publicBytes, Math.min(12, length)),
          "mldsa_private_key_missing");
    }
    final byte[] encodedPublic = keyPair.getPublic().getEncoded();
    final byte[] expected;
    try {
      expected =
          NativeSignerBridge.publicKeyFromPrivate(
              SigningAlgorithm.ML_DSA, keyPair.getPrivate().getEncoded());
    } catch (final RuntimeException ex) {
      return KeyMaterialValidation.invalid(
          encodedPublic.length,
          encodedPublic.length,
          toHex(encodedPublic, Math.min(12, encodedPublic.length)),
          "mldsa_public_key_derivation_failed");
    }
    if (!java.util.Arrays.equals(expected, encodedPublic)) {
      return KeyMaterialValidation.invalid(
          encodedPublic.length,
          expected.length,
          toHex(encodedPublic, Math.min(12, encodedPublic.length)),
          "mldsa_public_key_mismatch");
    }
    return KeyMaterialValidation.valid(
        encodedPublic.length,
        expected.length,
        toHex(encodedPublic, Math.min(12, encodedPublic.length)));
  }

  private static String toHex(final byte[] bytes, final int length) {
    if (bytes == null || length <= 0) {
      return "";
    }
    final int limit = Math.min(bytes.length, length);
    final StringBuilder builder = new StringBuilder(limit * 2);
    for (int i = 0; i < limit; i++) {
      builder.append(String.format("%02x", bytes[i]));
    }
    return builder.toString();
  }

  private static final class KeyMaterialValidation {
    private final boolean valid;
    private final int length;
    private final int expectedLength;
    private final String prefixHex;
    private final String reason;

    private KeyMaterialValidation(
        final boolean valid,
        final int length,
        final int expectedLength,
        final String prefixHex,
        final String reason) {
      this.valid = valid;
      this.length = length;
      this.expectedLength = expectedLength;
      this.prefixHex = prefixHex == null ? "" : prefixHex;
      this.reason = reason == null ? "unknown" : reason;
    }

    private static KeyMaterialValidation valid(
        final int length, final int expectedLength, final String prefixHex) {
      return new KeyMaterialValidation(true, length, expectedLength, prefixHex, "ok");
    }

    private static KeyMaterialValidation invalid(
        final int length, final int expectedLength, final String prefixHex, final String reason) {
      return new KeyMaterialValidation(false, length, expectedLength, prefixHex, reason);
    }

    private String detail() {
      return "reason=" + reason
          + ", key_len=" + length
          + ", expected_len=" + expectedLength
          + ", prefix=" + (prefixHex.isEmpty() ? "unknown" : prefixHex);
    }
  }

  /**
   * Generates an ephemeral key pair suitable for offline transaction signing.
   *
   * <p>Ephemeral keys are never persisted; providers may favour software-backed generation even
   * when hardware-backed providers are present to avoid exhausting secure hardware key slots.
   */
  public KeyPair generateEphemeral() throws KeyManagementException {
    KeyManagementException lastError = null;
    for (final KeyProvider provider : providers) {
      try {
        final KeyPair keyPair = provider.generateEphemeral();
        ensureExpectedKeyPair(null, null, keyPair, provider.metadata(), "ephemeral");
        return keyPair;
      } catch (final KeyManagementException e) {
        lastError = e;
      }
    }
    if (lastError != null) {
      throw lastError;
    }
    throw new KeyManagementException("No key providers available for ephemeral keys");
  }

  /**
   * Produces a signer bound to the key referenced by {@code alias}. The key is lazily created if it
   * does not exist yet, matching {@link #generateOrLoad(String, KeySecurityPreference)} semantics.
   */
  public Signer signerForAlias(final String alias, final KeySecurityPreference preference)
      throws KeyManagementException, SigningException {
    final KeyPair keyPair = generateOrLoad(alias, preference);
    return switch (signingAlgorithm) {
      case ED25519 -> new Ed25519Signer(keyPair.getPrivate(), keyPair.getPublic());
      case ML_DSA -> {
        if (!(keyPair.getPrivate() instanceof MlDsaPrivateKey)) {
          throw new SigningException("Expected an ML-DSA private key for alias=" + alias);
        }
        if (!(keyPair.getPublic() instanceof MlDsaPublicKey)) {
          throw new SigningException("Expected an ML-DSA public key for alias=" + alias);
        }
        yield new MlDsaSigner(
            (MlDsaPrivateKey) keyPair.getPrivate(), (MlDsaPublicKey) keyPair.getPublic());
      }
    };
  }

  /** Returns the algorithm selected for app-level transaction and offline signing. */
  public SigningAlgorithm signingAlgorithm() {
    return signingAlgorithm;
  }

  /** Returns metadata for each configured key provider in priority order. */
  public List<KeyProviderMetadata> providerMetadata() {
    final List<KeyProviderMetadata> metadata = new ArrayList<>(providers.size());
    for (final KeyProvider provider : providers) {
      metadata.add(provider.metadata());
    }
    return Collections.unmodifiableList(metadata);
  }

  /** Returns {@code true} if any provider advertises hardware-backed support. */
  public boolean hasHardwareBackedProvider() {
    return providers.stream().anyMatch(provider -> provider.metadata().hardwareBacked());
  }

  /** Returns {@code true} when a StrongBox-backed provider is registered. */
  public boolean hasStrongBoxProvider() {
    return providers.stream().anyMatch(provider -> provider.metadata().strongBoxBacked());
  }

  /**
   * Exports the software-backed key referenced by {@code alias} using deterministic HKDF + AES-GCM
   * derivation. The passphrase is consumed as UTF-8 and cleared after derivation.
   *
   * @throws KeyExportException when the software provider cannot export the key material
   * @throws KeyManagementException when the alias is unknown or no software provider is available
   */
  public KeyExportBundle exportDeterministicKey(final String alias, final char[] passphrase)
      throws KeyManagementException, KeyExportException {
    Objects.requireNonNull(alias, "alias");
    Objects.requireNonNull(passphrase, "passphrase");
    final SoftwareKeyProvider softwareProvider = softwareProvider();
    return softwareProvider.exportDeterministic(alias, passphrase);
  }

  /**
   * Imports the provided deterministic export into the software provider, replacing any existing key
   * registered under {@code bundle.alias()}.
   *
   * @throws KeyExportException when the bundle cannot be decoded
   * @throws KeyManagementException when no software provider is available
   */
  public KeyPair importDeterministicKey(final KeyExportBundle bundle, final char[] passphrase)
      throws KeyExportException, KeyManagementException {
    Objects.requireNonNull(bundle, "bundle");
    Objects.requireNonNull(passphrase, "passphrase");
    final SoftwareKeyProvider softwareProvider = softwareProvider();
    return softwareProvider.importDeterministic(bundle, passphrase);
  }

  /**
   * Verifies attestation material produced by hardware-backed providers for {@code alias}.
   *
   * <p>The first provider that returns a non-empty attestation result is treated as authoritative.
   * Providers that do not expose attestation simply return an empty optional.
   *
   * @param alias alias whose attestation should be verified
   * @param verifier verifier configured with trusted roots and policy expectations
   * @param expectedChallenge optional challenge value that must match the attested payload when
   *     provided
   * @return the attestation verification result when available, or an empty optional when no
   *     attestation is recorded for {@code alias}
   * @throws AttestationVerificationException when attestation verification fails for the alias
   */
  public Optional<AttestationResult> verifyAttestation(
      final String alias, final AttestationVerifier verifier, final byte[] expectedChallenge)
      throws AttestationVerificationException {
    Objects.requireNonNull(alias, "alias");
    Objects.requireNonNull(verifier, "verifier");
    for (final KeyProvider provider : providers) {
      try {
        final Optional<AttestationResult> result =
            provider.verifyAttestation(alias, verifier, expectedChallenge);
        if (result.isPresent()) {
          keystoreTelemetry.recordResult(alias, provider.metadata(), result.get());
          return result;
        }
      } catch (final AttestationVerificationException ex) {
        keystoreTelemetry.recordFailure(alias, provider.metadata(), ex.getMessage());
        throw ex;
      }
    }
    return Optional.empty();
  }

  /**
   * Convenience overload that verifies attestation without enforcing a challenge match.
   *
   * @param alias alias whose attestation should be verified
   * @param verifier verifier configured with trusted roots and policy expectations
   * @return the attestation verification result when available, or an empty optional when no
   *     attestation is recorded for {@code alias}
   * @throws AttestationVerificationException when attestation verification fails for the alias
   */
  public Optional<AttestationResult> verifyAttestation(
      final String alias, final AttestationVerifier verifier) throws AttestationVerificationException {
    return verifyAttestation(alias, verifier, null);
  }

  /**
   * Requests fresh attestation material for {@code alias}. Providers that do not support
   * attestation generation return {@link Optional#empty()}.
   *
   * @param alias alias to attest
   * @param challenge attestation challenge (may be {@code null} if provider does not require it)
   * @return attestation bundle when generated, otherwise {@link Optional#empty()}
   * @throws KeyManagementException when provider-specific errors occur during attestation
   */
  public Optional<KeyAttestation> generateAttestation(
      final String alias, final byte[] challenge) throws KeyManagementException {
    Objects.requireNonNull(alias, "alias");
    if (alias.isBlank()) {
      throw new IllegalArgumentException("alias must not be blank");
    }

    final List<KeyProvider> ordered = new ArrayList<>(providers);
    ordered.sort(
        (left, right) ->
            Boolean.compare(
                right.metadata().supportsAttestationCertificates(),
                left.metadata().supportsAttestationCertificates()));

    KeyManagementException lastError = null;
    for (final KeyProvider provider : ordered) {
      try {
        final byte[] clonedChallenge = challenge == null ? null : challenge.clone();
        final Optional<KeyAttestation> attestation =
            provider.generateAttestation(alias, clonedChallenge);
        if (attestation.isPresent()) {
          return attestation;
        }
      } catch (final KeyManagementException e) {
        keystoreTelemetry.recordFailure(alias, provider.metadata(), e.getMessage());
        lastError = e;
      }
    }
    if (lastError != null) {
      throw lastError;
    }
    return Optional.empty();
  }

  private List<KeyProvider> orderedProviders(final KeySecurityPreference preference) {
    final List<KeyProvider> ordered = new ArrayList<>(providers);
    switch (preference) {
      case STRONGBOX_REQUIRED:
        ordered.removeIf(provider -> !provider.metadata().strongBoxBacked());
        break;
      case STRONGBOX_PREFERRED:
        ordered.sort(
            (left, right) -> {
              final int strongBoxComparison =
                  Boolean.compare(
                      right.metadata().strongBoxBacked(), left.metadata().strongBoxBacked());
              if (strongBoxComparison != 0) {
                return strongBoxComparison;
              }
              return Boolean.compare(
                  right.metadata().hardwareBacked(), left.metadata().hardwareBacked());
            });
        break;
      case HARDWARE_REQUIRED:
        ordered.removeIf(provider -> !provider.metadata().hardwareBacked());
        break;
      case HARDWARE_PREFERRED:
        ordered.sort(
            (left, right) ->
                Boolean.compare(right.metadata().hardwareBacked(), left.metadata().hardwareBacked()));
        break;
      case SOFTWARE_ONLY:
        ordered.removeIf(provider -> provider.metadata().hardwareBacked());
        break;
      default:
        throw new IllegalStateException("Unhandled preference " + preference);
    }
    return Collections.unmodifiableList(ordered);
  }

  private SoftwareKeyProvider softwareProvider() throws KeyManagementException {
    for (final KeyProvider provider : providers) {
      if (provider instanceof SoftwareKeyProvider) {
        return (SoftwareKeyProvider) provider;
      }
    }
    throw new KeyManagementException("No software key provider available for deterministic export");
  }

  /**
   * Describes the desired security posture when generating or loading keys.
   *
   * <p>Future revisions may extend this enum with additional hardware-class specific options
   * (StrongBox-only, SE-provided identity keys, etc.).
   */
  public enum KeySecurityPreference {
    STRONGBOX_REQUIRED,
    STRONGBOX_PREFERRED,
    HARDWARE_REQUIRED,
    HARDWARE_PREFERRED,
    SOFTWARE_ONLY
  }

  /**
   * Implemented by actors that can generate, store, and retrieve signing keys.
   *
   * <p>Providers may wrap the Android Keystore, StrongBox secure elements, or software fallbacks. A
   * provider can choose to ignore {@code alias} when generating ephemeral keys.
   */
  public interface KeyProvider {
    /**
     * Loads a key for {@code alias} if it exists.
     *
     * @return the stored key pair or an empty optional when the alias is unknown
     */
    Optional<KeyPair> load(String alias) throws KeyManagementException;

    /**
     * Generates and stores a key pair under {@code alias}.
     *
     * @throws KeyManagementException if generation fails
     */
    KeyPair generate(String alias) throws KeyManagementException;

    /**
     * Generates and stores a key pair honouring the requested security preference when possible.
     *
     * <p>The default implementation falls back to {@link #generate(String)}. Providers that can
     * route generation to specific hardware classes (StrongBox vs TEE) should override this method.
     *
     * @throws KeyManagementException if generation fails
     */
    default KeyPair generate(String alias, final KeySecurityPreference preference)
        throws KeyManagementException {
      return generate(alias);
    }

    /**
     * Generates a key pair and reports the hardware route used.
     *
     * <p>The default implementation derives the route from provider metadata; providers that can
     * detect StrongBox fallback should override this method to surface the actual path used.
     *
     * @throws KeyManagementException if generation fails
     */
    default org.hyperledger.iroha.android.crypto.KeyGenerationOutcome generateWithOutcome(
        final String alias, final KeySecurityPreference preference) throws KeyManagementException {
      final KeyPair pair = generate(alias, preference);
      return new org.hyperledger.iroha.android.crypto.KeyGenerationOutcome(
          pair, routeFromMetadata(metadata()));
    }

    /**
     * Generates a transient key pair that must not be persisted by the provider.
     *
     * @throws KeyManagementException if generation fails
     */
    KeyPair generateEphemeral() throws KeyManagementException;

    /** Indicates whether keys produced by this provider are hardware-backed. */
    boolean isHardwareBacked();

    /** Generates fresh attestation material for {@code alias} when supported. */
    default Optional<KeyAttestation> generateAttestation(
        final String alias, final byte[] challenge) throws KeyManagementException {
      return Optional.empty();
    }

    /**
     * Metadata describing this provider's capabilities. Providers should override this when they can
     * supply richer details (StrongBox, discrete secure element, attestation certificates).
     */
    default KeyProviderMetadata metadata() {
      return KeyProviderMetadata.builder(name())
          .setHardwareBacked(isHardwareBacked())
          .build();
    }

    /**
     * Verifies attestation material associated with {@code alias}. Providers that do not expose
     * attestation should return an empty optional.
     */
    default Optional<AttestationResult> verifyAttestation(
        final String alias,
        final AttestationVerifier verifier,
        final byte[] expectedChallenge)
        throws AttestationVerificationException {
      return Optional.empty();
    }

    /**
     * Convenience overload that validates attestation without an explicit challenge.
     *
     * <p>The default implementation delegates to the three-argument overload.
     */
    default Optional<AttestationResult> verifyAttestation(
        final String alias, final AttestationVerifier verifier)
        throws AttestationVerificationException {
      return verifyAttestation(alias, verifier, null);
    }

    private static org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route routeFromMetadata(
        final KeyProviderMetadata metadata) {
      if (metadata == null) {
        return org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.SOFTWARE;
      }
      if (metadata.strongBoxBacked()) {
        return org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.STRONGBOX;
      }
      if (metadata.hardwareBacked()) {
        return org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.HARDWARE;
      }
      return org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.SOFTWARE;
    }

    /** Diagnostic string for logging / telemetry. */
    String name();
  }
}
