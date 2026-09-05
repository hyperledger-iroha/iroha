// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;

import org.hyperledger.iroha.sdk.offline.KagemushaOperationKindV1;
import org.junit.Test;

public final class KagemushaWalletV1Tests {
  @Test
  public void monetaryOperationTagsAreTheSixAggregateBalanceTransitions() {
    final int[] tags = new int[KagemushaOperationKindV1.values().length];
    for (int index = 0; index < tags.length; index++) {
      tags[index] = KagemushaOperationKindV1.values()[index].wireTag;
    }
    assertArrayEquals(new int[] {0, 1, 2, 3, 4, 5}, tags);
  }
}
