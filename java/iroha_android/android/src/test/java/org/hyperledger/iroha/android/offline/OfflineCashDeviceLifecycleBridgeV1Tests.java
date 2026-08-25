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
            OfflineCashDeviceLifecycleBridgeV1.Operation.COMMIT_INTENT_EXACT_NEXT,
            requestId,
            command));
    assertArrayEquals(fixed(0x11, 32), requestId);
    assertArrayEquals(new byte[] {1}, command);
  }

  @Test
  public void javaInventoryExactlyMirrorsTheKotlinContract() {
    assertEquals(14, OfflineCashDeviceLifecycleBridgeV1.Operation.values().length);
    assertEquals(10, OfflineCashDeviceLifecycleBridgeV1.Status.values().length);
    assertEquals(
        "[SUCCESS, UNAVAILABLE, STALE_OR_CONCURRENT, INTENT_MISMATCH, "
            + "TRUSTED_TIME_REJECTED, POLICY_REJECTED, MISSING, CONFLICT, CORRUPT, "
            + "MALFORMED_REQUEST]",
        Arrays.toString(OfflineCashDeviceLifecycleBridgeV1.Status.values()));
    assertEquals(
        Arrays.toString(
            org.hyperledger.iroha.sdk.offline.OfflineCashDeviceLifecycleBridgeV1.Operation
                .values()),
        Arrays.toString(OfflineCashDeviceLifecycleBridgeV1.Operation.values()));
  }

  private static byte[] fixed(final int value, final int count) {
    final byte[] result = new byte[count];
    Arrays.fill(result, (byte) value);
    return result;
  }
}
