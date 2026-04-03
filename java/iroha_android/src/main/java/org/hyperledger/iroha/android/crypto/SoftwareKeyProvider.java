package org.hyperledger.iroha.android.crypto;

import java.security.InvalidParameterException;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.NoSuchAlgorithmException;
import java.security.NoSuchProviderException;
import java.security.Provider;
import java.security.SecureRandom;
import java.security.Security;
import java.util.Arrays;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;
import org.hyperledger.iroha.android.IrohaKeyManager.KeyProvider;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;
import org.hyperledger.iroha.android.crypto.export.DeterministicKeyExporter;
import org.hyperledger.iroha.android.crypto.export.KeyExportBundle;
import org.hyperledger.iroha.android.crypto.export.KeyExportException;
import org.hyperledger.iroha.android.crypto.export.KeyExportStore;
import org.hyperledger.iroha.android.crypto.export.KeyPassphraseProvider;

/**
 * JVM friendly key provider that generates Ed25519 key pairs using {@link KeyPairGenerator}.
 *
 * <p>This provider is intended for desktop tooling, tests, and Android devices without secure
 * elements. It can optionally persist deterministic key exports via a {@link KeyExportStore} so
 * software-backed accounts can be restored across sessions and devices.
 */
public final class SoftwareKeyProvider implements KeyProvider {
  private static final int ML_DSA_SEED_LENGTH_BYTES = 32;

  /** Controls which JCA provider is used for Ed25519 key generation. */
  public enum ProviderPolicy {
    /** Use the default JCA provider order, falling back to BouncyCastle when needed. */
    DEFAULT,
    /** Prefer BouncyCastle when available to keep keys exportable. */
    BOUNCY_CASTLE_PREFERRED,
    /** Require BouncyCastle; fail if the provider is unavailable. */
    BOUNCY_CASTLE_REQUIRED
  }

  private final ConcurrentMap<String, KeyPair> aliasCache = new ConcurrentHashMap<>();
  private final SecureRandom secureRandom = new SecureRandom();
  private final ProviderPolicy providerPolicy;
  private final KeyExportStore exportStore;
  private final KeyPassphraseProvider passphraseProvider;
  private final SigningAlgorithm signingAlgorithm;

  public SoftwareKeyProvider() {
    this(ProviderPolicy.DEFAULT, null, null, SigningAlgorithm.ED25519);
  }

  public SoftwareKeyProvider(final ProviderPolicy providerPolicy) {
    this(providerPolicy, null, null, SigningAlgorithm.ED25519);
  }

  public SoftwareKeyProvider(final SigningAlgorithm signingAlgorithm) {
    this(ProviderPolicy.DEFAULT, null, null, signingAlgorithm);
  }

  public SoftwareKeyProvider(
      final KeyExportStore exportStore,
      final KeyPassphraseProvider passphraseProvider) {
    this(exportStore, passphraseProvider, SigningAlgorithm.ED25519);
  }

  public SoftwareKeyProvider(
      final ProviderPolicy providerPolicy,
      final KeyExportStore exportStore,
      final KeyPassphraseProvider passphraseProvider) {
    this(providerPolicy, exportStore, passphraseProvider, SigningAlgorithm.ED25519);
  }

  public SoftwareKeyProvider(
      final KeyExportStore exportStore,
      final KeyPassphraseProvider passphraseProvider,
      final SigningAlgorithm signingAlgorithm) {
    this(ProviderPolicy.BOUNCY_CASTLE_PREFERRED, exportStore, passphraseProvider, signingAlgorithm);
  }

  public SoftwareKeyProvider(
      final ProviderPolicy providerPolicy,
      final KeyExportStore exportStore,
      final KeyPassphraseProvider passphraseProvider,
      final SigningAlgorithm signingAlgorithm) {
    this.providerPolicy = providerPolicy == null ? ProviderPolicy.DEFAULT : providerPolicy;
    this.exportStore = exportStore;
    this.passphraseProvider = passphraseProvider;
    this.signingAlgorithm =
        signingAlgorithm == null ? SigningAlgorithm.ED25519 : signingAlgorithm;
    if (this.exportStore != null && this.passphraseProvider == null) {
      throw new IllegalArgumentException("passphraseProvider is required when exportStore is set");
    }
  }

  @Override
  public Optional<KeyPair> load(final String alias) throws KeyManagementException {
    if (alias == null || alias.isBlank()) {
      return Optional.empty();
    }
    final KeyPair cached = aliasCache.get(alias);
    if (cached != null) {
      return Optional.of(cached);
    }
    if (exportStore == null) {
      return Optional.empty();
    }
    final Optional<KeyPair> restored = loadFromExportStore(alias);
    restored.ifPresent(pair -> aliasCache.put(alias, pair));
    return restored;
  }

