package org.hyperledger.iroha.sdk.musubi

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.util.Collections
import java.util.LinkedHashMap
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.NetworkId

/** Strict Norito-JSON codec for the first-release Musubi read surface. */
internal object MusubiJsonV1 {
    private val QUERY_PATHS = setOf(
        "/v1/musubi/queries/exact-package",
        "/v1/musubi/queries/exact-release",
        "/v1/musubi/queries/provider-bundle-attestation",
        "/v1/musubi/queries/resolver-index",
        "/v1/musubi/queries/versions",
        "/v1/musubi/queries/maintainers",
        "/v1/musubi/queries/archive-locations",
        "/v1/musubi/queries/archive-retention",
        "/v1/musubi/queries/alias",
        "/v1/musubi/queries/alias-history",
        "/v1/musubi/queries/ordered-prefix",
        "/v1/musubi/queries/search",
    )

    fun encode(value: Any?): ByteArray =
        JsonEncoder.encode(value).toByteArray(StandardCharsets.UTF_8)

    fun parse(payload: ByteArray, field: String): Any? {
        require(payload.isNotEmpty()) { "$field must not be empty" }
        return try {
            JsonParser.parse(String(payload, StandardCharsets.UTF_8))
        } catch (error: RuntimeException) {
            throw IllegalArgumentException("invalid $field JSON", error)
        }
    }

    fun parseExactPackage(payload: ByteArray): MusubiPackageRecordV1 =
        parsePackageRecord(parse(payload, "Musubi exact-package response"), "response")

    fun parseExactRelease(payload: ByteArray): MusubiExactReleaseSnapshotV1 {
        val root = exactObject(
            parse(payload, "Musubi exact-release response"),
            "response",
            setOf("network_id", "snapshot", "home_release", "universal_release"),
        )
        val snapshot = parseSnapshot(root["snapshot"], "response.snapshot")
        val homeRelease = parseReleaseRecord(root["home_release"], "response.home_release")
        val universalRelease = parseResolverRow(
            root["universal_release"],
            "response.universal_release",
        )
        return MusubiExactReleaseSnapshotV1(
            NetworkId.parse(string(root["network_id"], "response.network_id")),
            snapshot,
            homeRelease,
            universalRelease,
        )
    }

    internal fun validateExactReleaseSnapshot(
        home: Map<String, Any?>,
        universal: Map<String, Any?>,
        networkId: NetworkId,
        snapshot: MusubiRegistrySnapshotV1,
    ) {
        val manifest = objectMap(home["manifest"], "response.home_release.manifest")
        val yank = objectMap(home["yank"], "response.home_release.yank")
        val governance = objectMap(
            home["artifact_governance"],
            "response.home_release.artifact_governance",
        )
        val revisions = objectMap(home["revisions"], "response.home_release.revisions")
        val selection = objectMap(
            universal["selection"],
            "response.universal_release.selection",
        )
        val storage = objectMap(
            selection["storage"],
            "response.universal_release.selection.storage",
        )

        val publishedAtHeight = nonZeroU64(
            home["published_at_height"],
            "response.home_release.published_at_height",
        )
        val yankChangedAtHeight = nonZeroU64(
            yank["changed_at_height"],
            "response.home_release.yank.changed_at_height",
        )
        val yankRevision = nonZeroU64(
            yank["revision"],
            "response.home_release.yank.revision",
        )
        val homeYankRevision = nonZeroU64(
            revisions["yank"],
            "response.home_release.revisions.yank",
        )
        val homeGovernanceRevision = nonZeroU64(
            revisions["artifact_governance"],
            "response.home_release.revisions.artifact_governance",
        )
        val universalRevision = nonZeroU64(
            universal["index_revision"],
            "response.universal_release.index_revision",
        )
        val storageRevision = nonZeroU64(
            storage["index_revision"],
            "response.universal_release.selection.storage.index_revision",
        )
        val storageFinalizedHeight = nonZeroU64(
            storage["finalized_height"],
            "response.universal_release.selection.storage.finalized_height",
        )
        val storageFinalizedHash = fixedBytes(
            storage["finalized_block_hash"],
            "response.universal_release.selection.storage.finalized_block_hash",
        )
        val governanceKind = string(
            governance["kind"],
            "response.home_release.artifact_governance.kind",
        )
        val takedownHeight = if (governanceKind == "TakenDown") {
            val takedown = objectMap(
                governance["value"],
                "response.home_release.artifact_governance.value",
            )
            nonZeroU64(
                takedown["applied_at_height"],
                "response.home_release.artifact_governance.value.applied_at_height",
            )
        } else {
            BigInteger.ZERO
        }

        require(
            home["release_digest"] == universal["release_digest"] &&
                manifest["archive_id"] == universal["archive_id"] &&
                manifest["archive_id"] == storage["archive_id"] &&
                manifest["interface_digest"] == universal["interface_digest"] &&
                manifest["abi"] == universal["abi"] &&
                manifest["dependencies"] == universal["dependencies"] &&
                home["yank"] == selection["yank"] &&
                home["artifact_governance"] == selection["governance"] &&
                yankRevision == homeYankRevision &&
                homeYankRevision <= snapshot.indexRevision &&
                homeGovernanceRevision <= snapshot.indexRevision &&
                universalRevision <= snapshot.indexRevision &&
                storageRevision <= universalRevision &&
                storageRevision <= snapshot.indexRevision &&
                publishedAtHeight <= snapshot.finalizedHeight &&
                yankChangedAtHeight >= publishedAtHeight &&
                yankChangedAtHeight <= snapshot.finalizedHeight &&
                (takedownHeight == BigInteger.ZERO || takedownHeight >= publishedAtHeight) &&
                takedownHeight <= snapshot.finalizedHeight &&
                storageFinalizedHeight <= snapshot.finalizedHeight &&
                (snapshot.finalizedHeight != BigInteger.ONE ||
                    networkId.bytes().contentEquals(snapshot.finalizedBlockHash())) &&
                (storageFinalizedHeight != snapshot.finalizedHeight ||
                    storageFinalizedHash.contentEquals(snapshot.finalizedBlockHash())),
        ) {
            "Musubi exact release snapshot is inconsistent or not finalized"
        }
    }

    fun parseProviderBundleAttestation(
        payload: ByteArray,
    ): MusubiProviderBundleAttestationRecordV1 =
        parseProviderBundleAttestationRecord(
            parse(payload, "Musubi provider-bundle-attestation response"),
            "response",
        )

    fun parseResolverPage(payload: ByteArray): MusubiResolverIndexPageV1 {
        val root = exactObject(
            parse(payload, "Musubi resolver-index response"),
            "response",
            setOf("query", "network_id", "items", "next_cursor", "snapshot"),
        )
        val snapshot = parseSnapshot(root["snapshot"], "response.snapshot")
        val cursor = root["next_cursor"]?.let { parseCursor(it, "response.next_cursor") }
        val items = list(root["items"], "response.items").mapIndexed { index, item ->
            parseResolverRow(item, "response.items[$index]")
        }
        return MusubiResolverIndexPageV1(
            decodeQuery("/v1/musubi/queries/resolver-index", root["query"])
                as MusubiResolverIndexQueryV1,
            NetworkId.parse(string(root["network_id"], "response.network_id")),
            items,
            cursor,
            snapshot,
        ).also { it.requireMatches(it.query) }
    }

    fun parseVersionPage(payload: ByteArray): MusubiPageV1<MusubiVersionV1> {
        val page = parsePage(
            parse(payload, "Musubi versions response"),
            "response",
            "/v1/musubi/queries/versions",
            ::parseVersion,
        )
        require(page.items.zipWithNext().all { (left, right) -> left < right }) {
            "Musubi version page must be sorted and distinct"
        }
        page.requireVersionMatches(page.query as MusubiPackagePageQueryV1)
        return page
    }

