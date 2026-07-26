package org.hyperledger.iroha.android.crypto.keystore;

import java.security.KeyPair;
import java.util.Optional;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;

/**
 * Desktop fallback backend used when {@code android.security.keystore} is not available at runtime.
 *
 * <p>The backend advertises zero hardware capability and fails generation calls so application code
 * can detect that it is not running on an Android Keystore runtime.
 */
public final class AndroidKeystoreStubBackend implements KeystoreBackend {

  private final KeyProviderMetadata metadata =
      KeyProviderMetadata.builder("android-keystore-stub")
          .setHardwareBacked(false)
          .setSupportsAttestationCertificates(false)
          .build();

  @Override
  public Optional<KeyPair> load(final String alias) {
    return Optional.empty();
  }

  @Override
  public KeyGenerationResult generate(final String alias, final KeyGenParameters parameters)
      throws KeyManagementException {
    throw unsupported();
  }

  @Override
  public KeyPair generateEphemeral(final KeyGenParameters parameters) throws KeyManagementException {
    throw unsupported();
  }

  @Override
  public KeyProviderMetadata metadata() {
    return metadata;
  }

  @Override
  public String name() {
    return "android-keystore-stub";
  }

  private static KeyManagementException unsupported() {
    return new KeyManagementException(
        "Android Keystore is unavailable in the current runtime; provide a platform backend");
  }
}
