package org.hyperledger.iroha.sdk.nexus

import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonNumbers
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.numeric.NumericV1Codec

/** Exact first-release JSON parser for UAID responses. */
object UaidJsonParser {

    @JvmStatic
    fun parsePortfolio(payload: ByteArray): UaidPortfolioResponse {
        val root = exactObject(
            parse(payload),
            "uaid portfolio",
            setOf("uaid", "totals", "dataspaces"),
        )
        val uaid = UaidLiteral.canonicalize(
            exactString(root["uaid"], "uaid portfolio.uaid"),
            "uaid portfolio.uaid",
        )
        val totalsObj = exactObject(
            root["totals"],
            "uaid portfolio.totals",
            setOf("accounts", "positions"),
        )
        val totals = UaidPortfolioResponse.UaidPortfolioTotals(
            asUnsignedLong(totalsObj["accounts"], "uaid portfolio.totals.accounts"),
            asUnsignedLong(totalsObj["positions"], "uaid portfolio.totals.positions"),
        )
        val dataspaces = requiredArray(root["dataspaces"], "uaid portfolio.dataspaces")
            .mapIndexed { i, item -> parsePortfolioDataspace(item, i) }
        return UaidPortfolioResponse(uaid, totals, dataspaces)
    }

    @JvmStatic
    fun parseBindings(payload: ByteArray): UaidBindingsResponse {
        val root = exactObject(
            parse(payload),
            "uaid bindings",
            setOf("uaid", "dataspaces"),
        )
        val uaid = UaidLiteral.canonicalize(
            exactString(root["uaid"], "uaid bindings.uaid"),
            "uaid bindings.uaid",
        )
        val dataspaces = requiredArray(root["dataspaces"], "uaid bindings.dataspaces")
            .mapIndexed { i, item ->
                val path = "uaid bindings.dataspaces[$i]"
                val entry = exactObject(
                    item,
                    path,
                    setOf("dataspace_id", "dataspace_alias", "accounts"),
                )
                UaidBindingsResponse.UaidBindingsDataspace(
                    asUnsignedLong(entry["dataspace_id"], "$path.dataspace_id"),
                    nullableExactString(entry["dataspace_alias"], "$path.dataspace_alias"),
                    exactStringList(entry["accounts"], "$path.accounts"),
                )
            }
        return UaidBindingsResponse(uaid, dataspaces)
    }

    @JvmStatic
    fun parseManifests(payload: ByteArray): UaidManifestsResponse {
        val root = exactObject(
            parse(payload),
            "uaid manifests",
            setOf("uaid", "total", "has_more", "count_mode", "manifests"),
        )
        val uaid = UaidLiteral.canonicalize(
            exactString(root["uaid"], "uaid manifests.uaid"),
            "uaid manifests.uaid",
        )
        val total = asUnsignedLong(root["total"], "uaid manifests.total")
        val hasMore = asBoolean(root["has_more"], "uaid manifests.has_more")
        val countMode = parseCountMode(
            exactString(root["count_mode"], "uaid manifests.count_mode"),
        )
        val manifests = requiredArray(root["manifests"], "uaid manifests.manifests")
            .mapIndexed { i, item -> parseManifestRecord(item, i, uaid) }
        return UaidManifestsResponse(uaid, total, hasMore, countMode, manifests)
    }

    private fun parsePortfolioDataspace(
        item: Any?,
        index: Int,
    ): UaidPortfolioResponse.UaidPortfolioDataspace {
        val path = "uaid portfolio.dataspaces[$index]"
        val entry = exactObject(
            item,
            path,
            setOf("dataspace_id", "dataspace_alias", "accounts"),
        )
        val accounts = requiredArray(entry["accounts"], "$path.accounts")
            .mapIndexed { accountIndex, accountItem ->
                parsePortfolioAccount(accountItem, "$path.accounts[$accountIndex]")
            }
        return UaidPortfolioResponse.UaidPortfolioDataspace(
            asUnsignedLong(entry["dataspace_id"], "$path.dataspace_id"),
            nullableExactString(entry["dataspace_alias"], "$path.dataspace_alias"),
            accounts,
        )
    }

    private fun parsePortfolioAccount(
        item: Any?,
        path: String,
    ): UaidPortfolioResponse.UaidPortfolioAccount {
        val account = exactObject(
            item,
            path,
            setOf("account_id", "label", "assets"),
        )
        val assets = requiredArray(account["assets"], "$path.assets")
            .mapIndexed { assetIndex, assetItem ->
                val assetPath = "$path.assets[$assetIndex]"
                val asset = exactObject(
                    assetItem,
                    assetPath,
                    setOf("asset_id", "asset_definition_id", "quantity"),
                )
                UaidPortfolioResponse.UaidPortfolioAsset(
                    exactString(asset["asset_id"], "$assetPath.asset_id"),
                    exactString(
                        asset["asset_definition_id"],
                        "$assetPath.asset_definition_id",
                    ),
                    NumericV1Codec.decodeQuantityJsonValue(asset["quantity"]).toString(),
                )
            }
        return UaidPortfolioResponse.UaidPortfolioAccount(
            exactString(account["account_id"], "$path.account_id"),
            nullableExactString(account["label"], "$path.label"),
            assets,
        )
    }