  @Override
  public KeyPair generate(final String alias) throws KeyManagementException {
    if (alias == null || alias.isBlank()) {
      throw new KeyManagementException("alias must be provided for persistent keys");
    }
    final KeyPair keyPair = generateKeyPair();
    aliasCache.put(alias, keyPair);
    persistKey(alias, keyPair);
    return keyPair;
  }

  @Override
  public KeyPair generateEphemeral() throws KeyManagementException {
    return generateKeyPair();
  }

  @Override
  public boolean isHardwareBacked() {
    return false;
  }

  @Override
  public String name() {
    return "software-key-provider";
  }

  @Override
  public KeyProviderMetadata metadata() {
    return KeyProviderMetadata.software(name());
  }

  /**
   * Exports the key associated with {@code alias} deterministically using the supplied
   * {@code passphrase}.
   */
  public KeyExportBundle exportDeterministic(final String alias, final char[] passphrase)
      throws KeyManagementException, KeyExportException {
    final KeyPair keyPair =
        load(alias).orElseThrow(() -> new KeyManagementException("Unknown alias: " + alias));
    ensureExpectedSigningAlgorithm(keyPair);
    return DeterministicKeyExporter.exportKeyPair(keyPair.getPrivate(), keyPair.getPublic(), alias, passphrase);
  }

  /**
   * Imports a deterministic key bundle into the provider, replacing any existing entry for the same
   * alias.
   */
  public KeyPair importDeterministic(final KeyExportBundle bundle, final char[] passphrase)
      throws KeyExportException, KeyManagementException {
    if (bundle.signingAlgorithm() != signingAlgorithm) {
      throw new KeyExportException(
          "Key export algorithm "
              + bundle.signingAlgorithm().providerName()
              + " does not match provider "
              + signingAlgorithm.providerName());
    }
    final DeterministicKeyExporter.KeyPairData data =
        DeterministicKeyExporter.importKeyPair(bundle, passphrase);
    final KeyPair keyPair = new KeyPair(data.publicKey(), data.privateKey());
    ensureExpectedSigningAlgorithm(keyPair);
    aliasCache.put(bundle.alias(), keyPair);
    if (exportStore != null) {
      exportStore.store(bundle.alias(), bundle.encodeBase64());
    }
    return keyPair;
  }

  private KeyPair generateKeyPair() throws KeyManagementException {
    if (signingAlgorithm == SigningAlgorithm.ML_DSA) {
      final byte[] seed = new byte[ML_DSA_SEED_LENGTH_BYTES];
      secureRandom.nextBytes(seed);
      try {
        final NativeSignerBridge.KeypairBytes pair =
            NativeSignerBridge.keypairFromSeed(SigningAlgorithm.ML_DSA, seed);
        return new KeyPair(
            new MlDsaPublicKey(pair.publicKey()),
            new MlDsaPrivateKey(pair.privateKey(), pair.publicKey()));
      } catch (final RuntimeException ex) {
        throw new KeyManagementException("ML-DSA key generation is not supported on this runtime", ex);
      } finally {
        Arrays.fill(seed, (byte) 0);
      }
    }
    final KeyPairGenerator generator = newKeyPairGenerator();
    final boolean usedBouncyCastle =
        generator.getProvider() != null && "BC".equals(generator.getProvider().getName());
    final KeyPair keyPair = generateWithGenerator(generator);
    if (isExportable(keyPair)) {
      return keyPair;
    }
    if (!usedBouncyCastle && providerPolicy != ProviderPolicy.BOUNCY_CASTLE_REQUIRED) {
      final Optional<KeyPairGenerator> fallback = tryBouncyCastleGenerator();
      if (fallback.isPresent()) {
        final KeyPair fallbackPair = generateWithGenerator(fallback.get());
        if (isExportable(fallbackPair)) {
          return fallbackPair;
        }
      }
    }
    throw new KeyManagementException(
        "Ed25519 key material is not exportable; use BouncyCastle provider");
  }

  private KeyPairGenerator newKeyPairGenerator() throws KeyManagementException {
    if (providerPolicy == ProviderPolicy.BOUNCY_CASTLE_REQUIRED) {
      return bouncyCastleGeneratorOrThrow();
    }
    if (providerPolicy == ProviderPolicy.BOUNCY_CASTLE_PREFERRED) {
      final Optional<KeyPairGenerator> preferred = tryBouncyCastleGenerator();
      if (preferred.isPresent()) {
        return preferred.get();
      }
    }
    try {
      return KeyPairGenerator.getInstance("Ed25519");
    } catch (final NoSuchAlgorithmException ignored) {
      final Optional<KeyPairGenerator> fallback = tryBouncyCastleGenerator();
      if (fallback.isPresent()) {
        return fallback.get();
      }
      try {
        return KeyPairGenerator.getInstance("EdDSA");
      } catch (final NoSuchAlgorithmException ex) {
        throw new KeyManagementException("Ed25519 key generation is not supported on this JVM", ex);
      }
    }
  }

