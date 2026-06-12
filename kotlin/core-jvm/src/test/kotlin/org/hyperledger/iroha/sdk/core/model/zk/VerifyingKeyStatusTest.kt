package org.hyperledger.iroha.sdk.core.model.zk

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class VerifyingKeyStatusTest {

    @Test
    fun `parse resolves exact wire name`() {
        assertEquals(VerifyingKeyStatus.ACTIVE, VerifyingKeyStatus.parse("Active"))
    }

    @Test
    fun `parse rejects leading and trailing whitespace`() {
        assertFailsWith<IllegalArgumentException> {
            VerifyingKeyStatus.parse("  Proposed  ")
        }
    }

    @Test
    fun `parse rejects case mutated wire names`() {
        for (value in listOf("withdrawn", "ACTIVE")) {
            assertFailsWith<IllegalArgumentException>(value) {
                VerifyingKeyStatus.parse(value)
            }
        }
    }

    @Test
    fun `parse throws on unknown value`() {
        assertFailsWith<IllegalArgumentException> {
            VerifyingKeyStatus.parse("nonexistent")
        }
    }
}
