package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonNumbers
import org.hyperledger.iroha.sdk.client.JsonParser

object OfflineJsonParser {

    @JvmStatic
    fun parseOfflineReadiness(payload: ByteArray): OfflineReadiness {
        val root = parse(payload)
        val obj = expectObject(root, "root")
        return OfflineReadiness(
            asBoolean(obj["offline_note"], "offline_note"),
            asBoolean(obj["offline_one_use_keys"], "offline_one_use_keys"),
            asBoolean(obj["offline_recursive_note_proof"], "offline_recursive_note_proof"),
            asBoolean(obj["offline_fountain_qr"], "offline_fountain_qr"),
            asBoolean(obj["offline_sync_optional"], "offline_sync_optional"),
            asBoolean(obj["offline_telemetry"], "offline_telemetry"),
        )
    }

    /** Returns a canonical JSON string for the provided payload (keys sorted). */
    @JvmStatic
    fun canonicalJson(payload: ByteArray): String {
        val root = parse(payload)
        return JsonEncoder.encode(root)
    }

    @JvmStatic
    fun parseCashEnvelope(payload: ByteArray): OfflineCashEnvelope {
        val obj = expectObject(parse(payload), "root")
        return parseCashEnvelopeObject(obj, "root")
    }

    @JvmStatic
    fun parseCashState(payload: ByteArray): OfflineCashState {
        val obj = expectObject(parse(payload), "root")
        return parseCashStateObject(obj, "root")
    }

    @JvmStatic
    fun parseCashReadiness(payload: ByteArray): OfflineCashReadiness {
        val obj = expectObject(parse(payload), "root")
        val flag = obj["offline_recursive_stark"]
        check(flag is Boolean) { "offline_recursive_stark is not a boolean" }
        return OfflineCashReadiness(flag)
    }

    @JvmStatic
    fun parseMutationSettlement(payload: ByteArray): OfflineMutationSettlement {
        val obj = expectObject(parse(payload), "root")
        return parseMutationSettlementObject(obj, "root")
    }

    @JvmStatic
    fun parseRedeemRequestProof(payload: ByteArray): OfflineRedeemRequestProof {
        val obj = expectObject(parse(payload), "root")
        return parseRedeemRequestProofObject(obj, "root")
    }

    @JvmStatic
    fun parseStarkEnvelope(payload: ByteArray): OfflineStarkVerifyEnvelope {
        val obj = expectObject(parse(payload), "root")
        return parseStarkEnvelopeObject(obj, "root")
    }

    private fun parseCashEnvelopeObject(obj: Map<String, Any>, path: String): OfflineCashEnvelope {
        val state = parseCashStateObject(
            expectObject(obj["lineage_state"], "$path.lineage_state"),
            "$path.lineage_state",
        )
        val settlement = obj["settlement"]?.let {
            parseMutationSettlementObject(expectObject(it, "$path.settlement"), "$path.settlement")
        }
        return OfflineCashEnvelope(state, settlement)
    }

    private fun parseCashStateObject(obj: Map<String, Any>, path: String): OfflineCashState =
        OfflineCashState(
            asString(obj["lineage_id"], "$path.lineage_id"),
            asString(obj["account_id"], "$path.account_id"),
            asString(obj["device_id"], "$path.device_id"),
            asString(obj["offline_public_key"], "$path.offline_public_key"),
            asString(obj["asset_definition_id"], "$path.asset_definition_id"),
            asString(obj["balance"], "$path.balance"),
            asString(obj["locked_balance"], "$path.locked_balance"),
            asLong(obj["server_revision"], "$path.server_revision"),
            asString(obj["server_state_hash"], "$path.server_state_hash"),
            asLong(obj["pending_local_revision"], "$path.pending_local_revision"),
            parseSpendAuthorizationObject(
                expectObject(obj["authorization"], "$path.authorization"),
                "$path.authorization",
            ),
            asString(obj["issuer_signature_base64"], "$path.issuer_signature_base64"),
        )

