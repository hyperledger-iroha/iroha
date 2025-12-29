package org.hyperledger.iroha.android.crypto.keystore;

import java.security.KeyPair;
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
 * {@link IrohaKeyManager.KeyProvider} backed by an Android Keystore-compatible backend.
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
  private final Map<CacheKey, KeyAttestation> attestationCache = new ConcurrentHashMap<>();

  public KeystoreKeyProvider(final KeystoreBackend backend, final KeyGenParameters parameters) {
    this.backend = Objects.requireNonNull(backend, "backend");
    this.parameters = Objects.requireNonNull(parameters, "parameters");
  }

  @Override
  public Optional<KeyPair> load(final String alias) throws KeyManagementException {
    return backend.load(alias);
  }

  @Override
  public KeyPair generate(final String alias) throws KeyManagementException {
    evictAttestations(alias);
    final KeyGenerationResult result = backend.generate(alias, parameters);
    if (parameters.requireStrongBox()
        && !result.strongBoxBacked()
        && !backend.metadata().strongBoxBacked()) {
      throw new KeyManagementException("StrongBox required but backend is not StrongBox-capable");
    }
    return result.keyPair();
  }

  @Override
  public KeyPair generate(
      final String alias, final org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference preference)
      throws KeyManagementException {
    evictAttestations(alias);
    final KeyGenParameters effective = parametersFor(preference);
    if (effective.requireStrongBox() && !backend.metadata().strongBoxBacked()) {
      throw new KeyManagementException("StrongBox required but backend is not StrongBox-capable");
    }
    final KeyGenerationResult result = backend.generate(alias, effective);
    if (preference == org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference.STRONGBOX_REQUIRED
        && !result.strongBoxBacked()) {
      throw new KeyManagementException(
          "StrongBox required but backend fell back to a weaker security level");
    }
    return result.keyPair();
  }

  @Override
  public org.hyperledger.iroha.android.crypto.KeyGenerationOutcome generateWithOutcome(
      final String alias, final org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference preference)
      throws KeyManagementException {
    evictAttestations(alias);
    final KeyGenParameters effective = parametersFor(preference);
    final KeyGenerationResult result = backend.generate(alias, effective);
    final org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route route;
    if (result.strongBoxBacked()) {
      route = org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.STRONGBOX;
    } else if (backend.metadata().hardwareBacked()) {
      route = org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.HARDWARE;
    } else {
      route = org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.SOFTWARE;
    }
    if (preference == org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference.STRONGBOX_REQUIRED
        && route != org.hyperledger.iroha.android.crypto.KeyGenerationOutcome.Route.STRONGBOX) {
      throw new KeyManagementException(
          "StrongBox required but backend fell back to a weaker security level");
    }
    return new org.hyperledger.iroha.android.crypto.KeyGenerationOutcome(result.keyPair(), route);
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

  /** Returns attestation material recorded for {@code alias}, when available. */
  public Optional<KeyAttestation> attestation(final String alias) {
    try {
      return fetchAttestation(alias, NO_CHALLENGE);
    } catch (final KeyManagementException ex) {
      return Optional.empty();
    }
  }

  /**
   * Requests fresh attestation for {@code alias}. Providers that do not support attestation return
   * {@link Optional#empty()}.
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
    if (attestation.isEmpty()) {
      throw new KeyManagementException(
          "Attestation challenge is not supported by backend " + backend.name());
    }
    return attestation.map(att -> cacheAttestation(alias, normalizedChallenge, att));
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
    final byte[] challenge = expectedChallenge == null ? NO_CHALLENGE : expectedChallenge.clone();
    final Optional<KeyAttestation> attestation;
    try {
      attestation = fetchAttestation(alias, challenge);
    } catch (final KeyManagementException ex) {
      return Optional.empty();
    }
    if (attestation.isEmpty()) {
      return Optional.empty();
    }
    try {
      if (expectedChallenge == null) {
        return Optional.of(verifier.verify(attestation.get()));
      }
      return Optional.of(verifier.verify(attestation.get(), expectedChallenge.clone()));
    } catch (final AttestationVerificationException ex) {
      evictAttestations(alias);
      throw ex;
    }
  }

  /** Verifies attestation without enforcing a challenge match. */
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
    return new KeystoreKeyProvider(this.backend, parameters);
  }

  /** Returns a copy of this provider tuned for the requested security preference. */
  public KeystoreKeyProvider withPreference(final org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference preference) {
    return new KeystoreKeyProvider(this.backend, parametersFor(preference));
  }

  private KeyGenParameters parametersFor(
      final org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference preference) {
    if (preference == null) {
      return this.parameters;
    }
    final KeyGenParameters.Builder builder = this.parameters.toBuilder();
    switch (preference) {
      case STRONGBOX_REQUIRED:
        builder.setRequireStrongBox(true).setPreferStrongBox(true).setAllowStrongBoxFallback(false);
        break;
      case STRONGBOX_PREFERRED:
        builder.setRequireStrongBox(false).setPreferStrongBox(true).setAllowStrongBoxFallback(true);
        break;
      case HARDWARE_REQUIRED:
        builder.setRequireStrongBox(false).setPreferStrongBox(false).setAllowStrongBoxFallback(true);
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
    if (normalized.length > 0) {
      final Optional<KeyAttestation> generated = backend.generateAttestation(alias, normalized);
      if (generated.isPresent()) {
        return Optional.of(cacheAttestation(alias, normalized, generated.get()));
      }
    }
    return backend.attestation(alias).map(att -> cacheAttestation(alias, normalized, att));
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
      if (alias.isBlank()) {
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