    private fun parseManifestRecord(
        item: Any?,
        index: Int,
        responseUaid: String,
    ): UaidManifestsResponse.UaidManifestRecord {
        val path = "uaid manifests.manifests[$index]"
        val entry = exactObject(
            item,
            path,
            setOf(
                "dataspace_id",
                "dataspace_alias",
                "manifest_hash",
                "status",
                "lifecycle",
                "accounts",
                "manifest",
            ),
        )
        val dataspaceId = asUnsignedLong(entry["dataspace_id"], "$path.dataspace_id")
        val manifestHash = exactString(entry["manifest_hash"], "$path.manifest_hash")
        check(manifestHash.matches(Regex("[0-9a-f]{64}"))) {
            "$path.manifest_hash must be exactly 64 lowercase hexadecimal characters"
        }
        val lifecycleMap = exactObject(
            entry["lifecycle"],
            "$path.lifecycle",
            setOf("activated_epoch", "expired_epoch", "revocation"),
        )
        val revocation = lifecycleMap["revocation"]?.let { value ->
            val revocationPath = "$path.lifecycle.revocation"
            val revocationMap = exactObject(
                value,
                revocationPath,
                setOf("epoch", "reason"),
            )
            UaidManifestsResponse.UaidManifestRevocation(
                asUnsignedLong(revocationMap["epoch"], "$revocationPath.epoch"),
                nullableString(revocationMap["reason"], "$revocationPath.reason"),
            )
        }
        val manifest = validateManifest(
            entry["manifest"],
            "$path.manifest",
            responseUaid,
            dataspaceId,
        )
        return UaidManifestsResponse.UaidManifestRecord(
            dataspaceId,
            nullableExactString(entry["dataspace_alias"], "$path.dataspace_alias"),
            manifestHash,
            parseManifestStatus(exactString(entry["status"], "$path.status")),
            UaidManifestsResponse.UaidManifestLifecycle(
                nullableUnsignedLong(
                    lifecycleMap["activated_epoch"],
                    "$path.lifecycle.activated_epoch",
                ),
                nullableUnsignedLong(
                    lifecycleMap["expired_epoch"],
                    "$path.lifecycle.expired_epoch",
                ),
                revocation,
            ),
            exactStringList(entry["accounts"], "$path.accounts"),
            JsonEncoder.encode(manifest),
        )
    }

    private fun validateManifest(
        value: Any?,
        path: String,
        responseUaid: String,
        responseDataspace: Long,
    ): Map<String, Any?> {
        val manifest = exactObject(
            value,
            path,
            required = setOf(
                "version",
                "uaid",
                "dataspace",
                "issued_ms",
                "activation_epoch",
                "entries",
            ),
            optional = setOf("expiry_epoch"),
        )
        check(asUnsignedLong(manifest["version"], "$path.version") == 1L) {
            "$path.version must be the numeric value 1"
        }
        val manifestUaid = UaidLiteral.canonicalize(
            exactString(manifest["uaid"], "$path.uaid"),
            "$path.uaid",
        )
        check(manifestUaid == responseUaid) { "$path.uaid must match the response UAID" }
        check(asUnsignedLong(manifest["dataspace"], "$path.dataspace") == responseDataspace) {
            "$path.dataspace must match the manifest record dataspace_id"
        }
        asUnsignedLong(manifest["issued_ms"], "$path.issued_ms")
        asUnsignedLong(manifest["activation_epoch"], "$path.activation_epoch")
        if (manifest.containsKey("expiry_epoch")) {
            check(manifest["expiry_epoch"] != null) {
                "$path.expiry_epoch must be omitted instead of null"
            }
            asUnsignedLong(manifest["expiry_epoch"], "$path.expiry_epoch")
        }
        requiredArray(manifest["entries"], "$path.entries").forEachIndexed { index, entry ->
            validateManifestEntry(entry, "$path.entries[$index]")
        }
        return manifest
    }

    private fun validateManifestEntry(value: Any?, path: String) {
        val entry = exactObject(
            value,
            path,
            required = setOf("scope", "effect"),
            optional = setOf("notes"),
        )
        if (entry.containsKey("notes")) {
            check(entry["notes"] != null) { "$path.notes must be omitted instead of null" }
            asString(entry["notes"], "$path.notes")
        }
        validateManifestScope(entry["scope"], "$path.scope")
        validateManifestEffect(entry["effect"], "$path.effect")
    }

