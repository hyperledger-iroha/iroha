package org.hyperledger.iroha.sdk.sorafs

import org.hyperledger.iroha.sdk.client.JsonParser
import java.math.BigInteger
import java.nio.charset.StandardCharsets

/** Strict parser for the closed V1 SoraFS reputation JSON projections. */
internal object SorafsReputationJsonParser {
    private val U64_MAX = BigInteger("18446744073709551615")
    private val SNAPSHOT_FIELDS = setOf(
        "snapshot_id_hex",
        "generated_at_unix",
        "previous_snapshot_id_hex",
        "merkle_root_hex",
        "provider_count",
        "returned_provider_count",
        "limit",
        "truncated_providers",
        "alpha_bps",
        "current_score_weight_bps",
        "weights",
        "providers",
    )
    private val PROVIDER_RESPONSE_FIELDS = setOf(
        "snapshot_id_hex",
        "generated_at_unix",
        "merkle_root_hex",
        "provider",
        "proof",
    )
    private val WEIGHTS_RESPONSE_FIELDS = setOf(
        "snapshot_id_hex",
        "generated_at_unix",
        "alpha_bps",
        "current_score_weight_bps",
        "weights",
    )
    private val WEIGHTS_FIELDS = setOf(
        "version",
        "por_success_bps",
        "pdp_success_bps",
        "potr_success_bps",
        "latency_bps",
        "dispute_bps",
        "token_violation_bps",
        "repair_breach_bps",
    )
    private val PROVIDER_FIELDS = setOf(
        "provider_id",
        "score_bps",
        "degradation_flags",
        "raw_metrics",
        "raw_metrics_hash_hex",
    )
    private val PROVIDER_METRICS_FIELDS = setOf(
        "version",
        "por_success_bps",
        "pdp_success_bps",
        "potr_success_bps",
        "latency_health_bps",
        "dispute_rate_bps",
        "token_violation_rate_bps",
        "repair_breach_rate_bps",
    )
    private val DEGRADATION_FLAG_FIELDS = setOf("flag", "value")
    private val PROOF_FIELDS = setOf("provider_id", "leaf_index", "leaf_count", "siblings_hex")
    private val EVENT_FIELDS = setOf(
        "version",
        "sequence",
        "snapshot_id_hex",
        "generated_at_unix",
        "merkle_root_hex",
        "provider_count",
        "previous_snapshot_id_hex",
    )
    private val EVENT_PAGE_FIELDS = setOf("since", "limit", "count", "next_since", "events")
    private val DEGRADATION_FLAG_ORDER = listOf(
        "reserve_warning",
        "reserve_grace",
        "reserve_delinquent",
        "reserve_default",
        "proof_success_below90",
        "proof_success_below80",
        "active_dispute",
        "slashing_event",
        "low_score",
    )
    private val DEGRADATION_FLAGS = DEGRADATION_FLAG_ORDER.toSet()

