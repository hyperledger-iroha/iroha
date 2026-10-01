package org.hyperledger.iroha.sdk.crypto.keystore

import android.security.keystore.KeyProperties
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNotEquals
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test

/** Local policy/alias agreement only; no hardware key or attestation is qualified by this suite. */
class KagemushaAndroidHardwareAppKeyPolicyV1Test {
    private fun check(level: Int = KeyProperties.SECURITY_LEVEL_STRONGBOX, origin: Int = KeyProperties.ORIGIN_GENERATED,
        purposes: Int = KeyProperties.PURPOSE_SIGN, digests: Set<String> = setOf(KeyProperties.DIGEST_SHA256),
        remaining: Int = KeyProperties.UNRESTRICTED_USAGE_COUNT, exportable: Boolean = false,
        policy: KagemushaAndroidAppKeyHardwarePolicyV1 = KagemushaAndroidAppKeyHardwarePolicyV1.STRONGBOX_ONLY) =
        requirePersistentHardwareAppKeyV1(level, origin, purposes, digests, remaining, exportable, policy)
    @Test fun persistentGeneratedStrongBoxDoesNotRequireFiniteUsageOrRollbackTags() { check() }
    @Test fun genuineTeeIsAcceptedOnlyUnderItsExplicitPolicy() {
        check(level = KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT, policy = KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX)
        assertThrows(IllegalStateException::class.java) { check(level = KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT) }
        for (policy in KagemushaAndroidAppKeyHardwarePolicyV1.values()) {
            assertThrows(IllegalStateException::class.java) { check(level = KeyProperties.SECURITY_LEVEL_SOFTWARE, policy = policy) }
        }
    }
    @Test fun importedExportableFiniteUseOrOtherPurposeKeysCannotReplaceTheOriginalAppKey() {
        assertThrows(IllegalStateException::class.java) { check(origin = KeyProperties.ORIGIN_IMPORTED) }
        assertThrows(IllegalStateException::class.java) { check(exportable = true) }
        for (count in listOf(0, 1, 10)) assertThrows(IllegalStateException::class.java) { check(remaining = count) }
        for (purpose in listOf(0, KeyProperties.PURPOSE_VERIFY, KeyProperties.PURPOSE_SIGN or KeyProperties.PURPOSE_VERIFY)) {
            assertThrows(IllegalStateException::class.java) { check(purposes = purpose) }
        }
        for (digests in listOf(emptySet(), setOf(KeyProperties.DIGEST_NONE), setOf(KeyProperties.DIGEST_SHA256, KeyProperties.DIGEST_SHA512))) {
            assertThrows(IllegalStateException::class.java) { check(digests = digests) }
        }
    }
    @Test fun aliasRetainsOriginalPreparationAndOwnerAcrossProductsAndRetries() {
        fun alias(account: String = "independently-admitted-owner", nonce: String = "01".repeat(32), prep: ByteArray = ByteArray(273) { 7 }) =
            KagemushaAndroidHardwareAppKeyStoreV1.originalAlias(account, nonce, prep)
        assertEquals(alias(), alias())
        assertNotEquals(alias(), alias(account = "other-admitted-owner"))
        assertNotEquals(alias(), alias(nonce = "02".repeat(32)))
        assertNotEquals(alias(), alias(prep = ByteArray(273) { 8 }))
    }
}
