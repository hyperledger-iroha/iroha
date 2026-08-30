package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;

import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.Arrays;
import java.util.List;
import org.hyperledger.iroha.sdk.privacy.PrivacyAnonymousPgcPoolStateRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyFinalizedStateRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyOrchardNullifierRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyOrchardPoolStateRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyProofManagedPoolStateRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyProtocolIdV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyZkAceReplayNullifierRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyZkAmsAdmissionRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyZkAmsProvisionRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyZkX509CertificateNullifierRequestV1;
import org.junit.Test;

public final class AuthenticatedPrivacyStateQueryNativeBridgeTests {
  @Test
  public void nativeSurfaceIsClosedAndPairedWithAbi22() throws Exception {
    assertEquals(22, AuthenticatedPrivacyStateQueryNativeBridge.REQUIRED_BRIDGE_ABI_VERSION);
    assertNative("nativeBridgeAbiVersion", int.class);
    assertNative(
        "nativePreparePrivacyStateQueryV1",
        byte[][].class,
        byte[].class,
        byte[].class,
        int.class,
        int.class,
        byte[].class,
        long.class,
        byte[].class);
    assertNative(
        "nativeFinalizePrivacyStateQueryV1",
        byte[].class,
        byte[].class,
        byte[].class);
    assertNative(
        "nativeProjectPrivacyStateQueryV1",
        byte[].class,
        byte[].class,
        byte[].class);
  }

  @Test
  public void selectorsCoverExactlyIds97Through104() {
    final List<PrivacyFinalizedStateRequestV1> requests =
        List.of(
            new PrivacyZkAceReplayNullifierRequestV1(fixed32(1), fixed32(2)),
            new PrivacyProofManagedPoolStateRequestV1(
                PrivacyProtocolIdV1.MONERO_FCMP_PLUS_PLUS_V1, fixed32(3)),
            new PrivacyOrchardPoolStateRequestV1(fixed32(4)),
            new PrivacyOrchardNullifierRequestV1(fixed32(5), fixed32(6)),
            new PrivacyAnonymousPgcPoolStateRequestV1(fixed32(7)),
            new PrivacyZkAmsAdmissionRequestV1(
                fixed32(8), fixed32(9), fixed32(10), fixed32(11)),
            new PrivacyZkAmsProvisionRequestV1(
                fixed32(12), fixed32(13), fixed32(14), fixed32(15)),
            new PrivacyZkX509CertificateNullifierRequestV1(
                fixed32(16), fixed32(17), fixed32(18)));
    final int[] widths = {64, 32, 32, 64, 32, 128, 128, 96};
    for (int index = 0; index < requests.size(); index++) {
      final PrivacyFinalizedStateRequestV1 request = requests.get(index);
      assertEquals(97 + index, request.getQueryId());
      assertEquals(0, request.getProtocolIndex());
      assertEquals(widths[index], request.requestBinding().length);
    }
  }

  @Test
  public void x509BindingContainsNoDuplicatedPolicyChunk() {
    final byte[] trustAnchor = fixed32(0x31);
    final byte[] policy = fixed32(0x32);
    final byte[] nullifier = fixed32(0x33);
    final byte[] binding =
        new PrivacyZkX509CertificateNullifierRequestV1(trustAnchor, policy, nullifier)
            .requestBinding();
    assertEquals(96, binding.length);
    assertArrayEquals(trustAnchor, Arrays.copyOfRange(binding, 0, 32));
    assertArrayEquals(policy, Arrays.copyOfRange(binding, 32, 64));
    assertArrayEquals(nullifier, Arrays.copyOfRange(binding, 64, 96));
  }

  private static byte[] fixed32(final int value) {
    final byte[] output = new byte[32];
    Arrays.fill(output, (byte) value);
    return output;
  }

  private static void assertNative(
      final String name, final Class<?> result, final Class<?>... arguments) throws Exception {
    final Method method =
        AuthenticatedPrivacyStateQueryNativeBridge.class.getDeclaredMethod(name, arguments);
    assertEquals(result, method.getReturnType());
    assertEquals(true, Modifier.isNative(method.getModifiers()));
    assertEquals(true, Modifier.isStatic(method.getModifiers()));
  }
}
