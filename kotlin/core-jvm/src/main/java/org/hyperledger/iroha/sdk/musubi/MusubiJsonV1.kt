package org.hyperledger.iroha.sdk.musubi

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.util.Collections
import java.util.LinkedHashMap
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser

/** Strict Norito-JSON codec for the first-release Musubi read surface. */
internal object MusubiJsonV1 {
    private val QUERY_PATHS = setOf(
        "/v1/musubi/queries/exact-package",
        "/v1/musubi/queries/exact-release",
        "/v1/musubi/queries/resolver-index",
        "/v1/musubi/queries/versions",
        "/v1/musubi/queries/maintainers",
        "/v1/musubi/queries/archive-locations",
        "/v1/musubi/queries/alias",
        "/v1/musubi/queries/alias-history",
        "/v1/musubi/queries/ordered-prefix",
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

    fun parseExactRelease(payload: ByteArray): MusubiReleaseRecordV1 =
        parseReleaseRecord(parse(payload, "Musubi exact-release response"), "response")

    fun parseResolverPage(payload: ByteArray): MusubiResolverIndexPageV1 {
        val root = exactObject(
            parse(payload, "Musubi resolver-index response"),
            "response",
            setOf("chain_id", "genesis_hash", "items", "next_cursor", "snapshot"),
        )
        val snapshot = parseSnapshot(root["snapshot"], "response.snapshot")
        val cursor = root["next_cursor"]?.let { parseCursor(it, "response.next_cursor") }
        val items = list(root["items"], "response.items").mapIndexed { index, item ->
            parseResolverRow(item, "response.items[$index]")
        }
        return MusubiResolverIndexPageV1(
            string(root["chain_id"], "response.chain_id"),
            fixedBytes(root["genesis_hash"], "response.genesis_hash"),
            items,
            cursor,
            snapshot,
        )
    }

    fun parseVersionPage(payload: ByteArray): MusubiPageV1<MusubiVersionV1> =
        parsePage(parse(payload, "Musubi versions response"), "response", ::parseVersion)

    fun parseMaintainerPage(payload: ByteArray): MusubiPageV1<MusubiPackageMemberV1> =
        parsePage(parse(payload, "Musubi maintainers response"), "response", ::parseMember)

    fun parseArchiveLocationPage(payload: ByteArray): MusubiPageV1<MusubiArchiveLocationV1> =
        parsePage(
            parse(payload, "Musubi archive-locations response"),
            "response",
            ::parseArchiveLocation,
        )

    fun parseAlias(payload: ByteArray): MusubiAliasRecordV1 =
        parseAliasRecord(parse(payload, "Musubi alias response"), "response")

    fun parseAliasHistoryPage(payload: ByteArray): MusubiPageV1<MusubiAliasHistoryEntryV1> =
        parsePage(parse(payload, "Musubi alias-history response"), "response", ::parseAliasHistory)

    fun parseOrderedPackagePage(payload: ByteArray): MusubiOrderedPrefixPageV1 {
        val root = exactObject(
            parse(payload, "Musubi ordered-prefix response"),
            "response",
            setOf("chain_id", "genesis_hash", "items", "next_cursor", "snapshot"),
        )
        val snapshot = parseSnapshot(root["snapshot"], "response.snapshot")
        val cursor = root["next_cursor"]?.let { parseCursor(it, "response.next_cursor") }
        val items = list(root["items"], "response.items").mapIndexed { index, item ->
            parseOrderedEntry(item, "response.items[$index]")
        }
        return MusubiOrderedPrefixPageV1(
            string(root["chain_id"], "response.chain_id"),
            fixedBytes(root["genesis_hash"], "response.genesis_hash"),
            items,
            cursor,
            snapshot,
        )
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
            "/v1/musubi/queries/alias", "/v1/musubi/queries/alias-history" -> {
                val root = exactObject(value, "request", setOf("alias", "page"))
                MusubiAliasQueryV1(
                    newtypeText(root["alias"], "request.alias"),
                    parsePageRequest(root["page"], "request.page"),
                )
            }
            else -> {
                val root = exactObject(value, "request", setOf("prefix", "page"))
                MusubiOrderedPrefixQueryV1(
                    newtypeText(root["prefix"], "request.prefix"),
                    parsePageRequest(root["page"], "request.page"),
                )
            }
        }
    }