    fun parseSnapshot(payload: ByteArray): SorafsReputationSnapshotSummaryV1 {
        val root = rootObject(payload, "SoraFS reputation snapshot")
        exactFields(root, SNAPSHOT_FIELDS, "SoraFS reputation snapshot")
        val providers = list(root["providers"], "SoraFS reputation snapshot.providers")
            .mapIndexed { index, value ->
                parseProvider(
                    objectValue(value, "SoraFS reputation snapshot.providers[$index]"),
                    "SoraFS reputation snapshot.providers[$index]",
                )
            }
        check(
            providers.zipWithNext().all { (previous, current) ->
                previous.providerId < current.providerId
            },
        ) {
            "SoraFS reputation snapshot.providers must be strictly ordered by provider_id"
        }
        val providerCount = boundedInt(
            root["provider_count"],
            "SoraFS reputation snapshot.provider_count",
            1,
            65_536,
        )
        val returnedProviderCount = boundedInt(
            root["returned_provider_count"],
            "SoraFS reputation snapshot.returned_provider_count",
            1,
            500,
        )
        val limit = boundedInt(root["limit"], "SoraFS reputation snapshot.limit", 1, 500)
        check(returnedProviderCount == providers.size) {
            "SoraFS reputation snapshot.returned_provider_count must equal providers.length"
        }
        check(returnedProviderCount == minOf(providerCount, limit)) {
            "SoraFS reputation snapshot.returned_provider_count must equal min(provider_count, limit)"
        }
        check(providerCount >= returnedProviderCount) {
            "SoraFS reputation snapshot.provider_count must cover the returned provider prefix"
        }
        val truncated = booleanValue(
            root["truncated_providers"],
            "SoraFS reputation snapshot.truncated_providers",
        )
        check(truncated == (providerCount > returnedProviderCount)) {
            "SoraFS reputation snapshot.truncated_providers is inconsistent with provider counts"
        }
        val snapshotIdHex = snapshotId(
            root["snapshot_id_hex"],
            "SoraFS reputation snapshot.snapshot_id_hex",
        )
        val previousSnapshotIdHex = optionalSnapshotId(
            root["previous_snapshot_id_hex"],
            "SoraFS reputation snapshot.previous_snapshot_id_hex",
        )
        check(previousSnapshotIdHex != snapshotIdHex) {
            "SoraFS reputation snapshot.previous_snapshot_id_hex must differ from snapshot_id_hex"
        }
        return SorafsReputationSnapshotSummaryV1(
            snapshotIdHex = snapshotIdHex,
            generatedAtUnix = positiveU64(
                root["generated_at_unix"],
                "SoraFS reputation snapshot.generated_at_unix",
            ),
            previousSnapshotIdHex = previousSnapshotIdHex,
            merkleRootHex = digest(root["merkle_root_hex"], "SoraFS reputation snapshot.merkle_root_hex"),
            providerCount = providerCount,
            returnedProviderCount = returnedProviderCount,
            limit = limit,
            truncatedProviders = truncated,
            alphaBps = exactInt(root["alpha_bps"], "SoraFS reputation snapshot.alpha_bps", 8_500),
            currentScoreWeightBps = exactInt(
                root["current_score_weight_bps"],
                "SoraFS reputation snapshot.current_score_weight_bps",
                7_000,
            ),
            weights = parseWeights(
                objectValue(root["weights"], "SoraFS reputation snapshot.weights"),
                "SoraFS reputation snapshot.weights",
            ),
            providers = providers,
        )
    }

    fun parseProviderResponse(payload: ByteArray): SorafsReputationProviderResponseV1 {
        val root = rootObject(payload, "SoraFS reputation provider response")
        exactFields(root, PROVIDER_RESPONSE_FIELDS, "SoraFS reputation provider response")
        val provider = parseProvider(
            objectValue(root["provider"], "SoraFS reputation provider response.provider"),
            "SoraFS reputation provider response.provider",
        )
        val proof = parseProof(
            objectValue(root["proof"], "SoraFS reputation provider response.proof"),
            "SoraFS reputation provider response.proof",
        )
        check(provider.providerId == proof.providerId) {
            "SoraFS reputation provider response proof must reference the returned provider"
        }
        return SorafsReputationProviderResponseV1(
            snapshotIdHex = snapshotId(
                root["snapshot_id_hex"],
                "SoraFS reputation provider response.snapshot_id_hex",
            ),
            generatedAtUnix = positiveU64(
                root["generated_at_unix"],
                "SoraFS reputation provider response.generated_at_unix",
            ),
            merkleRootHex = digest(
                root["merkle_root_hex"],
                "SoraFS reputation provider response.merkle_root_hex",
            ),
            provider = provider,
            proof = proof,
        )
    }

    fun parseWeightsResponse(payload: ByteArray): SorafsReputationWeightsResponseV1 {
        val root = rootObject(payload, "SoraFS reputation weights response")
        exactFields(root, WEIGHTS_RESPONSE_FIELDS, "SoraFS reputation weights response")
        return SorafsReputationWeightsResponseV1(
            snapshotIdHex = snapshotId(
                root["snapshot_id_hex"],
                "SoraFS reputation weights response.snapshot_id_hex",
            ),
            generatedAtUnix = positiveU64(
                root["generated_at_unix"],
                "SoraFS reputation weights response.generated_at_unix",
            ),
            alphaBps = exactInt(
                root["alpha_bps"],
                "SoraFS reputation weights response.alpha_bps",
                8_500,
            ),
            currentScoreWeightBps = exactInt(
                root["current_score_weight_bps"],
                "SoraFS reputation weights response.current_score_weight_bps",
                7_000,
            ),
            weights = parseWeights(
                objectValue(root["weights"], "SoraFS reputation weights response.weights"),
                "SoraFS reputation weights response.weights",
            ),
        )
    }

