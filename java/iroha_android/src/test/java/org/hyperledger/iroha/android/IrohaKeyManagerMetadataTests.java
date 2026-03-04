package org.hyperledger.iroha.android;

import java.util.List;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;

public final class IrohaKeyManagerMetadataTests {

  private IrohaKeyManagerMetadataTests() {}

  public static void main(final String[] args) throws Exception {
    providerMetadataReflectsSoftwareFallback();
    providerMetadataIsUnmodifiable();
    System.out.println("[IrohaAndroid] Key manager metadata tests passed.");
  }

  private static void providerMetadataIsUnmodifiable() throws Exception {
    final IrohaKeyManager manager = IrohaKeyManager.withSoftwareFallback();
    final List<KeyProviderMetadata> metadata = manager.providerMetadata();
    boolean threw = false;
    try {
      metadata.add(KeyProviderMetadata.software("dummy"));
    } catch (final UnsupportedOperationException expected) {
      threw = true;
    }
    assert threw : "provider metadata list must be unmodifiable";
  }

  private static void providerMetadataReflectsSoftwareFallback() throws Exception {
    final IrohaKeyManager manager = IrohaKeyManager.withSoftwareFallback();
    final List<KeyProviderMetadata> metadata = manager.providerMetadata();
    assert metadata.size() == 1 : "Expected only software provider";
    final KeyProviderMetadata entry = metadata.get(0);
    assert entry.name().equals("software-key-provider") : "Unexpected provider name";
    assert !entry.hardwareBacked() : "Software provider must not be hardware-backed";
    assert entry.securityLevel() == KeyProviderMetadata.HardwareSecurityLevel.NONE
        : "Software provider security level must be NONE";
  }
}