    private fun parseSpendAuthorizationObject(
        obj: Map<String, Any>,
        path: String,
    ): OfflineSpendAuthorization {
        val binding = obj["device_binding"]?.let {
            parseCashDeviceBindingObject(expectObject(it, "$path.device_binding"), "$path.device_binding")
        }
        return OfflineSpendAuthorization(
            asString(obj["authorization_id"], "$path.authorization_id"),
            asString(obj["lineage_id"], "$path.lineage_id"),
            asString(obj["account_id"], "$path.account_id"),
            asOptionalString(obj["device_id"]),
            asOptionalString(obj["offline_public_key"]),
            asString(obj["verdict_id"], "$path.verdict_id"),
            asString(obj["max_balance"], "$path.max_balance"),
            asString(obj["max_tx_value"], "$path.max_tx_value"),
            asLong(obj["issued_at_ms"], "$path.issued_at_ms"),
            asLong(obj["refresh_at_ms"], "$path.refresh_at_ms"),
            asLong(obj["expires_at_ms"], "$path.expires_at_ms"),
            binding,
            asString(obj["issuer_signature_base64"], "$path.issuer_signature_base64"),
        )
    }

    private fun parseCashDeviceBindingObject(
        obj: Map<String, Any>,
        path: String,
    ): OfflineCashDeviceBinding =
        OfflineCashDeviceBinding(
            asString(obj["platform"], "$path.platform"),
            asString(obj["attestation_key_id"], "$path.attestation_key_id"),
            asString(obj["device_id"], "$path.device_id"),
            asString(obj["offline_public_key"], "$path.offline_public_key"),
            asString(obj["attestation_report_base64"], "$path.attestation_report_base64"),
            asOptionalString(obj["ios_team_id"]),
            asOptionalString(obj["ios_bundle_id"]),
            asOptionalString(obj["ios_environment"]),
        )

    private fun parseMutationSettlementObject(
        obj: Map<String, Any>,
        path: String,
    ): OfflineMutationSettlement =
        OfflineMutationSettlement(
            asString(obj["kind"], "$path.kind"),
            asString(obj["operation_id"], "$path.operation_id"),
            asString(obj["chain_tx_hash"], "$path.chain_tx_hash"),
            asString(obj["entry_hash"], "$path.entry_hash"),
            asLong(obj["block_height"], "$path.block_height"),
            asString(obj["pre_state_hash"], "$path.pre_state_hash"),
            asString(obj["post_state_hash"], "$path.post_state_hash"),
            asString(obj["settlement_commitment_hex"], "$path.settlement_commitment_hex"),
            parseTransparentZkProofObject(expectObject(obj["proof"], "$path.proof"), "$path.proof"),
        )

    private fun parseTransparentZkProofObject(
        obj: Map<String, Any>,
        path: String,
    ): OfflineTransparentZkProof =
        OfflineTransparentZkProof(
            asString(obj["backend"], "$path.backend"),
            asString(obj["circuit_id"], "$path.circuit_id"),
            JsonNumbers.asInt(obj["recursion_depth"], "$path.recursion_depth"),
            asString(obj["public_inputs_hex"], "$path.public_inputs_hex"),
            parseStarkEnvelopeObject(expectObject(obj["envelope"], "$path.envelope"), "$path.envelope"),
        )

    private fun parseRedeemRequestProofObject(
        obj: Map<String, Any>,
        path: String,
    ): OfflineRedeemRequestProof =
        OfflineRedeemRequestProof(
            asString(obj["backend"], "$path.backend"),
            asString(obj["circuit_id"], "$path.circuit_id"),
            JsonNumbers.asInt(obj["recursion_depth"], "$path.recursion_depth"),
            asString(obj["public_inputs_hex"], "$path.public_inputs_hex"),
            parseStarkEnvelopeObject(expectObject(obj["envelope"], "$path.envelope"), "$path.envelope"),
        )

    private fun parseStarkEnvelopeObject(
        obj: Map<String, Any>,
        path: String,
    ): OfflineStarkVerifyEnvelope =
        OfflineStarkVerifyEnvelope(
            parseStarkFriParamsObject(expectObject(obj["params"], "$path.params"), "$path.params"),
            parseStarkProofObject(expectObject(obj["proof"], "$path.proof"), "$path.proof"),
            asString(obj["transcript_label"], "$path.transcript_label"),
        )

