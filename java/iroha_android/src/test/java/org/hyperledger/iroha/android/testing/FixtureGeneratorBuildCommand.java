// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.testing;

import java.util.Arrays;
import java.util.Collections;
import java.util.List;

/** Canonical serialized Cargo command for Java tests that regenerate Rust wire fixtures. */
public final class FixtureGeneratorBuildCommand {
  private static final List<String> COMMAND =
      Collections.unmodifiableList(
          Arrays.asList(
              "cargo", "build", "--locked", "--jobs", "1", "-p", "kotlin-fixture-gen"));

  private FixtureGeneratorBuildCommand() {}

  /** Returns the immutable build command shared by every Java fixture fallback. */
  public static List<String> command() {
    return COMMAND;
  }
}
