package org.hyperledger.iroha.android.crypto.keystore;

import java.security.KeyPair;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationResult;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationVerificationException;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationVerifier;

/**
 * {@link IrohaKeyManager.KeyProvider} backed by an Android Keystore backend.
 *
 * <p>The backend is supplied through {@link KeystoreBackend}; desktop JVM builds can rely on fake
 * implementations for tests while Android builds will provide a real backend that bridges to
 * {@code android.security.keystore.KeyStore} / {@code KeyGenParameterSpec}. Support for StrongBox
 * and discrete secure elements will land by swapping the backend at runtime depending on device
 * capabilities.
 */
public final class KeystoreKeyProvider implements IrohaKeyManager.KeyProvider {

  private static final byte[] NO_CHALLENGE = new byte[0];

  private final KeystoreBackend backend;
  private final KeyGenParameters parameters;
  private final IrohaKeyManager.KeySecurityPreference defaultPreference;
  private final Map<CacheKey, KeyAttestation> attestationCache = new ConcurrentHashMap<>();

  public KeystoreKeyProvider(final KeystoreBackend backend, final KeyGenParameters parameters) {
    this(backend, parameters, null);
  }

  private KeystoreKeyProvider(
      final KeystoreBackend backend,
      final KeyGenParameters parameters,
      final IrohaKeyManager.KeySecurityPreference defaultPreference) {
    this.backend = Objects.requireNonNull(backend, "backend");
    this.parameters = Objects.requireNonNull(parameters, "parameters");
    this.defaultPreference = defaultPreference;
  }

  @Override
  public Optional<KeyPair> load(final String alias) throws KeyManagementException {
    return backend.load(alias);
  }

  @Override
  public KeyPair generate(final String alias) throws KeyManagementException {
    if (defaultPreference != null) {
      return generateWithOutcome(alias, defaultPreference).keyPair();
    }
    evictAttestations(alias);
    if (parameters.requireStrongBox() && !backend.metadata().strongBoxBacked()) {
      throw new KeyManagementException("StrongBox required but backend is not StrongBox-capable");
    }
    final KeyGenerationResult result = backend.generate(alias, parameters);
    final org.hyperledger.iroha.android.crypto.KeyGenerationOutcome outcome =
        outcomeFor(alias, result.keyPair());
    if (parameters.requireStrongBox()
        && outcome.route()
            != org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.STRONGBOX) {
      throw new KeyManagementException(
          "StrongBox required but backend produced a weaker security level");
    }
    return outcome.keyPair();
  }

  @Override
  public KeyPair generate(
      final String alias, final org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference preference)
      throws KeyManagementException {
    return generateWithOutcome(alias, preference).keyPair();
  }

  @Override
  public org.hyperledger.iroha.android.crypto.KeyGenerationOutcome generateWithOutcome(
      final String alias, final org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference preference)
      throws KeyManagementException {
    evictAttestations(alias);
    final KeyGenParameters effective = parametersFor(preference);
    if (effective.requireStrongBox() && !backend.metadata().strongBoxBacked()) {
      throw new KeyManagementException("StrongBox required but backend is not StrongBox-capable");
    }
    final KeyGenerationResult result = backend.generate(alias, effective);
    final org.hyperledger.iroha.android.crypto.KeyGenerationOutcome outcome =
        outcomeFor(alias, result.keyPair());
    enforcePreference(preference, outcome);
    return outcome;
  }

  /** Returns the measured security route for an existing or newly generated key. */
  @Override
  public org.hyperledger.iroha.android.crypto.KeyGenerationOutcome outcomeFor(
      final String alias, final KeyPair keyPair) throws KeyManagementException {
    final KeyProviderMetadata keyMetadata = backend.keyMetadata(alias, keyPair);
    final org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route route;
    if (keyMetadata.strongBoxBacked()) {
      route = org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.STRONGBOX;
    } else if (keyMetadata.hardwareBacked()) {
      route = org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.HARDWARE;
    } else {
      route = org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.SOFTWARE;
    }
    return new org.hyperledger.iroha.android.crypto.KeyGenerationOutcome(keyPair, route);
  }

  @Override
  public KeyPair generateEphemeral() throws KeyManagementException {
    return backend.generateEphemeral(parameters);
  }

  @Override
  public boolean isHardwareBacked() {
    return backend.metadata().hardwareBacked();
  }

  @Override
  public KeyProviderMetadata metadata() {
    return backend.metadata();
  }

  @Override
  public String name() {
    return backend.name();
  }

