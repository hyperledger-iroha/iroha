package org.hyperledger.iroha.sdk.subscriptions

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/** Numeric-boundary coverage for subscription response JSON. */
class SubscriptionJsonParserBoundaryTest {

    @Test
    fun listTotalsPreserveNullFallbackButRejectInvalidUnsignedValues() {
        assertEquals(
            0L,
            SubscriptionJsonParser.parsePlanList(
                """{"total":null,"items":[]}""".toByteArray(),
            ).total,
        )
        for (total in listOf("-1", "9223372036854775808")) {
            assertFailsWith<IllegalStateException> {
                SubscriptionJsonParser.parsePlanList(
                    """{"total":$total,"items":[]}""".toByteArray(),
                )
            }
            assertFailsWith<IllegalStateException> {
                SubscriptionJsonParser.parseSubscriptionList(
                    """{"total":$total,"items":[]}""".toByteArray(),
                )
            }
        }
    }

    @Test
    fun createResponseRejectsInvalidUnsignedFirstCharge() {
        for (firstChargeMs in listOf("-1", "9223372036854775808")) {
            assertFailsWith<IllegalStateException> {
                SubscriptionJsonParser.parseSubscriptionCreateResponse(
                    """
                        {
                          "ok": true,
                          "subscription_id": "sub-1${'$'}subscriptions",
                          "billing_trigger_id": "sub-1${'$'}subscriptions#billing",
                          "usage_trigger_id": null,
                          "first_charge_ms": $firstChargeMs,
                          "tx_hash_hex": "00"
                        }
                    """.trimIndent().toByteArray(),
                )
            }
        }
    }

    @Test
    fun requiredIdentifiersRejectNonStringValues() {
        assertFailsWith<IllegalStateException> {
            SubscriptionJsonParser.parsePlanCreateResponse(
                """{"ok":true,"plan_id":7,"tx_hash_hex":"00"}""".toByteArray(),
            )
        }
        assertFailsWith<IllegalStateException> {
            SubscriptionJsonParser.parseActionResponse(
                """{"ok":true,"subscription_id":false,"tx_hash_hex":"00"}""".toByteArray(),
            )
        }
    }
}