  private KeyPairGenerator bouncyCastleGeneratorOrThrow() throws KeyManagementException {
    final Optional<KeyPairGenerator> generator = tryBouncyCastleGenerator();
    if (generator.isPresent()) {
      return generator.get();
    }
    throw new KeyManagementException("BouncyCastle provider is required for exportable keys");
  }

  private KeyPair generateWithGenerator(final KeyPairGenerator generator) {
    try {
      generator.initialize(255, secureRandom);
    } catch (final InvalidParameterException ex) {
      // Providers that expose fixed-parameter Ed25519 generators reject custom sizes.
    }
    return generator.generateKeyPair();
  }

  private static boolean isExportable(final KeyPair keyPair) {
    if (keyPair == null || keyPair.getPrivate() == null || keyPair.getPublic() == null) {
      return false;
    }
    final byte[] encoded = keyPair.getPrivate().getEncoded();
    return encoded != null && encoded.length > 0;
  }

  private Optional<KeyPair> loadFromExportStore(final String alias) throws KeyManagementException {
    try {
      final Optional<String> encoded = exportStore.load(alias);
      if (encoded.isEmpty()) {
        return Optional.empty();
      }
      final KeyExportBundle bundle = KeyExportBundle.decodeBase64(encoded.get());
      if (bundle.signingAlgorithm() != signingAlgorithm) {
        throw new KeyManagementException(
            "Stored key for alias="
                + alias
                + " uses "
                + bundle.signingAlgorithm().providerName()
                + ", expected "
                + signingAlgorithm.providerName());
      }
      final char[] passphrase = requirePassphrase();
      try {
        final DeterministicKeyExporter.KeyPairData data =
            DeterministicKeyExporter.importKeyPair(bundle, passphrase);
        final KeyPair keyPair = new KeyPair(data.publicKey(), data.privateKey());
        ensureExpectedSigningAlgorithm(keyPair);
        return Optional.of(keyPair);
      } finally {
        Arrays.fill(passphrase, '\0');
      }
    } catch (final KeyExportException ex) {
      throw new KeyManagementException("Failed to load deterministic key export", ex);
    }
  }

  private void persistKey(final String alias, final KeyPair keyPair) throws KeyManagementException {
    if (exportStore == null) {
      return;
    }
    final char[] passphrase = requirePassphrase();
    try {
      final KeyExportBundle bundle =
          DeterministicKeyExporter.exportKeyPair(
              keyPair.getPrivate(), keyPair.getPublic(), alias, passphrase);
      exportStore.store(alias, bundle.encodeBase64());
    } catch (final KeyExportException ex) {
      throw new KeyManagementException("Failed to persist deterministic key export", ex);
    } finally {
      Arrays.fill(passphrase, '\0');
    }
  }

  private char[] requirePassphrase() throws KeyManagementException {
    if (passphraseProvider == null) {
      throw new KeyManagementException("Passphrase provider must be configured for export store");
    }
    final char[] passphrase = passphraseProvider.passphrase();
    if (passphrase == null || passphrase.length == 0) {
      throw new KeyManagementException("Passphrase must not be empty");
    }
    return passphrase;
  }

  private void ensureExpectedSigningAlgorithm(final KeyPair keyPair)
      throws KeyManagementException {
    final boolean matches =
        switch (signingAlgorithm) {
          case ED25519 ->
              !(keyPair.getPrivate() instanceof MlDsaPrivateKey)
                  && !(keyPair.getPublic() instanceof MlDsaPublicKey);
          case ML_DSA ->
              keyPair.getPrivate() instanceof MlDsaPrivateKey
                  && keyPair.getPublic() instanceof MlDsaPublicKey;
        };
    if (!matches) {
      throw new KeyManagementException(
          "Provider expected " + signingAlgorithm.providerName() + " key material");
    }
  }

  static Optional<KeyPairGenerator> tryBouncyCastleGenerator() {
    try {
      final Class<?> providerClass =
          Class.forName("org.bouncycastle.jce.provider.BouncyCastleProvider");
      final Provider provider =
          (Provider) providerClass.getDeclaredConstructor().newInstance();
      final String providerName = provider.getName();
      if (Security.getProvider(providerName) == null) {
        Security.addProvider(provider);
      }
      try {
        return Optional.of(KeyPairGenerator.getInstance("EdDSA", providerName));
      } catch (final NoSuchAlgorithmException | NoSuchProviderException ex) {
        return Optional.of(KeyPairGenerator.getInstance("EdDSA"));
      }
    } catch (final ClassNotFoundException e) {
      return Optional.empty();
    } catch (final ReflectiveOperationException | ClassCastException e) {
      return Optional.empty();
    } catch (final NoSuchAlgorithmException ex) {
      return Optional.empty();
    }
  }
}
