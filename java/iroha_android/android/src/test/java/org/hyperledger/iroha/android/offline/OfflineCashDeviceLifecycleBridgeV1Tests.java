// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertThrows;

import java.util.Arrays;
import org.junit.Test;

/** Migration-facade checks for the fail-closed Offline Cash V1 device bridge. */
public final class OfflineCashDeviceLifecycleBridgeV1Tests {
  @Test
  public void explicitUnsupportedDeviceRemainsOnlineOnly() {
    final OfflineCashDeviceLifecycleBridgeV1 bridge =
        OfflineCashDeviceLifecycleBridgeV1.onlineOnly();
    assertEquals(
        OfflineCashDeviceLifecycleBridgeV1.Availability.ONLINE_ONLY,
        bridge.availability());
    assertNull(bridge.capabilities());
    final byte[] requestId = fixed(0x11, 32);
    final byte[] command = new byte[] {1};
    assertThrows(
        IllegalStateException.class,
        () -> bridge.execute(
            OfflineCashDeviceLifecycleBridgeV1.Operation.PREPARE_EXACT_NEXT_TRANSITION,
            requestId,
            command));
    assertArrayEquals(fixed(0x11, 32), requestId);
    assertArrayEquals(new byte[] {1}, command);
  }

  @Test
  public void javaInventoryExactlyMirrorsTheKotlinContract() {
    assertEquals(24, OfflineCashDeviceLifecycleBridgeV1.Operation.values().length);
    assertEquals(16, OfflineCashDeviceLifecycleBridgeV1.Capability.values().length);
    assertEquals(11, OfflineCashDeviceLifecycleBridgeV1.Status.values().length);
    assertArrayEquals(
        new String[] {
          "READ_ACTIVE_HARDWARE_CREDENTIAL",
          "PREPARE_ACCEPTANCE_INTENT_AUTHORIZATION",
          "RECOVER_ACCEPTANCE_INTENT_AUTHORIZATION",
          "VERIFY_AUTHORIZATION_RESERVE_INBOX_AND_ISSUE_ACCEPTANCE_TICKET",
          "RECOVER_ACCEPTANCE_TICKET",
          "STAGE_INBOUND_PAYMENT",
          "RECOVER_STAGED_INBOUND_PAYMENT",
          "RECOVER_INBOUND_INBOX_PAGE",
          "PREPARE_EXACT_NEXT_TRANSITION",
          "RECOVER_PREPARED_TRANSITION",
          "ABANDON_UNCOMMITTED_PREPARED_TRANSITION",
          "COMMIT_VERIFIED_CANDIDATE",
          "RECOVER_TERMINAL_COMMIT_CERTIFICATE",
          "INSTALL_FINAL_COMMIT_WRAPPER",
          "RECOVER_INSTALLED_ENVELOPE_OR_STATE_PROOF",
          "SIGN_RECEIVE_ACKNOWLEDGEMENT",
          "RELEASE_OUTBOX_ENTRY",
          "READ_TRUSTED_TIME_OR_LEASE",
          "PREPARE_MINT_AUTHORIZATION",
          "RECOVER_MINT_AUTHORIZATION",
          "VERIFY_AUTHORIZATION_AND_STAGE_MINT_CREDIT",
          "FOLD_RECEIVE",
          "READ_PENDING_CREDIT_WATERMARK",
          "ROTATE_HARDWARE_EPOCH",
        },
        names(OfflineCashDeviceLifecycleBridgeV1.Operation.values()));
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
        names(OfflineCashDeviceLifecycleBridgeV1.Status.values()));
    assertEquals(
        Arrays.toString(
            org.hyperledger.iroha.sdk.offline.OfflineCashDeviceLifecycleBridgeV1.Operation
                .values()),
        Arrays.toString(OfflineCashDeviceLifecycleBridgeV1.Operation.values()));
    assertEquals(
        Arrays.toString(
            org.hyperledger.iroha.sdk.offline.OfflineCashDeviceLifecycleBridgeV1.Capability
                .values()),
        Arrays.toString(OfflineCashDeviceLifecycleBridgeV1.Capability.values()));
    assertEquals(
        Arrays.toString(
            org.hyperledger.iroha.sdk.offline.OfflineCashDeviceLifecycleBridgeV1.Status.values()),
        Arrays.toString(OfflineCashDeviceLifecycleBridgeV1.Status.values()));
    for (int index = 0; index < OfflineCashDeviceLifecycleBridgeV1.Capability.values().length;
        index++) {
      assertEquals(
          1 << index, OfflineCashDeviceLifecycleBridgeV1.Capability.values()[index].mask());
      assertEquals(
          org.hyperledger.iroha.sdk.offline.OfflineCashDeviceLifecycleBridgeV1.Capability.values()[
                  index]
              .getMask(),
          OfflineCashDeviceLifecycleBridgeV1.Capability.values()[index].mask());
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
