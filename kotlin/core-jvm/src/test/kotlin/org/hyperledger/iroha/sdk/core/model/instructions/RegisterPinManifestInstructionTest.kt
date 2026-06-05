package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class RegisterPinManifestInstructionTest {

    @Test
    fun `builder rejects malformed digest hex`() {
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.builder().setDigestHex("zz")
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.builder().setDigestHex("a0".repeat(31))
        }
    }

    @Test
    fun `builder rejects malformed chunk digest hex`() {
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.builder().setChunkDigestSha3Hex("not-hex")
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.builder().setChunkDigestSha3Hex("b0".repeat(33))
        }
    }

    @Test
    fun `builder rejects malformed successor hex`() {
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.builder().setSuccessorOfHex("01")
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.builder().setSuccessorOfHex("gg".repeat(32))
        }
    }

    @Test
    fun `alias binding rejects malformed proof hex`() {
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.AliasBinding.builder()
                .setName("docs")
                .setNamespace("sora")
                .setProofHex("proof")
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.AliasBinding.builder()
                .setName("docs")
                .setNamespace("sora")
                .setProofHex("a")
        }
    }

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
    fun `pin policy canonicalizes storage class`() {
        val instruction = baseBuilder()
            .setContentLength(4096)
            .setPinPolicy(
                RegisterPinManifestInstruction.PinPolicy.builder()
                    .setMinReplicas(1)
                    .setStorageClass("hot")
                    .setRetentionEpoch(10)
                    .build(),
            )
            .build()

        assertEquals("Hot", instruction.pinPolicy.storageClass)
        assertEquals("Hot", instruction.arguments["policy.storage_class"])
    }

    @Test
    fun `pin policy rejects unsupported storage class`() {
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.PinPolicy.builder()
                .setMinReplicas(1)
                .setStorageClass("lava")
        }
    }

    @Test
    fun `chunker profile rejects nonpositive profile id`() {
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.ChunkerProfile.builder().setProfileId(0)
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.ChunkerProfile.builder().setProfileId(-1)
        }
    }

    @Test
    fun `chunker profile rejects negative multihash code`() {
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.ChunkerProfile.builder().setMultihashCode(-1)
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
    fun `from arguments rejects unsupported storage class`() {
        val arguments = baseBuilder()
            .setContentLength(4096)
            .build()
            .arguments
            .toMutableMap()
        arguments["policy.storage_class"] = "lava"

        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(arguments)
        }
    }

    @Test
    fun `from arguments rejects nonpositive chunker profile id`() {
        val arguments = baseArguments()
        arguments["chunker.profile_id"] = "0"

        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(arguments)
        }

        arguments["chunker.profile_id"] = "-1"

        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(arguments)
        }
    }

    @Test
    fun `from arguments rejects negative chunker multihash code`() {
        val arguments = baseArguments()
        arguments["chunker.multihash_code"] = "-1"

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
    fun `from arguments rejects malformed digest hex`() {
        val arguments = baseArguments()
        arguments["digest_hex"] = "zz"

        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(arguments)
        }
    }

    @Test
    fun `from arguments rejects malformed chunk digest hex`() {
        val arguments = baseArguments()
        arguments["chunk_digest_sha3_256_hex"] = "b0".repeat(31)

        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(arguments)
        }
    }

    @Test
    fun `from arguments rejects malformed successor hex`() {
        val arguments = baseArguments()
        arguments["successor_of_hex"] = "not-hex"

        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(arguments)
        }
    }

    @Test
    fun `from arguments rejects malformed alias proof hex`() {
        val arguments = baseArguments()
        arguments["alias.name"] = "docs"
        arguments["alias.namespace"] = "sora"
        arguments["alias.proof_hex"] = "abc"

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

    private fun baseArguments(): MutableMap<String, String> =
        baseBuilder()
            .setContentLength(4096)
            .build()
            .arguments
            .toMutableMap()
}
