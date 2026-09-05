// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertThrows;

import java.util.Arrays;
import java.security.MessageDigest;
import org.junit.Test;

/** Java-facade checks for the fail-closed KAGEMUSHA V1 device bridge. */
public final class KagemushaDeviceLifecycleBridgeV1Tests {
  @Test
  public void nativeContractVectorProbeIsBoundedWhenLinked() {
    assertEquals(4 * 1024, KagemushaDeviceLifecycleBridgeV1.MAXIMUM_NATIVE_CONTRACT_VECTOR_BYTES);
    final byte[] vector = KagemushaDeviceLifecycleBridgeV1.nativeContractVector();
    if (vector != null) {
      org.junit.Assert.assertTrue(vector.length > 0);
      org.junit.Assert.assertTrue(
          vector.length <= KagemushaDeviceLifecycleBridgeV1.MAXIMUM_NATIVE_CONTRACT_VECTOR_BYTES);
    }
  }

  @Test
  public void explicitUnsupportedDeviceRemainsOnlineOnly() {
    final KagemushaDeviceLifecycleBridgeV1 bridge =
        KagemushaDeviceLifecycleBridgeV1.onlineOnly();
    assertEquals(
        KagemushaDeviceLifecycleBridgeV1.Availability.ONLINE_ONLY,
        bridge.availability());
    assertNull(bridge.capabilities());
    final byte[] requestId = fixed(0x11, 32);
    final byte[] command = new byte[] {1};
    assertThrows(
        IllegalStateException.class,
        () -> bridge.executeAuthenticated(
            KagemushaDeviceLifecycleBridgeV1.Operation.PREPARE_EXACT_NEXT_TRANSITION,
            requestId,
            command,
            null));
    assertArrayEquals(fixed(0x11, 32), requestId);
    assertArrayEquals(new byte[] {1}, command);
  }

  /** Real SDK framing and both enum-delegation directions; no native provider is installed. */
  @Test
  public void receiverOperationsRoundTripThroughTheCanonicalKotlinCodec() throws Exception {
    final KagemushaDeviceLifecycleBridgeV1.Operation[] operations = {
      KagemushaDeviceLifecycleBridgeV1.Operation.STAGE_INBOUND_PAYMENT,
      KagemushaDeviceLifecycleBridgeV1.Operation.RECOVER_STAGED_INBOUND_PAYMENT,
      KagemushaDeviceLifecycleBridgeV1.Operation.RECOVER_INBOUND_INBOX_PAGE,
    };
    final org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.Codec codec =
        org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.Codec.INSTANCE;
    final byte[] requestId = fixed(0x11, 32);
    // Opaque framing input only; this test does not assert a valid monetary operation payload.
    final byte[] payload = {1, 2, 3};
    for (int index = 0; index < operations.length; index++) {
      final KagemushaDeviceLifecycleBridgeV1.Operation operation = operations[index];
      final org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.Operation
          kotlinOperation =
              org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.Operation
                  .valueOf(operation.name());
      final byte[] command = codec.encodeCommand(kotlinOperation, requestId, payload);
      assertEquals(80 + payload.length, command.length);
      assertEquals(index + 2, command[10] & 0xff);
      assertArrayEquals(requestId, Arrays.copyOfRange(command, 12, 44));
      assertArrayEquals(
          MessageDigest.getInstance("SHA-256").digest(payload),
          Arrays.copyOfRange(command, 48, 80));
      assertArrayEquals(payload, Arrays.copyOfRange(command, 80, command.length));

      final byte[] response = codec.encodeResponseForTests$client_android(
          kotlinOperation,
          org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.Status.UNAVAILABLE,
          requestId,
          new byte[0],
          new byte[0]);
      final org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.Result decoded =
          codec.decodeResponse(response, kotlinOperation, requestId);
      assertEquals(operation, KagemushaDeviceLifecycleBridgeV1.Operation.valueOf(
          decoded.getOperation().name()));
      assertEquals(KagemushaDeviceLifecycleBridgeV1.Status.UNAVAILABLE,
          KagemushaDeviceLifecycleBridgeV1.Status.valueOf(decoded.getStatus().name()));
      assertThrows(IllegalArgumentException.class, () -> codec.decodeResponse(
          response,
          org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.Operation
              .READ_ACTIVE_HARDWARE_CREDENTIAL,
          requestId));
      assertThrows(IllegalArgumentException.class,
          () -> codec.decodeResponse(response, kotlinOperation, fixed(0x12, 32)));
      assertThrows(IllegalStateException.class, () ->
          KagemushaDeviceLifecycleBridgeV1.onlineOnly()
              .executeAuthenticated(operation, requestId, payload, null));
    }
  }

