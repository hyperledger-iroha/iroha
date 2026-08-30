// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.testing;

import static org.junit.Assert.assertArrayEquals;

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import org.junit.Test;

/** Prevents the Android-module copy of the shared network fixture from drifting. */
public final class TestNetworkIdsSourceParityTests {
  private static final Path CORE_FIXTURE =
      Paths.get(
          "src",
          "test",
          "java",
          "org",
          "hyperledger",
          "iroha",
          "android",
          "testing",
          "TestNetworkIds.java");
  private static final Path ANDROID_FIXTURE =
      Paths.get(
          "android",
          "src",
          "test",
          "java",
          "org",
          "hyperledger",
          "iroha",
          "android",
          "testing",
          "TestNetworkIds.java");

  @Test
  public void androidCopyIsByteIdenticalToCoreFixture() throws Exception {
    final Path root = projectRoot();
    assertArrayEquals(
        "Android TestNetworkIds must remain byte-identical to the core test fixture",
        Files.readAllBytes(root.resolve(CORE_FIXTURE)),
        Files.readAllBytes(root.resolve(ANDROID_FIXTURE)));
  }

  private static Path projectRoot() {
    Path candidate = Paths.get(System.getProperty("user.dir")).toAbsolutePath().normalize();
    while (candidate != null) {
      if (Files.isRegularFile(candidate.resolve("settings.gradle.kts"))
          && Files.isRegularFile(candidate.resolve(CORE_FIXTURE))
          && Files.isRegularFile(candidate.resolve(ANDROID_FIXTURE))) {
        return candidate;
      }
      candidate = candidate.getParent();
    }
    throw new IllegalStateException("Unable to locate the iroha-android project root");
  }
}
