package org.hyperledger.iroha.sdk.nexus

import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser

/** Minimal JSON parser for UAID responses. */
object UaidJsonParser {

    @JvmStatic
    fun parsePortfolio(payload: ByteArray): UaidPortfolioResponse {
        val root = expectObject(parse(payload), "uaid portfolio")
        val uaid = UaidLiteral.canonicalize(asString(root["uaid"], "uaid portfolio.uaid"), "uaid")
        val totalsObj = asObjectOrEmpty(root["totals"], "uaid portfolio.totals")
        val accounts = asLong(totalsObj.getOrDefault("accounts", 0L), "uaid portfolio.totals.accounts")
        val positions = asLong(totalsObj.getOrDefault("positions", 0L), "uaid portfolio.totals.positions")
        val totals = UaidPortfolioResponse.UaidPortfolioTotals(accounts, positions)

        val dataspaceItems = asArrayOrEmpty(root["dataspaces"], "uaid portfolio.dataspaces")
        val dataspaces = dataspaceItems.mapIndexed { i, item ->
            val entry = expectObject(item, "uaid portfolio.dataspaces[$i]")
            val dataspaceId = asLong(entry["dataspace_id"], "uaid portfolio.dataspaces[$i].dataspace_id")
            val dataspaceAlias = asOptionalString(entry["dataspace_alias"])
            val accountItems = asArrayOrEmpty(entry["accounts"], "uaid portfolio.dataspaces[$i].accounts")
            val accountsList = accountItems.mapIndexed { j, accountItem ->
                val account = expectObject(accountItem, "uaid portfolio.dataspaces[$i].accounts[$j]")
                val accountId = asString(account["account_id"], "uaid portfolio.dataspaces[$i].accounts[$j].account_id")
                val label = asOptionalString(account["label"])
                val assetItems = asArrayOrEmpty(account["assets"], "uaid portfolio.dataspaces[$i].accounts[$j].assets")
                val assets = assetItems.mapIndexed { k, assetItem ->
                    val asset = expectObject(assetItem, "uaid portfolio.dataspaces[$i].accounts[$j].assets[$k]")
                    UaidPortfolioResponse.UaidPortfolioAsset(
                        assetId = asString(asset["asset_id"], "uaid portfolio.dataspaces[$i].accounts[$j].assets[$k].asset_id"),
                        assetDefinitionId = asString(asset["asset_definition_id"], "uaid portfolio.dataspaces[$i].accounts[$j].assets[$k].asset_definition_id"),
                        quantity = asString(asset["quantity"], "uaid portfolio.dataspaces[$i].accounts[$j].assets[$k].quantity"),
                    )
                }
                UaidPortfolioResponse.UaidPortfolioAccount(accountId, label, assets)
            }
            UaidPortfolioResponse.UaidPortfolioDataspace(dataspaceId, dataspaceAlias, accountsList)
        }
        return UaidPortfolioResponse(uaid, totals, dataspaces)
    }

    @JvmStatic
    fun parseBindings(payload: ByteArray): UaidBindingsResponse {
        val root = expectObject(parse(payload), "uaid bindings")
        val uaid = UaidLiteral.canonicalize(asString(root["uaid"], "uaid bindings.uaid"), "uaid")
        val dataspaceItems = asArrayOrEmpty(root["dataspaces"], "uaid bindings.dataspaces")
        val dataspaces = dataspaceItems.mapIndexed { i, item ->
            val entry = expectObject(item, "uaid bindings.dataspaces[$i]")
            val dataspaceId = asLong(entry["dataspace_id"], "uaid bindings.dataspaces[$i].dataspace_id")
            val dataspaceAlias = asOptionalString(entry["dataspace_alias"])
            val accounts = asStringList(entry["accounts"], "uaid bindings.dataspaces[$i].accounts")
            UaidBindingsResponse.UaidBindingsDataspace(dataspaceId, dataspaceAlias, accounts)
        }
        return UaidBindingsResponse(uaid, dataspaces)
    }

