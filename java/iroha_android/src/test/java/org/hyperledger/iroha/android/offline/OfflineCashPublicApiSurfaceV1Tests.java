package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertFalse;

import java.lang.reflect.Modifier;
import java.util.Arrays;
import java.util.List;
import org.junit.Test;

/** Prevents the retained Kagemusha implementation substrate from re-entering Java's public API. */
public final class OfflineCashPublicApiSurfaceV1Tests {

  @Test
  public void legacyKagemushaAndOfflineV2TypesArePackagePrivate() {
    final List<Class<?>> retainedInternalTypes =
        Arrays.asList(
            DeviceAttestationRegistration.class,
            RegisterOfflineDeviceAttestation.class,
            IrohaPeerKagemushaAdapterV1.class,
            KagemushaDevicePublicKeyV2.class,
            KagemushaDeviceSignatureV2.class,
            KagemushaNearby.class,
            KagemushaNfcProtocol.class,
            KagemushaP256Codec.class,
            KagemushaPeerTransport.class,
            KagemushaQrStream.class,
            KagemushaRecursiveSpendProver.class,
            KagemushaScaledAmount.class,
            OfflineAndroidAttestedDevicePropertiesV2.class,
            OfflineAndroidDeviceSecurityLevelV2.class);

    for (final Class<?> type : retainedInternalTypes) {
      assertFalse(type.getName() + " must be package-private", Modifier.isPublic(type.getModifiers()));
    }
  }
}
