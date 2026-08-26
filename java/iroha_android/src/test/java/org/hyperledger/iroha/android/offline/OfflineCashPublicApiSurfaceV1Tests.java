package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import java.lang.reflect.Constructor;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.lang.reflect.Type;
import java.util.ArrayList;
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

  @Test
  public void publicFacadeExposesOnlyOfflineCashV1TypesAndExactRoutes() {
    assertTrue(Modifier.isPublic(OfflineCashToriiV1.class.getModifiers()));
    assertEquals("/v1/offline/readiness", OfflineCashToriiV1.ClientV1.READINESS_PATH);
    assertEquals("/v1/offline/top-up", OfflineCashToriiV1.ClientV1.TOP_UP_PATH);
    assertEquals("/v1/offline/redeem", OfflineCashToriiV1.ClientV1.REDEEM_PATH);
    assertEquals("/v1/offline/operations", OfflineCashToriiV1.ClientV1.OPERATIONS_PATH);

    final List<String> leaks = new ArrayList<>();
    final List<Class<?>> facadeTypes = new ArrayList<>();
    facadeTypes.add(OfflineCashToriiV1.class);
    facadeTypes.addAll(Arrays.asList(OfflineCashToriiV1.class.getDeclaredClasses()));
    for (final Class<?> type : facadeTypes) {
      for (final Constructor<?> constructor : type.getDeclaredConstructors()) {
        if (Modifier.isPublic(constructor.getModifiers())) {
          inspectTypes(constructor.toGenericString(), constructor.getGenericParameterTypes(), leaks);
        }
      }
      for (final Method method : type.getDeclaredMethods()) {
        if (Modifier.isPublic(method.getModifiers())) {
          inspectTypes(
              method.toGenericString(),
              concat(method.getGenericReturnType(), method.getGenericParameterTypes()),
              leaks);
        }
      }
    }
    assertTrue("public Offline Cash V1 signatures expose internal types: " + leaks, leaks.isEmpty());
  }

  private static void inspectTypes(
      final String signature, final Type[] types, final List<String> leaks) {
    for (final Type type : types) {
      final String name = type.getTypeName();
      if (name.contains("Kagemusha") || name.contains("OfflineV2")) {
        leaks.add(signature);
        return;
      }
    }
  }

  private static Type[] concat(final Type first, final Type[] remainder) {
    final Type[] values = new Type[remainder.length + 1];
    values[0] = first;
    System.arraycopy(remainder, 0, values, 1, remainder.length);
    return values;
  }
}
