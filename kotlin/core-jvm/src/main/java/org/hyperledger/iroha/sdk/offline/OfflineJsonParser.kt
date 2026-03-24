package org.hyperledger.iroha.sdk.offline

import java.util.Locale
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser

object OfflineJsonParser {

    @JvmStatic
    fun parseAllowances(payload: ByteArray): OfflineAllowanceList {
        val root = parse(payload)
        val obj = expectObject(root, "root")
        val total = asLong(obj["total"], "total")
        val items = asArray(obj["items"], "items")
        val parsed = ArrayList<OfflineAllowanceItem>(items.size)
        for (i in items.indices) {
            val entry = expectObject(items[i], "items[$i]")
            parsed.add(
                OfflineAllowanceItem(
                    asString(entry["certificate_id_hex"], "items[$i].certificate_id_hex"),
                    asString(entry["controller_id"], "items[$i].controller_id"),
                    asString(entry["controller_display"], "items[$i].controller_display"),
                    asString(entry["asset_id"], "items[$i].asset_id"),
                    asString(entry["asset_definition_id"], "items[$i].asset_definition_id"),
                    asString(entry["asset_definition_name"], "items[$i].asset_definition_name"),
                    requireOptionalString(entry, "asset_definition_alias", "items[$i].asset_definition_alias"),
                    asLong(entry["registered_at_ms"], "items[$i].registered_at_ms"),
                    asLongOrDefault(entry["expires_at_ms"], "items[$i].expires_at_ms", 0L),
                    asLongOrDefault(entry["policy_expires_at_ms"], "items[$i].policy_expires_at_ms", 0L),
                    asOptionalLong(entry["refresh_at_ms"], "items[$i].refresh_at_ms"),
                    asOptionalString(entry["verdict_id_hex"]),
                    asOptionalString(entry["attestation_nonce_hex"]),
                    asString(entry["remaining_amount"], "items[$i].remaining_amount"),
                    JsonEncoder.encode(entry["record"]),
                )
            )
        }
        return OfflineAllowanceList(parsed, total)
    }

    @JvmStatic
    fun parseTransfers(payload: ByteArray): OfflineTransferList {
        val root = parse(payload)
        val obj = expectObject(root, "root")
        val total = asLong(obj["total"], "total")
        val items = asArray(obj["items"], "items")
        val parsed = ArrayList<OfflineTransferList.OfflineTransferItem>(items.size)
        for (i in items.indices) {
            val entry = expectObject(items[i], "items[$i]")
            parsed.add(parseTransferItemObject(entry, "items[$i]"))
        }
        return OfflineTransferList(parsed, total)
    }

    @JvmStatic
    fun parseTransferItem(payload: ByteArray): OfflineTransferList.OfflineTransferItem {
        val root = parse(payload)
        val obj = expectObject(root, "root")
        return parseTransferItemObject(obj, "root")
    }

    @JvmStatic
    fun parseSummaries(payload: ByteArray): OfflineSummaryList {
        val root = parse(payload)
        val obj = expectObject(root, "root")
        val total = asLong(obj["total"], "total")
        val items = asArray(obj["items"], "items")
        val parsed = ArrayList<OfflineSummaryList.OfflineSummaryItem>(items.size)
        for (i in items.indices) {
            val entry = expectObject(items[i], "items[$i]")
            parsed.add(
                OfflineSummaryList.OfflineSummaryItem(
                    asString(entry["certificate_id_hex"], "items[$i].certificate_id_hex"),
                    asString(entry["controller_id"], "items[$i].controller_id"),
                    asString(entry["controller_display"], "items[$i].controller_display"),
                    asString(entry["summary_hash_hex"], "items[$i].summary_hash_hex"),
                    asCounterMap(entry["apple_key_counters"], "items[$i].apple_key_counters"),
                    asCounterMap(entry["android_series_counters"], "items[$i].android_series_counters"),
                )
            )
        }
        return OfflineSummaryList(parsed, total)
    }

    @JvmStatic
    fun parseRevocations(payload: ByteArray): OfflineRevocationList {
        val root = parse(payload)
        val obj = expectObject(root, "root")
        val total = asLong(obj["total"], "total")
        val items = asArray(obj["items"], "items")
        val parsed = ArrayList<OfflineRevocationList.OfflineRevocationItem>(items.size)
        for (i in items.indices) {
            val entry = expectObject(items[i], "items[$i]")
            val metadataJson =
                if (entry.containsKey("metadata")) JsonEncoder.encode(entry["metadata"]) else null
            parsed.add(
                OfflineRevocationList.OfflineRevocationItem(
                    asString(entry["verdict_id_hex"], "items[$i].verdict_id_hex"),
                    asString(entry["issuer_id"], "items[$i].issuer_id"),
                    asString(entry["issuer_display"], "items[$i].issuer_display"),
                    asLong(entry["revoked_at_ms"], "items[$i].revoked_at_ms"),
                    asString(entry["reason"], "items[$i].reason"),
                    asOptionalString(entry["note"]),
                    metadataJson,
                    JsonEncoder.encode(entry["record"]),
                )
            )
        }
        return OfflineRevocationList(parsed, total)
    }

