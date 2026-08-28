package org.hyperledger.iroha.sdk.validationfee

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.client.JsonParser

/** Current native-Norito Hijiri validation-fee quote layout. */
const val VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1: Int = 1

/** Maximum aggregate transfer count accepted by the V1 quote route. */
const val VALIDATION_FEE_HIJIRI_QUOTE_MAX_QUALIFYING_TRANSFERS_V1: Int = 100_000

/** Maximum canonical V1 request size accepted by Torii. */
const val VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES_V1: Int = 4 * 1024

/** Maximum canonical V1 response size accepted by clients. */
const val VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES_V1: Int = 64 * 1024

/** Stable schema marker returned by the native verifier. */
const val VALIDATION_FEE_HIJIRI_QUOTE_SCHEMA_V1: String =
    "iroha.torii.v1.validation_fee.hijiri_quote.response"

/** Honest assurance marker for a live state-evaluated quote. */
const val VALIDATION_FEE_HIJIRI_QUOTE_ASSURANCE_V1: String =
    "EVALUATED_PROJECTION_NOT_INDEPENDENTLY_WITNESS_VERIFIED"

/** Typed input for one exact aggregate Hijiri validation-fee quote. */
class ValidationFeeHijiriQuoteRequestV1(
    accountId: String,
    qualifyingTransferCount: Int,
) {
    /** Frozen request layout version. */
    @JvmField
    val version: Int = VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1

    /** Canonical universal account whose effective Hijiri risk is priced. */
    @JvmField
    val accountId: String = requireCanonicalQuoteAccountId(accountId, "accountId")

    /** Number of qualifying transfers aggregated before the single Q16 ceiling. */
    @JvmField
    val qualifyingTransferCount: Int = qualifyingTransferCount.also {
        require(it in 1..VALIDATION_FEE_HIJIRI_QUOTE_MAX_QUALIFYING_TRANSFERS_V1) {
            "qualifyingTransferCount must be between 1 and " +
                VALIDATION_FEE_HIJIRI_QUOTE_MAX_QUALIFYING_TRANSFERS_V1
        }
    }

    /** Encode this request with the authoritative native Norito codec. */
    fun toNoritoBytes(): ByteArray = ValidationFeeHijiriQuoteBridge.encodeRequestV1(this)

    override fun equals(other: Any?): Boolean =
        other is ValidationFeeHijiriQuoteRequestV1 &&
            accountId == other.accountId &&
            qualifyingTransferCount == other.qualifyingTransferCount

    override fun hashCode(): Int = 31 * accountId.hashCode() + qualifyingTransferCount
}

/**
 * Native-verified V1 Hijiri validation-fee quote.
 *
 * Torii evaluates both policy and Hijiri state at one committed snapshot. The assurance marker is
 * intentionally explicit: the live projection is authenticated by the account-signed transport,
 * but is not an independently witness-verified state proof. Admission later binds the advertised
 * policy and Hijiri hashes and rejects a stale quote.
 */