  private static void enforcePreference(
      final org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference preference,
      final org.hyperledger.iroha.android.crypto.KeyGenerationOutcome outcome)
      throws KeyManagementException {
    if (preference
            == org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference.STRONGBOX_REQUIRED
        && outcome.route()
            != org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.STRONGBOX) {
      throw new KeyManagementException(
          "StrongBox required but backend produced a weaker security level");
    }
    if (preference
            == org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference.HARDWARE_REQUIRED
        && outcome.route()
            == org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.SOFTWARE) {
      throw new KeyManagementException(
          "Hardware-backed key required but backend produced a software key");
    }
  }

  /** Returns attestation material recorded for {@code alias}, when available. */
  public Optional<KeyAttestation> attestation(final String alias) {
    try {
      return fetchAttestation(alias, NO_CHALLENGE);
    } catch (final KeyManagementException ex) {
      return Optional.empty();
    }
  }

  /**
   * Requests backend-generated attestation for {@code alias}. Android Keystore cannot re-attest an
   * existing alias with a new challenge; callers must provision a new alias with the challenge in
   * {@link KeyGenParameters} when fresh evidence is required.
   */
  public Optional<KeyAttestation> generateAttestation(
      final String alias, final byte[] challenge) throws KeyManagementException {
    final byte[] normalizedChallenge = challenge == null ? NO_CHALLENGE : challenge.clone();
    final byte[] fingerprint = fingerprintChallenge(normalizedChallenge);
    if (normalizedChallenge.length == 0) {
      final Optional<KeyAttestation> cached = lookupCachedAttestation(alias, normalizedChallenge);
      if (cached.isPresent()) {
        return cached;
      }
      final Optional<KeyAttestation> existing = backend.attestation(alias);
      if (existing.isPresent()) {
        return Optional.of(cacheAttestation(alias, normalizedChallenge, existing.get()));
      }
      return backend.generateAttestation(alias, normalizedChallenge)
          .map(att -> cacheAttestation(alias, normalizedChallenge, att));
    }
    evictAttestationEntry(alias, fingerprint);
    final Optional<KeyAttestation> attestation =
        backend.generateAttestation(alias, normalizedChallenge);
    if (!attestation.isPresent()) {
      throw new KeyManagementException(
          "Attestation challenge is not supported by backend " + backend.name());
    }
    return attestation;
  }

  /**
   * Verifies attestation material (when present) using the supplied verifier.
   *
   * <p>Returns an empty optional when the backend has no attestation for {@code alias}.
   */
  @Override
  public Optional<AttestationResult> verifyAttestation(
      final String alias, final AttestationVerifier verifier, final byte[] expectedChallenge)
      throws AttestationVerificationException {
    Objects.requireNonNull(alias, "alias");
    Objects.requireNonNull(verifier, "verifier");
    if (expectedChallenge == null || expectedChallenge.length == 0) {
      throw new AttestationVerificationException(
          "Expected attestation challenge must be non-empty");
    }
    final byte[] challenge = expectedChallenge.clone();
    final Optional<KeyAttestation> attestation;
    try {
      attestation = fetchRecordedAttestation(alias);
    } catch (final KeyManagementException | RuntimeException ex) {
      throw new AttestationVerificationException(
          "Failed to obtain challenge-bound attestation material", ex);
    }
    if (!attestation.isPresent()) {
      return Optional.empty();
    }
    try {
      final AttestationResult result = verifier.verify(attestation.get(), challenge);
      final Optional<KeyPair> loadedKey;
      try {
        loadedKey = backend.load(alias);
      } catch (final KeyManagementException | RuntimeException ex) {
        throw new AttestationVerificationException(
            "Failed to load the attested alias key", ex);
      }
      if (loadedKey == null || !loadedKey.isPresent() || loadedKey.get().getPublic() == null) {
        throw new AttestationVerificationException("Attested alias key is unavailable");
      }
      final byte[] aliasPublicKey = loadedKey.get().getPublic().getEncoded();
      final java.security.PublicKey attestedLeafKey = result.leafCertificate().getPublicKey();
      final byte[] attestedPublicKey =
          attestedLeafKey == null ? null : attestedLeafKey.getEncoded();
      if (aliasPublicKey == null
          || attestedPublicKey == null
          || !MessageDigest.isEqual(aliasPublicKey, attestedPublicKey)) {
        throw new AttestationVerificationException(
            "Attested leaf public key does not match the alias key");
      }
      return Optional.of(result);
    } catch (final AttestationVerificationException ex) {
      evictAttestations(alias);
      throw ex;
    }
  }