    private fun parseStarkFriParamsObject(
        obj: Map<String, Any>,
        path: String,
    ): OfflineStarkFriParams =
        OfflineStarkFriParams(
            JsonNumbers.asInt(obj["version"], "$path.version"),
            JsonNumbers.asInt(obj["n_log2"], "$path.n_log2"),
            JsonNumbers.asInt(obj["blowup_log2"], "$path.blowup_log2"),
            JsonNumbers.asInt(obj["fold_arity"], "$path.fold_arity"),
            JsonNumbers.asInt(obj["queries"], "$path.queries"),
            JsonNumbers.asInt(obj["merkle_arity"], "$path.merkle_arity"),
            JsonNumbers.asInt(obj["hash_fn"], "$path.hash_fn"),
            asString(obj["domain_tag"], "$path.domain_tag"),
        )

    private fun parseStarkProofObject(obj: Map<String, Any>, path: String): OfflineStarkProof {
        val queriesRaw = asArray(obj["queries"], "$path.queries")
        val queries = queriesRaw.indices.map { i ->
            val chain = asArray(queriesRaw[i], "$path.queries[$i]")
            chain.indices.map { j ->
                parseFoldDecommitObject(
                    expectObject(chain[j], "$path.queries[$i][$j]"),
                    "$path.queries[$i][$j]",
                )
            }
        }
        val compValues = obj["comp_values"]?.let { rawNode ->
            val raw = asArray(rawNode, "$path.comp_values")
            raw.indices.map { i ->
                parseCompositionValueObject(expectObject(raw[i], "$path.comp_values[$i]"), "$path.comp_values[$i]")
            }
        }
        val air = obj["air"]?.let { parseStarkAirProofObject(expectObject(it, "$path.air"), "$path.air") }
        return OfflineStarkProof(
            JsonNumbers.asInt(obj["version"], "$path.version"),
            parseStarkCommitmentsObject(expectObject(obj["commits"], "$path.commits"), "$path.commits"),
            queries,
            compValues,
            air,
        )
    }

    private fun parseStarkCommitmentsObject(
        obj: Map<String, Any>,
        path: String,
    ): OfflineStarkCommitments {
        val rootsRaw = asArray(obj["roots"], "$path.roots")
        val roots = rootsRaw.indices.map { i -> asBytes(rootsRaw[i], "$path.roots[$i]") }
        val compRoot = obj["comp_root"]?.let { asBytes(it, "$path.comp_root") }
        return OfflineStarkCommitments(
            JsonNumbers.asInt(obj["version"], "$path.version"),
            roots,
            compRoot,
        )
    }

    private fun parseFoldDecommitObject(
        obj: Map<String, Any>,
        path: String,
    ): OfflineFoldDecommit =
        OfflineFoldDecommit(
            asLong(obj["j"], "$path.j"),
            asBigInteger(obj["y0"], "$path.y0"),
            asBigInteger(obj["y1"], "$path.y1"),
            parseMerklePathObject(expectObject(obj["path_y0"], "$path.path_y0"), "$path.path_y0"),
            parseMerklePathObject(expectObject(obj["path_y1"], "$path.path_y1"), "$path.path_y1"),
            asBigInteger(obj["z"], "$path.z"),
            parseMerklePathObject(expectObject(obj["path_z"], "$path.path_z"), "$path.path_z"),
        )

    private fun parseMerklePathObject(obj: Map<String, Any>, path: String): OfflineMerklePath {
        val dirs = asBytes(obj["dirs"], "$path.dirs")
        val siblingsRaw = asArray(obj["siblings"], "$path.siblings")
        val siblings = siblingsRaw.indices.map { i -> asBytes(siblingsRaw[i], "$path.siblings[$i]") }
        return OfflineMerklePath(dirs, siblings)
    }

    private fun parseCompositionValueObject(
        obj: Map<String, Any>,
        path: String,
    ): OfflineStarkCompositionValue {
        val termsRaw = asArray(obj["aux_terms"], "$path.aux_terms")
        val terms = termsRaw.indices.map { i ->
            parseCompositionTermObject(expectObject(termsRaw[i], "$path.aux_terms[$i]"), "$path.aux_terms[$i]")
        }
        return OfflineStarkCompositionValue(
            asBigInteger(obj["leaf"], "$path.leaf"),
            asBigInteger(obj["constant"], "$path.constant"),
            asBigInteger(obj["z_coeff"], "$path.z_coeff"),
            terms,
            parseMerklePathObject(expectObject(obj["path"], "$path.path"), "$path.path"),
        )
    }