    @JvmStatic
    fun parseCertificateIssueResponse(payload: ByteArray): OfflineCertificateIssueResponse {
        val root = parse(payload)
        val obj = expectObject(root, "root")
        val certificateIdHex = asString(obj["certificate_id_hex"], "certificate_id_hex")
        val certificate = expectObject(obj["certificate"], "certificate")
        val parsed = parseCertificate(certificate, "certificate")
        return OfflineCertificateIssueResponse(certificateIdHex, parsed)
    }

    @JvmStatic
    fun parseBuildClaimIssueResponse(payload: ByteArray): OfflineBuildClaimIssueResponse {
        val root = parse(payload)
        val obj = expectObject(root, "root")
        val claimIdHex = OfflineHashLiteral.parseHex(
            asString(obj["claim_id_hex"], "claim_id_hex"), "claim_id_hex",
        )
        val buildClaimRaw = expectObject(obj["build_claim"], "build_claim")
        val typedBuildClaim = parseBuildClaim(buildClaimRaw, "build_claim")
        return OfflineBuildClaimIssueResponse(claimIdHex, buildClaimRaw, typedBuildClaim)
    }

    @JvmStatic
    fun parseAllowanceRegisterResponse(payload: ByteArray): OfflineAllowanceRegisterResponse {
        val root = parse(payload)
        val obj = expectObject(root, "root")
        val certificateIdHex = asString(obj["certificate_id_hex"], "certificate_id_hex")
        return OfflineAllowanceRegisterResponse(certificateIdHex)
    }

    @JvmStatic
    fun parseSettlementSubmitResponse(payload: ByteArray): OfflineSettlementSubmitResponse {
        val root = parse(payload)
        val obj = expectObject(root, "root")
        val bundleIdHex = asString(obj["bundle_id_hex"], "bundle_id_hex")
        val txHashHex = asString(obj["transaction_hash_hex"], "transaction_hash_hex")
        return OfflineSettlementSubmitResponse(bundleIdHex, txHashHex)
    }

    @JvmStatic
    fun parseBundleProofStatus(payload: ByteArray): OfflineBundleProofStatus {
        val root = parse(payload)
        val obj = expectObject(root, "root")
        val summaryNode = obj["proof_summary"]
        val summary: OfflineBundleProofStatus.ProofSummary? = if (summaryNode == null) {
            null
        } else {
            val summaryObject = expectObject(summaryNode, "proof_summary")
            OfflineBundleProofStatus.ProofSummary(
                asLong(summaryObject["version"], "proof_summary.version").toInt(),
                asOptionalLong(summaryObject["proof_sum_bytes"], "proof_summary.proof_sum_bytes"),
                asOptionalLong(summaryObject["proof_counter_bytes"], "proof_summary.proof_counter_bytes"),
                asOptionalLong(summaryObject["proof_replay_bytes"], "proof_summary.proof_replay_bytes"),
                asStringList(summaryObject["metadata_keys"], "proof_summary.metadata_keys"),
            )
        }
        return OfflineBundleProofStatus(
            asString(obj["bundle_id_hex"], "bundle_id_hex"),
            asString(obj["receipts_root_hex"], "receipts_root_hex"),
            asOptionalString(obj["aggregate_proof_root_hex"]),
            asOptionalBoolean(obj["receipts_root_matches"], "receipts_root_matches"),
            asString(obj["proof_status"], "proof_status"),
            summary,
        )
    }

    /** Returns a canonical JSON string for the provided payload (keys sorted). */
    @JvmStatic
    fun canonicalJson(payload: ByteArray): String {
        val root = parse(payload)
        return JsonEncoder.encode(root)
    }

    private fun parse(payload: ByteArray): Any {
        val json = String(payload, Charsets.UTF_8).trim()
        check(json.isNotEmpty()) { "Empty JSON payload" }
        return JsonParser.parse(json) as Any
    }

    @Suppress("UNCHECKED_CAST")
    private fun expectObject(value: Any?, path: String): Map<String, Any> {
        check(value is Map<*, *>) { "$path is not a JSON object" }
        return value as Map<String, Any>
    }

