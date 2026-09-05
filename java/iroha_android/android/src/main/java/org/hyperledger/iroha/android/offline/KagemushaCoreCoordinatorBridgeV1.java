// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.util.List;
import org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorMethodV1;

/** Java mirror of the exact Kotlin native coordinator transport, without a software backend. */
public final class KagemushaCoreCoordinatorBridgeV1 {
  private final org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorBridgeV1 delegate;

  private KagemushaCoreCoordinatorBridgeV1(
      final org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorBridgeV1 delegate) {
    this.delegate = delegate;
  }

  /** Open only the current native contract; an absent qualified backend remains unavailable. */
  public static KagemushaCoreCoordinatorBridgeV1 open(final String storagePath) {
    return new KagemushaCoreCoordinatorBridgeV1(
        org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorBridgeV1.open(storagePath));
  }

  /** Invoke and correlate one native method while retaining embedded archives as opaque bytes. */
  public List<byte[]> invoke(
      final KagemushaCoreCoordinatorMethodV1 method, final List<byte[]> fields) {
    return delegate.invoke(method, fields);
  }
}
