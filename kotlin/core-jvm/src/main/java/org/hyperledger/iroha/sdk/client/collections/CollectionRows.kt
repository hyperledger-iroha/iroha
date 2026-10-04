package org.hyperledger.iroha.sdk.client.collections

import java.math.BigDecimal
import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.json.JsonArray
import org.hyperledger.iroha.sdk.json.JsonBoolean
import org.hyperledger.iroha.sdk.json.JsonNull
import org.hyperledger.iroha.sdk.json.JsonNumber
import org.hyperledger.iroha.sdk.json.JsonObject
import org.hyperledger.iroha.sdk.json.JsonString

/**
 * A typed collection row. Every row keeps its complete JSON object in [json] (rows may gain
 * fields; unknown members are kept there and otherwise ignored). Equality compares [json].
 */
abstract class CollectionRow internal constructor(
    /** The complete row as returned by Torii. */
    @JvmField val json: JsonObject,
) {
    override fun equals(other: Any?): Boolean =
        other != null && other.javaClass == javaClass && (other as CollectionRow).json == json

    override fun hashCode(): Int = json.hashCode()

    override fun toString(): String = "${javaClass.simpleName}$json"
}

/** A domain. Only [id] is always present. */
class DomainRow internal constructor(json: JsonObject) : CollectionRow(json) {
    private val row = RowReader(json)

    @JvmField val id: String = row.string("id")
    @JvmField val ownedBy: String? = row.stringOrNull("owned_by")
    @JvmField val logo: String? = row.stringOrNull("logo")

    /** Metadata entries (`metadata.<key>` in filters). */
    @JvmField val metadata: JsonObject = row.metadata()
}

/** An account (`id` is the canonical I105 literal). Only [id] is always present. */
class AccountRow internal constructor(json: JsonObject) : CollectionRow(json) {
    private val row = RowReader(json)

    @JvmField val id: String = row.string("id")
    @JvmField val label: String? = row.stringOrNull("label")
    @JvmField val uaid: String? = row.stringOrNull("uaid")
    @JvmField val metadata: JsonObject = row.metadata()
}

/**
 * An asset definition: the complete definition record plus [aliasBinding] when an alias is bound.
 * Ids are Base58 literals. Only [id] is always present.
 */
class AssetDefinitionRow internal constructor(json: JsonObject) : CollectionRow(json) {
    private val row = RowReader(json)

    @JvmField val id: String = row.string("id")
    @JvmField val name: String? = row.stringOrNull("name")
    @JvmField val alias: String? = row.stringOrNull("alias")
    @JvmField val ownedBy: String? = row.stringOrNull("owned_by")
    @JvmField val owningDomain: String? = row.stringOrNull("owning_domain")
    @JvmField val mintable: String? = row.stringOrNull("mintable")
    @JvmField val description: String? = row.stringOrNull("description")
    @JvmField val logo: String? = row.stringOrNull("logo")

    /** The numeric spec record, as JSON. */
    @JvmField val spec: Json? = row.value("spec")

    /** The balance-scope policy record, as JSON. */
    @JvmField val balanceScopePolicy: Json? = row.value("balance_scope_policy")
    @JvmField val aliasBinding: AliasBinding? = row.objectOrNull("alias_binding")?.let(::AliasBinding)
    @JvmField val metadata: JsonObject = row.metadata()

    /** The bound alias of an asset definition. */
    class AliasBinding internal constructor(json: JsonObject) {
        private val row = RowReader(json)

        @JvmField val alias: String? = row.stringOrNull("alias")
        @JvmField val status: String? = row.stringOrNull("status")
        @JvmField val leaseExpiryMs: Long? = row.longOrNull("lease_expiry_ms")
        @JvmField val graceUntilMs: Long? = row.longOrNull("grace_until_ms")
        @JvmField val boundAtMs: Long? = row.longOrNull("bound_at_ms")
    }
}

/** An NFT and its content in [metadata]. Only [id] is always present. */
class NftRow internal constructor(json: JsonObject) : CollectionRow(json) {
    private val row = RowReader(json)

    @JvmField val id: String = row.string("id")
    @JvmField val ownedBy: String? = row.stringOrNull("owned_by")

    /** The NFT content. */
    @JvmField val metadata: JsonObject = row.metadata()
}

