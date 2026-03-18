package org.hyperledger.iroha.samples.wallet

import android.content.Context
import java.io.BufferedReader
import java.io.InputStream
import java.util.Locale
import kotlin.sequences.asSequence
import org.json.JSONArray
import org.json.JSONObject

data class SecurityPolicy(
    val version: String,
    val pinnedRoots: List<PinnedRoot>,
    val enforcedCertificateIdHex: String,
    val expectedAttestationNonceHex: String?,
    val verdictGracePeriodMs: Long,
    val gracePeriodSource: GracePeriodSource,
    val gracePeriodProfile: String?,
    val defaultGracePeriodMs: Long,
    val availableGraceProfiles: Map<String, Long>
) {
    data class PinnedRoot(
        val alias: String,
        val sha256: String
    )

    enum class GracePeriodSource {
        POLICY_DEFAULT,
        PROFILE,
        OVERRIDE
    }
}

data class SecurityPolicyOverrides(
    val graceOverrideMs: Long? = null,
    val graceProfile: String? = null
)

object SecurityPolicyLoader {

    private const val POLICY_ASSET = "security_policy.json"

    fun load(
        context: Context,
        overrides: SecurityPolicyOverrides? = null
    ): SecurityPolicy {
        val raw = context.assets.open(POLICY_ASSET).use(InputStream::bufferedReader).use(BufferedReader::readText)
        val overrideMs = overrides?.graceOverrideMs
            ?: BuildConfig.VERDICT_GRACE_PERIOD_OVERRIDE_MS.takeIf { it > 0L }
        val profile = overrides?.graceProfile
            ?: BuildConfig.VERDICT_GRACE_PERIOD_PROFILE.takeIf { it.isNotBlank() }
        return parse(raw, SecurityPolicyOverrides(overrideMs, profile))
    }

    fun parse(rawJson: String, overrides: SecurityPolicyOverrides?): SecurityPolicy {
        val root = JSONObject(rawJson)
        val version = root.optString("policy_version", "unversioned")
        val certificateId = root.getString("enforced_certificate_id_hex")
        val expectedNonce = root.optString("expected_attestation_nonce_hex", null)
        val defaultGrace = root.optLong("verdict_grace_period_ms", 0L)
        val graceProfiles = root.optJSONObject("grace_period_profiles")
        val availableProfiles = mutableMapOf<String, Long>()
        graceProfiles?.keys()?.asSequence()?.forEach { key ->
            val value = graceProfiles.optLong(key, Long.MIN_VALUE)
            if (value > 0) {
                availableProfiles[key] = value
            }
        }
        val (grace, graceSource, activeProfile) = resolveGracePeriod(
            defaultGrace,
            availableProfiles,
            overrides?.graceOverrideMs,
            overrides?.graceProfile
        )
        val pinnedRootsJson = root.optJSONArray("pinned_roots") ?: JSONArray()
        val pinnedRoots = mutableListOf<SecurityPolicy.PinnedRoot>()
        for (index in 0 until pinnedRootsJson.length()) {
            val entry = pinnedRootsJson.getJSONObject(index)
            val alias = entry.getString("alias")
            val sha256 = entry.getString("sha256")
            pinnedRoots.add(
                SecurityPolicy.PinnedRoot(
                    alias = alias,
                    sha256 = sha256.lowercase(Locale.US)
                )
            )
        }
        return SecurityPolicy(
            version = version,
            pinnedRoots = pinnedRoots,
            enforcedCertificateIdHex = certificateId.lowercase(Locale.US),
            expectedAttestationNonceHex = expectedNonce?.lowercase(Locale.US),
            verdictGracePeriodMs = grace,
            gracePeriodSource = graceSource,
            gracePeriodProfile = activeProfile,
            defaultGracePeriodMs = defaultGrace,
            availableGraceProfiles = availableProfiles.toMap()
        )
    }

    private fun resolveGracePeriod(
        defaultGrace: Long,
        profiles: Map<String, Long>,
        graceOverrideMs: Long?,
        requestedProfile: String?
    ): Triple<Long, SecurityPolicy.GracePeriodSource, String?> {
        val override = graceOverrideMs?.takeIf { it > 0 }
        if (override != null) {
            return Triple(override, SecurityPolicy.GracePeriodSource.OVERRIDE, null)
        }
        if (!requestedProfile.isNullOrBlank() && profiles.isNotEmpty()) {
            val matching = profiles.entries.firstOrNull { entry ->
                entry.key.equals(requestedProfile, ignoreCase = true)
            }
            if (matching != null) {
                return Triple(matching.value, SecurityPolicy.GracePeriodSource.PROFILE, matching.key)
            }
        }
        return Triple(
            defaultGrace,
            SecurityPolicy.GracePeriodSource.POLICY_DEFAULT,
            null
        )
    }
}
