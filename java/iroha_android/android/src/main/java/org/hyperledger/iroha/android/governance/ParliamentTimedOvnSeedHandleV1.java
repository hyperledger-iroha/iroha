// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.governance;

import java.util.Objects;

/**
 * Opaque Java handle to one generation of a Parliament timed-OVN seed protected by
 * AndroidKeyStore.
 *
 * <p>Deleting and recreating the same alias creates a distinct handle generation, so an older
 * handle cannot be retargeted to the replacement seed.
 */
public final class ParliamentTimedOvnSeedHandleV1 {
  private final String alias;
  private final Object delegate;

  ParliamentTimedOvnSeedHandleV1(final String alias, final Object delegate) {
    this.alias = Objects.requireNonNull(alias, "alias");
    this.delegate = Objects.requireNonNull(delegate, "delegate");
  }

  /** Application-scoped non-secret name used to reopen this handle. */
  public String alias() {
    return alias;
  }

  Object delegate() {
    return delegate;
  }

  @Override
  public String toString() {
    return "ParliamentTimedOvnSeedHandleV1(redacted)";
  }

  @Override
  public boolean equals(final Object other) {
    return other instanceof ParliamentTimedOvnSeedHandleV1
        && alias.equals(((ParliamentTimedOvnSeedHandleV1) other).alias)
        && delegate.equals(((ParliamentTimedOvnSeedHandleV1) other).delegate);
  }

  @Override
  public int hashCode() {
    return 31 * alias.hashCode() + delegate.hashCode();
  }
}