    @Suppress("UNCHECKED_CAST")
    private fun asArray(value: Any?, path: String): List<Any> {
        check(value is List<*>) { "$path is not a JSON array" }
        return value as List<Any>
    }

    private fun asString(value: Any?, path: String): String {
        checkNotNull(value) { "$path is missing" }
        if (value is String) return value
        return value.toString()
    }

    private fun asOptionalString(value: Any?): String? {
        if (value == null) return null
        return if (value is String) value else value.toString()
    }

    private fun requireOptionalString(obj: Map<String, Any>, key: String, path: String): String? {
        check(key in obj) { "$path is missing" }
        return asOptionalString(obj[key])
    }

    private fun requireNonBlank(value: String?, path: String): String {
        val normalized = normalizeOptionalNonBlank(value)
        checkNotNull(normalized) { "$path must be a non-empty string" }
        return normalized
    }

    private fun normalizeOptionalNonBlank(value: String?): String? {
        if (value == null) return null
        val trimmed = value.trim()
        return trimmed.ifEmpty { null }
    }

    private fun asLong(value: Any?, path: String): Long {
        check(value is Number) { "$path is not a number" }
        check(value !is Float && value !is Double) { "$path must be an integer" }
        return value.toLong()
    }

    private fun asNonNegativeLong(value: Any?, path: String): Long {
        val parsed = asLong(value, path)
        check(parsed >= 0) { "$path must be non-negative" }
        return parsed
    }

    private fun asLongOrDefault(value: Any?, path: String, defaultValue: Long): Long {
        if (value == null) return defaultValue
        return asLong(value, path)
    }

    private fun asOptionalLong(value: Any?, path: String): Long? {
        if (value == null) return null
        return asLong(value, path)
    }

    private fun asOptionalBoolean(value: Any?, path: String): Boolean? {
        if (value == null) return null
        check(value is Boolean) { "$path is not a boolean" }
        return value
    }

    private fun asStringList(value: Any?, path: String): List<String> {
        val items = asArray(value, path)
        return items.indices.map { i -> asString(items[i], "$path[$i]") }
    }

    private fun asCounterMap(value: Any?, path: String): Map<String, Long> {
        val counters = expectObject(value, path)
        val normalized = LinkedHashMap<String, Long>()
        for ((key, v) in counters) {
            normalized[key] = asLong(v, "$path.$key")
        }
        return normalized.toMap()
    }

    private fun parseTransferItemObject(
        entry: Map<String, Any>,
        path: String,
    ): OfflineTransferList.OfflineTransferItem {
        val statusTransitionsJson =
            if (entry.containsKey("status_transitions") && entry["status_transitions"] != null)
                JsonEncoder.encode(entry["status_transitions"])
            else null
        return OfflineTransferList.OfflineTransferItem(
            asString(entry["bundle_id_hex"], "$path.bundle_id_hex"),
            asString(entry["receiver_id"], "$path.receiver_id"),
            asString(entry["receiver_display"], "$path.receiver_display"),
            asString(entry["deposit_account_id"], "$path.deposit_account_id"),
            asString(entry["deposit_account_display"], "$path.deposit_account_display"),
            asOptionalString(entry["asset_id"]),
            asLong(entry["receipt_count"], "$path.receipt_count"),
            asString(entry["total_amount"], "$path.total_amount"),
            asString(entry["claimed_delta"], "$path.claimed_delta"),
            asOptionalString(entry["status"]),
            asOptionalLong(entry["recorded_at_ms"], "$path.recorded_at_ms"),
            asOptionalLong(entry["recorded_at_height"], "$path.recorded_at_height"),
            statusTransitionsJson,
            asOptionalString(entry["platform_policy"]),
            asPlatformTokenSnapshot(entry["platform_token_snapshot"], "$path.platform_token_snapshot"),
            JsonEncoder.encode(entry["transfer"]),
        )
    }