    fun parseEventPage(payload: ByteArray): SorafsReputationEventsResponseV1 {
        val root = rootObject(payload, "SoraFS reputation events response")
        exactFields(root, EVENT_PAGE_FIELDS, "SoraFS reputation events response")
        val events = list(root["events"], "SoraFS reputation events response.events")
            .mapIndexed { index, value ->
                parseEventObject(
                    objectValue(value, "SoraFS reputation events response.events[$index]"),
                    "SoraFS reputation events response.events[$index]",
                )
            }
        val count = boundedInt(root["count"], "SoraFS reputation events response.count", 0, 500)
        check(count == events.size) {
            "SoraFS reputation events response.count must equal events.length"
        }
        val limit = boundedInt(root["limit"], "SoraFS reputation events response.limit", 1, 500)
        check(count <= limit) {
            "SoraFS reputation events response.count must not exceed limit"
        }
        val nextSince = optionalPositiveU64(
            root["next_since"],
            "SoraFS reputation events response.next_since",
        )
        val expectedNext = events.lastOrNull()?.sequence
        check(nextSince == expectedNext) {
            "SoraFS reputation events response.next_since must equal the last event sequence"
        }
        val since = optionalU64(root["since"], "SoraFS reputation events response.since")
        var previousSequence = BigInteger(since ?: "0")
        for ((index, event) in events.withIndex()) {
            val sequence = BigInteger(event.sequence)
            check(
                if (index == 0) sequence > previousSequence
                else sequence == previousSequence.add(BigInteger.ONE),
            ) {
                "SoraFS reputation events response sequences must increase after since and be contiguous within the page"
            }
            previousSequence = sequence
        }
        for ((previous, current) in events.zipWithNext()) {
            check(current.previousSnapshotIdHex == previous.snapshotIdHex) {
                "SoraFS reputation events response previous_snapshot_id_hex must link adjacent events"
            }
            check(BigInteger(current.generatedAtUnix) > BigInteger(previous.generatedAtUnix)) {
                "SoraFS reputation events response generated_at_unix must strictly increase"
            }
        }
        return SorafsReputationEventsResponseV1(
            since = since,
            limit = limit,
            count = count,
            nextSince = nextSince,
            events = events,
        )
    }

    fun parseEventJson(payload: String): SorafsReputationSnapshotEventV1 {
        check(
            payload.isNotEmpty() &&
                payload == payload.trim() &&
                payload.none(Char::isWhitespace),
        ) {
            "SoraFS reputation SSE snapshot data must be exact compact JSON"
        }
        return parseEventObject(
            objectValue(JsonParser.parse(payload), "SoraFS reputation SSE snapshot data"),
            "SoraFS reputation SSE snapshot data",
        )
    }

    private fun parseWeights(root: Map<String, Any?>, path: String): SorafsReputationWeightsV1 {
        exactFields(root, WEIGHTS_FIELDS, path)
        val weights = SorafsReputationWeightsV1(
            version = exactInt(root["version"], "$path.version", 1),
            porSuccessBps = basisPoints(root["por_success_bps"], "$path.por_success_bps"),
            pdpSuccessBps = basisPoints(root["pdp_success_bps"], "$path.pdp_success_bps"),
            potrSuccessBps = basisPoints(root["potr_success_bps"], "$path.potr_success_bps"),
            latencyBps = basisPoints(root["latency_bps"], "$path.latency_bps"),
            disputeBps = basisPoints(root["dispute_bps"], "$path.dispute_bps"),
            tokenViolationBps = basisPoints(
                root["token_violation_bps"],
                "$path.token_violation_bps",
            ),
            repairBreachBps = basisPoints(root["repair_breach_bps"], "$path.repair_breach_bps"),
        )
        val total = weights.porSuccessBps +
            weights.pdpSuccessBps +
            weights.potrSuccessBps +
            weights.latencyBps +
            weights.disputeBps +
            weights.tokenViolationBps +
            weights.repairBreachBps
        check(total == 10_000) { "$path basis-point fields must sum to exactly 10000" }
        return weights
    }

