package org.hyperledger.iroha.android.crypto.keystore;

import java.security.KeyPair;
import java.util.Optional;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;
import org.hyperledger.iroha.android.crypto.keystore.KeyAttestation;

/**
 * Facade over the Android Keystore (and StrongBox) primitives.
 *
 * <p>This abstraction allows the desktop JVM build to compile without depending on the Android SDK
 * while enabling platform-specific backends in instrumentation builds. The default implementation
 * will land alongside the Android integration and will delegate to {@code android.security.keystore}
 * APIs.
 */
public interface KeystoreBackend {

  /** Load an existing key identified by {@code alias}. */
  Optional<KeyPair> load(String alias) throws KeyManagementException;

  /** Generate and persist a key for {@code alias}. */
  KeyGenerationResult generate(String alias, KeyGenParameters parameters) throws KeyManagementException;

  /** Generate a transient key that must not be persisted. */
  KeyPair generateEphemeral(KeyGenParameters parameters) throws KeyManagementException;

  /** Metadata describing the backing store/hardware. */
  KeyProviderMetadata metadata();

  /** Human readable backend name (e.g., {@code android-keystore}). */
  String name();

  /**
   * Returns the attestation material for {@code alias}, if available.
   *
   * <p>Backends should return an empty optional when attestation is unsupported or the alias has no
   * recorded certificates. The Android implementation will populate this with the StrongBox/TEE
   * attestation chain.
   */
  default Optional<KeyAttestation> attestation(final String alias) {
    return Optional.empty();
  }

  /**
   * Generates fresh attestation material for {@code alias} using the provided challenge. Providers
   * that do not support attestation should return {@link Optional#empty()}.
   */
  default Optional<KeyAttestation> generateAttestation(
      final String alias, final byte[] challenge) throws KeyManagementException {
    return Optional.empty();
  }
}