    private fun parseCompositionTermObject(
        obj: Map<String, Any>,
        path: String,
    ): OfflineStarkCompositionTerm =
        OfflineStarkCompositionTerm(
            asLong(obj["wire_index"], "$path.wire_index"),
            asBigInteger(obj["value"], "$path.value"),
            asBigInteger(obj["coeff"], "$path.coeff"),
        )

    private fun parseStarkAirProofObject(
        obj: Map<String, Any>,
        path: String,
    ): OfflineStarkAirProof {
        val openingsRaw = asArray(obj["openings"], "$path.openings")
        val openings = openingsRaw.indices.map { i ->
            parseStarkAirOpeningObject(expectObject(openingsRaw[i], "$path.openings[$i]"), "$path.openings[$i]")
        }
        return OfflineStarkAirProof(
            JsonNumbers.asInt(obj["version"], "$path.version"),
            asString(obj["circuit_id"], "$path.circuit_id"),
            asBytes(obj["public_digest"], "$path.public_digest"),
            asBytes(obj["trace_root"], "$path.trace_root"),
            asBytes(obj["composition_root"], "$path.composition_root"),
            JsonNumbers.asInt(obj["trace_width"], "$path.trace_width"),
            openings,
        )
    }

    private fun parseStarkAirOpeningObject(
        obj: Map<String, Any>,
        path: String,
    ): OfflineStarkAirOpening {
        val rowRaw = asArray(obj["row"], "$path.row")
        val row = rowRaw.indices.map { i -> asBigInteger(rowRaw[i], "$path.row[$i]") }
        val nextRowRaw = asArray(obj["next_row"], "$path.next_row")
        val nextRow = nextRowRaw.indices.map { i -> asBigInteger(nextRowRaw[i], "$path.next_row[$i]") }
        return OfflineStarkAirOpening(
            asLong(obj["index"], "$path.index"),
            row,
            nextRow,
            parseMerklePathObject(expectObject(obj["row_path"], "$path.row_path"), "$path.row_path"),
            parseMerklePathObject(expectObject(obj["next_row_path"], "$path.next_row_path"), "$path.next_row_path"),
            asBigInteger(obj["composition_value"], "$path.composition_value"),
            parseMerklePathObject(expectObject(obj["composition_path"], "$path.composition_path"), "$path.composition_path"),
        )
    }

    private fun asBigInteger(value: Any?, path: String): BigInteger {
        checkNotNull(value) { "$path is missing" }
        return when (value) {
            is BigInteger -> value
            is Long -> BigInteger.valueOf(value)
            is Int -> BigInteger.valueOf(value.toLong())
            is Number -> {
                check(value !is Float && value !is Double) { "$path must be an integer" }
                BigInteger.valueOf(value.toLong())
            }
            else -> error("$path is not a number")
        }
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

    private fun asLong(value: Any?, path: String): Long {
        return JsonNumbers.asLong(value, path)
    }

    private fun asBoolean(value: Any?, path: String): Boolean {
        check(value is Boolean) { "$path must be a boolean" }
        return value
    }

    private fun asBytes(value: Any?, path: String): ByteArray {
        if (value is String) return decodeHexBytes(value, path)
        val items = asArray(value, path)
        val bytes = ByteArray(items.size)
        for (i in items.indices) {
            val numeric = asLong(items[i], "$path[$i]")
            check(numeric in 0..255) { "$path[$i] is not a byte" }
            bytes[i] = (numeric and 0xff).toByte()
        }
        return bytes
    }

    private fun decodeHexBytes(hex: String, path: String): ByteArray {
        check(hex.length % 2 == 0) { "$path must be a hex string of even length" }
        val out = ByteArray(hex.length / 2)
        for (i in out.indices) {
            val hi = hexDigit(hex[2 * i], path, 2 * i)
            val lo = hexDigit(hex[2 * i + 1], path, 2 * i + 1)
            out[i] = ((hi shl 4) or lo).toByte()
        }
        return out
    }

    private fun hexDigit(c: Char, path: String, index: Int): Int {
        return when (c) {
            in '0'..'9' -> c - '0'
            in 'a'..'f' -> c - 'a' + 10
            in 'A'..'F' -> c - 'A' + 10
            else -> error("invalid hex digit `$c` at $path[$index]")
        }
    }
}
