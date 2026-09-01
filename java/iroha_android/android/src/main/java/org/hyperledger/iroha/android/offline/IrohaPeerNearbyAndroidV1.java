package org.hyperledger.iroha.android.offline;

import android.content.Context;
import java.util.Arrays;
import java.util.Objects;
import java.util.concurrent.Executor;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyConnectionsConfigurationV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyConnectionsTransportV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyDiscoveryContextV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbySendCompletionV1;

/** Java facade that keeps Google Nearby BYTES behind the authenticated IPN1 core. */
public final class IrohaPeerNearbyAndroidV1 {
  private IrohaPeerNearbyAndroidV1() {}

  public static IrohaPeerNearbyDiscoveryContextV1 discoveryContext(
      final IrohaPeerPayloadProfile profile,
      final IrohaPeerNearbyRoleV1 role,
      final byte[] sessionId,
      final byte[] requestCanonicalHash) {
    final byte[] requiredSession = requireLength(sessionId, 16, "sessionId");
    final byte[] requiredRequest = requireLength(requestCanonicalHash, 32, "requestCanonicalHash");
    final byte[] ownedSession = requiredSession.clone();
    final byte[] ownedRequest = requiredRequest.clone();
    try {
      return new IrohaPeerNearbyDiscoveryContextV1(
          IrohaPeerNfcV1.sharedProfile(Objects.requireNonNull(profile, "profile")),
          Objects.requireNonNull(role, "role").toShared(),
          ownedSession,
          ownedRequest);
    } finally {
      Arrays.fill(ownedRequest, (byte) 0);
      Arrays.fill(ownedSession, (byte) 0);
    }
  }

  /** Discovery-only all-zero sentinel; it is never accepted by the IPN1 secure channel. */
  public static IrohaPeerNearbyDiscoveryContextV1 bootstrapContext(
      final IrohaPeerPayloadProfile profile) {
    return IrohaPeerNearbyDiscoveryContextV1.senderBootstrap(
        IrohaPeerNfcV1.sharedProfile(Objects.requireNonNull(profile, "profile")));
  }

  public static IrohaPeerNearbyConnectionsTransportV1 transport(final Context context) {
    return new IrohaPeerNearbyConnectionsTransportV1(
        Objects.requireNonNull(context, "context"));
  }

  public static IrohaPeerNearbyConnectionsTransportV1 transport(
      final Context context,
      final IrohaPeerNearbyConnectionsConfigurationV1 configuration,
      final Executor callbackExecutor) {
    return new IrohaPeerNearbyConnectionsTransportV1(
        Objects.requireNonNull(context, "context"),
        Objects.requireNonNull(configuration, "configuration"),
        Objects.requireNonNull(callbackExecutor, "callbackExecutor"));
  }

  public static void advertise(
      final IrohaPeerNearbyConnectionsTransportV1 transport,
      final IrohaPeerNearbyDiscoveryContextV1 context) {
    transport.startAdvertising(context);
  }

  public static void discover(
      final IrohaPeerNearbyConnectionsTransportV1 transport,
      final IrohaPeerNearbyDiscoveryContextV1 context) {
    transport.startDiscovering(context);
  }

  /** Seals a verified IPM1 message before it reaches the radio adapter. */
  public static void sendIpm1(
      final IrohaPeerNearbyConnectionsTransportV1 transport,
      final IrohaPeerNearbySecureChannelV1 channel,
      final byte[] encodedIpm1,
      final IrohaPeerNearbySendCompletionV1 completion) {
    transport.send(channel.sealIpm1(encodedIpm1), completion);
  }

  /** Authenticates, sequence-checks, decrypts, and verifies an incoming IPM1 record. */
  public static byte[] openIpm1(
      final IrohaPeerNearbySecureChannelV1 channel, final byte[] encryptedRecord) {
    return channel.openIpm1(encryptedRecord);
  }

  private static byte[] requireLength(
      final byte[] value, final int expectedLength, final String name) {
    final byte[] required = Objects.requireNonNull(value, name);
    if (required.length != expectedLength) {
      throw new IllegalArgumentException(name + " length is outside its Nearby V1 bound");
    }
    return required;
  }
}
