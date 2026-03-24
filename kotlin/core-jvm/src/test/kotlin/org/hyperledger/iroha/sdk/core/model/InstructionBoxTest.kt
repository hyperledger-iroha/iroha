@file:OptIn(ExperimentalEncodingApi::class)

package org.hyperledger.iroha.sdk.core.model

import org.hyperledger.iroha.sdk.core.model.instructions.InstructionKind
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi
import org.junit.jupiter.params.ParameterizedTest
import org.junit.jupiter.params.provider.Arguments
import org.junit.jupiter.params.provider.MethodSource
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNotEquals

class InstructionBoxTest {

    private val samplePayloadBytes = byteArrayOf(1, 2, 3)
    private val samplePayloadBase64 = Base64.encode(samplePayloadBytes)

    @Test
    fun `fromWirePayload creates box with correct kind`() {
        val box = InstructionBox.fromWirePayload("iroha.register.domain", samplePayloadBytes)
        assertEquals(InstructionKind.REGISTER, box.kind)
    }

    @Test
    fun `fromWirePayload sets name to wireName`() {
        val box = InstructionBox.fromWirePayload("iroha.mint.asset", samplePayloadBytes)
        assertEquals("iroha.mint.asset", box.name)
    }

    @Test
    fun `fromWirePayload throws on blank wireName`() {
        assertFailsWith<IllegalArgumentException> {
            InstructionBox.fromWirePayload("  ", samplePayloadBytes)
        }
    }

    @Test
    fun `fromWirePayload throws on empty payloadBytes`() {
        assertFailsWith<IllegalArgumentException> {
            InstructionBox.fromWirePayload("iroha.register", byteArrayOf())
        }
    }

    @Test
    fun `fromNorito creates box from valid arguments`() {
        val args = mapOf("wire_name" to "iroha.transfer.asset", "payload_base64" to samplePayloadBase64)
        val box = InstructionBox.fromNorito(args)
        assertEquals(InstructionKind.TRANSFER, box.kind)
    }

    @Test
    fun `fromNorito detects wire payload arguments`() {
        val wirePayload = NoritoCodec.encode(
            "wire-payload",
            "iroha.test.WirePayload",
            NoritoAdapters.stringAdapter(),
        )
        val args = mapOf(
            "wire_name" to "iroha.custom",
            "payload_base64" to Base64.encode(wirePayload),
        )

        val box = InstructionBox.fromNorito(args)
        val wirePayloadBox = assertIs<WirePayload>(box.payload)
        assertEquals("iroha.custom", wirePayloadBox.wireName)
        assertContentEquals(wirePayload, wirePayloadBox.payloadBytes)
    }

    @Test
    fun `fromNorito throws when both fields missing`() {
        assertFailsWith<IllegalArgumentException> {
            InstructionBox.fromNorito(emptyMap())
        }
    }

    @Test
    fun `fromNorito throws when one field missing`() {
        assertFailsWith<IllegalArgumentException> {
            InstructionBox.fromNorito(mapOf("wire_name" to "iroha.mint"))
        }
    }

    @Test
    fun `fromNorito throws on invalid base64`() {
        assertFailsWith<IllegalArgumentException> {
            InstructionBox.fromNorito(mapOf("wire_name" to "iroha.mint", "payload_base64" to "!!!invalid!!!"))
        }
    }

    @Test
    fun `defensive copy on input - mutating original does not affect instance`() {
        val original = byteArrayOf(1, 2, 3)
        val box = InstructionBox.fromWirePayload("iroha.mint", original)
        original[0] = 99
        assertEquals(1, (box.payload as WirePayload).payloadBytes[0])
    }

    @Test
    fun `defensive copy on output - mutating returned bytes does not affect instance`() {
        val box = InstructionBox.fromWirePayload("iroha.mint", byteArrayOf(1, 2, 3))
        val payload = box.payload as WirePayload
        payload.payloadBytes[0] = 99
        assertEquals(1, payload.payloadBytes[0])
    }

    @Test
    fun `equals returns true for identical boxes`() {
        val a = InstructionBox.fromWirePayload("iroha.mint", samplePayloadBytes)
        val b = InstructionBox.fromWirePayload("iroha.mint", samplePayloadBytes)
        assertEquals(a, b)
        assertEquals(a.hashCode(), b.hashCode())
    }

    @Test
    fun `equals returns false for different boxes`() {
        val a = InstructionBox.fromWirePayload("iroha.mint", samplePayloadBytes)
        val b = InstructionBox.fromWirePayload("iroha.burn", samplePayloadBytes)
        assertNotEquals(a, b)
    }

    @ParameterizedTest
    @MethodSource("wireKindCases")
    fun `wireKindForName maps known prefixes`(wireName: String, expectedKind: InstructionKind) {
        val box = InstructionBox.fromWirePayload(wireName, byteArrayOf(1, 2, 3))
        assertEquals(expectedKind, box.kind)
    }

    companion object {
        @JvmStatic
        fun wireKindCases() = listOf(
            Arguments.of("iroha.register.domain", InstructionKind.REGISTER),
            Arguments.of("iroha.unregister.account", InstructionKind.UNREGISTER),
            Arguments.of("iroha.transfer.asset", InstructionKind.TRANSFER),
            Arguments.of("iroha.mint.asset", InstructionKind.MINT),
            Arguments.of("iroha.burn.asset", InstructionKind.BURN),
            Arguments.of("iroha.grant.permission", InstructionKind.GRANT),
            Arguments.of("iroha.revoke.permission", InstructionKind.REVOKE),
            Arguments.of("iroha.set_key_value", InstructionKind.SET_KEY_VALUE),
            Arguments.of("iroha.remove_key_value", InstructionKind.REMOVE_KEY_VALUE),
            Arguments.of("iroha.set_parameter", InstructionKind.SET_PARAMETER),
            Arguments.of("iroha.execute_trigger", InstructionKind.EXECUTE_TRIGGER),
            Arguments.of("iroha.log", InstructionKind.LOG),
            Arguments.of("iroha.upgrade", InstructionKind.UPGRADE),
            Arguments.of("iroha.runtime_upgrade", InstructionKind.UPGRADE),
            Arguments.of("custom.something", InstructionKind.CUSTOM),
        )
    }
}
