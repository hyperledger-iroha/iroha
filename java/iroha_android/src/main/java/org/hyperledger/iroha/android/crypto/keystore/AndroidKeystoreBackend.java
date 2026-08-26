package org.hyperledger.iroha.android.crypto.keystore;

import java.security.KeyPair;
import java.util.Optional;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;

/**
 * Operations supplied by the Android Keystore runtime.
 *
 * <p>{@link SystemAndroidKeystoreBackend} provides the platform implementation. Keeping this interface
 * in the shared source set lets desktop callers detect that the Android runtime is unavailable without
 * introducing a hard dependency on {@code android.jar}.
 */
public interface AndroidKeystoreBackend extends KeystoreBackend {

  @Override
  Optional<KeyPair> load(String alias) throws KeyManagementException;

  @Override
  KeyGenerationResult generate(String alias, KeyGenParameters parameters) throws KeyManagementException;

  @Override
  KeyPair generateEphemeral(KeyGenParameters parameters) throws KeyManagementException;

  @Override
  KeyProviderMetadata metadata();

  @Override
  String name();

  @Override
  Optional<KeyAttestation> attestation(String alias);

  /**
   * Factory that resolves to the platform implementation when running on Android, or returns an empty
   * optional otherwise.
   */
  static Optional<AndroidKeystoreBackend> maybeCreate() {
    return SystemAndroidKeystoreBackend.create();
  }
}
