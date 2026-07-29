package org.hyperledger.iroha.sdk.core.model.instructions

import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class FixtureGeneratorRunnerConfigurationTest {
    private val repoRoot = File("/iroha-repository")

    @Test
    fun `uses deterministic Cargo defaults`() {
        val configuration = FixtureGeneratorRunner.configurationFor(repoRoot, emptyMap())
            as FixtureGeneratorConfiguration.CargoBuild

        assertEquals(
            listOf("cargo", "build", "-p", "kotlin-fixture-gen", "--locked", "--jobs", "1"),
            configuration.command(),
        )
        assertEquals(
            File(repoRoot, "target/kotlin-fixture-gen-test").absolutePath,
            configuration.targetDirectory.absolutePath,
        )
    }

    @Test
    fun `honors Cargo target offline and absolute lockfile configuration`() {
        val lockfile = File("/tmp/kotlin-fixture-gen-Cargo.lock")
        val configuration = FixtureGeneratorRunner.configurationFor(
            repoRoot,
            mapOf(
                "CARGO" to "/opt/rust/bin/cargo",
                "CARGO_TARGET_DIR" to "build/fixture-target",
                "CARGO_NET_OFFLINE" to "1",
                "IROHA_KOTLIN_FIXTURE_GEN_LOCKFILE_PATH" to lockfile.absolutePath,
            ),
        ) as FixtureGeneratorConfiguration.CargoBuild

        assertEquals(File(repoRoot, "build/fixture-target").absolutePath, configuration.targetDirectory.absolutePath)
        assertEquals(
            listOf(
                "/opt/rust/bin/cargo",
                "build",
                "-p",
                "kotlin-fixture-gen",
                "--locked",
                "--jobs",
                "1",
                "--offline",
                "-Z",
                "unstable-options",
                "--lockfile-path",
                lockfile.absolutePath,
            ),
            configuration.command(),
        )
    }

    @Test
    fun `uses prebuilt binary resolved against repository root`() {
        val configuration = FixtureGeneratorRunner.configurationFor(
            repoRoot,
            mapOf("IROHA_KOTLIN_FIXTURE_GEN_BIN" to "tools/bin/kotlin-fixture-gen"),
        ) as FixtureGeneratorConfiguration.Prebuilt

        assertEquals(
            File(repoRoot, "tools/bin/kotlin-fixture-gen").absolutePath,
            configuration.binary.absolutePath,
        )
    }

    @Test
    fun `prebuilt configuration bypasses invalid Cargo settings`() {
        val configuration = FixtureGeneratorRunner.configurationFor(
            repoRoot,
            mapOf(
                "IROHA_KOTLIN_FIXTURE_GEN_BIN" to "/opt/fixture-gen",
                "CARGO_NET_OFFLINE" to "unexpected",
                "IROHA_KOTLIN_FIXTURE_GEN_LOCKFILE_PATH" to "relative.lock",
            ),
        )

        assertTrue(configuration is FixtureGeneratorConfiguration.Prebuilt)
    }

    @Test
    fun `rejects unsupported offline values and relative lockfiles`() {
        assertFailsWith<IllegalStateException> {
            FixtureGeneratorRunner.configurationFor(
                repoRoot,
                mapOf("CARGO_NET_OFFLINE" to "yes"),
            )
        }
        assertFailsWith<IllegalStateException> {
            FixtureGeneratorRunner.configurationFor(
                repoRoot,
                mapOf("CARGO_NET_OFFLINE" to " true "),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            FixtureGeneratorRunner.configurationFor(
                repoRoot,
                mapOf("IROHA_KOTLIN_FIXTURE_GEN_LOCKFILE_PATH" to "Cargo.lock"),
            )
        }
    }
}
