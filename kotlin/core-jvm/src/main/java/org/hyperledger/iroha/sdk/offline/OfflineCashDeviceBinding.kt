package org.hyperledger.iroha.sdk.offline

/** Device binding descriptor posted alongside every cash-route mutation. */
class OfflineCashDeviceBinding(
    val platform: String,
    val attestationKeyId: String,
    val deviceId: String,
    val offlinePublicKey: String,
    val attestationReportBase64: String,
    val iosTeamId: String? = null,
    val iosBundleId: String? = null,
    val iosEnvironment: String? = null,
) {
    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["platform"] = platform
        map["attestation_key_id"] = attestationKeyId
        map["device_id"] = deviceId
        map["offline_public_key"] = offlinePublicKey
        map["attestation_report_base64"] = attestationReportBase64
        if (iosTeamId != null) map["ios_team_id"] = iosTeamId
        if (iosBundleId != null) map["ios_bundle_id"] = iosBundleId
        if (iosEnvironment != null) map["ios_environment"] = iosEnvironment
        return map
    }
}
