package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.test.Test
import kotlin.test.assertEquals

/** Tests the Rust fixture-generator process contract used by Kotlin parity tests. */
class FixtureGeneratorRunnerTest {
    /** Keeps test-time Rust builds pinned to the checked-in dependency graph. */
    @Test
    fun `fixture generator build is locked and offline`() {
        assertEquals(
            listOf("cargo", "build", "--locked", "--offline", "-p", "kotlin-fixture-gen"),
            FixtureGeneratorRunner.cargoBuildCommand(),
        )
    }
}
