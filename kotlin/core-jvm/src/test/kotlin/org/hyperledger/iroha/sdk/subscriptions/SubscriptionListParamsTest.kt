package org.hyperledger.iroha.sdk.subscriptions

import org.junit.jupiter.api.Test
import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.query.field
import org.hyperledger.iroha.sdk.query.listQuery
import kotlin.test.assertEquals

class SubscriptionListParamsTest {
    @Test
    fun `status is a shared collection field`() {
        val query = listQuery { filter(field("status") eq "paused") }
        assertEquals(Json.parse("""{"filter":{"op":"eq","args":["status","paused"]}}"""), query.toJson())
    }
}