    fun parseMaintainerPage(
        payload: ByteArray,
    ): MusubiPageV1<MusubiMaintainerDirectoryEntryV1> {
        val page = parsePage(
            parse(payload, "Musubi maintainers response"),
            "response",
            "/v1/musubi/queries/maintainers",
            ::parseMaintainerDirectoryEntry,
        )
        require(page.items.zipWithNext().all { (left, right) ->
            MusubiValidationV1.compareMaintainerEntries(left, right) < 0
        }) { "Musubi maintainer page must be sorted and distinct" }
        page.requireMaintainerMatches(page.query as MusubiPackagePageQueryV1)
        return page
    }

    fun parseArchiveLocationPage(payload: ByteArray): MusubiArchiveLocationPageV1 {
        val root = exactObject(
            parse(payload, "Musubi archive-locations response"),
            "response",
            setOf("network_id", "archive", "items", "next_cursor", "snapshot"),
        )
        val snapshot = parseSnapshot(root["snapshot"], "response.snapshot")
        val cursor = root["next_cursor"]?.let { parseCursor(it, "response.next_cursor") }
        val items = list(root["items"], "response.items").mapIndexed { index, item ->
            parseArchiveLocation(item, "response.items[$index]")
        }
        return MusubiArchiveLocationPageV1(
            NetworkId.parse(string(root["network_id"], "response.network_id")),
            parseArchiveRecord(root["archive"], "response.archive"),
            items,
            cursor,
            snapshot,
        )
    }

    fun parseArchiveRetentionPage(payload: ByteArray): MusubiArchiveRetentionPageV1 {
        val root = exactObject(
            parse(payload, "Musubi archive-retention response"),
            "response",
            setOf("network_id", "items", "finalized_time_ms", "snapshot"),
        )
        val items = list(root["items"], "response.items").mapIndexed { index, item ->
            parseArchiveRetentionDecision(item, "response.items[$index]")
        }
        return MusubiArchiveRetentionPageV1(
            NetworkId.parse(string(root["network_id"], "response.network_id")),
            items,
            u64(root["finalized_time_ms"], "response.finalized_time_ms"),
            parseSnapshot(root["snapshot"], "response.snapshot"),
        )
    }

    fun parseAlias(payload: ByteArray): MusubiAliasRecordV1 =
        parseAliasRecord(parse(payload, "Musubi alias response"), "response")

    fun parseAliasHistoryPage(payload: ByteArray): MusubiPageV1<MusubiAliasHistoryEntryV1> {
        val page = parsePage(
            parse(payload, "Musubi alias-history response"),
            "response",
            "/v1/musubi/queries/alias-history",
            ::parseAliasHistory,
        )
        require(page.items.zipWithNext().all { (left, right) ->
            val aliasOrder = MusubiValidationV1.compareUtf8(left.alias, right.alias)
            aliasOrder < 0 || aliasOrder == 0 && left.revision < right.revision
        }) { "Musubi alias-history page must be sorted and distinct" }
        page.requireAliasHistoryMatches(page.query as MusubiAliasQueryV1)
        return page
    }

    fun parseOrderedPackagePage(payload: ByteArray): MusubiOrderedPrefixPageV1 {
        val root = exactObject(
            parse(payload, "Musubi ordered-prefix response"),
            "response",
            setOf(
                "query", "network_id", "namespace_binding", "items",
                "next_cursor", "snapshot",
            ),
        )
        val snapshot = parseSnapshot(root["snapshot"], "response.snapshot")
        val cursor = root["next_cursor"]?.let { parseCursor(it, "response.next_cursor") }
        val items = list(root["items"], "response.items").mapIndexed { index, item ->
            parseOrderedEntry(item, "response.items[$index]")
        }
        return MusubiOrderedPrefixPageV1(
            decodeQuery("/v1/musubi/queries/ordered-prefix", root["query"])
                as MusubiOrderedPrefixQueryV1,
            NetworkId.parse(string(root["network_id"], "response.network_id")),
            parseNamespaceBinding(root["namespace_binding"], "response.namespace_binding"),
            items,
            cursor,
            snapshot,
        ).also { it.requireMatches(it.query) }
    }

    fun parseSearchPage(payload: ByteArray): MusubiSearchPageV1 {
        val root = exactObject(
            parse(payload, "Musubi search response"),
            "response",
            setOf("query", "items", "next_cursor", "snapshot"),
        )
        val snapshot = parseSearchSnapshot(root["snapshot"], "response.snapshot")
        val cursor = root["next_cursor"]?.let {
            parseSearchCursor(it, "response.next_cursor")
        }
        val items = list(root["items"], "response.items").mapIndexed { index, item ->
            parseSearchHit(item, "response.items[$index]")
        }
        return MusubiSearchPageV1(
            decodeQuery("/v1/musubi/queries/search", root["query"])
                as MusubiSearchQueryV1,
            items,
            cursor,
            snapshot,
        ).also { it.requireMatches(it.query) }
    }

    fun decodeQuery(path: String, value: Any?): MusubiWireValueV1 {
        require(path in QUERY_PATHS) { "unsupported Musubi V1 query path: $path" }
        return when (path) {
            "/v1/musubi/queries/exact-package" -> {
                val root = exactObject(value, "request", setOf("package"))
                MusubiExactPackageQueryV1(parsePackage(root["package"], "request.package"))
            }
            "/v1/musubi/queries/exact-release" -> {
                val root = exactObject(value, "request", setOf("release"))
                MusubiExactReleaseQueryV1(parseRelease(root["release"], "request.release"))
            }
            "/v1/musubi/queries/provider-bundle-attestation" ->
                parseProviderBundleAttestationKey(value, "request")
            "/v1/musubi/queries/resolver-index" -> {
                val root = exactObject(value, "request", setOf("package", "requirement", "page"))
                MusubiResolverIndexQueryV1(
                    parsePackage(root["package"], "request.package"),
                    root["requirement"]?.let { parseRequirement(it, "request.requirement") },
                    parsePageRequest(root["page"], "request.page"),
                )
            }
            "/v1/musubi/queries/versions", "/v1/musubi/queries/maintainers" -> {
                val root = exactObject(value, "request", setOf("package", "page"))
                MusubiPackagePageQueryV1(
                    parsePackage(root["package"], "request.package"),
                    parsePageRequest(root["page"], "request.page"),
                )
            }
            "/v1/musubi/queries/archive-locations" -> {
                val root = exactObject(value, "request", setOf("archive_id", "page"))
                MusubiArchiveLocationQueryV1(
                    digest(root["archive_id"], "request.archive_id"),
                    parsePageRequest(root["page"], "request.page"),
                )
            }
            "/v1/musubi/queries/archive-retention" -> {
                val root = exactObject(
                    value,
                    "request",
                    setOf("archive_ids", "expected_snapshot"),
                )
                MusubiArchiveRetentionQueryV1(
                    list(root["archive_ids"], "request.archive_ids").mapIndexed { index, item ->
                        digest(item, "request.archive_ids[$index]")
                    },
                    root["expected_snapshot"]?.let {
                        parseSnapshot(it, "request.expected_snapshot")
                    },
                )
            }
            "/v1/musubi/queries/alias", "/v1/musubi/queries/alias-history" -> {
                val root = exactObject(value, "request", setOf("alias", "page"))
                MusubiAliasQueryV1(
                    newtypeText(root["alias"], "request.alias"),
                    parsePageRequest(root["page"], "request.page"),
                )
            }
            "/v1/musubi/queries/ordered-prefix" -> {
                val root = exactObject(value, "request", setOf("prefix", "page"))
                MusubiOrderedPrefixQueryV1(
                    newtypeText(root["prefix"], "request.prefix"),
                    parsePageRequest(root["page"], "request.page"),
                )
            }
            else -> {
                val root = exactObject(value, "request", setOf("query", "page"))
                MusubiSearchQueryV1(
                    string(root["query"], "request.query"),
                    parseSearchPageRequest(root["page"], "request.page"),
                )
            }
        }
    }

