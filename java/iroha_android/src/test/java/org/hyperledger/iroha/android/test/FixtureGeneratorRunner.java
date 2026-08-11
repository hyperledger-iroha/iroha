// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.test;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;

/** Runs the explicitly configured Rust fixture generator for Java parity tests. */
public final class FixtureGeneratorRunner {
  static final String BINARY_ENVIRONMENT_VARIABLE = "IROHA_KOTLIN_FIXTURE_GEN_BIN";

  private FixtureGeneratorRunner() {}

  /**
   * Runs one fixture subcommand.
   *
   * <p>The environment variable must name an executable file. Relative paths are resolved from the
   * Iroha repository root. This helper never invokes Cargo or builds the generator.
   */
  public static List<String> run(final String subcommand)
      throws IOException, InterruptedException {
    final File repoRoot = locateRepoRoot();
    final File binary = resolveBinary(repoRoot, System.getenv());
    final Process process =
        new ProcessBuilder(commandFor(binary, subcommand))
            .directory(repoRoot)
            .redirectErrorStream(false)
            .start();

    final ExecutorService readers = Executors.newFixedThreadPool(2);
    final Future<String> stdout = readers.submit(() -> readStream(process.getInputStream()));
    final Future<String> stderr = readers.submit(() -> readStream(process.getErrorStream()));
    try {
      final int exit = process.waitFor();
      return outputLines(subcommand, exit, awaitStream(stdout), awaitStream(stderr));
    } catch (InterruptedException error) {
      process.destroyForcibly();
      Thread.currentThread().interrupt();
      throw error;
    } finally {
      readers.shutdownNow();
    }
  }

  static File resolveBinary(final File repoRoot, final Map<String, String> environment) {
    final String configuredPath = environment.get(BINARY_ENVIRONMENT_VARIABLE);
    if (configuredPath == null) {
      throw new IllegalStateException(
          BINARY_ENVIRONMENT_VARIABLE + " must be set to the kotlin-fixture-gen executable");
    }
    if (configuredPath.isBlank()) {
      throw new IllegalStateException(BINARY_ENVIRONMENT_VARIABLE + " must not be blank");
    }

    final File configured = new File(configuredPath);
    final File binary =
        (configured.isAbsolute() ? configured : new File(repoRoot, configuredPath)).getAbsoluteFile();
    if (!binary.isFile()) {
      throw new IllegalStateException(
          BINARY_ENVIRONMENT_VARIABLE
              + " must name an existing regular file: "
              + binary.getAbsolutePath());
    }
    if (!binary.canExecute()) {
      throw new IllegalStateException(
          BINARY_ENVIRONMENT_VARIABLE + " is not executable: " + binary.getAbsolutePath());
    }
    return binary;
  }

  static List<String> commandFor(final File binary, final String subcommand) {
    if (subcommand == null || subcommand.trim().isEmpty()) {
      throw new IllegalArgumentException("fixture-generator subcommand must not be blank");
    }
    return Arrays.asList(binary.getAbsolutePath(), subcommand);
  }

  /** Keeps diagnostics out of fixture rows and exposes them only on failure. */
  static List<String> outputLines(
      final String subcommand,
      final int exit,
      final String stdout,
      final String stderr) {
    final String output = stdout.strip();
    final String diagnostic = stderr.strip();
    if (exit != 0) {
      final String suffix = diagnostic.isEmpty() ? "" : ": " + diagnostic;
      throw new IllegalStateException(
          "kotlin-fixture-gen " + subcommand + " failed (" + exit + ")" + suffix);
    }
    if (output.isBlank()) {
      throw new IllegalStateException(
          "kotlin-fixture-gen " + subcommand + " produced no output");
    }
    return Arrays.asList(output.split("\\R"));
  }

  private static String awaitStream(final Future<String> stream)
      throws IOException, InterruptedException {
    try {
      return stream.get();
    } catch (ExecutionException error) {
      final Throwable cause = error.getCause();
      if (cause instanceof IOException) {
        throw (IOException) cause;
      }
      throw new IOException("failed to read fixture-generator output", cause);
    }
  }

  private static String readStream(final InputStream stream) throws IOException {
    final byte[] buffer = new byte[8192];
    final ByteArrayOutputStream output = new ByteArrayOutputStream();
    int read;
    while ((read = stream.read(buffer)) != -1) {
      output.write(buffer, 0, read);
    }
    return new String(output.toByteArray(), StandardCharsets.UTF_8);
  }

  private static File locateRepoRoot() {
    File directory = new File("").getAbsoluteFile();
    while (directory != null && !new File(directory, "Cargo.toml").isFile()) {
      directory = directory.getParentFile();
    }
    if (directory == null) {
      throw new IllegalStateException("could not locate Iroha repository root");
    }
    return directory;
  }
}
