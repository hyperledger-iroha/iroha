package org.hyperledger.iroha.android.crypto;

import java.security.InvalidParameterException;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.NoSuchAlgorithmException;
import java.security.NoSuchProviderException;
import java.security.Provider;
import java.security.SecureRandom;
import java.security.Security;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;
import org.hyperledger.iroha.android.IrohaKeyManager.KeyProvider;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;
import org.hyperledger.iroha.android.crypto.export.DeterministicKeyExporter;
import org.hyperledger.iroha.android.crypto.export.KeyExportBundle;
import org.hyperledger.iroha.android.crypto.export.KeyExportException;

/**
 * JVM friendly key provider that generates Ed25519 key pairs using {@link KeyPairGenerator}.
 *
 * <p>This provider is intended for desktop tooling, tests, and Android devices without secure
 * elements. It stores generated aliases in-memory; persistent storage will be introduced once the
 * Android Keystore integration lands.
 */
public final class SoftwareKeyProvider implements KeyProvider {

  private final ConcurrentMap<String, KeyPair> aliasCache = new ConcurrentHashMap<>();
  private final SecureRandom secureRandom = new SecureRandom();

  @Override
  public Optional<KeyPair> load(final String alias) {
    if (alias == null) {
      return Optional.empty();
    }
    return Optional.ofNullable(aliasCache.get(alias));
  }

  @Override
  public KeyPair generate(final String alias) throws KeyManagementException {
    if (alias == null || alias.isBlank()) {
      throw new KeyManagementException("alias must be provided for persistent keys");
    }
    final KeyPair keyPair = generateKeyPair();
    aliasCache.put(alias, keyPair);
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
        Optional.ofNullable(aliasCache.get(alias))
            .orElseThrow(() -> new KeyManagementException("Unknown alias: " + alias));
    return DeterministicKeyExporter.exportKeyPair(keyPair.getPrivate(), keyPair.getPublic(), alias, passphrase);
  }

  /**
   * Imports a deterministic key bundle into the provider, replacing any existing entry for the same
   * alias.
   */
  public KeyPair importDeterministic(final KeyExportBundle bundle, final char[] passphrase)
      throws KeyExportException {
    final DeterministicKeyExporter.KeyPairData data =
        DeterministicKeyExporter.importKeyPair(bundle, passphrase);
    final KeyPair keyPair = new KeyPair(data.publicKey(), data.privateKey());
    aliasCache.put(bundle.alias(), keyPair);
    return keyPair;
  }

  private KeyPair generateKeyPair() throws KeyManagementException {
    final KeyPairGenerator generator = newKeyPairGenerator();
    try {
      generator.initialize(255, secureRandom);
    } catch (final InvalidParameterException ex) {
      // Providers that expose fixed-parameter Ed25519 generators reject custom sizes.
    }
    return generator.generateKeyPair();
  }

  private KeyPairGenerator newKeyPairGenerator() throws KeyManagementException {
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