    fun decodeResponse(path: String, value: Any?): MusubiWireValueV1 {
        val payload = encode(value)
        return when (path) {
            "/v1/musubi/queries/exact-package" -> parseExactPackage(payload)
            "/v1/musubi/queries/exact-release" -> parseExactRelease(payload)
            "/v1/musubi/queries/provider-bundle-attestation" ->
                parseProviderBundleAttestation(payload)
            "/v1/musubi/queries/resolver-index" -> parseResolverPage(payload)
            "/v1/musubi/queries/versions" -> parseVersionPage(payload)
            "/v1/musubi/queries/maintainers" -> parseMaintainerPage(payload)
            "/v1/musubi/queries/archive-locations" -> parseArchiveLocationPage(payload)
            "/v1/musubi/queries/archive-retention" -> parseArchiveRetentionPage(payload)
            "/v1/musubi/queries/alias" -> parseAlias(payload)
            "/v1/musubi/queries/alias-history" -> parseAliasHistoryPage(payload)
            "/v1/musubi/queries/ordered-prefix" -> parseOrderedPackagePage(payload)
            "/v1/musubi/queries/search" -> parseSearchPage(payload)
            else -> throw IllegalArgumentException("unsupported Musubi V1 query path: $path")
        }
    }

    fun immutableObject(value: Map<String, Any?>): Map<String, Any?> =
        Collections.unmodifiableMap(deepCopyObject(value))

    private fun deepCopyObject(value: Map<String, Any?>): Map<String, Any?> {
        val copy = LinkedHashMap<String, Any?>()
        value.forEach { (key, item) -> copy[key] = deepCopy(item) }
        return copy
    }

    private fun deepCopy(value: Any?): Any? = when (value) {
        is Map<*, *> -> immutableObject(objectMap(value, "JSON value"))
        is List<*> -> Collections.unmodifiableList(value.map(::deepCopy))
        else -> value
    }

    private fun parsePackage(value: Any?, field: String): MusubiPackageIdV1 {
        val root = exactObject(value, field, setOf("home_dataspace", "scope", "name"))
        return MusubiPackageIdV1(
            u64(root["home_dataspace"], "$field.home_dataspace"),
            parseScope(root["scope"], "$field.scope"),
            MusubiPackageNameV1(newtypeText(root["name"], "$field.name")),
        )
    }

    private fun parseScope(value: Any?, field: String): MusubiPackageScopeV1 {
        val root = tagged(value, field)
        return when (string(root["kind"], "$field.kind")) {
            "DataspaceRoot" -> {
                require(root["value"] == null) { "$field.value must be null" }
                MusubiPackageScopeV1.dataspaceRoot()
            }
            "Domain" -> MusubiPackageScopeV1.domain(string(root["value"], "$field.value"))
            else -> throw IllegalArgumentException("$field.kind is unsupported in Musubi V1")
        }
    }

    private fun parseNamespaceBinding(value: Any?, field: String): MusubiNamespaceBindingV1 {
        val root = exactObject(
            value,
            field,
            setOf("namespace", "home_dataspace", "scope", "generation"),
        )
        return MusubiNamespaceBindingV1(
            MusubiNamespaceV1(newtypeText(root["namespace"], "$field.namespace")),
            u64(root["home_dataspace"], "$field.home_dataspace"),
            parseScope(root["scope"], "$field.scope"),
            nonZeroU64(root["generation"], "$field.generation"),
        )
    }

    private fun parseSelector(value: Any?, field: String): MusubiPackageSelectorV1 {
        val root = exactObject(value, field, setOf("namespace", "name"))
        return MusubiPackageSelectorV1(
            MusubiNamespaceV1(newtypeText(root["namespace"], "$field.namespace")),
            MusubiPackageNameV1(newtypeText(root["name"], "$field.name")),
        )
    }

    private fun parseRelease(value: Any?, field: String): MusubiReleaseIdV1 {
        val root = exactObject(value, field, setOf("package", "version"))
        return MusubiReleaseIdV1(
            parsePackage(root["package"], "$field.package"),
            parseVersion(root["version"], "$field.version"),
        )
    }

    private fun parsePrerelease(value: Any?, field: String): MusubiPrereleaseIdentifierV1 {
        val root = tagged(value, field)
        return when (string(root["kind"], "$field.kind")) {
            "Numeric" -> MusubiPrereleaseIdentifierV1.numeric(u64(root["value"], "$field.value"))
            "AlphaNumeric" -> MusubiPrereleaseIdentifierV1.alphaNumeric(
                string(root["value"], "$field.value"),
            )
            else -> throw IllegalArgumentException("$field.kind is unsupported in Musubi V1")
        }
    }

    private fun parseVersion(value: Any?, field: String): MusubiVersionV1 {
        val root = exactObject(value, field, setOf("major", "minor", "patch", "prerelease"))
        val prerelease = list(root["prerelease"], "$field.prerelease").mapIndexed { index, item ->
            parsePrerelease(item, "$field.prerelease[$index]")
        }
        return MusubiVersionV1(
            u64(root["major"], "$field.major"),
            u64(root["minor"], "$field.minor"),
            u64(root["patch"], "$field.patch"),
            prerelease,
        )
    }

    private fun parseRequirement(value: Any?, field: String): MusubiVersionReqV1 {
        val root = tagged(value, field)
        return when (string(root["kind"], "$field.kind")) {
            "Any" -> {
                require(root["value"] == null) { "$field.value must be null" }
                MusubiVersionReqV1.fromWire(MusubiVersionReqV1.Kind.ANY)
            }
            "Caret" -> MusubiVersionReqV1.fromWire(
                MusubiVersionReqV1.Kind.CARET,
                version = parseVersion(root["value"], "$field.value"),
            )
            "Tilde" -> MusubiVersionReqV1.fromWire(
                MusubiVersionReqV1.Kind.TILDE,
                version = parseVersion(root["value"], "$field.value"),
            )
            "Exact" -> MusubiVersionReqV1.fromWire(
                MusubiVersionReqV1.Kind.EXACT,
                version = parseVersion(root["value"], "$field.value"),
            )
            "MajorWildcard" -> MusubiVersionReqV1.fromWire(
                MusubiVersionReqV1.Kind.MAJOR_WILDCARD,
                major = u64(root["value"], "$field.value"),
            )
            "MinorWildcard" -> {
                val wildcard = exactObject(root["value"], "$field.value", setOf("major", "minor"))
                MusubiVersionReqV1.fromWire(
                    MusubiVersionReqV1.Kind.MINOR_WILDCARD,
                    major = u64(wildcard["major"], "$field.value.major"),
                    minor = u64(wildcard["minor"], "$field.value.minor"),
                )
            }
            "Comparators" -> {
                val comparators = list(root["value"], "$field.value").mapIndexed { index, item ->
                    parseComparator(item, "$field.value[$index]")
                }
                MusubiVersionReqV1.fromWire(
                    MusubiVersionReqV1.Kind.COMPARATORS,
                    comparators = comparators,
                )
            }
            else -> throw IllegalArgumentException("$field.kind is unsupported in Musubi V1")
        }
    }

