package org.hyperledger.iroha.sdk.offline

/** Immutable metadata describing a cached attestation verdict for an allowance. */
class OfflineVerdictMetadata(
    val certificateIdHex: String,
    val controllerId: String,
    val controllerDisplay: String,
    val verdictIdHex: String?,
    val attestationNonceHex: String?,
    val certificateExpiresAtMs: Long,
    val policyExpiresAtMs: Long,
    val refreshAtMs: Long?,
    remainingAmount: String?,
    val recordedAtMs: Long,
    val integrityPolicy: String? = null,
    val playIntegrity: PlayIntegrityMetadata? = null,
    val playIntegrityToken: PlayIntegrityTokenSnapshot? = null,
    val hmsSafetyDetect: SafetyDetectMetadata? = null,
    val hmsSafetyDetectToken: SafetyDetectTokenSnapshot? = null,
    val provisionedMetadata: ProvisionedMetadata? = null,
) {
    val remainingAmount: String =
        if (remainingAmount.isNullOrBlank()) "0" else remainingAmount.trim()

    fun withSafetyDetectToken(tokenSnapshot: SafetyDetectTokenSnapshot): OfflineVerdictMetadata =
        OfflineVerdictMetadata(
            certificateIdHex, controllerId, controllerDisplay, verdictIdHex,
            attestationNonceHex, certificateExpiresAtMs, policyExpiresAtMs, refreshAtMs,
            remainingAmount, recordedAtMs, integrityPolicy, playIntegrity, playIntegrityToken,
            hmsSafetyDetect, tokenSnapshot, provisionedMetadata,
        )

    fun withPlayIntegrityToken(tokenSnapshot: PlayIntegrityTokenSnapshot): OfflineVerdictMetadata =
        OfflineVerdictMetadata(
            certificateIdHex, controllerId, controllerDisplay, verdictIdHex,
            attestationNonceHex, certificateExpiresAtMs, policyExpiresAtMs, refreshAtMs,
            remainingAmount, recordedAtMs, integrityPolicy, playIntegrity, tokenSnapshot,
            hmsSafetyDetect, hmsSafetyDetectToken, provisionedMetadata,
        )

    internal fun toJson(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["controller_id"] = controllerId
        map["controller_display"] = controllerDisplay
        map["verdict_id_hex"] = verdictIdHex
        map["attestation_nonce_hex"] = attestationNonceHex
        map["certificate_expires_at_ms"] = certificateExpiresAtMs
        map["policy_expires_at_ms"] = policyExpiresAtMs
        map["refresh_at_ms"] = refreshAtMs
        map["remaining_amount"] = remainingAmount
        map["recorded_at_ms"] = recordedAtMs
        if (!integrityPolicy.isNullOrBlank()) map["integrity_policy"] = integrityPolicy
        if (playIntegrity != null) map["play_integrity"] = playIntegrity.toJson()
        if (playIntegrityToken != null) map["play_integrity_token"] = playIntegrityToken.toJson()
        if (hmsSafetyDetect != null) map["hms_safety_detect"] = hmsSafetyDetect.toJson()
        if (hmsSafetyDetectToken != null) map["hms_safety_detect_token"] = hmsSafetyDetectToken.toJson()
        if (provisionedMetadata != null) map["android_provisioned"] = provisionedMetadata.toJson()
        return map
    }

    /** Subset of Play Integrity metadata tracked for offline policy enforcement. */
    class PlayIntegrityMetadata(
        val cloudProjectNumber: Long,
        val environment: String?,
        packageNames: List<String>?,
        signingDigestsSha256: List<String>?,
        allowedAppVerdicts: List<String>?,
        allowedDeviceVerdicts: List<String>?,
        val maxTokenAgeMs: Long?,
    ) {
        val packageNames: List<String> = packageNames?.toList() ?: emptyList()
        val signingDigestsSha256: List<String> = signingDigestsSha256?.toList() ?: emptyList()
        val allowedAppVerdicts: List<String> = allowedAppVerdicts?.toList() ?: emptyList()
        val allowedDeviceVerdicts: List<String> = allowedDeviceVerdicts?.toList() ?: emptyList()

        internal fun toJson(): Map<String, Any?> {
            val map = LinkedHashMap<String, Any?>()
            map["cloud_project_number"] = cloudProjectNumber
            map["environment"] = environment
            map["package_names"] = packageNames
            map["signing_digests_sha256"] = signingDigestsSha256
            map["allowed_app_verdicts"] = allowedAppVerdicts
            map["allowed_device_verdicts"] = allowedDeviceVerdicts
            map["max_token_age_ms"] = maxTokenAgeMs
            return map
        }

        internal companion object {
            fun fromJson(value: Any?): PlayIntegrityMetadata? {
                if (value !is Map<*, *>) return null
                val project = asOptionalLong(value["cloud_project_number"])
                val environment = asOptionalString(value["environment"])
                if (project == null || project <= 0 || environment.isNullOrBlank()) return null
                val packages = asStringList(value["package_names"])
                val digests = asStringList(value["signing_digests_sha256"])
                val appVerdicts = asStringList(value["allowed_app_verdicts"])
                val deviceVerdicts = asStringList(value["allowed_device_verdicts"])
                if (packages.isEmpty() || digests.isEmpty() || appVerdicts.isEmpty() || deviceVerdicts.isEmpty()) {
                    return null
                }
                val maxAge = asOptionalLong(value["max_token_age_ms"])
                return PlayIntegrityMetadata(
                    project, environment.trim(), packages, digests,
                    appVerdicts, deviceVerdicts, maxAge,
                )
            }
        }
    }

    /** Snapshot of the cached Play Integrity attestation token. */
    class PlayIntegrityTokenSnapshot(
        val token: String,
        val fetchedAtMs: Long,
    ) {
        internal fun toJson(): Map<String, Any> {
            val map = LinkedHashMap<String, Any>()
            map["token"] = token
            map["fetched_at_ms"] = fetchedAtMs
            return map
        }

        internal companion object {
            fun fromJson(value: Any?): PlayIntegrityTokenSnapshot? {
                if (value !is Map<*, *>) return null
                val tokenValue = value["token"]
                val fetchedValue = value["fetched_at_ms"]
                if (tokenValue !is String || tokenValue.isBlank()) return null
                if (fetchedValue !is Number || fetchedValue is Float || fetchedValue is Double) return null
                return PlayIntegrityTokenSnapshot(tokenValue, fetchedValue.toLong())
            }
        }
    }

    /** Subset of HMS Safety Detect metadata tracked for offline policy enforcement. */
    class SafetyDetectMetadata(
        val appId: String?,
        packageNames: List<String>?,
        signingDigestsSha256: List<String>?,
        requiredEvaluations: List<String>?,
        val maxTokenAgeMs: Long?,
    ) {
        val packageNames: List<String> = packageNames?.toList() ?: emptyList()
        val signingDigestsSha256: List<String> = signingDigestsSha256?.toList() ?: emptyList()
        val requiredEvaluations: List<String> = requiredEvaluations?.toList() ?: emptyList()

        internal fun toJson(): Map<String, Any?> {
            val map = LinkedHashMap<String, Any?>()
            map["app_id"] = appId
            map["package_names"] = packageNames
            map["signing_digests_sha256"] = signingDigestsSha256
            map["required_evaluations"] = requiredEvaluations
            map["max_token_age_ms"] = maxTokenAgeMs
            return map
        }

        internal companion object {
            fun fromJson(value: Any?): SafetyDetectMetadata? {
                if (value !is Map<*, *>) return null
                val appId = asOptionalString(value["app_id"])
                val packages = asStringList(value["package_names"])
                val digests = asStringList(value["signing_digests_sha256"])
                val evaluations = asStringList(value["required_evaluations"])
                val maxAge = asOptionalLong(value["max_token_age_ms"])
                return SafetyDetectMetadata(appId, packages, digests, evaluations, maxAge)
            }
        }
    }

    /** Snapshot of the cached HMS Safety Detect attestation token. */
    class SafetyDetectTokenSnapshot(
        val token: String,
        val fetchedAtMs: Long,
    ) {
        internal fun toJson(): Map<String, Any> {
            val map = LinkedHashMap<String, Any>()
            map["token"] = token
            map["fetched_at_ms"] = fetchedAtMs
            return map
        }

        internal companion object {
            fun fromJson(value: Any?): SafetyDetectTokenSnapshot? {
                if (value !is Map<*, *>) return null
                val tokenValue = value["token"]
                val fetchedValue = value["fetched_at_ms"]
                if (tokenValue !is String || tokenValue.isBlank()) return null
                if (fetchedValue !is Number || fetchedValue is Float || fetchedValue is Double) return null
                return SafetyDetectTokenSnapshot(tokenValue, fetchedValue.toLong())
            }
        }
    }

    /** Subset of provisioned inspector metadata tracked for policy enforcement. */
    class ProvisionedMetadata(
        inspectorPublicKey: String?,
        manifestSchema: String?,
        val manifestVersion: Int?,
        val maxManifestAgeMs: Long?,
        manifestDigest: String?,
    ) {
        val inspectorPublicKey: String? = inspectorPublicKey?.trim()
        val manifestSchema: String? = manifestSchema?.trim()
        val manifestDigest: String? =
            if (manifestDigest.isNullOrBlank()) null else manifestDigest.trim()

        internal fun toJson(): Map<String, Any?> {
            val map = LinkedHashMap<String, Any?>()
            map["inspector_public_key"] = inspectorPublicKey
            map["manifest_schema"] = manifestSchema
            map["manifest_version"] = manifestVersion
            map["max_manifest_age_ms"] = maxManifestAgeMs
            map["manifest_digest"] = manifestDigest
            return map
        }

        internal companion object {
            fun fromJson(value: Any?): ProvisionedMetadata? {
                if (value !is Map<*, *>) return null
                val inspector = asOptionalString(value["inspector_public_key"])
                val schema = asOptionalString(value["manifest_schema"])
                if (inspector.isNullOrBlank() || schema.isNullOrBlank()) return null
                val version = asOptionalInteger(value["manifest_version"])
                val maxAge = asOptionalLong(value["max_manifest_age_ms"])
                val digest = asOptionalString(value["manifest_digest"])
                return ProvisionedMetadata(inspector, schema, version, maxAge, digest)
            }
        }
    }

    internal companion object {
        @Suppress("UNCHECKED_CAST")
        fun fromJson(certificateIdHex: String, value: Any?): OfflineVerdictMetadata {
            check(value is Map<*, *>) { "verdict entry for $certificateIdHex is not an object" }
            val map = value as Map<String, Any>
            val controllerId = asString(map["controller_id"], "$certificateIdHex.controller")
            val controllerDisplay =
                asString(map["controller_display"], "$certificateIdHex.controller_display")
            val verdictIdHex = asOptionalString(map["verdict_id_hex"])
            val attestationNonceHex = asOptionalString(map["attestation_nonce_hex"])
            val certificateExpiresAtMs =
                asLong(map["certificate_expires_at_ms"], "$certificateIdHex.certificate_expires_at_ms")
            val policyExpiresAtMs =
                asLong(map["policy_expires_at_ms"], "$certificateIdHex.policy_expires_at_ms")
            val refreshAtMs = asOptionalLong(map["refresh_at_ms"])
            val remainingAmount =
                asOptionalString(map["remaining_amount"]) ?: "0"
            val recordedAtMs = asLong(map["recorded_at_ms"], "$certificateIdHex.recorded_at_ms")
            val integrityPolicy = asOptionalString(map["integrity_policy"])
            val playIntegrity = if ("play_integrity" in map)
                PlayIntegrityMetadata.fromJson(map["play_integrity"]) else null
            val playIntegrityToken = if ("play_integrity_token" in map)
                PlayIntegrityTokenSnapshot.fromJson(map["play_integrity_token"]) else null
            val hms = if ("hms_safety_detect" in map)
                SafetyDetectMetadata.fromJson(map["hms_safety_detect"]) else null
            val token = if ("hms_safety_detect_token" in map)
                SafetyDetectTokenSnapshot.fromJson(map["hms_safety_detect_token"]) else null
            val provisioned = if ("android_provisioned" in map)
                ProvisionedMetadata.fromJson(map["android_provisioned"]) else null
            return OfflineVerdictMetadata(
                certificateIdHex, controllerId, controllerDisplay, verdictIdHex,
                attestationNonceHex, certificateExpiresAtMs, policyExpiresAtMs, refreshAtMs,
                remainingAmount, recordedAtMs, integrityPolicy, playIntegrity, playIntegrityToken,
                hms, token, provisioned,
            )
        }

        private fun asLong(value: Any?, field: String): Long {
            check(value is Number) { "$field is not a number" }
            check(value !is Float && value !is Double) { "$field must be an integer" }
            return value.toLong()
        }

        internal fun asOptionalLong(value: Any?): Long? {
            if (value is Number) {
                if (value is Float || value is Double) return null
                return value.toLong()
            }
            return null
        }

        internal fun asString(value: Any?, field: String): String {
            checkNotNull(value) { "$field is missing" }
            if (value is String) return value
            return value.toString()
        }

        internal fun asOptionalString(value: Any?): String? {
            if (value == null) return null
            return if (value is String) value else value.toString()
        }

        internal fun asStringList(value: Any?): List<String> {
            if (value !is List<*>) return emptyList()
            return value.filterNotNull().map { it.toString() }
        }

        private fun asOptionalInteger(value: Any?): Int? {
            if (value is Number) {
                if (value is Float || value is Double) return null
                return value.toInt()
            }
            if (value == null) return null
            return try {
                value.toString().toInt()
            } catch (_: NumberFormatException) {
                null
            }
        }
    }
}