class ValidationFeeHijiriQuoteV1 internal constructor(
    @JvmField val schema: String,
    @JvmField val version: Int,
    @JvmField val assurance: String,
    @JvmField val evaluatedStateHeight: String,
    @JvmField val quotedExecutionHeight: String,
    @JvmField val accountId: String,
    @JvmField val activePolicyVersion: String,
    @JvmField val activePolicyHash: String,
    @JvmField val feeAssetDefinitionId: String,
    @JvmField val treasuryAccountId: String,
    @JvmField val feeScale: Int,
    @JvmField val hijiriParametersVersion: Int,
    @JvmField val hijiriParametersRevision: String,
    @JvmField val hijiriParametersDigest: String,
    @JvmField val defaultAccountRiskQ16: Long,
    @JvmField val effectiveAccountRiskQ16: Long,
    @JvmField val accountRiskRevision: String?,
    @JvmField val accountRiskDigest: String?,
    @JvmField val feeMultiplierQ16: Long,
    @JvmField val hijiriFeeQuoteHash: String,
    @JvmField val basePerTransferFeeMinorUnits: String,
    @JvmField val adjustedPerTransferFeeMinorUnits: String,
    @JvmField val qualifyingTransferCount: Int,
    @JvmField val aggregateBaseFeeMinorUnits: String,
    @JvmField val aggregateAdjustedFeeMinorUnits: String,
) {
    override fun equals(other: Any?): Boolean =
        other is ValidationFeeHijiriQuoteV1 &&
            schema == other.schema &&
            version == other.version &&
            assurance == other.assurance &&
            evaluatedStateHeight == other.evaluatedStateHeight &&
            quotedExecutionHeight == other.quotedExecutionHeight &&
            accountId == other.accountId &&
            activePolicyVersion == other.activePolicyVersion &&
            activePolicyHash == other.activePolicyHash &&
            feeAssetDefinitionId == other.feeAssetDefinitionId &&
            treasuryAccountId == other.treasuryAccountId &&
            feeScale == other.feeScale &&
            hijiriParametersVersion == other.hijiriParametersVersion &&
            hijiriParametersRevision == other.hijiriParametersRevision &&
            hijiriParametersDigest == other.hijiriParametersDigest &&
            defaultAccountRiskQ16 == other.defaultAccountRiskQ16 &&
            effectiveAccountRiskQ16 == other.effectiveAccountRiskQ16 &&
            accountRiskRevision == other.accountRiskRevision &&
            accountRiskDigest == other.accountRiskDigest &&
            feeMultiplierQ16 == other.feeMultiplierQ16 &&
            hijiriFeeQuoteHash == other.hijiriFeeQuoteHash &&
            basePerTransferFeeMinorUnits == other.basePerTransferFeeMinorUnits &&
            adjustedPerTransferFeeMinorUnits == other.adjustedPerTransferFeeMinorUnits &&
            qualifyingTransferCount == other.qualifyingTransferCount &&
            aggregateBaseFeeMinorUnits == other.aggregateBaseFeeMinorUnits &&
            aggregateAdjustedFeeMinorUnits == other.aggregateAdjustedFeeMinorUnits

    override fun hashCode(): Int = listOf(
        schema,
        version,
        assurance,
        evaluatedStateHeight,
        quotedExecutionHeight,
        accountId,
        activePolicyVersion,
        activePolicyHash,
        feeAssetDefinitionId,
        treasuryAccountId,
        feeScale,
        hijiriParametersVersion,
        hijiriParametersRevision,
        hijiriParametersDigest,
        defaultAccountRiskQ16,
        effectiveAccountRiskQ16,
        accountRiskRevision,
        accountRiskDigest,
        feeMultiplierQ16,
        hijiriFeeQuoteHash,
        basePerTransferFeeMinorUnits,
        adjustedPerTransferFeeMinorUnits,
        qualifyingTransferCount,
        aggregateBaseFeeMinorUnits,
        aggregateAdjustedFeeMinorUnits,
    ).hashCode()
}

internal object ValidationFeeHijiriQuoteProjectionParser {
    private val exactFields = setOf(
        "schema",
        "version",
        "assurance",
        "evaluatedStateHeight",
        "quotedExecutionHeight",
        "accountId",
        "activePolicyVersion",
        "activePolicyHash",
        "feeAssetDefinitionId",
        "treasuryAccountId",
        "feeScale",
        "hijiriParametersVersion",
        "hijiriParametersRevision",
        "hijiriParametersDigest",
        "defaultAccountRiskQ16",
        "effectiveAccountRiskQ16",
        "accountRiskRevision",
        "accountRiskDigest",
        "feeMultiplierQ16",
        "hijiriFeeQuoteHash",
        "basePerTransferFeeMinorUnits",
        "adjustedPerTransferFeeMinorUnits",
        "qualifyingTransferCount",
        "aggregateBaseFeeMinorUnits",
        "aggregateAdjustedFeeMinorUnits",
    )

