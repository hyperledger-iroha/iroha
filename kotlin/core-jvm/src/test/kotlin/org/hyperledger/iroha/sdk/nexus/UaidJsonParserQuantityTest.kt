package org.hyperledger.iroha.sdk.nexus

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class UaidJsonParserQuantityTest {
    @Test
    fun `portfolio readback requires canonical quantity strings`() {
        assertEquals("1.25", UaidJsonParser.parsePortfolio(portfolio("\"1.25\"")).firstQuantity())
        listOf("1", "\"01\"", "\"1e0\"", "\"-1\"", "\"1.0\"", "\"1.2300\"").forEach { value ->
            assertFailsWith<IllegalArgumentException>("portfolio accepted quantity $value") {
                UaidJsonParser.parsePortfolio(portfolio(value))
            }
        }
        assertFailsWith<IllegalArgumentException> {
            UaidPortfolioResponse.UaidPortfolioAsset("asset", "definition", "1.0")
        }
    }

    private fun UaidPortfolioResponse.firstQuantity(): String =
        dataspaces.single().accounts.single().assets.single().quantity

    private fun portfolio(quantityJson: String): ByteArray =
        """
        {
          "uaid":"uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
          "dataspaces":[{
            "dataspace_id":7,
            "accounts":[{
              "account_id":"account",
              "assets":[{
                "asset_id":"asset",
                "asset_definition_id":"definition",
                "quantity":$quantityJson
              }]
            }]
          }]
        }
        """.trimIndent().encodeToByteArray()
}