    private fun parseComparator(value: Any?, field: String): MusubiVersionComparatorV1 {
        val root = exactObject(value, field, setOf("op", "version"))
        val opValue = tagged(root["op"], "$field.op")
        require(opValue["value"] == null) { "$field.op.value must be null" }
        val op = when (string(opValue["kind"], "$field.op.kind")) {
            "Greater" -> MusubiComparatorOpV1.GREATER
            "GreaterOrEqual" -> MusubiComparatorOpV1.GREATER_OR_EQUAL
            "Less" -> MusubiComparatorOpV1.LESS
            "LessOrEqual" -> MusubiComparatorOpV1.LESS_OR_EQUAL
            "Equal" -> MusubiComparatorOpV1.EQUAL
            else -> throw IllegalArgumentException("$field.op.kind is unsupported")
        }
        return MusubiVersionComparatorV1(op, parseVersion(root["version"], "$field.version"))
    }

    private fun parseSnapshot(value: Any?, field: String): MusubiRegistrySnapshotV1 {
        val root = exactObject(
            value,
            field,
            setOf("finalized_height", "finalized_block_hash", "index_revision"),
        )
        return MusubiRegistrySnapshotV1(
            u64(root["finalized_height"], "$field.finalized_height"),
            fixedBytes(root["finalized_block_hash"], "$field.finalized_block_hash"),
            u64(root["index_revision"], "$field.index_revision"),
        )
    }

    private fun parseCursor(value: Any?, field: String): MusubiFinalizedCursorV1 {
        val root = exactObject(value, field, setOf("snapshot", "query_hash", "last_key", "caller"))
        return MusubiFinalizedCursorV1(
            parseSnapshot(root["snapshot"], "$field.snapshot"),
            digest(root["query_hash"], "$field.query_hash"),
            string(root["last_key"], "$field.last_key"),
            root["caller"]?.let { string(it, "$field.caller") },
        )
    }

    private fun parsePageRequest(value: Any?, field: String): MusubiPageRequestV1 {
        val root = exactObject(value, field, setOf("limit", "cursor"))
        val limit = u32(root["limit"], "$field.limit")
        return MusubiPageRequestV1(
            limit,
            root["cursor"]?.let { parseCursor(it, "$field.cursor") },
        )
    }

    private fun parseSearchSnapshot(value: Any?, field: String): MusubiSearchSnapshotV1 {
        val root = exactObject(
            value,
            field,
            setOf("finalized_height", "finalized_block_hash", "projection_revision"),
        )
        return MusubiSearchSnapshotV1(
            nonZeroU64(root["finalized_height"], "$field.finalized_height"),
            fixedBytes(root["finalized_block_hash"], "$field.finalized_block_hash"),
            nonZeroU64(root["projection_revision"], "$field.projection_revision"),
        )
    }

    private fun parseSearchCursor(value: Any?, field: String): MusubiSearchCursorV1 {
        val root = exactObject(
            value,
            field,
            setOf("snapshot", "query_hash", "last_package"),
        )
        return MusubiSearchCursorV1(
            parseSearchSnapshot(root["snapshot"], "$field.snapshot"),
            digest(root["query_hash"], "$field.query_hash"),
            parsePackage(root["last_package"], "$field.last_package"),
        )
    }

    private fun parseSearchPageRequest(value: Any?, field: String): MusubiSearchPageRequestV1 {
        val root = exactObject(value, field, setOf("limit", "cursor"))
        return MusubiSearchPageRequestV1(
            u32(root["limit"], "$field.limit"),
            root["cursor"]?.let { parseSearchCursor(it, "$field.cursor") },
        )
    }

    private fun parsePackageRecord(value: Any?, field: String): MusubiPackageRecordV1 {
        val root = exactObject(
            value,
            field,
            setOf(
                "package", "claimed_namespace", "claimed_namespace_binding", "owners",
                "member_accounts", "claimed_at_height", "revisions",
            ),
        )
        val revisions = exactObject(
            root["revisions"],
            "$field.revisions",
            setOf("governance", "metadata", "archive_locations"),
        )
        return MusubiPackageRecordV1(
            parsePackage(root["package"], "$field.package"),
            MusubiNamespaceV1(newtypeText(root["claimed_namespace"], "$field.claimed_namespace")),
            digest(root["claimed_namespace_binding"], "$field.claimed_namespace_binding"),
            stringList(root["owners"], "$field.owners"),
            stringList(root["member_accounts"], "$field.member_accounts"),
            u64(root["claimed_at_height"], "$field.claimed_at_height"),
            MusubiPackageRevisionsV1(
                u64(revisions["governance"], "$field.revisions.governance"),
                u64(revisions["metadata"], "$field.revisions.metadata"),
                u64(revisions["archive_locations"], "$field.revisions.archive_locations"),
            ),
        )
    }

    private fun parseReleaseRecord(value: Any?, field: String): MusubiReleaseRecordV1 {
        val root = exactObject(
            value,
            field,
            setOf(
                "manifest", "release_digest", "published_by", "published_at_height", "yank",
                "artifact_governance", "revisions",
            ),
        )
        val manifest = parseManifest(root["manifest"], "$field.manifest")
        val releaseDigest = digest(root["release_digest"], "$field.release_digest")
        val publisher = string(root["published_by"], "$field.published_by")
        val height = nonZeroU64(root["published_at_height"], "$field.published_at_height")
        validateYank(root["yank"], "$field.yank", manifest.release)
        validateGovernance(root["artifact_governance"], "$field.artifact_governance")
        val revisions = exactObject(
            root["revisions"],
            "$field.revisions",
            setOf("yank", "artifact_governance"),
        )
        nonZeroU64(revisions["yank"], "$field.revisions.yank")
        nonZeroU64(revisions["artifact_governance"], "$field.revisions.artifact_governance")
        return MusubiReleaseRecordV1(manifest, releaseDigest, publisher, height, root)
    }

    private fun parseManifest(value: Any?, field: String): MusubiReleaseManifestV1 {
        val root = exactObject(
            value,
            field,
            setOf(
                "release", "edition", "abi", "dependencies", "exports", "interface_digest",
                "metadata", "archive_id", "verification_lock_digest",
            ),
        )
        val release = parseRelease(root["release"], "$field.release")
        taggedUnit(root["edition"], "$field.edition", setOf("V1"))
        val abi = parseAbi(root["abi"], "$field.abi")
        val dependencies =
            list(root["dependencies"], "$field.dependencies").mapIndexed { index, item ->
                parseDependency(item, "$field.dependencies[$index]")
            }
        val exports = stringList(root["exports"], "$field.exports")
        exports.forEach {
            MusubiValidationV1.requireName(it, "Musubi export")
        }
        return MusubiReleaseManifestV1(
            release,
            MusubiKotodamaEditionV1.V1,
            abi,
            dependencies,
            exports,
            digest(root["interface_digest"], "$field.interface_digest"),
            parseMetadata(root["metadata"], "$field.metadata"),
            digest(root["archive_id"], "$field.archive_id"),
            digest(root["verification_lock_digest"], "$field.verification_lock_digest"),
        )
    }

    private fun parseAbi(value: Any?, field: String): MusubiAbiBindingV1 {
        val root = exactObject(value, field, setOf("abi_version", "abi_hash"))
        require(u16(root["abi_version"], "$field.abi_version") == 1) {
            "$field.abi_version is unsupported; Musubi only supports IVM ABI V1"
        }
        return MusubiAbiBindingV1(fixedBytes(root["abi_hash"], "$field.abi_hash"))
    }

    private fun parseDependency(value: Any?, field: String): MusubiDependencyReqV1 {
        val root = exactObject(value, field, setOf("alias", "package", "requirement"))
        return MusubiDependencyReqV1(
            string(root["alias"], "$field.alias"),
            parsePackage(root["package"], "$field.package"),
            parseRequirement(root["requirement"], "$field.requirement"),
        )
    }

