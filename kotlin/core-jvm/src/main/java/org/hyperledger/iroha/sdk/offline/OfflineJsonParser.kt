package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonNumbers
import org.hyperledger.iroha.sdk.client.JsonParser

object OfflineJsonParser {
    @JvmStatic
    fun parseOfflineReadiness(payload: ByteArray): OfflineReadiness {
        val root = parse(payload)
        val obj = expectObject(root, "root")
        val blockers = asArray(obj["blockers"], "blockers").mapIndexed { index, value ->
            val path = "blockers[$index]"
            val blocker = expectObject(value, path)
            OfflineReadinessBlocker(
                asExactReadinessString(blocker["code"], "$path.code"),
                asExactReadinessString(blocker["message"], "$path.message"),
            )
        }
        val assetScaleValue = required(obj, "asset_scale", "root")
        val activeVerifierValue = required(obj, "active_transfer_verifier", "root")
        val activeTopUpShieldVerifierValue =
            required(obj, "active_topup_shield_verifier", "root")
        val evaluatedBlockHeight = asReadinessU64(
            required(obj, "evaluated_block_height", "root"),
            "evaluated_block_height",
        )
        return OfflineReadiness(
            asExactReadinessString(
                required(obj, "asset_definition_id", "root"),
                "asset_definition_id",
            ),
            assetScaleValue?.let { asReadinessU32(it, "asset_scale") },
            evaluatedBlockHeight,
            asExactLowercaseHex(
                required(obj, "evaluated_block_hash", "root"),
                "evaluated_block_hash",
                32,
            ),
            activeVerifierValue?.let {
                parseActiveTransferVerifier(it, evaluatedBlockHeight, "active_transfer_verifier")
            },
            activeTopUpShieldVerifierValue?.let {
                parseActiveTransferVerifier(
                    it,
                    evaluatedBlockHeight,
                    "active_topup_shield_verifier",
                )
            },
            asBoolean(required(obj, "ready", "root"), "ready"),
            blockers,
        )
    }

    private fun parseActiveTransferVerifier(
        value: Any,
        evaluatedBlockHeight: BigInteger,
        path: String,
    ): OfflineActiveTransferVerifier {
        val obj = expectObject(value, path)
        val idPath = "$path.id"
        val id = expectObject(required(obj, "id", path), idPath)
        return OfflineActiveTransferVerifier(
            OfflineVerifierId(
                asExactReadinessString(required(id, "backend", idPath), "$idPath.backend"),
                asExactReadinessString(required(id, "name", idPath), "$idPath.name"),
            ),
            asReadinessU32(required(obj, "version", path), "$path.version"),
            asExactReadinessString(required(obj, "circuit_id", path), "$path.circuit_id"),
            asExactLowercaseHex(required(obj, "commitment", path), "$path.commitment", 32),
            asExactLowercaseHex(
                required(obj, "public_inputs_schema_hash", path),
                "$path.public_inputs_schema_hash",
                32,
            ),
            asReadinessU32(required(obj, "max_proof_bytes", path), "$path.max_proof_bytes"),
            asReadinessU64(required(obj, "activation_height", path), "$path.activation_height"),
            required(obj, "withdrawal_height", path)?.let {
                asReadinessU64(it, "$path.withdrawal_height")
            },
        ).also {
            check(it.isActiveAt(evaluatedBlockHeight)) {
                "$path must be active at evaluated_block_height"
            }
        }
    }

    /** Returns a canonical JSON string for the provided payload (keys sorted). */
    @JvmStatic
    fun canonicalJson(payload: ByteArray): String {
        val root = parse(payload)
        return JsonEncoder.encode(root)
    }

    @JvmStatic
    fun parseCashState(payload: ByteArray): OfflineCashState {
        val obj = expectObject(parse(payload), "root")
        return parseCashStateObject(obj, "root")
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
            is Byte, is Short, is Int, is Long -> BigInteger.valueOf((value as Number).toLong())
            is Number -> error("$path must be an integer")
            else -> error("$path is not a number")
        }
    }

    private fun parse(payload: ByteArray): Any {
        val json = try {
            Charsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .decode(ByteBuffer.wrap(payload))
                .toString()
        } catch (error: java.nio.charset.CharacterCodingException) {
            throw IllegalStateException("Offline JSON payload must be valid UTF-8", error)
        }
        check(json.isNotEmpty()) { "Empty JSON payload" }
        return JsonParser.parse(json) as Any
    }

    @Suppress("UNCHECKED_CAST")
    private fun expectObject(value: Any?, path: String): Map<String, Any> {
        check(value is Map<*, *>) { "$path is not a JSON object" }
        return value as Map<String, Any>
    }

    private fun required(obj: Map<String, Any>, field: String, path: String): Any? {
        check(obj.containsKey(field)) { "$path.$field is required" }
        return obj[field]
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

    private fun asExactReadinessString(value: Any?, path: String): String {
        check(value is String) { "$path must be a string" }
        check(value.isNotEmpty() && value == value.trim()) { "$path must be an exact non-empty string" }
        return value
    }

    private fun asExactLowercaseHex(value: Any?, path: String, bytes: Int): String {
        val text = asExactReadinessString(value, path)
        check(text.length == bytes * 2 && text.all { it in '0'..'9' || it in 'a'..'f' }) {
            "$path must be exact lowercase $bytes-byte hexadecimal"
        }
        return text
    }

    private fun asReadinessU64(value: Any?, path: String): BigInteger {
        val parsed = when (value) {
            is BigInteger -> value
            is java.math.BigDecimal -> error("$path must be a JSON integer number")
            is Byte, is Short, is Int, is Long -> BigInteger.valueOf((value as Number).toLong())
            is Float, is Double -> error("$path must be an integer")
            else -> error("$path must be a JSON integer number")
        }
        check(parsed >= BigInteger.ZERO && parsed <= BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)) {
            "$path must fit in an unsigned 64-bit integer"
        }
        return parsed
    }

    private fun asReadinessU32(value: Any?, path: String): Long {
        val parsed = asReadinessU64(value, path)
        check(parsed <= BigInteger.valueOf(0xffff_ffffL)) {
            "$path must fit in an unsigned 32-bit integer"
        }
        return parsed.toLong()
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
        check(hex.length % 2 == 0) { "$path must be a lowercase hex string of even length" }
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
            else -> error("invalid lowercase hex digit `$c` at $path[$index]")
        }
    }
}
