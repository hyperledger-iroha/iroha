package org.hyperledger.iroha.sdk.core.model.instructions

import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Tests the Rust fixture-generator process contract used by Kotlin parity tests. */
class FixtureGeneratorRunnerTest {
    @Test
    fun `command invokes only the explicit fixture generator`() {
        val binary = File("/iroha-repository/tools/bin/kotlin-fixture-gen")
        val command = FixtureGeneratorRunner.commandFor(binary, "transfer-asset")

        assertEquals(listOf(binary.absolutePath, "transfer-asset"), command)
        assertFalse(command.any { it == "cargo" })
    }

    @Test
    fun `successful stderr diagnostics never become fixture rows`() {
        val rows = FixtureGeneratorRunner.outputLines(
            subcommand = "transfer-asset",
            exitCode = 0,
            stdout = "first-row\nsecond-row\n",
            stderr = "diagnostic only\n",
        )

        assertEquals(listOf("first-row", "second-row"), rows)
    }

    @Test
    fun `failed command reports stderr without accepting stdout`() {
        val error = assertFailsWith<IllegalArgumentException> {
            FixtureGeneratorRunner.outputLines(
                subcommand = "transfer-asset",
                exitCode = 9,
                stdout = "not-a-fixture\n",
                stderr = "generator failed\n",
            )
        }

        assertTrue(error.message.orEmpty().contains("generator failed"))
        assertFalse(error.message.orEmpty().contains("not-a-fixture"))
    }
}
