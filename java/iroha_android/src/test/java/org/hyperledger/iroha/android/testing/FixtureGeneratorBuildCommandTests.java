// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.testing;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import java.util.Arrays;
import org.junit.Test;

/** Locks Java fixture regeneration to the checked-in dependency graph and one Cargo job. */
public final class FixtureGeneratorBuildCommandTests {

  @Test
  public void buildCommandIsLockedAndSerialized() {
    assertEquals(
        Arrays.asList(
            "cargo", "build", "--locked", "--jobs", "1", "-p", "kotlin-fixture-gen"),
        FixtureGeneratorBuildCommand.command());
    assertThrows(
        UnsupportedOperationException.class,
        () -> FixtureGeneratorBuildCommand.command().set(0, "substituted-cargo"));
  }
}