    private fun parseMetadata(value: Any?, field: String): MusubiReleaseMetadataV1 {
        val root = exactObject(
            value,
            field,
            setOf("description", "readme", "license", "repository", "keywords"),
        )
        return MusubiReleaseMetadataV1(
            root["description"]?.let {
                MusubiDescriptionV1(newtypeText(it, "$field.description"))
            },
            root["readme"]?.let { MusubiDocumentRefV1(newtypeText(it, "$field.readme")) },
            root["license"]?.let { MusubiDocumentRefV1(newtypeText(it, "$field.license")) },
            root["repository"]?.let {
                MusubiDocumentRefV1(newtypeText(it, "$field.repository"))
            },
            list(root["keywords"], "$field.keywords").mapIndexed { index, item ->
                MusubiKeywordV1(newtypeText(item, "$field.keywords[$index]"))
            },
        )
    }

    private fun validateYank(value: Any?, field: String, expected: MusubiReleaseIdV1) {
        val root = exactObject(
            value,
            field,
            setOf("release", "yanked", "reason", "changed_by", "changed_at_height", "revision"),
        )
        require(parseRelease(root["release"], "$field.release") == expected) {
            "$field.release does not match the manifest release"
        }
        boolean(root["yanked"], "$field.yanked")
        newtypeText(root["reason"], "$field.reason")
        string(root["changed_by"], "$field.changed_by")
        nonZeroU64(root["changed_at_height"], "$field.changed_at_height")
        nonZeroU64(root["revision"], "$field.revision")
    }

    private fun validateGovernance(value: Any?, field: String) {
        val root = tagged(value, field)
        when (string(root["kind"], "$field.kind")) {
            "Available" -> require(root["value"] == null) { "$field.value must be null" }
            "TakenDown" -> {
                val takedown = exactObject(
                    root["value"],
                    "$field.value",
                    setOf("action_digest", "reason", "applied_at_height"),
                )
                digest(takedown["action_digest"], "$field.value.action_digest")
                newtypeText(takedown["reason"], "$field.value.reason")
                nonZeroU64(takedown["applied_at_height"], "$field.value.applied_at_height")
            }
            else -> throw IllegalArgumentException("$field.kind is unsupported in Musubi V1")
        }
    }

    private fun parseResolverRow(value: Any?, field: String): MusubiResolverReleaseRowV1 {
        val root = exactObject(
            value,
            field,
            setOf(
                "release", "release_digest", "archive_id", "source_digest", "interface_digest",
                "abi", "dependencies", "selection", "index_revision",
            ),
        )
        val release = parseRelease(root["release"], "$field.release")
        listOf("release_digest", "archive_id", "source_digest", "interface_digest").forEach {
            digest(root[it], "$field.$it")
        }
        parseAbi(root["abi"], "$field.abi")
        val dependencies =
            list(root["dependencies"], "$field.dependencies").mapIndexed { index, item ->
                parseDependency(item, "$field.dependencies[$index]")
            }
        require(dependencies.size <= 256 &&
            dependencies.zipWithNext().all { (left, right) -> left < right }) {
            "$field.dependencies must be bounded, sorted, and distinct"
        }
        MusubiValidationV1.requireUniqueParentLocalAliases(
            dependencies.map { it.alias },
            "$field.dependencies",
        )
        val storageRevision = validateSelection(root["selection"], "$field.selection", release)
        val revision = nonZeroU64(root["index_revision"], "$field.index_revision")
        return MusubiResolverReleaseRowV1(release, revision, storageRevision, root)
    }

    private fun validateSelection(
        value: Any?,
        field: String,
        release: MusubiReleaseIdV1,
    ): BigInteger {
        val root = exactObject(value, field, setOf("yank", "storage", "governance"))
        validateYank(root["yank"], "$field.yank", release)
        val storage = exactObject(
            root["storage"],
            "$field.storage",
            setOf(
                "archive_id", "availability", "healthy_replicas", "active_locations",
                "finalized_height", "finalized_block_hash", "index_revision",
            ),
        )
        digest(storage["archive_id"], "$field.storage.archive_id")
        taggedUnit(
            storage["availability"],
            "$field.storage.availability",
            setOf("Selectable", "BelowQuorum", "Unavailable"),
        )
        u16(storage["healthy_replicas"], "$field.storage.healthy_replicas")
        u8(storage["active_locations"], "$field.storage.active_locations")
        nonZeroU64(storage["finalized_height"], "$field.storage.finalized_height")
        fixedBytes(storage["finalized_block_hash"], "$field.storage.finalized_block_hash")
        val storageRevision =
            nonZeroU64(storage["index_revision"], "$field.storage.index_revision")
        validateGovernance(root["governance"], "$field.governance")
        return storageRevision
    }

    private fun parseMaintainerDirectoryEntry(
        value: Any?,
        field: String,
    ): MusubiMaintainerDirectoryEntryV1 {
        val root = tagged(value, field)
        return when (string(root["kind"], "$field.kind")) {
            "Accepted" -> MusubiMaintainerDirectoryEntryV1.Accepted(
                parseMember(root["value"], "$field.value"),
            )
            "PendingInvitation" -> MusubiMaintainerDirectoryEntryV1.PendingInvitation(
                parseMaintainerInvitation(root["value"], "$field.value"),
            )
            else -> throw IllegalArgumentException("$field.kind is unsupported in Musubi V1")
        }
    }

    private fun parseMember(value: Any?, field: String): MusubiPackageMemberV1 {
        val root = exactObject(
            value,
            field,
            setOf("package", "account", "role", "accepted_at_height", "governance_revision"),
        )
        val roleKind = parsePackageRole(root["role"], "$field.role")
        val account = string(root["account"], "$field.account")
        MusubiValidationV1.canonicalAccountPayload(account, "$field.account")
        return MusubiPackageMemberV1(
            parsePackage(root["package"], "$field.package"),
            account,
            roleKind,
            nonZeroU64(root["accepted_at_height"], "$field.accepted_at_height"),
            nonZeroU64(root["governance_revision"], "$field.governance_revision"),
            root,
        )
    }

    private fun parseMaintainerInvitation(
        value: Any?,
        field: String,
    ): MusubiMaintainerInvitationV1 {
        val root = exactObject(
            value,
            field,
            setOf(
                "invite_id", "package", "invited_by", "invited_account", "role",
                "expected_governance_revision", "expires_at_height", "state",
            ),
        )
        val inviteId = digest(root["invite_id"], "$field.invite_id")
        require(inviteId.bytes().any { it.toInt() != 0 }) { "$field.invite_id must not be inert" }
        val invitedBy = string(root["invited_by"], "$field.invited_by")
        val invitedAccount = string(root["invited_account"], "$field.invited_account")
        MusubiValidationV1.canonicalAccountPayload(invitedBy, "$field.invited_by")
        MusubiValidationV1.canonicalAccountPayload(invitedAccount, "$field.invited_account")
        val roleKind = parsePackageRole(root["role"], "$field.role")
        val stateKind = taggedUnit(root["state"], "$field.state", setOf("Pending"))
        return MusubiMaintainerInvitationV1(
            inviteId,
            parsePackage(root["package"], "$field.package"),
            invitedBy,
            invitedAccount,
            roleKind,
            nonZeroU64(
                root["expected_governance_revision"],
                "$field.expected_governance_revision",
            ),
            nonZeroU64(root["expires_at_height"], "$field.expires_at_height"),
            stateKind,
            root,
        )
    }

    private fun parsePackageRole(value: Any?, field: String): String {
        val role = tagged(value, field)
        val roleKind = string(role["kind"], "$field.kind")
        when (roleKind) {
            "Owner" -> require(role["value"] == null) { "$field.value must be null" }
            "Maintainer" -> {
                val permissions = exactObject(
                    role["value"],
                    "$field.value",
                    setOf("publish", "yank", "metadata", "archive_locations"),
                )
                val grants = permissions.map { (key, item) ->
                    boolean(item, "$field.value.$key")
                }
                require(grants.any { it }) {
                    "$field.value must grant at least one permission"
                }
            }
            else -> throw IllegalArgumentException("$field.kind is unsupported in Musubi V1")
        }
        return roleKind
    }

