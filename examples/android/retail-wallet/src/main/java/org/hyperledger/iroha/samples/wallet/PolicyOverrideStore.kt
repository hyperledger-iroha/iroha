package org.hyperledger.iroha.samples.wallet

import android.content.Context
import android.content.SharedPreferences

/**
 * Persists operator-configurable overrides for the POS grace window.
 */
class PolicyOverrideStore(
    private val preferences: SharedPreferences
) {

    constructor(context: Context) : this(
        context.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
    )

    fun current(): SecurityPolicyOverrides {
        val overrideMs = preferences.getLong(KEY_OVERRIDE_MS, -1L).takeIf { it > 0 }
        val profile = preferences.getString(KEY_PROFILE, null)?.takeIf { it.isNotBlank() }
        return SecurityPolicyOverrides(overrideMs, profile)
    }

    fun applyDefault() {
        preferences.edit()
            .remove(KEY_OVERRIDE_MS)
            .remove(KEY_PROFILE)
            .apply()
    }

    fun applyProfile(profileName: String) {
        preferences.edit()
            .remove(KEY_OVERRIDE_MS)
            .putString(KEY_PROFILE, profileName)
            .apply()
    }

    fun applyOverrideMs(durationMs: Long) {
        if (durationMs <= 0) {
            applyDefault()
            return
        }
        preferences.edit()
            .putLong(KEY_OVERRIDE_MS, durationMs)
            .remove(KEY_PROFILE)
            .apply()
    }

    companion object {
        private const val PREF_NAME = "pos_policy_overrides"
        private const val KEY_OVERRIDE_MS = "grace_override_ms"
        private const val KEY_PROFILE = "grace_profile"
    }
}
