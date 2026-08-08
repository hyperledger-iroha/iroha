package org.hyperledger.iroha.sdk.core.model.instructions

import java.io.File
import java.nio.file.Files
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Tests fail-closed fixture-generator path resolution without starting a process. */
class FixtureGeneratorRunnerConfigurationTest {
    @Test
    fun `requires an explicit nonblank binary path`() {
        val repoRoot = File("/iroha-repository")
        val missing = assertFailsWith<IllegalArgumentException> {
            FixtureGeneratorRunner.executableFor(repoRoot, emptyMap())
        }
        assertContains(missing.message.orEmpty(), "must be set")

        val blank = assertFailsWith<IllegalArgumentException> {
            FixtureGeneratorRunner.executableFor(
                repoRoot,
                mapOf(FixtureGeneratorRunner.BINARY_ENVIRONMENT_VARIABLE to " \t"),
            )
        }
        assertContains(blank.message.orEmpty(), "must not be blank")
    }

    @Test
    fun `resolves relative paths from repository root`() = withTemporaryRepoRoot { repoRoot ->
        val binary = executable(repoRoot, "tools/bin/kotlin-fixture-gen")
        val resolved = FixtureGeneratorRunner.executableFor(
            repoRoot,
            mapOf(FixtureGeneratorRunner.BINARY_ENVIRONMENT_VARIABLE to "tools/bin/kotlin-fixture-gen"),
        )

        assertEquals(binary.absolutePath, resolved.absolutePath)
    }

    @Test
    fun `accepts an absolute executable and ignores Cargo settings`() =
        withTemporaryRepoRoot { repoRoot ->
            val binary = executable(repoRoot, "outside-target/kotlin-fixture-gen")
            val resolved = FixtureGeneratorRunner.executableFor(
                repoRoot,
                mapOf(
                    FixtureGeneratorRunner.BINARY_ENVIRONMENT_VARIABLE to binary.absolutePath,
                    "CARGO" to "/must/not/run/cargo",
                    "CARGO_TARGET_DIR" to "ignored",
                ),
            )

            assertEquals(binary.absolutePath, resolved.absolutePath)
        }

    @Test
    fun `rejects missing paths and directories`() = withTemporaryRepoRoot { repoRoot ->
        val missing = assertFailsWith<IllegalArgumentException> {
            FixtureGeneratorRunner.executableFor(
                repoRoot,
                mapOf(FixtureGeneratorRunner.BINARY_ENVIRONMENT_VARIABLE to "missing-generator"),
            )
        }
        assertContains(missing.message.orEmpty(), "existing regular file")

        val directory = File(repoRoot, "generator-directory")
        assertTrue(directory.mkdir())
        val notAFile = assertFailsWith<IllegalArgumentException> {
            FixtureGeneratorRunner.executableFor(
                repoRoot,
                mapOf(FixtureGeneratorRunner.BINARY_ENVIRONMENT_VARIABLE to directory.absolutePath),
            )
        }
        assertContains(notAFile.message.orEmpty(), "existing regular file")
    }

    @Test
    fun `rejects a nonexecutable regular file`() = withTemporaryRepoRoot { repoRoot ->
        val binary = File(repoRoot, "kotlin-fixture-gen")
        binary.writeText("fixture")
        binary.setExecutable(false, false)
        assertFalse(binary.canExecute())

        val error = assertFailsWith<IllegalArgumentException> {
            FixtureGeneratorRunner.executableFor(
                repoRoot,
                mapOf(FixtureGeneratorRunner.BINARY_ENVIRONMENT_VARIABLE to binary.absolutePath),
            )
        }
        assertContains(error.message.orEmpty(), "is not executable")
    }

    private fun executable(repoRoot: File, relativePath: String): File {
        val binary = File(repoRoot, relativePath)
        assertTrue(binary.parentFile.mkdirs() || binary.parentFile.isDirectory)
        binary.writeText("#!/bin/sh\n")
        assertTrue(binary.setExecutable(true, false) || binary.canExecute())
        return binary
    }

    private fun withTemporaryRepoRoot(test: (File) -> Unit) {
        val repoRoot = Files.createTempDirectory("iroha-fixture-runner-test").toFile()
        try {
            test(repoRoot)
        } finally {
            repoRoot.deleteRecursively()
        }
    }
}