  @Test
  public void javaInventoryExactlyMirrorsTheKotlinContract() {
    assertEquals(22, KagemushaDeviceLifecycleBridgeV1.Operation.values().length);
    assertEquals(16, KagemushaDeviceLifecycleBridgeV1.Capability.values().length);
    assertEquals(11, KagemushaDeviceLifecycleBridgeV1.Status.values().length);
    assertArrayEquals(
        new String[] {
          "READ_ACTIVE_HARDWARE_CREDENTIAL",
          "STAGE_INBOUND_PAYMENT",
          "RECOVER_STAGED_INBOUND_PAYMENT",
          "RECOVER_INBOUND_INBOX_PAGE",
          "PREPARE_EXACT_NEXT_TRANSITION",
          "RECOVER_PREPARED_TRANSITION",
          "COMMIT_VERIFIED_CANDIDATE_AND_SIGN_TERMINAL",
          "RECOVER_TERMINAL_OUTCOME",
          "INSTALL_TERMINAL_ENVELOPE",
          "RECOVER_INSTALLED_ENVELOPE_OR_STATE_PROOF",
          "SIGN_RECEIVE_ACKNOWLEDGEMENT",
          "RELEASE_OUTBOX_ENTRY",
          "READ_TRUSTED_TIME_OR_LEASE",
          "PREPARE_MINT_AUTHORIZATION",
          "RECOVER_MINT_AUTHORIZATION",
          "VERIFY_AUTHORIZATION_AND_STAGE_MINT_CREDIT",
          "FOLD_RECEIVE_CREDIT",
          "READ_PENDING_CREDIT_WATERMARK",
          "ROTATE_HARDWARE_EPOCH",
          "BOOTSTRAP_AGGREGATE_STATE",
          "RECOVER_WALLET_SNAPSHOT",
          "CREATE_SIGNED_PAYMENT_REQUEST",
        },
        names(KagemushaDeviceLifecycleBridgeV1.Operation.values()));
    assertArrayEquals(
        new String[] {
          "SUCCESS",
          "UNAVAILABLE",
          "STALE_OR_CONCURRENT",
          "BINDING_MISMATCH",
          "TRUSTED_TIME_REJECTED",
          "REJECTED",
          "MISSING",
          "CONFLICT",
          "CORRUPT",
          "MALFORMED_REQUEST",
          "RECOVERY_REQUIRED",
        },
        names(KagemushaDeviceLifecycleBridgeV1.Status.values()));
    assertEquals(
        Arrays.toString(
            org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.Operation
                .values()),
        Arrays.toString(KagemushaDeviceLifecycleBridgeV1.Operation.values()));
    assertEquals(
        Arrays.toString(
            org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.Capability
                .values()),
        Arrays.toString(KagemushaDeviceLifecycleBridgeV1.Capability.values()));
    assertEquals(
        Arrays.toString(
            org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.Status.values()),
        Arrays.toString(KagemushaDeviceLifecycleBridgeV1.Status.values()));
    for (int index = 0; index < KagemushaDeviceLifecycleBridgeV1.Capability.values().length;
        index++) {
      assertEquals(
          1 << index, KagemushaDeviceLifecycleBridgeV1.Capability.values()[index].mask());
      assertEquals(
          org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1.Capability.values()[
                  index]
              .getMask(),
          KagemushaDeviceLifecycleBridgeV1.Capability.values()[index].mask());
    }
  }

  private static String[] names(final Enum<?>[] values) {
    final String[] names = new String[values.length];
    for (int index = 0; index < values.length; index++) {
      names[index] = values[index].name();
    }
    return names;
  }

  private static byte[] fixed(final int value, final int count) {
    final byte[] result = new byte[count];
    Arrays.fill(result, (byte) value);
    return result;
  }
}