    fun decodeResponse(path: String, value: Any?): MusubiWireValueV1 {
        val payload = encode(value)
        return when (path) {
            "/v1/musubi/queries/exact-package" -> parseExactPackage(payload)
            "/v1/musubi/queries/exact-release" -> parseExactRelease(payload)
            "/v1/musubi/queries/resolver-index" -> parseResolverPage(payload)
            "/v1/musubi/queries/versions" -> parseVersionPage(payload)
            "/v1/musubi/queries/maintainers" -> parseMaintainerPage(payload)
            "/v1/musubi/queries/archive-locations" -> parseArchiveLocationPage(payload)
            "/v1/musubi/queries/alias" -> parseAlias(payload)
            "/v1/musubi/queries/alias-history" -> parseAliasHistoryPage(payload)
            "/v1/musubi/queries/ordered-prefix" -> parseOrderedPackagePage(payload)
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
        val release = validateManifest(root["manifest"], "$field.manifest")
        digest(root["release_digest"], "$field.release_digest")
        val publisher = string(root["published_by"], "$field.published_by")
        val height = u64(root["published_at_height"], "$field.published_at_height")
        validateYank(root["yank"], "$field.yank", release)
        validateGovernance(root["artifact_governance"], "$field.artifact_governance")
        val revisions = exactObject(
            root["revisions"],
            "$field.revisions",
            setOf("yank", "artifact_governance"),
        )
        nonZeroU64(revisions["yank"], "$field.revisions.yank")
        nonZeroU64(revisions["artifact_governance"], "$field.revisions.artifact_governance")
        return MusubiReleaseRecordV1(release, publisher, height, root)
    }

    private fun validateManifest(value: Any?, field: String): MusubiReleaseIdV1 {
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
        validateAbi(root["abi"], "$field.abi")
        list(root["dependencies"], "$field.dependencies").forEachIndexed { index, item ->
            validateDependency(item, "$field.dependencies[$index]")
        }
        stringList(root["exports"], "$field.exports").forEach {
            MusubiValidationV1.requireName(it, "Musubi export")
        }
        digest(root["interface_digest"], "$field.interface_digest")
        validateMetadata(root["metadata"], "$field.metadata")
        digest(root["archive_id"], "$field.archive_id")
        digest(root["verification_lock_digest"], "$field.verification_lock_digest")
        return release
    }

    private fun validateAbi(value: Any?, field: String) {
        val root = exactObject(value, field, setOf("abi_version", "abi_hash"))
        require(u16(root["abi_version"], "$field.abi_version") == 1) {
            "$field.abi_version is unsupported; Musubi only supports IVM ABI V1"
        }
        fixedBytes(root["abi_hash"], "$field.abi_hash")
    }

    private fun validateDependency(value: Any?, field: String) {
        val root = exactObject(value, field, setOf("alias", "package", "requirement"))
        MusubiValidationV1.requireName(string(root["alias"], "$field.alias"), "$field.alias")
        parsePackage(root["package"], "$field.package")
        parseRequirement(root["requirement"], "$field.requirement")
    }

    private fun validateMetadata(value: Any?, field: String) {
        val root = exactObject(
            value,
            field,
            setOf("description", "readme", "license", "repository", "keywords"),
        )
        root["description"]?.let { newtypeText(it, "$field.description") }
        root["readme"]?.let { newtypeText(it, "$field.readme") }
        root["license"]?.let { newtypeText(it, "$field.license") }
        root["repository"]?.let { newtypeText(it, "$field.repository") }
        list(root["keywords"], "$field.keywords").forEachIndexed { index, item ->
            MusubiValidationV1.requireAsciiKebab(
                newtypeText(item, "$field.keywords[$index]"),
                64,
                "keyword",
            )
        }
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
                    setOf("action_digest", "reason", "enacted_at_height"),
                )
                digest(takedown["action_digest"], "$field.value.action_digest")
                newtypeText(takedown["reason"], "$field.value.reason")
                nonZeroU64(takedown["enacted_at_height"], "$field.value.enacted_at_height")
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
        validateAbi(root["abi"], "$field.abi")
        list(root["dependencies"], "$field.dependencies").forEachIndexed { index, item ->
            validateDependency(item, "$field.dependencies[$index]")
        }
        validateSelection(root["selection"], "$field.selection", release)
        val revision = nonZeroU64(root["index_revision"], "$field.index_revision")
        return MusubiResolverReleaseRowV1(release, revision, root)
    }

