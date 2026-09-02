package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public final class IrohaPeerNearbySecureChannelBoundsTests {
  @Test
  public void facadeRejectsOversizedBuffersBeforeSharedDecoderCopies() {
    final IrohaPeerNearbySecureChannelV1 channel = new IrohaPeerNearbySecureChannelV1(
        IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
        IrohaPeerNearbyRoleV1.SENDER,
        filled(16, 1),
        filled(32, 2),
        filled(32, 3));

    final int maximumCertificate =
        org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1.MAXIMUM_CERTIFICATE_BYTES;
    final int maximumAuthenticationSignature =
        org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1
            .MAXIMUM_AUTHENTICATION_SIGNATURE_BYTES;
    final int maximumMessage =
        org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1.MAXIMUM_MESSAGE_BYTES;

    assertThrows(
        IllegalArgumentException.class,
        () -> channel.acceptPeerHello(new byte[10 + 16 + 32 + 32 + 2 + 65 + 4
            + maximumCertificate + 1]));
    assertThrows(
        IllegalArgumentException.class,
        () -> channel.makeAuthentication(new byte[maximumAuthenticationSignature + 1]));
    assertThrows(
        IllegalArgumentException.class,
        () -> channel.acceptPeerAuthentication(
            new byte[60 + maximumAuthenticationSignature + 1],
            (role, certificate, signedBytes, signature) -> true));
    assertThrows(
        IllegalArgumentException.class,
        () -> channel.sealIpm1(new byte[
            IrohaPeerWireMessageV1.HEADER_LENGTH
                + IrohaPeerWireMessageV1.MAXIMUM_KAGEMUSHA_ENCODED_BYTES + 1]));
    assertThrows(
        IllegalArgumentException.class,
        () -> channel.openIpm1(new byte[38 + maximumMessage + 16 + 1]));
  }

  @Test
  public void constructorRejectsOversizedBuffersBeforeCopying() {
    assertThrows(
        IllegalArgumentException.class,
        () -> new IrohaPeerNearbySecureChannelV1(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            IrohaPeerNearbyRoleV1.SENDER,
            new byte[17],
            filled(32, 2),
            filled(32, 3)));
    assertThrows(
        IllegalArgumentException.class,
        () -> new IrohaPeerNearbySecureChannelV1(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            IrohaPeerNearbyRoleV1.SENDER,
            filled(16, 1),
            filled(32, 2),
            new byte[
                org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1.MAXIMUM_CERTIFICATE_BYTES + 1]));
  }

  @Test
  public void destructionIsIdempotentAndRejectsOperationsBeforeCallbacksOrCopies() {
    final IrohaPeerNearbySecureChannelV1 channel = new IrohaPeerNearbySecureChannelV1(
        IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
        IrohaPeerNearbyRoleV1.SENDER,
        filled(16, 0x21),
        filled(32, 0x22),
        filled(32, 0x23));
    final byte[] hello = channel.localHello();
    final IrohaPeerNearbySecureChannelV1 alias = channel;

    channel.close();
    channel.destroy();

    assertTrue(alias.isDestroyed());
    assertFalse(alias.isAuthenticated());
    assertThrows(IllegalStateException.class, alias::localHello);
    assertThrows(IllegalStateException.class, () -> alias.acceptPeerHello(hello));
    assertThrows(IllegalStateException.class, alias::authenticationPreimage);
    assertThrows(IllegalStateException.class, () -> alias.makeAuthentication(new byte[] {1}));
    final int[] verifierCalls = {0};
    assertThrows(
        IllegalStateException.class,
        () -> alias.acceptPeerAuthentication(
            new byte[] {1},
            (role, certificate, signedBytes, signature) -> {
              verifierCalls[0] += 1;
              return true;
            }));
    assertEquals(0, verifierCalls[0]);
    assertThrows(IllegalStateException.class, () -> alias.sealIpm1(new byte[] {1}));
    assertThrows(IllegalStateException.class, () -> alias.openIpm1(new byte[] {1}));
  }

  @Test
  public void verifierCannotInstallKeysAfterConcurrentFacadeDestruction() {
    final byte[] session = filled(16, 0x31);
    final byte[] request = filled(32, 0x32);
    final IrohaPeerNearbySecureChannelV1 sender = new IrohaPeerNearbySecureChannelV1(
        IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
        IrohaPeerNearbyRoleV1.SENDER,
        session,
        request,
        new byte[] {1},
        filled(32, 0x33),
        scalar(5));
    final IrohaPeerNearbySecureChannelV1 receiver = new IrohaPeerNearbySecureChannelV1(
        IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
        IrohaPeerNearbyRoleV1.RECEIVER,
        session,
        request,
        new byte[] {2},
        filled(32, 0x34),
        scalar(6));
    sender.acceptPeerHello(receiver.localHello());
    receiver.acceptPeerHello(sender.localHello());
    final byte[] receiverAuthentication = receiver.makeAuthentication(new byte[] {3});
    final int[] verifierCalls = {0};

    assertThrows(
        IllegalStateException.class,
        () -> sender.acceptPeerAuthentication(
            receiverAuthentication,
            (role, certificate, signedBytes, signature) -> {
              verifierCalls[0] += 1;
              final Thread closer = new Thread(sender::close);
              closer.start();
              try {
                closer.join(2_000);
              } catch (InterruptedException failure) {
                Thread.currentThread().interrupt();
                throw new AssertionError("Interrupted while waiting for session close", failure);
              }
              assertFalse("Session close deadlocked behind verifier callback", closer.isAlive());
              return true;
            }));

    assertEquals(1, verifierCalls[0]);
    assertTrue(sender.isDestroyed());
    assertFalse(sender.isAuthenticated());
    assertThrows(IllegalStateException.class, () -> sender.makeAuthentication(new byte[] {4}));
    receiver.close();
  }

  private static byte[] filled(final int count, final int value) {
    final byte[] bytes = new byte[count];
    java.util.Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static byte[] scalar(final int value) {
    final byte[] bytes = new byte[32];
    bytes[31] = (byte) value;
    return bytes;
  }
}
