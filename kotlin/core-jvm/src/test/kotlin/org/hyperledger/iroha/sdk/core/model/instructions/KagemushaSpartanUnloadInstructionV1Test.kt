// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.core.model.WirePayload

class KagemushaSpartanUnloadInstructionV1Test {
    @Test fun `activation instruction agrees with the Rust typed registration frame`() {
        fun pin(n: Int) = ByteArray(32) { n.toByte() }
        val claim = byteArrayOf(0x4b, 0x47, 4)
        val value = KagemushaSpartanActivateInstructionV1(pin(1), pin(2), pin(3), pin(4), claim)
        val original = "4e52543000009a1a14c7f2632fd04b30d02624e1f63a0090000000000000004326b61176e2052b022001010101010101010101010101010101010101010101010101010101010101012002020202020202020202020202020202020202020202020202020202020202022003030303030303030303030303030303030303030303030303030303030303032004040404040404040404040404040404040404040404040404040404040404040b03000000000000004b4704".chunked(2).map { it.toInt(16).toByte() }.toByteArray()
        claim.fill(0)
        assertContentEquals(original, value.concreteFrame())
        assertContentEquals(original, (value.toInstructionBox().payload as WirePayload).payloadBytes)
        assertFailsWith<IllegalArgumentException> { KagemushaSpartanActivateInstructionV1(pin(1), pin(2), pin(3), pin(4), ByteArray(32_769)) }
    }

    @Test fun `signed transaction builder retains the Rust schema and all originals`() {
        val scope = ByteArray(32) { 1 }
        val claim = byteArrayOf(0x4b, 0x47, 4)
        val instruction = KagemushaSpartanUnloadInstructionV1(scope, ByteArray(32) { 2 }, ByteArray(32) { 3 }, claim)
        claim.fill(0)
        scope.fill(0)
        val expected = "4e5254300000634e443f1c2be14836147b0888245923006f0000000000000065c2f63fa1effb5b022001010101010101010101010101010101010101010101010101010101010101012002020202020202020202020202020202020202020202020202020202020202022003030303030303030303030303030303030303030303030303030303030303030b03000000000000004b4704".chunked(2).map { it.toInt(16).toByte() }.toByteArray()
        assertContentEquals(expected, instruction.concreteFrame())
        assertContentEquals(expected, (instruction.toInstructionBox().payload as WirePayload).payloadBytes)
        assertFailsWith<IllegalArgumentException> { KagemushaSpartanUnloadInstructionV1(scope, scope, scope, claim) }
        val pin = ByteArray(32){ 1 }
        assertFailsWith<IllegalArgumentException> { KagemushaSpartanUnloadInstructionV1(pin, pin, pin, ByteArray(16_385)) }
    }
}