    private fun parseCertificate(
        entry: Map<String, Any>,
        path: String,
    ): OfflineWalletCertificate {
        val controller = asString(entry["controller"], "$path.controller")
        val operator = asString(entry["operator"], "$path.operator")
        val allowanceEntry = expectObject(entry["allowance"], "$path.allowance")
        val allowance = parseAllowanceCommitment(allowanceEntry, "$path.allowance")
        val spendPublicKey = asString(entry["spend_public_key"], "$path.spend_public_key")
        val attestationReport = asBytes(entry["attestation_report"], "$path.attestation_report")
        val issuedAtMs = asLong(entry["issued_at_ms"], "$path.issued_at_ms")
        val expiresAtMs = asLong(entry["expires_at_ms"], "$path.expires_at_ms")
        val policyEntry = expectObject(entry["policy"], "$path.policy")
        val policy = parsePolicy(policyEntry, "$path.policy")
        val operatorSignature = asString(entry["operator_signature"], "$path.operator_signature")
        @Suppress("UNCHECKED_CAST")
        val metadata: Map<String, Any> =
            if (entry["metadata"] is Map<*, *>)
                expectObject(entry["metadata"], "$path.metadata").toMap()
            else emptyMap()
        val verdictIdLiteral = asOptionalString(entry["verdict_id"])
        val verdictIdHex = verdictIdLiteral?.let {
            OfflineHashLiteral.parseHex(it, "$path.verdict_id")
        }
        val attestationNonceLiteral = asOptionalString(entry["attestation_nonce"])
        val attestationNonceHex = attestationNonceLiteral?.let {
            OfflineHashLiteral.parseHex(it, "$path.attestation_nonce")
        }
        val refreshAtMs = asOptionalLong(entry["refresh_at_ms"], "$path.refresh_at_ms")
        return OfflineWalletCertificate(
            controller, operator, allowance, spendPublicKey, attestationReport,
            issuedAtMs, expiresAtMs, policy, operatorSignature, metadata,
            verdictIdHex, attestationNonceHex, refreshAtMs,
        )
    }

    private fun parseBuildClaim(
        entry: Map<String, Any>,
        path: String,
    ): OfflineBuildClaim {
        val claimId = OfflineHashLiteral.normalize(
            asString(entry["claim_id"], "$path.claim_id"), "$path.claim_id",
        )
        val nonce = OfflineHashLiteral.normalize(
            asString(entry["nonce"], "$path.nonce"), "$path.nonce",
        )
        val platform = parseBuildClaimPlatform(entry["platform"], "$path.platform")
        val appId = requireNonBlank(asString(entry["app_id"], "$path.app_id"), "$path.app_id")
        val buildNumber = asNonNegativeLong(entry["build_number"], "$path.build_number")
        val issuedAtMs = asNonNegativeLong(entry["issued_at_ms"], "$path.issued_at_ms")
        val expiresAtMs = asNonNegativeLong(entry["expires_at_ms"], "$path.expires_at_ms")
        val lineageScope = normalizeOptionalNonBlank(asOptionalString(entry["lineage_scope"]))
        val operatorSignature = requireNonBlank(
            asString(entry["operator_signature"], "$path.operator_signature"),
            "$path.operator_signature",
        )
        return OfflineBuildClaim(
            claimId, nonce, platform, appId, buildNumber,
            issuedAtMs, expiresAtMs, lineageScope, operatorSignature,
        )
    }

    private fun parseAllowanceCommitment(
        entry: Map<String, Any>,
        path: String,
    ): OfflineAllowanceCommitment {
        val asset = asString(entry["asset"], "$path.asset")
        val amount = asString(entry["amount"], "$path.amount")
        val commitment = asBytes(entry["commitment"], "$path.commitment")
        return OfflineAllowanceCommitment(asset, amount, commitment)
    }

    private fun parsePolicy(
        entry: Map<String, Any>,
        path: String,
    ): OfflineWalletPolicy {
        val maxBalance = asString(entry["max_balance"], "$path.max_balance")
        val maxTxValue = asString(entry["max_tx_value"], "$path.max_tx_value")
        val expiresAtMs = asLong(entry["expires_at_ms"], "$path.expires_at_ms")
        return OfflineWalletPolicy(maxBalance, maxTxValue, expiresAtMs)
    }

    private fun asBytes(value: Any?, path: String): ByteArray {
        val items = asArray(value, path)
        val bytes = ByteArray(items.size)
        for (i in items.indices) {
            val numeric = asLong(items[i], "$path[$i]")
            check(numeric in 0..255) { "$path[$i] is not a byte" }
            bytes[i] = (numeric and 0xff).toByte()
        }
        return bytes
    }

    private fun parseBuildClaimPlatform(value: Any?, path: String): String {
        val normalized = requireNonBlank(asString(value, path), path).lowercase(Locale.ROOT)
        return when (normalized) {
            "apple", "ios" -> "Apple"
            "android" -> "Android"
            else -> throw IllegalStateException("$path must be either \"Apple\" or \"Android\"")
        }
    }

    private fun asPlatformTokenSnapshot(
        value: Any?,
        path: String,
    ): OfflineTransferList.OfflineTransferItem.PlatformTokenSnapshot? {
        if (value == null) return null
        val snapshot = expectObject(value, path)
        val policy = asString(snapshot["policy"], "$path.policy")
        val token = asString(snapshot["attestation_jws_b64"], "$path.attestation_jws_b64")
        return OfflineTransferList.OfflineTransferItem.PlatformTokenSnapshot(policy, token)
    }
}
