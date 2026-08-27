package org.hyperledger.iroha.sdk.subscriptions

import org.junit.jupiter.api.Test
import kotlin.test.assertEquals

class SubscriptionListParamsTest {
    @Test
    fun `status stays typed until query encoding`() {
        val params = SubscriptionListParams(
            status = SubscriptionStatus.PAUSED,
        )

        assertEquals(SubscriptionStatus.PAUSED, params.status)
        assertEquals(mapOf("status" to "paused"), params.toQueryParameters())
    }
}
