package org.hyperledger.iroha.sdk.validationfee

import java.nio.charset.StandardCharsets
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys

class ValidationFeeHijiriQuoteTest {
    private val accountId =
        AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x51), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

    @Test
    fun `request rejects noncanonical account and unbounded count before native loading`() {
        val request = ValidationFeeHijiriQuoteRequestV1(accountId, 2)
        assertEquals(1, request.version)
        assertEquals(accountId, request.accountId)
        assertEquals(2, request.qualifyingTransferCount)

        assertFailsWith<IllegalArgumentException> {
            ValidationFeeHijiriQuoteRequestV1(" $accountId", 1)
        }
        assertFailsWith<IllegalArgumentException> {
            ValidationFeeHijiriQuoteRequestV1("alice@wonderland", 1)
        }
        assertFailsWith<IllegalArgumentException> {
            ValidationFeeHijiriQuoteRequestV1(accountId, 0)
        }
        assertFailsWith<IllegalArgumentException> {
            ValidationFeeHijiriQuoteRequestV1(
                accountId,
                VALIDATION_FEE_HIJIRI_QUOTE_MAX_QUALIFYING_TRANSFERS_V1 + 1,
            )
        }
    }

    @Test
    fun `native projection parser exposes every exact verified field`() {
        val quote = ValidationFeeHijiriQuoteProjectionParser.parse(projectionJson())

        assertEquals(VALIDATION_FEE_HIJIRI_QUOTE_SCHEMA_V1, quote.schema)
        assertEquals(VALIDATION_FEE_HIJIRI_QUOTE_ASSURANCE_V1, quote.assurance)
        assertEquals("42", quote.evaluatedStateHeight)
        assertEquals("43", quote.quotedExecutionHeight)
        assertEquals(accountId, quote.accountId)
        assertEquals(65_536L, quote.feeMultiplierQ16)
        assertEquals(2, quote.qualifyingTransferCount)
        assertEquals("20", quote.aggregateAdjustedFeeMinorUnits)
        assertNull(quote.accountRiskRevision)
        assertNull(quote.accountRiskDigest)
    }

    @Test
    fun `native projection parser rejects field drift and incomplete risk binding`() {
        val canonical = String(projectionJson(), StandardCharsets.UTF_8)
        val unknown = canonical.dropLast(1) + ",\"unexpected\":true}"
        assertFailsWith<IllegalArgumentException> {
            ValidationFeeHijiriQuoteProjectionParser.parse(
                unknown.toByteArray(StandardCharsets.UTF_8),
            )
        }

        val incomplete = canonical.replace(
            "\"accountRiskRevision\":null",
            "\"accountRiskRevision\":\"1\"",
        )
        assertFailsWith<IllegalArgumentException> {
            ValidationFeeHijiriQuoteProjectionParser.parse(
                incomplete.toByteArray(StandardCharsets.UTF_8),
            )
        }

        val excessiveCount = canonical.replace(
            "\"qualifyingTransferCount\":2",
            "\"qualifyingTransferCount\":100001",
        )
        assertFailsWith<IllegalArgumentException> {
            ValidationFeeHijiriQuoteProjectionParser.parse(
                excessiveCount.toByteArray(StandardCharsets.UTF_8),
            )
        }
    }

    @Test
    fun `bridge rejects invalid byte bounds before native loading`() {
        assertFailsWith<IllegalArgumentException> {
            ValidationFeeHijiriQuoteBridge.verifyResponseV1(
                ByteArray(0),
                byteArrayOf(1),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ValidationFeeHijiriQuoteBridge.verifyResponseV1(
                byteArrayOf(1),
                ByteArray(VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES_V1 + 1),
            )
        }
    }

    @Test
    fun `missing additive JNI methods fail with the stable capability error`() {
        for (method in listOf("nativeEncodeRequestV1", "nativeVerifyResponseV1")) {
            val failure = assertFailsWith<IllegalStateException> {
                ValidationFeeHijiriQuoteBridge.invokeRequiredQuoteNative<ByteArray>(method) {
                    throw UnsatisfiedLinkError("missing JNI sentinel")
                }
            }
            assertEquals(
                "native Hijiri validation-fee quote bridge is unavailable: " +
                    "required ABI-23 method $method is missing",
                failure.message,
            )
            assertTrue(failure.cause is UnsatisfiedLinkError)
        }
    }

    @Test
    fun `fresh native bridge encodes and verifies through the Kotlin JNI names`() {
        val request = ValidationFeeHijiriQuoteRequestV1(accountId, 2)
        val first = ValidationFeeHijiriQuoteBridge.encodeRequestV1(request)
        val second = ValidationFeeHijiriQuoteBridge.encodeRequestV1(request)

        assertTrue(first.isNotEmpty())
        assertTrue(first.size <= VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES_V1)
        assertContentEquals(first, second)
        assertFailsWith<IllegalArgumentException> {
            ValidationFeeHijiriQuoteBridge.verifyResponseV1(byteArrayOf(0), first)
        }
    }

    private fun projectionJson(): ByteArray =
        """
        {
          "schema":"$VALIDATION_FEE_HIJIRI_QUOTE_SCHEMA_V1",
          "version":1,
          "assurance":"$VALIDATION_FEE_HIJIRI_QUOTE_ASSURANCE_V1",
          "evaluatedStateHeight":"42",
          "quotedExecutionHeight":"43",
          "accountId":"$accountId",
          "activePolicyVersion":"1",
          "activePolicyHash":"${"03".repeat(32)}",
          "feeAssetDefinitionId":"asset",
          "treasuryAccountId":"$accountId",
          "feeScale":2,
          "hijiriParametersVersion":1,
          "hijiriParametersRevision":"1",
          "hijiriParametersDigest":"${"05".repeat(32)}",
          "defaultAccountRiskQ16":0,
          "effectiveAccountRiskQ16":0,
          "accountRiskRevision":null,
          "accountRiskDigest":null,
          "feeMultiplierQ16":65536,
          "hijiriFeeQuoteHash":"${"07".repeat(32)}",
          "basePerTransferFeeMinorUnits":"10",
          "adjustedPerTransferFeeMinorUnits":"10",
          "qualifyingTransferCount":2,
          "aggregateBaseFeeMinorUnits":"20",
          "aggregateAdjustedFeeMinorUnits":"20"
        }
        """.trimIndent()
            .toByteArray(StandardCharsets.UTF_8)
}
