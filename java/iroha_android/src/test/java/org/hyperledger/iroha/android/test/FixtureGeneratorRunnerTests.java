// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.test;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.io.File;
import java.io.IOException;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import org.junit.Rule;
import org.junit.Test;
import org.junit.rules.TemporaryFolder;

/** Tests fail-closed fixture-generator resolution and output separation. */
public final class FixtureGeneratorRunnerTests {
  @Rule public final TemporaryFolder temporary = new TemporaryFolder();

  @Test
  public void requiresExplicitNonblankBinaryPath() throws IOException {
    final File repoRoot = temporary.newFolder("repo-missing");
    final IllegalStateException missing =
        assertThrows(
            IllegalStateException.class,
            () -> FixtureGeneratorRunner.resolveBinary(repoRoot, Collections.emptyMap()));
    assertTrue(missing.getMessage().contains("must be set"));

    final IllegalStateException blank =
        assertThrows(
            IllegalStateException.class,
            () ->
                FixtureGeneratorRunner.resolveBinary(
                    repoRoot,
                    Collections.singletonMap(
                        FixtureGeneratorRunner.BINARY_ENVIRONMENT_VARIABLE, " \t")));
    assertTrue(blank.getMessage().contains("must not be blank"));
  }

  @Test
  public void resolvesRelativePathFromRepositoryRoot() throws IOException {
    final File repoRoot = temporary.newFolder("repo-relative");
    final File binary = executable(repoRoot, "tools/bin/kotlin-fixture-gen");

    final File resolved =
        FixtureGeneratorRunner.resolveBinary(
            repoRoot,
            Collections.singletonMap(
                FixtureGeneratorRunner.BINARY_ENVIRONMENT_VARIABLE,
                "tools/bin/kotlin-fixture-gen"));

    assertEquals(binary.getAbsolutePath(), resolved.getAbsolutePath());
  }

  @Test
  public void acceptsAbsoluteExecutableAndIgnoresCargoSettings() throws IOException {
    final File repoRoot = temporary.newFolder("repo-absolute");
    final File binary = executable(repoRoot, "outside-target/kotlin-fixture-gen");
    final Map<String, String> environment = new HashMap<>();
    environment.put(FixtureGeneratorRunner.BINARY_ENVIRONMENT_VARIABLE, binary.getAbsolutePath());
    environment.put("CARGO", "/must/not/run/cargo");
    environment.put("CARGO_TARGET_DIR", "ignored");

    final File resolved = FixtureGeneratorRunner.resolveBinary(repoRoot, environment);

    assertEquals(binary.getAbsolutePath(), resolved.getAbsolutePath());
  }

  @Test
  public void rejectsMissingPathAndDirectory() throws IOException {
    final File repoRoot = temporary.newFolder("repo-non-file");
    final IllegalStateException missing =
        assertThrows(
            IllegalStateException.class,
            () ->
                FixtureGeneratorRunner.resolveBinary(
                    repoRoot,
                    Collections.singletonMap(
                        FixtureGeneratorRunner.BINARY_ENVIRONMENT_VARIABLE, "missing-generator")));
    assertTrue(missing.getMessage().contains("existing regular file"));

    final File directory = new File(repoRoot, "generator-directory");
    assertTrue(directory.mkdir());
    final IllegalStateException notAFile =
        assertThrows(
            IllegalStateException.class,
            () ->
                FixtureGeneratorRunner.resolveBinary(
                    repoRoot,
                    Collections.singletonMap(
                        FixtureGeneratorRunner.BINARY_ENVIRONMENT_VARIABLE,
                        directory.getAbsolutePath())));
    assertTrue(notAFile.getMessage().contains("existing regular file"));
  }

  @Test
  public void rejectsNonExecutableRegularFile() throws IOException {
    final File repoRoot = temporary.newFolder("repo-non-executable");
    final File binary = new File(repoRoot, "kotlin-fixture-gen");
    assertTrue(binary.createNewFile());
    binary.setExecutable(false, false);
    assertFalse(binary.canExecute());

    final IllegalStateException error =
        assertThrows(
            IllegalStateException.class,
            () ->
                FixtureGeneratorRunner.resolveBinary(
                    repoRoot,
                    Collections.singletonMap(
                        FixtureGeneratorRunner.BINARY_ENVIRONMENT_VARIABLE,
                        binary.getAbsolutePath())));
    assertTrue(error.getMessage().contains("is not executable"));
  }

  @Test
  public void commandContainsOnlyExplicitBinaryAndSubcommand() throws IOException {
    final File repoRoot = temporary.newFolder("repo-command");
    final File binary = executable(repoRoot, "kotlin-fixture-gen");

    final List<String> command = FixtureGeneratorRunner.commandFor(binary, "claim-identifier");

    assertEquals(Arrays.asList(binary.getAbsolutePath(), "claim-identifier"), command);
    assertFalse(command.contains("cargo"));
  }

  @Test
  public void successfulStderrDiagnosticsNeverBecomeFixtureRows() {
    final List<String> rows =
        FixtureGeneratorRunner.outputLines(
            "claim-identifier", 0, "first-row\nsecond-row\n", "diagnostic only\n");

    assertEquals(Arrays.asList("first-row", "second-row"), rows);
  }

  @Test
  public void failedCommandReportsStderrWithoutAcceptingStdout() {
    final IllegalStateException error =
        assertThrows(
            IllegalStateException.class,
            () ->
                FixtureGeneratorRunner.outputLines(
                    "claim-identifier", 9, "not-a-fixture\n", "generator failed\n"));

    assertTrue(error.getMessage().contains("generator failed"));
    assertFalse(error.getMessage().contains("not-a-fixture"));
  }

  private static File executable(final File repoRoot, final String relativePath)
      throws IOException {
    final File binary = new File(repoRoot, relativePath);
    assertTrue(binary.getParentFile().mkdirs() || binary.getParentFile().isDirectory());
    assertTrue(binary.createNewFile());
    assertTrue(binary.setExecutable(true, false) || binary.canExecute());
    return binary;
  }
}
