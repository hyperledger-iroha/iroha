package org.hyperledger.iroha.samples.wallet

import android.content.SharedPreferences
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.Test

class PolicyOverrideStoreTest {

    @Test
    fun `store applies and clears overrides`() {
        val store = PolicyOverrideStore(InMemoryPreferences())
        val empty = store.current()
        assertNull(empty.graceOverrideMs)
        assertNull(empty.graceProfile)

        store.applyProfile("demo")
        val profileState = store.current()
        assertEquals("demo", profileState.graceProfile)
        assertNull(profileState.graceOverrideMs)

        store.applyOverrideMs(90_000L)
        val overrideState = store.current()
        assertEquals(90_000L, overrideState.graceOverrideMs)
        assertNull(overrideState.graceProfile)

        store.applyDefault()
        val reset = store.current()
        assertNull(reset.graceOverrideMs)
        assertNull(reset.graceProfile)
    }

    private class InMemoryPreferences : SharedPreferences {
        private val backing = mutableMapOf<String, Any>()

        override fun getAll(): MutableMap<String, *> = backing

        override fun getString(key: String?, defValue: String?): String? =
            backing[key] as? String ?: defValue

        override fun getStringSet(key: String?, defValues: MutableSet<String>?): MutableSet<String>? =
            backing[key] as? MutableSet<String> ?: defValues

        override fun getInt(key: String?, defValue: Int): Int =
            backing[key] as? Int ?: defValue

        override fun getLong(key: String?, defValue: Long): Long =
            backing[key] as? Long ?: defValue

        override fun getFloat(key: String?, defValue: Float): Float =
            backing[key] as? Float ?: defValue

        override fun getBoolean(key: String?, defValue: Boolean): Boolean =
            backing[key] as? Boolean ?: defValue

        override fun contains(key: String?): Boolean = backing.containsKey(key)

        override fun edit(): SharedPreferences.Editor = object : SharedPreferences.Editor {
            private val pending = mutableMapOf<String, Any?>()
            private val removals = mutableSetOf<String>()

            override fun putString(key: String?, value: String?): SharedPreferences.Editor {
                key?.let { pending[it] = value }
                return this
            }

            override fun putStringSet(
                key: String?,
                values: MutableSet<String>?
            ): SharedPreferences.Editor {
                key?.let { pending[it] = values }
                return this
            }

            override fun putInt(key: String?, value: Int): SharedPreferences.Editor {
                key?.let { pending[it] = value }
                return this
            }

            override fun putLong(key: String?, value: Long): SharedPreferences.Editor {
                key?.let { pending[it] = value }
                return this
            }

            override fun putFloat(key: String?, value: Float): SharedPreferences.Editor {
                key?.let { pending[it] = value }
                return this
            }

            override fun putBoolean(key: String?, value: Boolean): SharedPreferences.Editor {
                key?.let { pending[it] = value }
                return this
            }

            override fun remove(key: String?): SharedPreferences.Editor {
                key?.let { removals.add(it) }
                return this
            }

            override fun clear(): SharedPreferences.Editor {
                backing.clear()
                pending.clear()
                removals.clear()
                return this
            }

            override fun commit(): Boolean {
                apply()
                return true
            }

            override fun apply() {
                removals.forEach { backing.remove(it) }
                pending.forEach { (key, value) ->
                    if (value == null) {
                        backing.remove(key)
                    } else {
                        backing[key] = value
                    }
                }
            }
        }

        override fun registerOnSharedPreferenceChangeListener(
            listener: SharedPreferences.OnSharedPreferenceChangeListener?
        ) { }

        override fun unregisterOnSharedPreferenceChangeListener(
            listener: SharedPreferences.OnSharedPreferenceChangeListener?
        ) { }
    }
}
