package org.hyperledger.iroha.android.crypto.keystore;

import java.security.KeyPair;
import java.util.Optional;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;

/**
 * Placeholder interface capturing the operations required from the Android Keystore runtime.
 *
 * <p>The actual implementation will live in the Android-specific source set where the Android SDK is
 * available. This marker is provided so the main sources can reference the class without introducing
 * a hard dependency on android.jar in the desktop build.
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