    private fun validateSelection(value: Any?, field: String, release: MusubiReleaseIdV1) {
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
        nonZeroU64(storage["index_revision"], "$field.storage.index_revision")
        validateGovernance(root["governance"], "$field.governance")
    }

    private fun parseMember(value: Any?, field: String): MusubiPackageMemberV1 {
        val root = exactObject(
            value,
            field,
            setOf("package", "account", "role", "accepted_at_height", "governance_revision"),
        )
        val role = tagged(root["role"], "$field.role")
        val roleKind = string(role["kind"], "$field.role.kind")
        when (roleKind) {
            "Owner" -> require(role["value"] == null) { "$field.role.value must be null" }
            "Maintainer" -> {
                val permissions = exactObject(
                    role["value"],
                    "$field.role.value",
                    setOf("publish", "yank", "metadata", "archive_locations"),
                )
                permissions.forEach { (key, item) -> boolean(item, "$field.role.value.$key") }
            }
            else -> throw IllegalArgumentException("$field.role.kind is unsupported in Musubi V1")
        }
        return MusubiPackageMemberV1(
            parsePackage(root["package"], "$field.package"),
            string(root["account"], "$field.account"),
            roleKind,
            nonZeroU64(root["accepted_at_height"], "$field.accepted_at_height"),
            nonZeroU64(root["governance_revision"], "$field.governance_revision"),
            root,
        )
    }

    private fun parseArchiveLocation(value: Any?, field: String): MusubiArchiveLocationV1 {
        val root = exactObject(
            value,
            field,
            setOf(
                "location_id", "archive_id", "pin_manifest", "replication_order", "providers",
                "provider_attestations", "renew_after_epoch", "expires_at_epoch",
                "finalized_height", "revision", "state",
            ),
        )
        val location = digest(root["location_id"], "$field.location_id")
        val archive = digest(root["archive_id"], "$field.archive_id")
        list(root["providers"], "$field.providers")
        list(root["provider_attestations"], "$field.provider_attestations")
        u64(root["renew_after_epoch"], "$field.renew_after_epoch")
        u64(root["expires_at_epoch"], "$field.expires_at_epoch")
        nonZeroU64(root["finalized_height"], "$field.finalized_height")
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
        return MusubiArchiveLocationV1(location, archive, revision, state, root)
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

    private fun <T : MusubiWireValueV1> parsePage(
        value: Any?,
        field: String,
        parser: (Any?, String) -> T,
    ): MusubiPageV1<T> {
        val root = exactObject(value, field, setOf("items", "next_cursor", "snapshot"))
        val items = list(root["items"], "$field.items").mapIndexed { index, item ->
            parser(item, "$field.items[$index]")
        }
        val snapshot = parseSnapshot(root["snapshot"], "$field.snapshot")
        val cursor = root["next_cursor"]?.let { parseCursor(it, "$field.next_cursor") }
        return MusubiPageV1(items, cursor, snapshot)
    }

    private fun digest(value: Any?, field: String): MusubiDigest32V1 {
        val wrapper = list(value, field)
        require(wrapper.size == 1) { "$field must contain one Norito newtype item" }
        return MusubiDigest32V1(fixedBytes(wrapper[0], "$field[0]"))
    }

    private fun fixedBytes(value: Any?, field: String): ByteArray {
        val bytes = list(value, field)
        require(bytes.size == 32) { "$field must contain exactly 32 bytes" }
        return ByteArray(32) { index -> u8(bytes[index], "$field[$index]").toByte() }
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