    fun parse(canonicalJsonUtf8: ByteArray): ValidationFeeHijiriQuoteV1 {
        require(canonicalJsonUtf8.isNotEmpty()) {
            "native Hijiri quote verifier returned an empty projection"
        }
        require(canonicalJsonUtf8.size <= VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES_V1) {
            "native Hijiri quote projection exceeds the response bound"
        }
        val text = String(canonicalJsonUtf8, StandardCharsets.UTF_8)
        require(text.toByteArray(StandardCharsets.UTF_8).contentEquals(canonicalJsonUtf8)) {
            "native Hijiri quote projection is not valid UTF-8"
        }
        val parsed = JsonParser.parse(text)
        require(parsed is Map<*, *>) { "native Hijiri quote projection must be an object" }
        @Suppress("UNCHECKED_CAST")
        val root = parsed as Map<String, Any?>
        require(root.keys == exactFields) {
            "native Hijiri quote projection fields differ from the frozen V1 schema"
        }
        val quote = ValidationFeeHijiriQuoteV1(
            schema = requiredString(root, "schema"),
            version = requiredUnsigned(root, "version", 0xffffL).toInt(),
            assurance = requiredString(root, "assurance"),
            evaluatedStateHeight = requiredString(root, "evaluatedStateHeight"),
            quotedExecutionHeight = requiredString(root, "quotedExecutionHeight"),
            accountId = requireCanonicalQuoteAccountId(
                requiredString(root, "accountId"),
                "accountId",
            ),
            activePolicyVersion = requiredString(root, "activePolicyVersion"),
            activePolicyHash = requiredString(root, "activePolicyHash"),
            feeAssetDefinitionId = requiredString(root, "feeAssetDefinitionId"),
            treasuryAccountId = requiredString(root, "treasuryAccountId"),
            feeScale = requiredUnsigned(root, "feeScale", 0xffL).toInt(),
            hijiriParametersVersion =
                requiredUnsigned(root, "hijiriParametersVersion", 0xffffL).toInt(),
            hijiriParametersRevision = requiredString(root, "hijiriParametersRevision"),
            hijiriParametersDigest = requiredString(root, "hijiriParametersDigest"),
            defaultAccountRiskQ16 = requiredUnsigned(root, "defaultAccountRiskQ16", U32_MAX),
            effectiveAccountRiskQ16 = requiredUnsigned(root, "effectiveAccountRiskQ16", U32_MAX),
            accountRiskRevision = optionalString(root, "accountRiskRevision"),
            accountRiskDigest = optionalString(root, "accountRiskDigest"),
            feeMultiplierQ16 = requiredUnsigned(root, "feeMultiplierQ16", U32_MAX),
            hijiriFeeQuoteHash = requiredString(root, "hijiriFeeQuoteHash"),
            basePerTransferFeeMinorUnits = requiredString(root, "basePerTransferFeeMinorUnits"),
            adjustedPerTransferFeeMinorUnits =
                requiredString(root, "adjustedPerTransferFeeMinorUnits"),
            qualifyingTransferCount =
                requiredUnsigned(
                    root,
                    "qualifyingTransferCount",
                    VALIDATION_FEE_HIJIRI_QUOTE_MAX_QUALIFYING_TRANSFERS_V1.toLong(),
                ).toInt(),
            aggregateBaseFeeMinorUnits = requiredString(root, "aggregateBaseFeeMinorUnits"),
            aggregateAdjustedFeeMinorUnits =
                requiredString(root, "aggregateAdjustedFeeMinorUnits"),
        )
        require(quote.schema == VALIDATION_FEE_HIJIRI_QUOTE_SCHEMA_V1) {
            "native Hijiri quote projection has an unsupported schema"
        }
        require(quote.version == VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1) {
            "native Hijiri quote projection has an unsupported version"
        }
        require(quote.assurance == VALIDATION_FEE_HIJIRI_QUOTE_ASSURANCE_V1) {
            "native Hijiri quote projection has an unsupported assurance marker"
        }
        require((quote.accountRiskRevision == null) == (quote.accountRiskDigest == null)) {
            "native Hijiri quote projection has an incomplete account-risk binding"
        }
        return quote
    }

    private fun requiredString(root: Map<String, Any?>, field: String): String {
        val value = root[field]
        require(value is String && value.isNotEmpty()) {
            "native Hijiri quote projection.$field must be a non-empty string"
        }
        return value
    }

    private fun optionalString(root: Map<String, Any?>, field: String): String? {
        require(root.containsKey(field)) {
            "native Hijiri quote projection.$field is missing"
        }
        val value = root[field]
        require(value == null || value is String && value.isNotEmpty()) {
            "native Hijiri quote projection.$field must be null or a non-empty string"
        }
        return value
    }

    private fun requiredUnsigned(
        root: Map<String, Any?>,
        field: String,
        maximum: Long,
    ): Long {
        val integer = when (val value = root[field]) {
            is Byte -> BigInteger.valueOf(value.toLong())
            is Short -> BigInteger.valueOf(value.toLong())
            is Int -> BigInteger.valueOf(value.toLong())
            is Long -> BigInteger.valueOf(value)
            is BigInteger -> value
            else -> throw IllegalArgumentException(
                "native Hijiri quote projection.$field must be an integer",
            )
        }
        require(integer.signum() >= 0 && integer <= BigInteger.valueOf(maximum)) {
            "native Hijiri quote projection.$field is outside its unsigned range"
        }
        return integer.toLong()
    }

    private const val U32_MAX: Long = 0xffff_ffffL
}

internal fun requireCanonicalQuoteAccountId(value: String, field: String): String {
    require(value.isNotEmpty() && value == value.trim() && value.indexOf('@') < 0) {
        "$field must use one canonical domainless I105 account id"
    }
    val address = try {
        AccountAddress.parseEncodedIgnoringCurveSupport(value, null)
    } catch (error: AccountAddressException) {
        throw IllegalArgumentException(
            "$field must use one canonical domainless I105 account id",
            error,
        )
    }
    val discriminant = AccountAddress.detectI105Discriminant(value)
        ?: throw IllegalArgumentException("$field must use one canonical domainless I105 account id")
    val canonical = try {
        address.toI105(discriminant)
    } catch (error: AccountAddressException) {
        throw IllegalArgumentException(
            "$field must use one canonical domainless I105 account id",
            error,
        )
    }
    require(canonical == value) { "$field must use one canonical domainless I105 account id" }
    return canonical
}
