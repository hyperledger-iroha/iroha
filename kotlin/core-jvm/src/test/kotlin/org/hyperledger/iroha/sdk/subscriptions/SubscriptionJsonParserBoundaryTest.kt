package org.hyperledger.iroha.sdk.subscriptions

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.numeric.NumericV1Codec
import org.hyperledger.iroha.sdk.numeric.NumericV1Exception

/** Numeric-boundary coverage for subscription response JSON. */
class SubscriptionJsonParserBoundaryTest {

    @Test
    fun usageDeltaUsesCanonicalQuantityBoundary() {
        val canonicalValues = listOf(
            "0",
            "12.5",
            NumericV1Codec.intMax.toString(),
            "0.${"0".repeat(27)}1",
        )
        for (canonical in canonicalValues) {
            val quantity = NumericV1Codec.decodeQuantityJson(canonical)
            val request = SubscriptionUsageRequest(
                authority = "alice",
                unitKey = "compute_ms",
                delta = quantity,
            )
            assertEquals(quantity, request.delta)
            assertEquals(canonical, request.toJsonMap()["delta"])
        }

        val invalidValues = listOf(
            "+1",
            "01",
            "00",
            "00.1",
            "1.0",
            "1.20",
            "0.0",
            "-0",
            "-1",
            NumericV1Codec.intMax.add(BigInteger.ONE).toString(),
            "0.${"0".repeat(28)}1",
        )
        for (invalid in invalidValues) {
            assertFailsWith<NumericV1Exception>(invalid) {
                NumericV1Codec.decodeQuantityJson(invalid)
            }
        }
        assertFailsWith<NumericV1Exception> {
            NumericV1Codec.decodeQuantityJsonValue(1)
        }
    }

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

    @Test
    fun transactionHashResponsesRequireIrohaHashOfMarker() {
        val canonical = "ab".repeat(32)
        assertEquals(
            canonical,
            SubscriptionJsonParser.parsePlanCreateResponse(
                """{"ok":true,"plan_id":"plan-1","tx_hash_hex":"$canonical"}""".toByteArray(),
            ).txHashHex,
        )
        assertEquals(
            canonical,
            SubscriptionJsonParser.parseSubscriptionCreateResponse(
                """{"ok":true,"subscription_id":"sub-1","billing_trigger_id":"bill-1","usage_trigger_id":null,"first_charge_ms":1,"tx_hash_hex":"$canonical"}""".toByteArray(),
            ).txHashHex,
        )
        assertEquals(
            canonical,
            SubscriptionJsonParser.parseActionResponse(
                """{"ok":true,"subscription_id":"sub-1","tx_hash_hex":"$canonical"}""".toByteArray(),
            ).txHashHex,
        )
        assertFailsWith<IllegalStateException> {
            SubscriptionJsonParser.parsePlanCreateResponse(
                """{"ok":true,"plan_id":"plan-1","tx_hash_hex":"${"aa".repeat(32)}"}""".toByteArray(),
            )
        }
        assertFailsWith<IllegalStateException> {
            SubscriptionJsonParser.parseActionResponse(
                """{"ok":true,"subscription_id":"sub-1","tx_hash_hex":"${"aa".repeat(32)}"}""".toByteArray(),
            )
        }
    }
}
