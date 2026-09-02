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
    assertEquals(16, OfflineCashDeviceLifecycleBridgeV1.Operation.values().length);
    assertEquals(16, OfflineCashDeviceLifecycleBridgeV1.Capability.values().length);
    assertEquals(10, OfflineCashDeviceLifecycleBridgeV1.Status.values().length);
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
    for (int index = 0; index < OfflineCashDeviceLifecycleBridgeV1.Capability.values().length;
        index++) {
      assertEquals(
          org.hyperledger.iroha.sdk.offline.OfflineCashDeviceLifecycleBridgeV1.Capability.values()[
                  index]
              .getMask(),
          OfflineCashDeviceLifecycleBridgeV1.Capability.values()[index].mask());
    }
  }

  private static byte[] fixed(final int value, final int count) {
    final byte[] result = new byte[count];
    Arrays.fill(result, (byte) value);
    return result;
  }
}