/** A real-world-asset lot; [quantity] is exact. Only [id] is always present. */
class RwaLotRow internal constructor(json: JsonObject) : CollectionRow(json) {
    private val row = RowReader(json)

    @JvmField val id: String = row.string("id")
    @JvmField val ownedBy: String? = row.stringOrNull("owned_by")
    @JvmField val primaryReference: String? = row.stringOrNull("primary_reference")
    @JvmField val status: String? = row.stringOrNull("status")
    @JvmField val quantity: BigDecimal? = row.decimalOrNull("quantity")
    @JvmField val isFrozen: Boolean? = row.booleanOrNull("is_frozen")
    @JvmField val metadata: JsonObject = row.metadata()
}

/** One balance of an account; [asset], [scope], [accountId] and [quantity] are always present. */
class AccountAssetRow internal constructor(json: JsonObject) : CollectionRow(json) {
    private val row = RowReader(json)

    /** Asset definition id. */
    @JvmField val asset: String = row.string("asset")
    @JvmField val assetName: String? = row.stringOrNull("asset_name")
    @JvmField val assetAlias: String? = row.stringOrNull("asset_alias")

    /** Balance scope: `global` or `dataspace:<id>`. */
    @JvmField val scope: String = row.string("scope")
    @JvmField val accountId: String = row.string("account_id")
    @JvmField val quantity: BigDecimal = row.decimal("quantity")
}

/** One holder of an asset definition; [accountId], [asset], [scope] and [quantity] are always present. */
class AssetHolderRow internal constructor(json: JsonObject) : CollectionRow(json) {
    private val row = RowReader(json)

    @JvmField val accountId: String = row.string("account_id")
    @JvmField val asset: String = row.string("asset")
    @JvmField val assetAlias: String? = row.stringOrNull("asset_alias")
    @JvmField val scope: String = row.string("scope")
    @JvmField val quantity: BigDecimal = row.decimal("quantity")
}

/**
 * One committed transaction from a history collection (global `transactions` or one account's
 * transactions), newest first by ([blockHeight], [blockIndex]). [entrypointHash], [blockHeight]
 * and [blockIndex] are always present.
 */
class TransactionRow internal constructor(json: JsonObject) : CollectionRow(json) {
    private val row = RowReader(json)

    @JvmField val entrypointHash: String = row.string("entrypoint_hash")
    @JvmField val blockHeight: Long = row.unsignedLong("block_height")

    /** Position of the transaction within its block. */
    @JvmField val blockIndex: Long = row.unsignedLong("block_index")
    @JvmField val blockHash: String? = row.stringOrNull("block_hash")
    @JvmField val authority: String? = row.stringOrNull("authority")
    @JvmField val timestampMs: Long? = row.unsignedLongOrNull("timestamp_ms")
    @JvmField val entrypointKind: String? = row.stringOrNull("entrypoint_kind")
    @JvmField val resultOk: Boolean? = row.booleanOrNull("result_ok")

    /** Assets the transaction touched; filters match element-wise. */
    @JvmField val assetIds: List<String> = row.stringList("asset_ids")

    /** Asset definitions the transaction touched; filters match element-wise. */
    @JvmField val assetDefinitionIds: List<String> = row.stringList("asset_definition_ids")
    @JvmField val metadata: JsonObject = row.metadata()
}

/** A repo agreement; leg quantities are exact. Only [id] is always present. */
class RepoAgreementRow internal constructor(json: JsonObject) : CollectionRow(json) {
    private val row = RowReader(json)

    @JvmField val id: String = row.string("id")
    @JvmField val initiator: String? = row.stringOrNull("initiator")
    @JvmField val counterparty: String? = row.stringOrNull("counterparty")
    @JvmField val custodian: String? = row.stringOrNull("custodian")
    @JvmField val status: String? = row.stringOrNull("status")
    @JvmField val cashSource: String? = row.stringOrNull("cash_source")
    @JvmField val cashLeg: Leg? = row.objectOrNull("cash_leg")?.let(::Leg)
    @JvmField val collateralLeg: Leg? = row.objectOrNull("collateral_leg")?.let(::Leg)
    @JvmField val collateralCustodyAsset: String? = row.stringOrNull("collateral_custody_asset")
    @JvmField val rateBps: Long? = row.longOrNull("rate_bps")
    @JvmField val maturityTimestampMs: Long? = row.longOrNull("maturity_timestamp_ms")
    @JvmField val initiatedTimestampMs: Long? = row.longOrNull("initiated_timestamp_ms")
    @JvmField val lastMarginCheckTimestampMs: Long? = row.longOrNull("last_margin_check_timestamp_ms")
    @JvmField val settlementTimestampMs: Long? = row.longOrNull("settlement_timestamp_ms")
    @JvmField val governance: Governance? = row.objectOrNull("governance")?.let(::Governance)

