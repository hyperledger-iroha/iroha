package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class RegisterPinManifestInstructionTest {

    @Test
    fun `builder requires content length`() {
        assertFailsWith<IllegalStateException> {
            baseBuilder().build()
        }
    }

    @Test
    fun `builder rejects negative content length`() {
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.builder().setContentLength(-1)
        }
    }

    @Test
    fun `pin policy rejects zero replicas`() {
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.PinPolicy.builder().setMinReplicas(0)
        }
    }

    @Test
    fun `pin policy rejects negative replicas`() {
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.PinPolicy.builder().setMinReplicas(-1)
        }
    }

    @Test
    fun `arguments include content length and roundtrip`() {
        val instruction = baseBuilder()
            .setContentLength(4096)
            .build()

        assertEquals("4096", instruction.arguments["content_length"])
        assertEquals(
            instruction,
            RegisterPinManifestInstruction.fromArguments(instruction.arguments),
        )
    }

    @Test
    fun `from arguments rejects negative content length`() {
        val arguments = baseBuilder()
            .setContentLength(4096)
            .build()
            .arguments
            .toMutableMap()
        arguments["content_length"] = "-1"

        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(arguments)
        }
    }

    @Test
    fun `from arguments rejects nonnumeric content length`() {
        val arguments = baseBuilder()
            .setContentLength(4096)
            .build()
            .arguments
            .toMutableMap()
        arguments["content_length"] = "NaN"

        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(arguments)
        }
    }

    @Test
    fun `from arguments rejects negative submitted epoch`() {
        val arguments = baseBuilder()
            .setContentLength(4096)
            .build()
            .arguments
            .toMutableMap()
        arguments["submitted_epoch"] = "-1"

        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(arguments)
        }
    }

    @Test
    fun `from arguments rejects zero replicas`() {
        val arguments = baseBuilder()
            .setContentLength(4096)
            .build()
            .arguments
            .toMutableMap()
        arguments["policy.min_replicas"] = "0"

        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(arguments)
        }
    }

    @Test
    fun `from arguments rejects negative replicas`() {
        val arguments = baseBuilder()
            .setContentLength(4096)
            .build()
            .arguments
            .toMutableMap()
        arguments["policy.min_replicas"] = "-1"

        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(arguments)
        }
    }

    @Test
    fun `from arguments rejects nonnumeric replicas`() {
        val arguments = baseBuilder()
            .setContentLength(4096)
            .build()
            .arguments
            .toMutableMap()
        arguments["policy.min_replicas"] = "many"

        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(arguments)
        }
    }

    @Test
    fun `from arguments rejects partial alias binding`() {
        val arguments = baseBuilder()
            .setContentLength(4096)
            .build()
            .arguments
            .toMutableMap()
        arguments["alias.name"] = "docs"

        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(arguments)
        }
    }

    @Test
    fun `from arguments requires content length`() {
        val arguments = baseBuilder()
            .setContentLength(4096)
            .build()
            .arguments
            .toMutableMap()
        arguments.remove("content_length")

        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(arguments)
        }
    }

    private fun baseBuilder(): RegisterPinManifestInstruction.Builder =
        RegisterPinManifestInstruction.builder()
            .setDigestHex("a0".repeat(32))
            .setChunkDigestSha3Hex("b0".repeat(32))
            .setSubmittedEpoch(1)
            .setPinPolicy(
                RegisterPinManifestInstruction.PinPolicy.builder()
                    .setMinReplicas(1)
                    .setStorageClass("hot")
                    .setRetentionEpoch(10)
                    .build(),
            )
            .setChunkerProfile(
                RegisterPinManifestInstruction.ChunkerProfile.builder()
                    .setProfileId(1)
                    .setNamespace("sorafs")
                    .setName("sf1")
                    .setSemver("1.0.0")
                    .setMultihashCode(0)
                    .build(),
            )
}
