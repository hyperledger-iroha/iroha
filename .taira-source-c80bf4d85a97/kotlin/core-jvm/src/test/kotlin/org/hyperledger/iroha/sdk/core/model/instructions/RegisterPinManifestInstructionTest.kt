@file:OptIn(ExperimentalEncodingApi::class)

package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals

private const val CANONICAL_MANIFEST_BASE64 =
    "TlJUMAAAduKskROcpAXus8dyJtDtlwD5AAAAAAAAAP11VCbJ+r+OAgEBLCQAAAAAAAAAAXEfIGIKrspjqahUS44Ka/oZKH+vxtnoUoM+vR660uOpRn5yCQhxAAAAAAAAAF4FBAEAAAAHBnNvcmFmcwQDc2YxBgUxLjAuMAQAAAEABAAABAAEAAAIAAT//wAACB8AAAAAAAAAJgIAAAAAAAAAERBzb3JhZnMuc2YxQDEuMC4wCwpzb3JhZnMtc2YxCAAAEAAAAAAAIM5QqarfhOV1WSCNOSAWISYv0bGIeuSQylRHDioAFT8nCNwEEAAAAAAAEQIDAAQAAAAACIBRAQAAAAAACQgAAAAAAAAAAAgAAAAAAAAAAAgAAAAAAAAAAA=="

class RegisterPinManifestInstructionTest {

    @Test
    fun `builder emits only first release consensus fields and roundtrips`() {
        val alias = RegisterPinManifestInstruction.AliasBinding.builder()
            .setName("docs")
            .setNamespace("sora")
            .setProofHex("a1b2")
            .build()
        val instruction = baseBuilder()
            .setSuccessorOfHex("c1".repeat(32))
            .setAliasBinding(alias)
            .build()

        assertEquals(
            setOf(
                "action",
                "manifest_payload_base64",
                "submitted_epoch",
                "successor_of_hex",
                "alias.name",
                "alias.namespace",
                "alias.proof_hex",
            ),
            instruction.arguments.keys,
        )
        assertEquals(
            instruction,
            RegisterPinManifestInstruction.fromArguments(instruction.arguments),
        )
    }

    @Test
    fun `byte payload setter and accessor are defensively isolated`() {
        val source = Base64.decode(CANONICAL_MANIFEST_BASE64)
        val expected = source.copyOf()
        val instruction = RegisterPinManifestInstruction.builder()
            .setManifestPayload(source)
            .setSubmittedEpoch(1)
            .build()
        source.fill(0)

        val first = instruction.manifestPayloadBytes()
        assertContentEquals(expected, first)
        first.fill(0)
        assertContentEquals(expected, instruction.manifestPayloadBytes())
    }

    @Test
    fun `builder rejects empty invalid noncanonical and oversized manifest payloads`() {
        for (payload in listOf("", "%%%", "AQ")) {
            assertFailsWith<IllegalArgumentException> {
                RegisterPinManifestInstruction.builder().setManifestPayloadBase64(payload)
            }
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.builder().setManifestPayload(ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.builder()
                .setManifestPayload(ByteArray(512 * 1024 + 1) { 1 })
        }
        val oversized = Base64.encode(ByteArray(512 * 1024 + 1) { 1 })
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.builder().setManifestPayloadBase64(oversized)
        }
    }

    @Test
    fun `builder requires payload and epoch`() {
        assertFailsWith<IllegalStateException> {
            RegisterPinManifestInstruction.builder()
                .setSubmittedEpoch(1)
                .build()
        }
        assertFailsWith<IllegalStateException> {
            RegisterPinManifestInstruction.builder()
                .setManifestPayloadBase64(CANONICAL_MANIFEST_BASE64)
                .build()
        }
    }

    @Test
    fun `successor digest requires nonzero canonical lowercase 32 byte hex`() {
        for (digest in listOf(
            "00".repeat(32),
            "c1".repeat(31),
            "C1".repeat(32),
            "0x" + "c1".repeat(32),
        )) {
            assertFailsWith<IllegalArgumentException> {
                RegisterPinManifestInstruction.builder().setSuccessorOfHex(digest)
            }
        }
    }

    @Test
    fun `epoch must be nonnegative and numeric`() {
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.builder().setSubmittedEpoch(-1)
        }
        val arguments = baseBuilder().build().arguments.toMutableMap()
        arguments["submitted_epoch"] = "NaN"
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(arguments)
        }
    }

    @Test
    fun `alias fields are all or nothing and bounded canonical hex`() {
        val partial = baseBuilder().build().arguments.toMutableMap()
        partial["alias.name"] = "docs"
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(partial)
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.AliasBinding.builder()
                .setName(" docs")
        }
        for (name in listOf("Docs", "main site", "máin", "a".repeat(129))) {
            assertFailsWith<IllegalArgumentException> {
                RegisterPinManifestInstruction.AliasBinding.builder().setName(name)
            }
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.AliasBinding.builder()
                .setName("docs")
                .setNamespace("sora")
                .setProofHex("A1")
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.AliasBinding.builder()
                .setName("docs")
                .setNamespace("sora")
                .setProofHex("aa".repeat(1024 * 1024 + 1))
        }
    }

    @Test
    fun `from arguments rejects legacy unknown and missing fields`() {
        val legacy = baseBuilder().build().arguments.toMutableMap()
        legacy["digest_hex"] = "a0".repeat(32)
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(legacy)
        }
        val retiredChunkDigest = baseBuilder().build().arguments.toMutableMap()
        retiredChunkDigest["chunk_digest_sha3_256_hex"] = "b0".repeat(32)
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(retiredChunkDigest)
        }

        val wrongAction = baseBuilder().build().arguments.toMutableMap()
        wrongAction["action"] = "ApprovePinManifest"
        assertFailsWith<IllegalArgumentException> {
            RegisterPinManifestInstruction.fromArguments(wrongAction)
        }

        for (required in listOf(
            "action",
            "manifest_payload_base64",
            "submitted_epoch",
        )) {
            val missing = baseBuilder().build().arguments.toMutableMap()
            missing.remove(required)
            assertFailsWith<IllegalArgumentException> {
                RegisterPinManifestInstruction.fromArguments(missing)
            }
        }
    }

    @Test
    fun `equality includes payload and optional consensus fields`() {
        val base = baseBuilder().build()
        val successor = baseBuilder().setSuccessorOfHex("c1".repeat(32)).build()
        assertNotEquals(base, successor)
        assertEquals(base.hashCode(), RegisterPinManifestInstruction.fromArguments(base.arguments).hashCode())
    }

    private fun baseBuilder(): RegisterPinManifestInstruction.Builder =
        RegisterPinManifestInstruction.builder()
            .setManifestPayloadBase64(CANONICAL_MANIFEST_BASE64)
            .setSubmittedEpoch(1)
}
