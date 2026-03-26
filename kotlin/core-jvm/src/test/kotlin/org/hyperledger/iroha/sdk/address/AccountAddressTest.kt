package org.hyperledger.iroha.sdk.address

import kotlin.test.Test
import kotlin.test.assertEquals

class AccountAddressTest {
    @Test
    fun mixedI105LiteralRoundTripsToOriginalCanonicalPayload() {
        val literal =
            "sorauロ1PワdホシヒノNクdチムkiヌ3オモaPBQDTイKqシqオrラカwSQ1フナQU61Y7"
        val address = AccountAddress.fromI105(literal, AccountAddress.DEFAULT_I105_DISCRIMINANT)
        assertEquals(
            "0x02000120bc717326224e4b4119298e7b1db8133cb27d6cdf6b3e04d75a6d27b29a34c1cf",
            address.canonicalHex(),
        )
        assertEquals(literal, address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT))
    }
}