    private fun parseArchiveRetentionDecision(
        value: Any?,
        field: String,
    ): MusubiArchiveRetentionDecisionV1 {
        val root = exactObject(
            value,
            field,
            setOf(
                "archive_id", "disposition", "active_releases", "yanked_releases",
                "taken_down_releases", "storage",
            ),
        )
        val disposition = when (
            taggedUnit(
                root["disposition"],
                "$field.disposition",
                setOf(
                    "RetainUnknown", "RetainReferenced", "PruneUnreferenced",
                    "PruneGovernedTakedown",
                ),
            )
        ) {
            "RetainUnknown" -> MusubiArchiveRetentionDispositionV1.RETAIN_UNKNOWN
            "RetainReferenced" -> MusubiArchiveRetentionDispositionV1.RETAIN_REFERENCED
            "PruneUnreferenced" -> MusubiArchiveRetentionDispositionV1.PRUNE_UNREFERENCED
            else -> MusubiArchiveRetentionDispositionV1.PRUNE_GOVERNED_TAKEDOWN
        }
        return MusubiArchiveRetentionDecisionV1(
            digest(root["archive_id"], "$field.archive_id"),
            disposition,
            u16(root["active_releases"], "$field.active_releases"),
            u16(root["yanked_releases"], "$field.yanked_releases"),
            u16(root["taken_down_releases"], "$field.taken_down_releases"),
            root["storage"]?.let { parseArchiveAvailability(it, "$field.storage") },
        )
    }

    private fun parseArchiveAvailability(
        value: Any?,
        field: String,
    ): MusubiArchiveAvailabilityV1 {
        val root = exactObject(
            value,
            field,
            setOf(
                "archive_id", "availability", "healthy_replicas", "active_locations",
                "finalized_height", "finalized_block_hash", "index_revision",
            ),
        )
        val availability = when (
            taggedUnit(
                root["availability"],
                "$field.availability",
                setOf("Selectable", "BelowQuorum", "Unavailable"),
            )
        ) {
            "Selectable" -> MusubiStorageAvailabilityV1.SELECTABLE
            "BelowQuorum" -> MusubiStorageAvailabilityV1.BELOW_QUORUM
            else -> MusubiStorageAvailabilityV1.UNAVAILABLE
        }
        return MusubiArchiveAvailabilityV1(
            digest(root["archive_id"], "$field.archive_id"),
            availability,
            u16(root["healthy_replicas"], "$field.healthy_replicas"),
            u8(root["active_locations"], "$field.active_locations"),
            nonZeroU64(root["finalized_height"], "$field.finalized_height"),
            fixedBytes(root["finalized_block_hash"], "$field.finalized_block_hash"),
            nonZeroU64(root["index_revision"], "$field.index_revision"),
        )
    }

    private fun parseArchiveLocation(value: Any?, field: String): MusubiArchiveLocationV1 {
        val root = exactObject(
            value,
            field,
            setOf(
                "location_id", "archive_id", "pin_manifest", "replication_order", "providers",
                "provider_attestation_set_digest", "renew_after_epoch", "expires_at_epoch",
                "finalized_height", "revision", "state",
            ),
        )
        val location = digest(root["location_id"], "$field.location_id")
        val archive = digest(root["archive_id"], "$field.archive_id")
        val providers = list(root["providers"], "$field.providers").mapIndexed { index, item ->
            newtypeText(item, "$field.providers[$index]")
        }
        val providerAttestationSetDigest = MusubiProviderBundleAttestationSetDigestV1(
            digest(
                root["provider_attestation_set_digest"],
                "$field.provider_attestation_set_digest",
            ).bytes(),
        )
        u64(root["renew_after_epoch"], "$field.renew_after_epoch")
        u64(root["expires_at_epoch"], "$field.expires_at_epoch")
        val finalizedHeight = nonZeroU64(root["finalized_height"], "$field.finalized_height")
        val revision = nonZeroU64(root["revision"], "$field.revision")
        val state = taggedUnit(
            root["state"],
            "$field.state",
            setOf("Pending", "Healthy", "Degraded", "Retired"),
        )
        // Pin manifests and replication-order identifiers are SoraFS-owned JSON newtypes.
        require(root["pin_manifest"] != null && root["replication_order"] != null) {
            "$field must carry pin and replication-order identities"
        }
        return MusubiArchiveLocationV1(
            location,
            archive,
            providers,
            providerAttestationSetDigest,
            finalizedHeight,
            revision,
            state,
            root,
        )
    }

    private fun parseArchiveRecord(value: Any?, field: String): MusubiArchiveRecordV1 {
        val root = exactObject(
            value,
            field,
            setOf(
                "archive_id", "commitment", "staging_receipt", "registered_by",
                "registered_at_height", "location_revision", "location_ids",
            ),
        )
        return MusubiArchiveRecordV1(
            digest(root["archive_id"], "$field.archive_id"),
            parseArchiveCommitment(root["commitment"], "$field.commitment"),
            parseSeedIngressReceipt(root["staging_receipt"], "$field.staging_receipt"),
            string(root["registered_by"], "$field.registered_by"),
            nonZeroU64(root["registered_at_height"], "$field.registered_at_height"),
            nonZeroU64(root["location_revision"], "$field.location_revision"),
            list(root["location_ids"], "$field.location_ids").mapIndexed { index, item ->
                digest(item, "$field.location_ids[$index]")
            },
        )
    }

    private fun parseArchiveCommitment(value: Any?, field: String): MusubiArchiveCommitmentV1 {
        val root = exactObject(
            value,
            field,
            setOf(
                "root_cid", "chunker", "chunk_plan_digest", "por_root", "content_length",
                "car_digest", "car_size", "bundle_digest", "source_tree_digest",
                "descriptor_digest", "file_count", "chunk_count",
            ),
        )
        val chunker = exactObject(
            root["chunker"],
            "$field.chunker",
            setOf("profile_id", "namespace", "name", "semver", "multihash_code"),
        )
        val profile = MusubiChunkerProfileHandleV1(
            u32(chunker["profile_id"], "$field.chunker.profile_id"),
            string(chunker["namespace"], "$field.chunker.namespace"),
            string(chunker["name"], "$field.chunker.name"),
            string(chunker["semver"], "$field.chunker.semver"),
            u64(chunker["multihash_code"], "$field.chunker.multihash_code"),
        )
        return MusubiArchiveCommitmentV1(
            byteArray(root["root_cid"], "$field.root_cid", 36),
            profile,
            digest(root["chunk_plan_digest"], "$field.chunk_plan_digest"),
            digest(root["por_root"], "$field.por_root"),
            u64(root["content_length"], "$field.content_length"),
            digest(root["car_digest"], "$field.car_digest"),
            u64(root["car_size"], "$field.car_size"),
            digest(root["bundle_digest"], "$field.bundle_digest"),
            digest(root["source_tree_digest"], "$field.source_tree_digest"),
            digest(root["descriptor_digest"], "$field.descriptor_digest"),
            u32(root["file_count"], "$field.file_count"),
            u32(root["chunk_count"], "$field.chunk_count"),
        )
    }

