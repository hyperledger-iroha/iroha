package org.hyperledger.iroha.android.crypto.keystore;

import java.security.KeyPair;
import java.util.Optional;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;

/**
 * Placeholder backend that lives alongside the desktop JVM sources. The real Android implementation
 * will replace this class at build time with bindings that talk to {@code android.security.keystore}.
 *
 * <p>The stub advertises zero hardware capability and fails generation calls so application code can
 * detect the absence of the Android runtime and fall back to {@link
 * org.hyperledger.iroha.android.crypto.SoftwareKeyProvider}. Tests rely on {@link KeystoreBackend}
 * fakes instead of this stub.
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
