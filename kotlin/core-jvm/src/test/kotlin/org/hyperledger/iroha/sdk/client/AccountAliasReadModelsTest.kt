package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.address.AccountAddress
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class AccountAliasReadModelsTest {
    @Test
    fun parsesIndexAndVisibilityFilteredAccountResults() {
        val account = account()
        val index = AccountAliasReadJsonParser.parseIndexResolution(
            """{"index":7,"alias":"merchant@banka.paynet","account_id":"$account","source":"on_chain"}"""
                .toByteArray(StandardCharsets.UTF_8),
        )
        assertEquals(BigInteger.valueOf(7), index.index)
        assertEquals("merchant@banka.paynet", index.alias)

        val byAccount = AccountAliasReadJsonParser.parseByAccount(
            """{"account_id":"$account","total":2,"items":[{"alias":"alpha@paynet","dataspace":"paynet","is_primary":false},{"alias":"merchant@banka.paynet","dataspace":"paynet","domain":"banka","is_primary":true}],"source":"on_chain"}"""
                .toByteArray(StandardCharsets.UTF_8),
        )
        assertEquals(BigInteger.valueOf(2), byAccount.total)
        assertEquals(listOf("alpha@paynet", "merchant@banka.paynet"), byAccount.items.map { it.alias })
        assertEquals(true, byAccount.items.last().isPrimary)
    }

    @Test
    fun accountLookupRequestIsTypedAndRejectsAmbiguousScope() {
        val request = AccountAliasesByAccountRequest(account(), "Paynet", "Banka")
        assertEquals("paynet", request.dataspace)
        assertEquals("banka", request.domain)
        assertFailsWith<IllegalArgumentException> {
            AccountAliasesByAccountRequest(account(), null, "banka")
        }
    }

    @Test
    fun aliasIndexesPreserveTheFullRustU64Domain() {
        val account = account()
        val maximum = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
        val parsed = AccountAliasReadJsonParser.parseIndexResolution(
            """{"index":$maximum,"alias":"merchant@paynet","account_id":"$account"}"""
                .toByteArray(StandardCharsets.UTF_8),
        )
        assertEquals(maximum, parsed.index)

        val resolution = AccountAliasJsonParser.parseResolution(
            """{"alias":"merchant@paynet","account_id":"$account","index":$maximum}"""
                .toByteArray(StandardCharsets.UTF_8),
        )
        assertEquals(maximum, resolution.index)

        for (invalid in listOf("-1", BigInteger.ONE.shiftLeft(64).toString(), "1.0")) {
            assertFailsWith<IllegalStateException> {
                AccountAliasReadJsonParser.parseIndexResolution(
                    """{"index":$invalid,"alias":"merchant@paynet","account_id":"$account"}"""
                        .toByteArray(StandardCharsets.UTF_8),
                )
            }
        }
    }

    @Test
    fun responseParserRejectsInconsistentScopeAndPreFilterTotals() {
        val account = account()
        assertFailsWith<IllegalArgumentException> {
            AccountAliasReadJsonParser.parseByAccount(
                """{"account_id":"$account","total":1,"items":[{"alias":"merchant@banka.paynet","dataspace":"other","domain":"banka","is_primary":true}]}"""
                    .toByteArray(StandardCharsets.UTF_8),
            )
        }
        for (retiredField in listOf(
            "\"accountId\":\"$account\"",
            "\"account_ids\":[\"$account\"]",
            "\"accountIds\":[\"$account\"]",
            "\"unexpected\":true",
        )) {
            assertFailsWith<IllegalStateException> {
                AccountAliasJsonParser.parseResolution(
                    """{"alias":"merchant@paynet","account_id":"$account",$retiredField}"""
                        .toByteArray(StandardCharsets.UTF_8),
                )
            }
        }
        assertFailsWith<IllegalArgumentException> {
            AccountAliasJsonParser.parseResolution(
                """{"alias":"Merchant@paynet","account_id":"$account"}"""
                    .toByteArray(StandardCharsets.UTF_8),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            AccountAliasReadJsonParser.parseByAccount(
                """{"account_id":"$account","total":2,"items":[{"alias":"merchant@paynet","dataspace":"paynet","is_primary":true}]}"""
                    .toByteArray(StandardCharsets.UTF_8),
            )
        }
    }

    private fun account(): String = AccountAddress
        .fromAccount(ByteArray(32) { 0x22 }, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
}
