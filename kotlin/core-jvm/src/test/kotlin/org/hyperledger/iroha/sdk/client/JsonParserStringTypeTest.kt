package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class JsonParserStringTypeTest {

    @Test
    fun identifierParserRejectsNonStringRequiredAndOptionalFields() {
        val canonical = identifierPolicyJson()
        assertEquals(
            "retail policy",
            IdentifierJsonParser.parsePolicyList(canonical.bytes()).items.single().note,
        )

        assertRejects("identifier policy list.items[0].policy_id") {
            IdentifierJsonParser.parsePolicyList(
                canonical.replace("\"policy_id\":\"phone#retail\"", "\"policy_id\":7").bytes()
            )
        }
        assertRejects("identifier policy list.items[0].note") {
            IdentifierJsonParser.parsePolicyList(
                canonical.replace("\"note\":\"retail policy\"", "\"note\":false").bytes()
            )
        }
    }

    @Test
    fun identifierParserRejectsMissingOrMalformedActive() {
        val canonical = identifierPolicyJson()
        assertEquals(
            false,
            IdentifierJsonParser.parsePolicyList(
                canonical.replace("\"active\":true", "\"active\":false").bytes()
            ).items.single().active,
        )

        assertRejects("identifier policy list.items[0].active") {
            IdentifierJsonParser.parsePolicyList(
                canonical.replace("\"active\":true", "\"active\":\"true\"").bytes()
            )
        }
        assertRejects("identifier policy list.items[0].active") {
            IdentifierJsonParser.parsePolicyList(
                canonical.replace("\"active\":true,", "").bytes()
            )
        }
    }

    @Test
    fun ramLfeParserRejectsNonStringRequiredAndOptionalFields() {
        val canonicalPolicy = ramLfePolicyJson()
        assertEquals(
            "retail policy",
            RamLfeJsonParser.parsePolicyList(canonicalPolicy.bytes()).items.single().note,
        )
        assertEquals(
            "verification failed",
            RamLfeJsonParser.parseReceiptVerifyResponse(
                ramLfeVerifyJson(error = "\"verification failed\"").bytes()
            ).error,
        )

        assertRejects("ram-lfe program policy list.items[0].program_id") {
            RamLfeJsonParser.parsePolicyList(
                canonicalPolicy.replace("\"program_id\":\"lookup\"", "\"program_id\":7").bytes()
            )
        }
        assertRejects("ram-lfe program policy list.items[0].note") {
            RamLfeJsonParser.parsePolicyList(
                canonicalPolicy.replace("\"note\":\"retail policy\"", "\"note\":false").bytes()
            )
        }
        assertRejects("ram-lfe receipt verify response.error") {
            RamLfeJsonParser.parseReceiptVerifyResponse(ramLfeVerifyJson(error = "7").bytes())
        }
    }

    @Test
    fun ramLfeParserRejectsMissingOrMalformedRequiredBooleans() {
        val canonicalPolicy = ramLfePolicyJson()
        assertEquals(
            false,
            RamLfeJsonParser.parsePolicyList(
                canonicalPolicy.replace("\"active\":true", "\"active\":false").bytes()
            ).items.single().active,
        )

        assertRejects("ram-lfe program policy list.items[0].active") {
            RamLfeJsonParser.parsePolicyList(
                canonicalPolicy.replace("\"active\":true", "\"active\":1").bytes()
            )
        }
        assertRejects("ram-lfe program policy list.items[0].active") {
            RamLfeJsonParser.parsePolicyList(
                canonicalPolicy.replace("\"active\":true,", "").bytes()
            )
        }

        val canonicalVerify = ramLfeVerifyJson(error = "null")
        assertEquals(false, RamLfeJsonParser.parseReceiptVerifyResponse(canonicalVerify.bytes()).valid)
        assertRejects("ram-lfe receipt verify response.valid") {
            RamLfeJsonParser.parseReceiptVerifyResponse(
                canonicalVerify.replace("\"valid\":false", "\"valid\":0").bytes()
            )
        }
        assertRejects("ram-lfe receipt verify response.valid") {
            RamLfeJsonParser.parseReceiptVerifyResponse(
                canonicalVerify.replace("\"valid\":false,", "").bytes()
            )
        }

        val optionalNull = canonicalVerify.replace(
            "\"error\":",
            "\"output_hash_matches\":null,\n              \"error\":",
        )
        assertEquals(
            null,
            RamLfeJsonParser.parseReceiptVerifyResponse(optionalNull.bytes()).outputHashMatches,
        )
        assertRejects("ram-lfe receipt verify response.output_hash_matches") {
            RamLfeJsonParser.parseReceiptVerifyResponse(
                optionalNull.replace("\"output_hash_matches\":null", "\"output_hash_matches\":0").bytes()
            )
        }
    }

    private fun identifierPolicyJson(): String =
        """
            {
              "total":1,
              "items":[{
                "policy_id":"phone#retail",
                "owner":"owner",
                "active":true,
                "normalization":"phone_e164",
                "resolver_public_key":"$VALID_PUBLIC_KEY",
                "backend":"bfv-affine-sha3-256-v1",
                "note":"retail policy"
              }]
            }
        """.trimIndent()

    private fun ramLfePolicyJson(): String =
        """
            {
              "total":1,
              "items":[{
                "program_id":"lookup",
                "owner":"owner",
                "active":true,
                "resolver_public_key":"$VALID_PUBLIC_KEY",
                "backend":"bfv-programmed-sha3-256-v1",
                "verification_mode":"signed",
                "note":"retail policy"
              }]
            }
        """.trimIndent()

    private fun ramLfeVerifyJson(error: String): String =
        """
            {
              "valid":false,
              "program_id":"lookup",
              "backend":"bfv-programmed-sha3-256-v1",
              "verification_mode":"signed",
              "output_hash":"${"11".repeat(32)}",
              "associated_data_hash":"${"22".repeat(32)}",
              "error":$error
            }
        """.trimIndent()

    private fun String.bytes(): ByteArray = toByteArray(StandardCharsets.UTF_8)

    private fun assertRejects(path: String, parse: () -> Unit) {
        val error = assertFailsWith<IllegalStateException>(block = parse)
        assertContains(error.message.orEmpty(), path)
    }

    private companion object {
        const val VALID_PUBLIC_KEY =
            "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29"
    }
}
