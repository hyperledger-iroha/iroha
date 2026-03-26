package org.hyperledger.iroha.sdk.address

import kotlin.test.Test
import kotlin.test.assertEquals

class AccountAddressTest {
    @Test
    fun ambiguousKatakanaI105LiteralRoundTripsToOriginalCanonicalPayload() {
        val literal =
            "soraゴヂアヌプユドニャニョャニョユブゥワレボウュヒャメヌサネスヒダテガニャガュギィペジハネアヶァネフカアミキ"
        val address = AccountAddress.fromI105(literal, AccountAddress.DEFAULT_I105_DISCRIMINANT)
        assertEquals(
            "0x02000120bc717326224e4b4119298e7b1db8133cb27d6cdf6b3e04d75a6d27b29a34c1cf",
            address.canonicalHex(),
        )
        assertEquals(literal, address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT))
    }
}
