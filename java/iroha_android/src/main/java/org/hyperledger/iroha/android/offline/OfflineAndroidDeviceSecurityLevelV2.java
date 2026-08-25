// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

/** Hardware boundary authenticated by an Android Key Attestation leaf. */
enum OfflineAndroidDeviceSecurityLevelV2 {
  TRUSTED_ENVIRONMENT(0),
  STRONG_BOX(1);

  private final long noritoDiscriminant;

  OfflineAndroidDeviceSecurityLevelV2(final long noritoDiscriminant) {
    this.noritoDiscriminant = noritoDiscriminant;
  }

  long noritoDiscriminant() {
    return noritoDiscriminant;
  }
}
