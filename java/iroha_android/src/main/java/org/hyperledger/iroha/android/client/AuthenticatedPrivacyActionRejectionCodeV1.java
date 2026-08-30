// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.client;

/** Closed ABI-22 classification of a finalized Exact12 transaction rejection. */
public enum AuthenticatedPrivacyActionRejectionCodeV1 {
  ACCOUNT_DOES_NOT_EXIST("account_does_not_exist"),
  LIMIT_CHECK("limit_check"),
  VALIDATION("validation"),
  INSTRUCTION_EXECUTION("instruction_execution"),
  IVM_EXECUTION("ivm_execution"),
  TRIGGER_EXECUTION("trigger_execution");

  private final String canonicalLabel;

  AuthenticatedPrivacyActionRejectionCodeV1(final String canonicalLabel) {
    this.canonicalLabel = canonicalLabel;
  }

  public String canonicalLabel() {
    return canonicalLabel;
  }

  public static AuthenticatedPrivacyActionRejectionCodeV1 fromCanonicalLabel(
      final String label) {
    for (final AuthenticatedPrivacyActionRejectionCodeV1 value : values()) {
      if (value.canonicalLabel.equals(label)) return value;
    }
    throw new IllegalArgumentException("unknown finalized Exact12 rejection code");
  }
}