    private fun parseProvider(root: Map<String, Any?>, path: String): SorafsReputationProviderV1 {
        exactFields(root, PROVIDER_FIELDS, path)
        val flags = list(root["degradation_flags"], "$path.degradation_flags")
            .mapIndexed { index, value ->
                val flagPath = "$path.degradation_flags[$index]"
                val flag = objectValue(value, flagPath)
                exactFields(flag, DEGRADATION_FLAG_FIELDS, flagPath)
                check(flag["value"] == null) { "$flagPath.value must be null" }
                val label = string(flag["flag"], "$flagPath.flag")
                check(label in DEGRADATION_FLAGS) { "$flagPath.flag is unsupported" }
                label
            }
        check(flags.size <= 5 && flags.toSet().size == flags.size) {
            "$path.degradation_flags must be unique and contain at most five entries"
        }
        check(
            flags.zipWithNext().all { (previous, current) ->
                DEGRADATION_FLAG_ORDER.indexOf(previous) <
                    DEGRADATION_FLAG_ORDER.indexOf(current)
            },
        ) {
            "$path.degradation_flags must use canonical enum order"
        }
        return SorafsReputationProviderV1(
            providerId = providerId(root["provider_id"], "$path.provider_id"),
            scoreBps = boundedInt(root["score_bps"], "$path.score_bps", 500, 9_900),
            degradationFlags = flags,
            rawMetrics = parseProviderMetrics(
                objectValue(root["raw_metrics"], "$path.raw_metrics"),
                "$path.raw_metrics",
            ),
            rawMetricsHashHex = digest(root["raw_metrics_hash_hex"], "$path.raw_metrics_hash_hex"),
        )
    }

    private fun parseProviderMetrics(
        root: Map<String, Any?>,
        path: String,
    ): SorafsReputationProviderMetricsV1 {
        exactFields(root, PROVIDER_METRICS_FIELDS, path)
        return SorafsReputationProviderMetricsV1(
            version = exactInt(root["version"], "$path.version", 1),
            porSuccessBps = basisPoints(root["por_success_bps"], "$path.por_success_bps"),
            pdpSuccessBps = basisPoints(root["pdp_success_bps"], "$path.pdp_success_bps"),
            potrSuccessBps = basisPoints(root["potr_success_bps"], "$path.potr_success_bps"),
            latencyHealthBps = basisPoints(root["latency_health_bps"], "$path.latency_health_bps"),
            disputeRateBps = basisPoints(root["dispute_rate_bps"], "$path.dispute_rate_bps"),
            tokenViolationRateBps = basisPoints(
                root["token_violation_rate_bps"],
                "$path.token_violation_rate_bps",
            ),
            repairBreachRateBps = basisPoints(
                root["repair_breach_rate_bps"],
                "$path.repair_breach_rate_bps",
            ),
        )
    }

    private fun parseProof(root: Map<String, Any?>, path: String): SorafsReputationMerkleProofV1 {
        exactFields(root, PROOF_FIELDS, path)
        val leafIndex = boundedInt(root["leaf_index"], "$path.leaf_index", 0, 65_535)
        val leafCount = boundedInt(root["leaf_count"], "$path.leaf_count", 1, 65_536)
        check(leafIndex < leafCount) { "$path.leaf_index must be less than leaf_count" }
        val siblings = list(root["siblings_hex"], "$path.siblings_hex")
            .mapIndexed { index, value -> digest(value, "$path.siblings_hex[$index]") }
        check(siblings.size == merkleDepth(leafCount)) {
            "$path.siblings_hex must have the exact Merkle depth for leaf_count"
        }
        return SorafsReputationMerkleProofV1(
            providerId = providerId(root["provider_id"], "$path.provider_id"),
            leafIndex = leafIndex,
            leafCount = leafCount,
            siblingsHex = siblings,
        )
    }

    private fun parseEventObject(
        root: Map<String, Any?>,
        path: String,
    ): SorafsReputationSnapshotEventV1 {
        exactFields(root, EVENT_FIELDS, path)
        val snapshotIdHex = snapshotId(root["snapshot_id_hex"], "$path.snapshot_id_hex")
        val previousSnapshotIdHex = optionalSnapshotId(
            root["previous_snapshot_id_hex"],
            "$path.previous_snapshot_id_hex",
        )
        check(previousSnapshotIdHex != snapshotIdHex) {
            "$path.previous_snapshot_id_hex must differ from snapshot_id_hex"
        }
        return SorafsReputationSnapshotEventV1(
            version = exactInt(root["version"], "$path.version", 1),
            sequence = positiveU64(root["sequence"], "$path.sequence"),
            snapshotIdHex = snapshotIdHex,
            generatedAtUnix = positiveU64(root["generated_at_unix"], "$path.generated_at_unix"),
            merkleRootHex = digest(root["merkle_root_hex"], "$path.merkle_root_hex"),
            providerCount = boundedInt(root["provider_count"], "$path.provider_count", 1, 65_536),
            previousSnapshotIdHex = previousSnapshotIdHex,
        )
    }

    private fun merkleDepth(leafCount: Int): Int {
        var width = leafCount
        var depth = 0
        while (width > 1) {
            width = (width + 1) / 2
            depth += 1
        }
        return depth
    }