    /** One leg of the agreement. */
    class Leg internal constructor(json: JsonObject) {
        private val row = RowReader(json)

        @JvmField val assetDefinitionId: String? = row.stringOrNull("asset_definition_id")
        @JvmField val quantity: BigDecimal? = row.decimalOrNull("quantity")
    }

    /** Governance terms of the agreement. */
    class Governance internal constructor(json: JsonObject) {
        private val row = RowReader(json)

        @JvmField val haircutBps: Long? = row.longOrNull("haircut_bps")
        @JvmField val marginFrequencySecs: Long? = row.longOrNull("margin_frequency_secs")
    }
}

/** Typed member access for row decoding; failures become [IllegalArgumentException]. */
internal class RowReader(private val row: JsonObject) {
    fun value(name: String): Json? = row[name]?.takeUnless { it === JsonNull }

    fun string(name: String): String = stringOrNull(name) ?: missing(name, "a string")

    fun stringOrNull(name: String): String? = when (val value = value(name)) {
        null -> null
        is JsonString -> value.value
        else -> wrongType(name, "a string")
    }

    fun decimal(name: String): BigDecimal = decimalOrNull(name) ?: missing(name, "an exact decimal")

    fun decimalOrNull(name: String): BigDecimal? = when (val value = value(name)) {
        null -> null
        is JsonString -> parseDecimal(name, value.value)
        is JsonNumber -> value.toBigDecimal()
        else -> wrongType(name, "an exact decimal")
    }

    fun long(name: String): Long = longOrNull(name) ?: missing(name, "an integer")

    /** A `u64` member (within the signed 64-bit range). */
    fun unsignedLong(name: String): Long = unsignedLongOrNull(name) ?: missing(name, "an unsigned integer")

    fun unsignedLongOrNull(name: String): Long? = longOrNull(name)?.also {
        require(it >= 0) { "`$name` must be an unsigned integer, got $it" }
    }

    fun longOrNull(name: String): Long? = when (val value = value(name)) {
        null -> null
        is JsonNumber -> try {
            value.toLongExact()
        } catch (error: IllegalStateException) {
            throw IllegalArgumentException("`$name` must be a 64-bit integer, got ${value.text}", error)
        }
        else -> wrongType(name, "an integer")
    }

    fun booleanOrNull(name: String): Boolean? = when (val value = value(name)) {
        null -> null
        is JsonBoolean -> value.value
        else -> wrongType(name, "a boolean")
    }

    /** A list of strings; absent or `null` reads as empty. */
    fun stringList(name: String): List<String> = when (val value = value(name)) {
        null -> emptyList()
        is JsonArray -> java.util.Collections.unmodifiableList(
            value.items.map { item -> (item as? JsonString)?.value ?: wrongType(name, "an array of strings") },
        )
        else -> wrongType(name, "an array of strings")
    }

    fun objectOrNull(name: String): JsonObject? = when (val value = value(name)) {
        null -> null
        is JsonObject -> value
        else -> wrongType(name, "an object")
    }

    fun metadata(): JsonObject = objectOrNull("metadata") ?: JsonObject.EMPTY

    private fun parseDecimal(name: String, text: String): BigDecimal = try {
        BigDecimal(text)
    } catch (error: NumberFormatException) {
        throw IllegalArgumentException("`$name` must be an exact decimal, got `$text`", error)
    }

    private fun missing(name: String, type: String): Nothing =
        throw IllegalArgumentException("required member `$name` (expected $type) is missing")

    private fun wrongType(name: String, type: String): Nothing =
        throw IllegalArgumentException("`$name` must be $type")
}