    private fun parseSeedIngressReceipt(value: Any?, field: String): MusubiSeedIngressReceiptV1 {
        val root = exactObject(value, field, setOf("payload", "approvals"))
        val payload = exactObject(
            root["payload"],
            "$field.payload",
            setOf("version", "binding", "issued_at_ms", "expires_at_ms"),
        )
        require(u8(payload["version"], "$field.payload.version") == 1) {
            "$field.payload.version is unsupported in Musubi V1"
        }
        val binding = exactObject(
            payload["binding"],
            "$field.payload.binding",
            setOf(
                "network_id", "publisher", "ingress_broker",
                "seed_provider", "semantic_release_manifest_digest", "archive_id",
                "car_body_digest", "car_body_length", "nonce",
            ),
        )
        val typedBinding = MusubiSeedIngressReceiptBindingV1(
            NetworkId.parse(
                string(binding["network_id"], "$field.payload.binding.network_id"),
            ),
            string(binding["publisher"], "$field.payload.binding.publisher"),
            string(binding["ingress_broker"], "$field.payload.binding.ingress_broker"),
            newtypeText(binding["seed_provider"], "$field.payload.binding.seed_provider"),
            digest(
                binding["semantic_release_manifest_digest"],
                "$field.payload.binding.semantic_release_manifest_digest",
            ),
            digest(binding["archive_id"], "$field.payload.binding.archive_id"),
            digest(binding["car_body_digest"], "$field.payload.binding.car_body_digest"),
            u64(binding["car_body_length"], "$field.payload.binding.car_body_length"),
            fixedBytes(binding["nonce"], "$field.payload.binding.nonce"),
        )
        val typedPayload = MusubiSeedIngressReceiptPayloadV1(
            typedBinding,
            u64(payload["issued_at_ms"], "$field.payload.issued_at_ms"),
            u64(payload["expires_at_ms"], "$field.payload.expires_at_ms"),
        )
        val approvals = list(root["approvals"], "$field.approvals").mapIndexed { index, item ->
            val approval = exactObject(
                item,
                "$field.approvals[$index]",
                setOf("public_key", "signature"),
            )
            MusubiSeedIngressReceiptApprovalV1(
                string(approval["public_key"], "$field.approvals[$index].public_key"),
                string(approval["signature"], "$field.approvals[$index].signature"),
            )
        }
        return MusubiSeedIngressReceiptV1(typedPayload, approvals)
    }

    private fun parseProviderBundleAttestationKey(
        value: Any?,
        field: String,
    ): MusubiProviderBundleAttestationKeyV1 {
        val root = exactObject(
            value,
            field,
            setOf("archive_id", "replication_order", "provider_id"),
        )
        return MusubiProviderBundleAttestationKeyV1(
            digest(root["archive_id"], "$field.archive_id"),
            digest(root["replication_order"], "$field.replication_order"),
            newtypeText(root["provider_id"], "$field.provider_id"),
        )
    }

    private fun parseProviderBundleAttestationRecord(
        value: Any?,
        field: String,
    ): MusubiProviderBundleAttestationRecordV1 {
        val root = exactObject(
            value,
            field,
            setOf(
                "key", "attestation_digest", "attestation", "registered_by",
                "registered_at_height",
            ),
        )
        return MusubiProviderBundleAttestationRecordV1(
            parseProviderBundleAttestationKey(root["key"], "$field.key"),
            MusubiProviderBundleAttestationDigestV1(
                digest(root["attestation_digest"], "$field.attestation_digest").bytes(),
            ),
            parseProviderBundleAttestation(root["attestation"], "$field.attestation"),
            string(root["registered_by"], "$field.registered_by"),
            nonZeroU64(root["registered_at_height"], "$field.registered_at_height"),
        )
    }

    private fun parseProviderBundleAttestation(
        value: Any?,
        field: String,
    ): MusubiProviderBundleVerificationAttestationV1 {
        val root = exactObject(value, field, setOf("payload", "approvals"))
        val payload = exactObject(
            root["payload"],
            "$field.payload",
            setOf("version", "binding"),
        )
        require(u8(payload["version"], "$field.payload.version") == 1) {
            "$field.payload.version is unsupported in Musubi V1"
        }
        val binding = exactObject(
            payload["binding"],
            "$field.payload.binding",
            setOf(
                "network_id", "provider_id", "completed_by",
                "completion_authority", "replication_order", "assignment_revision",
                "completion_epoch", "finalized_anchor", "archive_id", "bundle_digest",
                "descriptor_digest", "semantic_release_manifest_digest",
                "verification_lock_digest", "source_tree_digest",
            ),
        )
        val authority = exactObject(
            binding["completion_authority"],
            "$field.payload.binding.completion_authority",
            setOf("provider_owner", "signer_policy"),
        )
        val signerPolicy = exactObject(
            authority["signer_policy"],
            "$field.payload.binding.completion_authority.signer_policy",
            setOf("policy_id", "revision", "predecessor_digest", "policy_digest"),
        )
        val finalizedAnchor = exactObject(
            binding["finalized_anchor"],
            "$field.payload.binding.finalized_anchor",
            setOf("height", "block_hash"),
        )
        val typedBinding = MusubiProviderBundleVerificationBindingV1(
            NetworkId.parse(
                string(binding["network_id"], "$field.payload.binding.network_id"),
            ),
            newtypeText(binding["provider_id"], "$field.payload.binding.provider_id"),
            string(binding["completed_by"], "$field.payload.binding.completed_by"),
            MusubiProviderIngestCompletionAuthorityV1(
                string(
                    authority["provider_owner"],
                    "$field.payload.binding.completion_authority.provider_owner",
                ),
                MusubiProviderIngestCompletionSignerPolicyV1(
                    fixedBytes(
                        signerPolicy["policy_id"],
                        "$field.payload.binding.completion_authority.signer_policy.policy_id",
                    ),
                    nonZeroU64(
                        signerPolicy["revision"],
                        "$field.payload.binding.completion_authority.signer_policy.revision",
                    ),
                    signerPolicy["predecessor_digest"]?.let {
                        fixedBytes(
                            it,
                            "$field.payload.binding.completion_authority.signer_policy." +
                                "predecessor_digest",
                        )
                    },
                    fixedBytes(
                        signerPolicy["policy_digest"],
                        "$field.payload.binding.completion_authority.signer_policy.policy_digest",
                    ),
                ),
            ),
            digest(binding["replication_order"], "$field.payload.binding.replication_order"),
            nonZeroU64(
                binding["assignment_revision"],
                "$field.payload.binding.assignment_revision",
            ),
            nonZeroU64(binding["completion_epoch"], "$field.payload.binding.completion_epoch"),
            MusubiProviderIngestFinalizedAnchorV1(
                nonZeroU64(
                    finalizedAnchor["height"],
                    "$field.payload.binding.finalized_anchor.height",
                ),
                fixedBytes(
                    finalizedAnchor["block_hash"],
                    "$field.payload.binding.finalized_anchor.block_hash",
                ),
            ),
            digest(binding["archive_id"], "$field.payload.binding.archive_id"),
            digest(binding["bundle_digest"], "$field.payload.binding.bundle_digest"),
            digest(binding["descriptor_digest"], "$field.payload.binding.descriptor_digest"),
            digest(
                binding["semantic_release_manifest_digest"],
                "$field.payload.binding.semantic_release_manifest_digest",
            ),
            digest(
                binding["verification_lock_digest"],
                "$field.payload.binding.verification_lock_digest",
            ),
            digest(binding["source_tree_digest"], "$field.payload.binding.source_tree_digest"),
        )
        val approvals = list(root["approvals"], "$field.approvals").mapIndexed { index, item ->
            val approvalField = "$field.approvals[$index]"
            val approval = exactObject(
                item,
                approvalField,
                setOf("public_key", "signature"),
            )
            MusubiProviderBundleVerificationApprovalV1(
                string(approval["public_key"], "$approvalField.public_key"),
                string(approval["signature"], "$approvalField.signature"),
            )
        }
        return MusubiProviderBundleVerificationAttestationV1(
            MusubiProviderBundleVerificationPayloadV1(typedBinding),
            approvals,
        )
    }

