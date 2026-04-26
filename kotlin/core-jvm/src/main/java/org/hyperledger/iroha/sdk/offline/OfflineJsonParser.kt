package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser

object OfflineJsonParser {

    @JvmStatic
    fun parseOfflineV2Readiness(payload: ByteArray): OfflineV2Readiness {
        val root = parse(payload)
        val obj = expectObject(root, "root")
        return OfflineV2Readiness(
            asBoolean(obj["offline_note_v2"], "offline_note_v2"),
            asBoolean(obj["offline_one_use_keys"], "offline_one_use_keys"),
            asBoolean(obj["offline_recursive_note_proof"], "offline_recursive_note_proof"),
            asBoolean(obj["offline_fountain_qr_v1"], "offline_fountain_qr_v1"),
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

    private fun asBoolean(value: Any?, path: String): Boolean {
        check(value is Boolean) { "$path must be a boolean" }
        return value
    }
}
