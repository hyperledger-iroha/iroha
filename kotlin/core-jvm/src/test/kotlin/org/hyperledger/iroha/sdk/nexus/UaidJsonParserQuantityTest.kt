package org.hyperledger.iroha.sdk.nexus

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull

class UaidJsonParserQuantityTest {
    private val uaid = "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11"

    @Test
    fun `portfolio readback requires the exact current shape and canonical quantity strings`() {
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

    @Test
    fun `uaid response parser rejects missing unknown and padded fields`() {
        listOf(
            """{"uaid":"$uaid","dataspaces":[]}""",
            """{"uaid":"$uaid","totals":{"accounts":0,"positions":0},"dataspaces":[],"legacy":true}""",
        ).forEach { payload ->
            assertFailsWith<IllegalStateException> {
                UaidJsonParser.parsePortfolio(payload.toByteArray())
            }
        }
        assertFailsWith<IllegalStateException> {
            UaidJsonParser.parseBindings(
                """{"uaid":"$uaid","dataspaces":[{"dataspace_id":7,"accounts":[]}]}"""
                    .toByteArray(),
            )
        }
        assertFailsWith<IllegalStateException> {
            UaidJsonParser.parseBindings(
                """{"uaid":"$uaid","dataspaces":[{"dataspace_id":7,"dataspace_alias":null,"accounts":[" account"]}]}"""
                    .toByteArray(),
            )
        }
        assertFailsWith<IllegalStateException> {
            UaidJsonParser.parseManifests(
                """{"uaid":"$uaid","total":0,"manifests":[]}""".toByteArray(),
            )
        }
        assertFailsWith<IllegalStateException> {
            UaidJsonParser.parseManifests(
                """{"uaid":"$uaid","total":0,"has_more":false,"count_mode":"EXACT","manifests":[]}"""
                    .toByteArray(),
            )
        }
    }

    @Test
    fun `uaid response parser rejects negative rust unsigned fields`() {
        assertFailsWith<IllegalArgumentException> {
            UaidPortfolioResponse.UaidPortfolioTotals(-1, 0)
        }
        assertFailsWith<IllegalArgumentException> {
            UaidPortfolioResponse.UaidPortfolioTotals(0, -1)
        }
        assertFailsWith<IllegalStateException> {
            UaidJsonParser.parsePortfolio(
                """{"uaid":"$uaid","totals":{"accounts":-1,"positions":0},"dataspaces":[]}"""
                    .toByteArray(),
            )
        }
        assertFailsWith<IllegalStateException> {
            UaidJsonParser.parseBindings(
                """{"uaid":"$uaid","dataspaces":[{"dataspace_id":-1,"dataspace_alias":null,"accounts":[]}]}"""
                    .toByteArray(),
            )
        }
        assertFailsWith<IllegalStateException> {
            UaidJsonParser.parseManifests(
                """{"uaid":"$uaid","total":-1,"has_more":false,"count_mode":"exact","manifests":[]}"""
                    .toByteArray(),
            )
        }
    }

    @Test
    fun `uaid response parser rejects malformed present string fields`() {
        assertFailsWith<IllegalStateException> {
            UaidJsonParser.parsePortfolio(
                """
                    {
                      "uaid":"$uaid",
                      "totals":{"accounts":0,"positions":0},
                      "dataspaces":[{"dataspace_id":7,"dataspace_alias":9,"accounts":[]}]
                    }
                """.trimIndent().toByteArray(),
            )
        }
        assertFailsWith<IllegalStateException> {
            UaidJsonParser.parseBindings(
                """
                    {"uaid":"$uaid","dataspaces":[{"dataspace_id":7,"dataspace_alias":null,"accounts":[null]}]}
                """.trimIndent().toByteArray(),
            )
        }
    }

    @Test
    fun `manifest response enforces current pagination and manifest json contract`() {
        val response = UaidJsonParser.parseManifests(manifestResponse())
        assertEquals(1, response.total)
        assertFalse(response.hasMore)
        assertEquals(UaidManifestCountMode.EXACT, response.countMode)
        val record = response.manifests.single()
        assertEquals("ab".repeat(32), record.manifestHash)
        assertEquals(UaidManifestsResponse.UaidManifestStatus.REVOKED, record.status)
        assertEquals(15, record.lifecycle.revocation?.epoch)
        assertNull(record.lifecycle.revocation?.reason)
        assertEquals(1L, record.manifestAsMap()["version"])
    }

    @Test
    fun `manifest response rejects retired or noncanonical manifest encodings`() {
        val current = manifestResponse().decodeToString()
        listOf(
            current.replace("\"version\":1", "\"version\":\"1\""),
            current.replace("\"issued_ms\":100,", ""),
            current.replace("\"issued_ms\":100,", "\"issued_ms\":100,\"expiry_epoch\":null,"),
            current.replace("\"notes\":\"test\"", "\"notes\":null"),
            current.replace("\"asset\":\"asset\"", "\"asset\":null"),
            current.replace("\"max_amount\":\"10\"", "\"max_amount\":null"),
            current.replace("\"manifest_hash\":\"${"ab".repeat(32)}\"", "\"manifest_hash\":\"deadbeef\""),
            current.replace("\"status\":\"Revoked\"", "\"status\":\"revoked\""),
        ).forEach { payload ->
            assertFailsWith<IllegalStateException> {
                UaidJsonParser.parseManifests(payload.toByteArray())
            }
        }
    }

    @Test
    fun `manifest query uses only exact first release parameters`() {
        assertEquals(
            mapOf(
                "dataspace" to "7",
                "status" to "inactive",
                "limit" to "25",
                "offset" to "5",
                "count_mode" to "exact",
            ),
            UaidManifestQuery(
                dataspaceId = 7,
                status = UaidManifestQuery.UaidManifestStatusFilter.INACTIVE,
                limit = 25,
                offset = 5,
                countMode = UaidManifestCountMode.EXACT,
            ).toQueryParameters(),
        )
        assertFailsWith<IllegalArgumentException> { UaidManifestQuery(limit = 0) }
    }

    private fun UaidPortfolioResponse.firstQuantity(): String =
        dataspaces.single().accounts.single().assets.single().quantity

    private fun portfolio(quantityJson: String): ByteArray =
        """
        {
          "uaid":"$uaid",
          "totals":{"accounts":1,"positions":1},
          "dataspaces":[{
            "dataspace_id":7,
            "dataspace_alias":null,
            "accounts":[{
              "account_id":"account",
              "label":null,
              "assets":[{
                "asset_id":"asset",
                "asset_definition_id":"definition",
                "quantity":$quantityJson
              }]
            }]
          }]
        }
        """.trimIndent().encodeToByteArray()

    private fun manifestResponse(): ByteArray =
        """
        {
          "uaid":"$uaid",
          "total":1,
          "has_more":false,
          "count_mode":"exact",
          "manifests":[{
            "dataspace_id":7,
            "dataspace_alias":"primary",
            "manifest_hash":"${"ab".repeat(32)}",
            "status":"Revoked",
            "lifecycle":{
              "activated_epoch":10,
              "expired_epoch":null,
              "revocation":{"epoch":15,"reason":null}
            },
            "accounts":["account"],
            "manifest":{
              "version":1,
              "uaid":"$uaid",
              "dataspace":7,
              "issued_ms":100,
              "activation_epoch":10,
              "entries":[{
                "scope":{
                  "dataspace":7,
                  "program":"cash",
                  "method":"transfer",
                  "asset":"asset",
                  "role":"Initiator"
                },
                "effect":{"Allow":{"max_amount":"10","window":"PerDay"}},
                "notes":"test"
              }]
            }
          }]
        }
        """.trimIndent().encodeToByteArray()
}
