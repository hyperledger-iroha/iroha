package org.hyperledger.iroha.sdk.address

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class AccountIdLiteralTest {
    @Test
    fun acceptsCanonicalI105Literal() {
        val address = sampleI105(0x11)
        val normalized = requireCanonicalI105Address(address, "accountId")
        assertEquals(address, normalized)
    }

    @Test
    fun trimsWhitespaceBeforeValidation() {
        val address = sampleI105(0x22)
        val normalized = requireCanonicalI105Address("  $address  ", "accountId")
        assertEquals(address, normalized)
    }

    @Test
    fun rejectsDomainSuffixedLiterals() {
        val address = sampleI105(0x33)
        val error = assertFailsWith<IllegalArgumentException> {
            requireCanonicalI105Address("$address@hbl.sbp", "accountId")
        }
        assertEquals(
            "accountId must use canonical i105 encoded account without @domain",
            error.message,
        )
    }

    @Test
    fun rejectsMalformedAndHexLiterals() {
        val address = AccountAddress.fromAccount(ByteArray(32) { 0x44.toByte() }, "ed25519")
        val malformed = "legacy-i105"
        val malformedError = assertFailsWith<IllegalArgumentException> {
            requireCanonicalI105Address(malformed, "accountId")
        }
        assertEquals(
            "accountId must use a canonical i105 encoded account literal",
            malformedError.message,
        )

        val hexError = assertFailsWith<IllegalArgumentException> {
            requireCanonicalI105Address(address.canonicalHex(), "accountId")
        }
        assertEquals(
            "accountId must use a canonical i105 encoded account literal",
            hexError.message,
        )
    }

    @Test
    fun rejectsBlankAccountId() {
        val error = assertFailsWith<IllegalArgumentException> {
            requireCanonicalI105Address("   ", "accountId")
        }
        assertEquals("accountId must not be blank", error.message)
    }

    private fun sampleI105(fill: Int): String = AccountAddress
        .fromAccount(ByteArray(32) { fill.toByte() }, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
}
