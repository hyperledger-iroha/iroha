// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertNull;

import org.junit.Test;

public final class IrohaPeerNfcV1AdversarialTests {
  @Test
  public void removedPublicMessageTagsAreRejected() {
    assertNull(IrohaPeerPayloadKind.fromCode(0));
    assertNull(IrohaPeerPayloadKind.fromCode(4));
    assertNull(IrohaPeerPayloadKind.fromCode(5));
  }
}
