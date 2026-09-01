package org.hyperledger.iroha.android;

import java.util.List;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;
import org.hyperledger.iroha.android.crypto.SigningAlgorithm;
import org.hyperledger.iroha.android.crypto.keystore.KeyGenParameters;
import org.hyperledger.iroha.android.telemetry.KeystoreTelemetryEmitter;

public final class IrohaKeyManagerMetadataTests {

  private IrohaKeyManagerMetadataTests() {}

  public static void main(final String[] args) throws Exception {
    providerMetadataReflectsSoftwareProvider();
    defaultProvidersAlwaysIncludeSoftwareSigning();
    providerMetadataIsUnmodifiable();
    System.out.println("[IrohaAndroid] Key manager metadata tests passed.");
  }

  private static void defaultProvidersAlwaysIncludeSoftwareSigning() {
    final KeyGenParameters parameters =
        KeyGenParameters.builder().setSigningAlgorithm(SigningAlgorithm.ML_DSA).build();
    for (final IrohaKeyManager manager :
        List.of(
            IrohaKeyManager.withDefaultProviders(parameters),
            IrohaKeyManager.withDefaultProviders(
                parameters, KeystoreTelemetryEmitter.noop()))) {
      final List<KeyProviderMetadata> metadata = manager.providerMetadata();
      assert metadata.size() == 1 : "Expected a software provider for a software-only algorithm";
      assert metadata.get(0).securityLevel() == KeyProviderMetadata.HardwareSecurityLevel.NONE
          : "Default providers must retain a software signing path";
    }
  }

  private static void providerMetadataIsUnmodifiable() throws Exception {
    final IrohaKeyManager manager = IrohaKeyManager.withSoftwareProvider();
    final List<KeyProviderMetadata> metadata = manager.providerMetadata();
    boolean threw = false;
    try {
      metadata.add(KeyProviderMetadata.software("dummy"));
    } catch (final UnsupportedOperationException expected) {
      threw = true;
    }
    assert threw : "provider metadata list must be unmodifiable";
  }

  private static void providerMetadataReflectsSoftwareProvider() throws Exception {
    final IrohaKeyManager manager = IrohaKeyManager.withSoftwareProvider();
    final List<KeyProviderMetadata> metadata = manager.providerMetadata();
    assert metadata.size() == 1 : "Expected only software provider";
    final KeyProviderMetadata entry = metadata.get(0);
    assert entry.name().equals("software-key-provider") : "Unexpected provider name";
    assert !entry.hardwareBacked() : "Software provider must not be hardware-backed";
    assert entry.securityLevel() == KeyProviderMetadata.HardwareSecurityLevel.NONE
        : "Software provider security level must be NONE";
  }
}