    @JvmStatic
    fun parseManifests(payload: ByteArray): UaidManifestsResponse {
        val root = expectObject(parse(payload), "uaid manifests")
        val uaid = UaidLiteral.canonicalize(asString(root["uaid"], "uaid manifests.uaid"), "uaid")
        val total = asLong(root.getOrDefault("total", 0L), "uaid manifests.total")
        val manifestItems = asArrayOrEmpty(root["manifests"], "uaid manifests.manifests")
        val manifests = manifestItems.mapIndexed { i, item ->
            val entry = expectObject(item, "uaid manifests.manifests[$i]")
            val dataspaceId = asLong(entry["dataspace_id"], "uaid manifests.manifests[$i].dataspace_id")
            val alias = asOptionalString(entry["dataspace_alias"])
            val manifestHash = asString(entry["manifest_hash"], "uaid manifests.manifests[$i].manifest_hash")
            val status = parseManifestStatus(asString(entry["status"], "uaid manifests.manifests[$i].status"))
            val lifecycleMap = asObjectOrEmpty(entry["lifecycle"], "uaid manifests.manifests[$i].lifecycle")
            val activatedEpoch = asOptionalLong(lifecycleMap["activated_epoch"], "uaid manifests.manifests[$i].lifecycle.activated_epoch")
            val expiredEpoch = asOptionalLong(lifecycleMap["expired_epoch"], "uaid manifests.manifests[$i].lifecycle.expired_epoch")
            val revocationMap = asObjectOrNull(lifecycleMap["revocation"], "uaid manifests.manifests[$i].lifecycle.revocation")
            val revocation = revocationMap?.let {
                UaidManifestsResponse.UaidManifestRevocation(
                    epoch = asLong(it["epoch"], "uaid manifests.manifests[$i].lifecycle.revocation.epoch"),
                    reason = asOptionalString(it["reason"]),
                )
            }
            val lifecycle = UaidManifestsResponse.UaidManifestLifecycle(activatedEpoch, expiredEpoch, revocation)
            val accounts = asStringList(entry["accounts"], "uaid manifests.manifests[$i].accounts")
            val manifestJson = JsonEncoder.encode(expectObject(entry["manifest"], "uaid manifests.manifests[$i].manifest"))
            UaidManifestsResponse.UaidManifestRecord(dataspaceId, alias, manifestHash, status, lifecycle, accounts, manifestJson)
        }
        return UaidManifestsResponse(uaid, total, manifests)
    }

    private fun parse(payload: ByteArray): Any {
        require(payload.isNotEmpty()) { "UAID endpoint returned an empty payload" }
        val json = String(payload, Charsets.UTF_8).trim()
        check(json.isNotEmpty()) { "UAID endpoint returned a blank payload" }
        return JsonParser.parse(json) ?: throw IllegalStateException("UAID endpoint returned null JSON")
    }

    @Suppress("UNCHECKED_CAST")
    private fun expectObject(value: Any?, path: String): Map<String, Any> {
        check(value is Map<*, *>) { "$path must be a JSON object" }
        return value as Map<String, Any>
    }

    private fun asObjectOrEmpty(value: Any?, path: String): Map<String, Any> =
        if (value == null) emptyMap() else expectObject(value, path)

    private fun asObjectOrNull(value: Any?, path: String): Map<String, Any>? =
        if (value == null) null else expectObject(value, path)

    @Suppress("UNCHECKED_CAST")
    private fun asArrayOrEmpty(value: Any?, path: String): List<Any> {
        if (value == null) return emptyList()
        check(value is List<*>) { "$path must be a JSON array" }
        return value as List<Any>
    }

    private fun asString(value: Any?, path: String): String {
        checkNotNull(value) { "$path is missing" }
        return if (value is String) value else value.toString()
    }

    private fun asOptionalString(value: Any?): String? {
        if (value == null) return null
        return if (value is String) value else value.toString()
    }

    private fun asLong(value: Any?, path: String): Long {
        check(value is Number) { "$path is not a number" }
        if (value is Float || value is Double) {
            throw IllegalStateException("$path must be an integer")
        }
        return value.toLong()
    }

    private fun asOptionalLong(value: Any?, path: String): Long? =
        if (value == null) null else asLong(value, path)

    private fun asStringList(value: Any?, path: String): List<String> {
        val items = asArrayOrEmpty(value, path)
        return items.mapNotNull { entry ->
            if (entry == null) null
            else if (entry is String) entry else entry.toString()
        }
    }

    private fun parseManifestStatus(value: String): UaidManifestsResponse.UaidManifestStatus =
        when (value) {
            "Pending" -> UaidManifestsResponse.UaidManifestStatus.PENDING
            "Active" -> UaidManifestsResponse.UaidManifestStatus.ACTIVE
            "Expired" -> UaidManifestsResponse.UaidManifestStatus.EXPIRED
            "Revoked" -> UaidManifestsResponse.UaidManifestStatus.REVOKED
            else -> throw IllegalStateException("Unsupported manifest status: $value")
        }
}
