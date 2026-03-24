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
    fun `parse resolves with leading and trailing whitespace`() {
        assertEquals(VerifyingKeyStatus.PROPOSED, VerifyingKeyStatus.parse("  Proposed  "))
    }

    @Test
    fun `parse is case insensitive`() {
        assertEquals(VerifyingKeyStatus.WITHDRAWN, VerifyingKeyStatus.parse("withdrawn"))
        assertEquals(VerifyingKeyStatus.ACTIVE, VerifyingKeyStatus.parse("ACTIVE"))
    }

    @Test
    fun `parse throws on unknown value`() {
        assertFailsWith<IllegalArgumentException> {
            VerifyingKeyStatus.parse("nonexistent")
        }
    }
}