    private fun parseAliasRecord(value: Any?, field: String): MusubiAliasRecordV1 {
        val root = exactObject(
            value,
            field,
            setOf(
                "alias", "target", "registered_by", "pricing_revision", "paid_xor",
                "registered_at_height", "history_revision",
            ),
        )
        return MusubiAliasRecordV1(
            newtypeText(root["alias"], "$field.alias"),
            parsePackage(root["target"], "$field.target"),
            string(root["registered_by"], "$field.registered_by"),
            nonZeroU64(root["pricing_revision"], "$field.pricing_revision"),
            u64(root["paid_xor"], "$field.paid_xor"),
            nonZeroU64(root["registered_at_height"], "$field.registered_at_height"),
            nonZeroU64(root["history_revision"], "$field.history_revision"),
        )
    }

    private fun parseAliasHistory(value: Any?, field: String): MusubiAliasHistoryEntryV1 {
        val root = exactObject(
            value,
            field,
            setOf(
                "alias", "revision", "action", "previous_target", "target", "governance_action",
                "finalized_height",
            ),
        )
        return MusubiAliasHistoryEntryV1(
            newtypeText(root["alias"], "$field.alias"),
            nonZeroU64(root["revision"], "$field.revision"),
            taggedUnit(
                root["action"],
                "$field.action",
                setOf("Registered", "ParliamentRetarget"),
            ),
            root["previous_target"]?.let { parsePackage(it, "$field.previous_target") },
            parsePackage(root["target"], "$field.target"),
            root["governance_action"]?.let { digest(it, "$field.governance_action") },
            nonZeroU64(root["finalized_height"], "$field.finalized_height"),
        )
    }

    private fun parseOrderedEntry(value: Any?, field: String): MusubiOrderedPackageEntryV1 {
        val root = exactObject(
            value,
            field,
            setOf("selector", "package", "latest_selectable", "metadata_revision", "index_revision"),
        )
        return MusubiOrderedPackageEntryV1(
            parseSelector(root["selector"], "$field.selector"),
            parsePackage(root["package"], "$field.package"),
            root["latest_selectable"]?.let { parseVersion(it, "$field.latest_selectable") },
            nonZeroU64(root["metadata_revision"], "$field.metadata_revision"),
            nonZeroU64(root["index_revision"], "$field.index_revision"),
        )
    }

    private fun parseSearchHit(value: Any?, field: String): MusubiSearchHitV1 {
        val root = exactObject(
            value,
            field,
            setOf(
                "package", "claimed_namespace", "description", "keywords", "metadata_revision",
            ),
        )
        return MusubiSearchHitV1(
            parsePackage(root["package"], "$field.package"),
            MusubiNamespaceV1(
                newtypeText(root["claimed_namespace"], "$field.claimed_namespace"),
            ),
            root["description"]?.let { newtypeText(it, "$field.description") },
            list(root["keywords"], "$field.keywords").mapIndexed { index, item ->
                newtypeText(item, "$field.keywords[$index]")
            },
            nonZeroU64(root["metadata_revision"], "$field.metadata_revision"),
        )
    }

    private fun <T : MusubiWireValueV1> parsePage(
        value: Any?,
        field: String,
        queryPath: String,
        parser: (Any?, String) -> T,
    ): MusubiPageV1<T> {
        val root = exactObject(value, field, setOf("query", "items", "next_cursor", "snapshot"))
        val items = list(root["items"], "$field.items").mapIndexed { index, item ->
            parser(item, "$field.items[$index]")
        }
        val snapshot = parseSnapshot(root["snapshot"], "$field.snapshot")
        val cursor = root["next_cursor"]?.let { parseCursor(it, "$field.next_cursor") }
        return MusubiPageV1(decodeQuery(queryPath, root["query"]), items, cursor, snapshot)
    }

    private fun digest(value: Any?, field: String): MusubiDigest32V1 {
        val wrapper = list(value, field)
        require(wrapper.size == 1) { "$field must contain one Norito newtype item" }
        return MusubiDigest32V1(fixedBytes(wrapper[0], "$field[0]"))
    }

    private fun fixedBytes(value: Any?, field: String): ByteArray {
        return byteArray(value, field, 32)
    }

    private fun byteArray(value: Any?, field: String, size: Int): ByteArray {
        val bytes = list(value, field)
        require(bytes.size == size) { "$field must contain exactly $size bytes" }
        return ByteArray(size) { index -> u8(bytes[index], "$field[$index]").toByte() }
    }

    private fun newtypeText(value: Any?, field: String): String {
        val wrapper = list(value, field)
        require(wrapper.size == 1) { "$field must contain one Norito newtype item" }
        return string(wrapper[0], "$field[0]")
    }

    private fun tagged(value: Any?, field: String): Map<String, Any?> =
        exactObject(value, field, setOf("kind", "value"))

    private fun taggedUnit(value: Any?, field: String, allowed: Set<String>): String {
        val root = tagged(value, field)
        val kind = string(root["kind"], "$field.kind")
        require(kind in allowed) { "$field.kind is unsupported in Musubi V1" }
        require(root["value"] == null) { "$field.value must be null" }
        return kind
    }

    private fun exactObject(value: Any?, field: String, keys: Set<String>): Map<String, Any?> {
        val root = objectMap(value, field)
        require(root.keys == keys) { "$field contains unknown or missing fields" }
        return root
    }

    @Suppress("UNCHECKED_CAST")
    private fun objectMap(value: Any?, field: String): Map<String, Any?> {
        val root = value as? Map<*, *> ?: throw IllegalArgumentException("$field must be an object")
        require(root.keys.all { it is String }) { "$field keys must be strings" }
        return root as Map<String, Any?>
    }

    @Suppress("UNCHECKED_CAST")
    private fun list(value: Any?, field: String): List<Any?> =
        value as? List<Any?> ?: throw IllegalArgumentException("$field must be an array")

    private fun string(value: Any?, field: String): String =
        value as? String ?: throw IllegalArgumentException("$field must be a string")

    private fun stringList(value: Any?, field: String): List<String> =
        list(value, field).mapIndexed { index, item -> string(item, "$field[$index]") }

    private fun boolean(value: Any?, field: String): Boolean =
        value as? Boolean ?: throw IllegalArgumentException("$field must be a boolean")

    private fun integer(value: Any?, field: String): BigInteger = when (value) {
        is BigInteger -> value
        is Byte -> BigInteger.valueOf(value.toLong())
        is Short -> BigInteger.valueOf(value.toLong())
        is Int -> BigInteger.valueOf(value.toLong())
        is Long -> BigInteger.valueOf(value)
        else -> throw IllegalArgumentException("$field must be an integer")
    }

    private fun u64(value: Any?, field: String): BigInteger = integer(value, field).also {
        MusubiValidationV1.requireU64(it, field)
    }

    private fun nonZeroU64(value: Any?, field: String): BigInteger = u64(value, field).also {
        require(it > BigInteger.ZERO) { "$field must be non-zero" }
    }

    private fun u32(value: Any?, field: String): Long = integer(value, field).let {
        require(it >= BigInteger.ZERO && it <= BigInteger("4294967295")) { "$field must fit u32" }
        it.toLong()
    }

    private fun u16(value: Any?, field: String): Int = integer(value, field).let {
        require(it >= BigInteger.ZERO && it <= BigInteger.valueOf(65_535)) { "$field must fit u16" }
        it.toInt()
    }

    private fun u8(value: Any?, field: String): Int = integer(value, field).let {
        require(it >= BigInteger.ZERO && it <= BigInteger.valueOf(255)) { "$field must fit u8" }
        it.toInt()
    }
}