  /** Retained for binary compatibility; always fails closed because a challenge is required. */
  @Override
  public Optional<AttestationResult> verifyAttestation(
      final String alias, final AttestationVerifier verifier)
      throws AttestationVerificationException {
    return verifyAttestation(alias, verifier, null);
  }

  /** Attempts to create a keystore-backed provider when running on Android hardware. */
  public static Optional<KeystoreKeyProvider> maybeCreate(final KeyGenParameters parameters) {
    return AndroidKeystoreBackend.maybeCreate()
        .map(backend -> new KeystoreKeyProvider(backend, parameters));
  }

  /** Returns a copy of this provider with adjusted generation parameters. */
  public KeystoreKeyProvider withParameters(final KeyGenParameters parameters) {
    return new KeystoreKeyProvider(this.backend, parameters, defaultPreference);
  }

  /** Returns a copy of this provider tuned for the requested security preference. */
  public KeystoreKeyProvider withPreference(
      final org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference preference) {
    return new KeystoreKeyProvider(this.backend, parametersFor(preference), preference);
  }

  private KeyGenParameters parametersFor(
      final org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference preference) {
    if (preference == null) {
      return this.parameters;
    }
    final KeyGenParameters.Builder builder = this.parameters.toBuilder();
    switch (preference) {
      case STRONGBOX_REQUIRED:
        builder.setRequireStrongBox(true).setPreferStrongBox(true);
        break;
      case STRONGBOX_PREFERRED:
        builder.setRequireStrongBox(false).setPreferStrongBox(true);
        break;
      case HARDWARE_REQUIRED:
        builder.setRequireStrongBox(false).setPreferStrongBox(false);
        break;
      case HARDWARE_PREFERRED:
      case SOFTWARE_ONLY:
      default:
        // leave defaults
        break;
    }
    return builder.build();
  }

  private Optional<KeyAttestation> lookupCachedAttestation(
      final String alias, final byte[] challenge) {
    final CacheKey cacheKey = new CacheKey(alias, fingerprintChallenge(challenge));
    return Optional.ofNullable(attestationCache.get(cacheKey));
  }

  private KeyAttestation cacheAttestation(
      final String alias, final byte[] challenge, final KeyAttestation attestation) {
    final CacheKey specific = new CacheKey(alias, fingerprintChallenge(challenge));
    attestationCache.put(specific, attestation);
    return attestation;
  }

  private void evictAttestations(final String alias) {
    attestationCache.keySet().removeIf(entry -> entry.alias().equals(alias));
  }

  private void evictAttestationEntry(final String alias, final byte[] challengeFingerprint) {
    final CacheKey cacheKey = new CacheKey(alias, challengeFingerprint);
    attestationCache.remove(cacheKey);
  }

  private Optional<KeyAttestation> fetchAttestation(
      final String alias, final byte[] challenge) throws KeyManagementException {
    final byte[] normalized = challenge == null ? NO_CHALLENGE : challenge.clone();
    final Optional<KeyAttestation> cached = lookupCachedAttestation(alias, normalized);
    if (cached.isPresent()) {
      return cached;
    }
    return backend.attestation(alias).map(att -> cacheAttestation(alias, normalized, att));
  }

  private Optional<KeyAttestation> fetchRecordedAttestation(final String alias)
      throws KeyManagementException {
    // Challenge-bound Android certificates are minted at key provisioning time. Always reread the
    // recorded chain here so verification cannot consume stale in-memory evidence.
    return backend.attestation(alias);
  }

  private static byte[] fingerprintChallenge(final byte[] challenge) {
    if (challenge == null || challenge.length == 0) {
      return NO_CHALLENGE;
    }
    return Arrays.copyOf(challenge, challenge.length);
  }

  private record CacheKey(String alias, byte[] challengeFingerprint) {
    private CacheKey(final String alias, final byte[] challengeFingerprint) {
      if (alias == null) {
        throw new NullPointerException("alias");
      }
      if (alias.trim().isEmpty()) {
        throw new IllegalArgumentException("alias must not be blank");
      }
      if (challengeFingerprint == null) {
        throw new NullPointerException("challengeFingerprint");
      }
      this.alias = alias;
      this.challengeFingerprint = Arrays.copyOf(challengeFingerprint, challengeFingerprint.length);
    }

    @Override
    public int hashCode() {
      int result = alias.hashCode();
      result = 31 * result + Arrays.hashCode(challengeFingerprint);
      return result;
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof CacheKey other)) {
        return false;
      }
      return alias.equals(other.alias)
          && Arrays.equals(challengeFingerprint, other.challengeFingerprint);
    }
  }
}
