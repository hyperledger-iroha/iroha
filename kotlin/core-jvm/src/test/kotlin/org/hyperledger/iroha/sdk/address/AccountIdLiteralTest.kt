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
            requireCanonicalI105Address("$address@banka.dataspace", "accountId")
        }
        assertEquals(
            "accountId must use canonical I105 encoded account without @domain",
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
            "accountId must use a canonical I105 encoded account literal",
            malformedError.message,
        )

        val hexError = assertFailsWith<IllegalArgumentException> {
            requireCanonicalI105Address(address.canonicalHex(), "accountId")
        }
        assertEquals(
            "accountId must use a canonical I105 encoded account literal",
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

    @Test
    fun parseEncodedRejectsFullwidthSentinelLiteral() {
        val canonical = sampleI105(0x55)
        val noncanonical = canonical.replaceFirst("sora", "ｓｏｒａ")

        val parseError = assertFailsWith<AccountAddressException> {
            AccountAddress.parseEncoded(noncanonical, AccountAddress.DEFAULT_I105_DISCRIMINANT)
        }
        assertEquals(AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT, parseError.code)

        val fromI105Error = assertFailsWith<AccountAddressException> {
            AccountAddress.fromI105(noncanonical, AccountAddress.DEFAULT_I105_DISCRIMINANT)
        }
        assertEquals(AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT, fromI105Error.code)
    }

    private fun sampleI105(fill: Int): String = AccountAddress
        .fromAccount(ByteArray(32) { fill.toByte() }, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
}