    private fun validateManifestScope(value: Any?, path: String) {
        val scope = exactObject(
            value,
            path,
            required = emptySet(),
            optional = setOf("dataspace", "program", "method", "asset", "role"),
        )
        scope.forEach { (field, fieldValue) ->
            check(fieldValue != null) { "$path.$field must be omitted instead of null" }
            when (field) {
                "dataspace" -> asUnsignedLong(fieldValue, "$path.dataspace")
                "program", "method", "asset" -> exactString(fieldValue, "$path.$field")
                "role" -> check(
                    exactString(fieldValue, "$path.role") in setOf("Initiator", "Participant"),
                ) { "$path.role must be Initiator or Participant" }
            }
        }
    }

    private fun validateManifestEffect(value: Any?, path: String) {
        val effect = expectObject(value, path)
        check(effect.size == 1 && effect.keys.single() in setOf("Allow", "Deny")) {
            "$path must contain exactly one of Allow or Deny"
        }
        if (effect.containsKey("Allow")) {
            val allowance = exactObject(
                effect["Allow"],
                "$path.Allow",
                required = setOf("window"),
                optional = setOf("max_amount"),
            )
            val window = exactString(allowance["window"], "$path.Allow.window")
            check(window in setOf("PerSlot", "PerMinute", "PerDay")) {
                "$path.Allow.window is not a canonical allowance window"
            }
            if (allowance.containsKey("max_amount")) {
                check(allowance["max_amount"] != null) {
                    "$path.Allow.max_amount must be omitted instead of null"
                }
                NumericV1Codec.decodeQuantityJsonValue(allowance["max_amount"])
            }
        } else {
            val denial = exactObject(
                effect["Deny"],
                "$path.Deny",
                required = emptySet(),
                optional = setOf("reason"),
            )
            if (denial.containsKey("reason")) {
                check(denial["reason"] != null) {
                    "$path.Deny.reason must be omitted instead of null"
                }
                asString(denial["reason"], "$path.Deny.reason")
            }
        }
    }

    private fun parse(payload: ByteArray): Any {
        require(payload.isNotEmpty()) { "UAID endpoint returned an empty payload" }
        val json = String(payload, Charsets.UTF_8).trim()
        check(json.isNotEmpty()) { "UAID endpoint returned a blank payload" }
        return JsonParser.parse(json) ?: throw IllegalStateException("UAID endpoint returned null JSON")
    }

    @Suppress("UNCHECKED_CAST")
    private fun expectObject(value: Any?, path: String): Map<String, Any?> {
        check(value is Map<*, *>) { "$path must be a JSON object" }
        check(value.keys.all { it is String }) { "$path field names must be strings" }
        return value as Map<String, Any?>
    }

    private fun exactObject(
        value: Any?,
        path: String,
        required: Set<String>,
        optional: Set<String> = emptySet(),
    ): Map<String, Any?> {
        val objectValue = expectObject(value, path)
        val allowed = required + optional
        val unknown = objectValue.keys - allowed
        check(unknown.isEmpty()) { "$path contains unknown fields: ${unknown.sorted()}" }
        val missing = required - objectValue.keys
        check(missing.isEmpty()) { "$path is missing required fields: ${missing.sorted()}" }
        return objectValue
    }

    private fun requiredArray(value: Any?, path: String): List<Any?> {
        check(value is List<*>) { "$path must be a JSON array" }
        return value
    }

    private fun asString(value: Any?, path: String): String {
        check(value is String) { "$path must be a string" }
        return value
    }

    private fun exactString(value: Any?, path: String): String {
        val string = asString(value, path)
        check(string.isNotEmpty()) { "$path must not be empty" }
        check(string.trim() == string) { "$path must not contain surrounding whitespace" }
        return string
    }

    private fun nullableString(value: Any?, path: String): String? =
        if (value == null) null else asString(value, path)

    private fun nullableExactString(value: Any?, path: String): String? =
        if (value == null) null else exactString(value, path)

    private fun asUnsignedLong(value: Any?, path: String): Long {
        val parsed = JsonNumbers.asLong(value, path)
        check(parsed >= 0) { "$path must be an unsigned integer" }
        return parsed
    }

    private fun nullableUnsignedLong(value: Any?, path: String): Long? =
        if (value == null) null else asUnsignedLong(value, path)

    private fun asBoolean(value: Any?, path: String): Boolean {
        check(value is Boolean) { "$path must be a boolean" }
        return value
    }

    private fun exactStringList(value: Any?, path: String): List<String> =
        requiredArray(value, path).mapIndexed { index, entry ->
            exactString(entry, "$path[$index]")
        }

    private fun parseCountMode(value: String): UaidManifestCountMode =
        when (value) {
            "bounded" -> UaidManifestCountMode.BOUNDED
            "exact" -> UaidManifestCountMode.EXACT
            else -> throw IllegalStateException("Unsupported manifest count_mode: $value")
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