    private fun rootObject(payload: ByteArray, context: String): Map<String, Any?> {
        check(payload.isNotEmpty()) { "$context returned an empty payload" }
        val json = String(payload, StandardCharsets.UTF_8)
        check(json.isNotEmpty() && json == json.trim()) { "$context must be exact JSON" }
        return objectValue(JsonParser.parse(json), context)
    }

    @Suppress("UNCHECKED_CAST")
    private fun objectValue(value: Any?, path: String): Map<String, Any?> {
        check(value is Map<*, *>) { "$path must be a JSON object" }
        check(value.keys.all { it is String }) { "$path must use string keys" }
        return value as Map<String, Any?>
    }

    private fun list(value: Any?, path: String): List<Any?> {
        check(value is List<*>) { "$path must be a JSON array" }
        return value
    }

    private fun exactFields(root: Map<String, Any?>, expected: Set<String>, path: String) {
        check(root.keys == expected) {
            val missing = expected - root.keys
            val extra = root.keys - expected
            "$path fields are not canonical; missing=$missing extra=$extra"
        }
    }

    private fun string(value: Any?, path: String): String {
        check(value is String && value.isNotEmpty() && value == value.trim()) {
            "$path must be an exact non-empty string"
        }
        return value
    }

    private fun providerId(value: Any?, path: String): String {
        val provider = string(value, path)
        check(
            provider.length <= 256 &&
                provider != "." &&
                provider != ".." &&
                provider.all(::isProviderCharacter),
        ) {
            "$path must be 1..256 ASCII characters from [A-Za-z0-9_.:-] and not be a dot segment"
        }
        return provider
    }

    private fun isProviderCharacter(value: Char): Boolean =
        value in 'A'..'Z' ||
            value in 'a'..'z' ||
            value in '0'..'9' ||
            value == '_' ||
            value == '.' ||
            value == ':' ||
            value == '-'

    private fun snapshotId(value: Any?, path: String): String {
        val literal = value as? String
        check(literal != null && literal.length == 32 && literal.all(::isLowerHex)) {
            "$path must be exactly 32 lowercase hexadecimal characters"
        }
        check(literal.any { it != '0' }) { "$path must be nonzero" }
        return literal
    }

    private fun optionalSnapshotId(value: Any?, path: String): String? =
        if (value == null) null else snapshotId(value, path)

    private fun digest(value: Any?, path: String): String {
        val literal = value as? String
        check(literal != null && literal.length == 64 && literal.all(::isLowerHex)) {
            "$path must be exactly 64 lowercase hexadecimal characters"
        }
        return literal
    }

    private fun isLowerHex(value: Char): Boolean = value in '0'..'9' || value in 'a'..'f'

    private fun booleanValue(value: Any?, path: String): Boolean {
        check(value is Boolean) { "$path must be a boolean" }
        return value
    }

    private fun basisPoints(value: Any?, path: String): Int = boundedInt(value, path, 0, 10_000)

    private fun exactInt(value: Any?, path: String, expected: Int): Int {
        val parsed = boundedInt(value, path, expected, expected)
        check(parsed == expected) { "$path must equal $expected" }
        return parsed
    }

    private fun boundedInt(value: Any?, path: String, minimum: Int, maximum: Int): Int {
        val canonical = canonicalU64(value, path, minimum == 0)
        val parsed = BigInteger(canonical)
        check(parsed >= BigInteger.valueOf(minimum.toLong()) &&
            parsed <= BigInteger.valueOf(maximum.toLong())) {
            "$path must be between $minimum and $maximum"
        }
        return parsed.toInt()
    }

    private fun optionalU64(value: Any?, path: String): String? =
        if (value == null) null else canonicalU64(value, path, true)

    private fun optionalPositiveU64(value: Any?, path: String): String? =
        if (value == null) null else canonicalU64(value, path, false)

    private fun positiveU64(value: Any?, path: String): String = canonicalU64(value, path, false)

    private fun canonicalU64(value: Any?, path: String, allowZero: Boolean): String {
        val literal = when (value) {
            is Long -> value.toString()
            is Int -> value.toString()
            is BigInteger -> value.toString()
            else -> throw IllegalStateException("$path must be a canonical unsigned integer")
        }
        check(literal.matches(Regex("(?:0|[1-9][0-9]*)"))) {
            "$path must be a canonical unsigned integer"
        }
        val parsed = BigInteger(literal)
        check(parsed.signum() >= 0 && parsed <= U64_MAX) { "$path must fit canonical u64" }
        check(allowZero || parsed.signum() > 0) { "$path must be positive" }
        return literal
    }
}
