// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.governance;

/** Closed first-release choice set for one secret-local Parliament timed-OVN ballot. */
public enum ParliamentTimedOvnBallotChoiceV1 {
  AYE(0),
  NAY(1),
  ABSTAIN(2);

  private final int code;

  ParliamentTimedOvnBallotChoiceV1(final int code) {
    this.code = code;
  }

  /** Exact native V1 discriminant. */
  public int code() {
    return code;
  }
}
