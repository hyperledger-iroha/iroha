package org.hyperledger.iroha.sdk.core.model

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class ContractAddressValidatorTest {
    @Test
    fun `contract subject derivation matches the locked Rust validation fee vector`() {
        assertEquals(
            "sorauﾛ1PjﾏｶﾏrfDWヱKmDRgH8ﾗﾐsｼﾓｼqSヰcpAKjGﾊﾇD8ﾁpAGH6E4T",
            contractSubjectAccountIdV1(
                "sorac1qyqqqqqqqqqqqqz6putm9wv6wkf4r22v02ktg4af7n3n7egq20h5l",
            ),
        )
    }

    @Test
    fun `contract subject derivation rejects noncanonical addresses`() {
        for (literal in listOf("", "not-a-contract", " SORAC1INVALID ")) {
            assertFailsWith<IllegalArgumentException>(literal) {
                contractSubjectAccountIdV1(literal)
            }
        }
    }
}
