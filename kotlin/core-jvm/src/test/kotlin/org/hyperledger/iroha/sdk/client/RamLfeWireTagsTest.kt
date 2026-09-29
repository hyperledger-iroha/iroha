package org.hyperledger.iroha.sdk.client

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/** JSON and canonical receipt codecs use exactly the same first-release tags. */
class RamLfeWireTagsTest {
    @Test
    fun jsonNamesAndNoritoDiscriminantsAgree() {
        listOf("hkdf-sha3-512-prf-v1", "bfv-affine-v1", "bfv-programmed-v1").forEachIndexed { tag, name ->
            assertEquals(name, RamLfeWireTags.parseBackend(name, "backend"))
            assertEquals(tag, RamLfeWireTags.backendTag(name))
            assertEquals(name, RamLfeWireTags.backendName(tag))
        }
        listOf("signed", "proof").forEachIndexed { tag, name ->
            assertEquals(name, RamLfeWireTags.parseVerificationMode(name, "verification_mode"))
            assertEquals(tag, RamLfeWireTags.verificationModeTag(name))
            assertEquals(name, RamLfeWireTags.verificationModeName(tag))
        }
    }

    @Test
    fun retiredUnknownAndNonCanonicalNamesAreRejectedByBothCodecs() {
        for (name in listOf("bfv-affine-sha3-256-v1", "bfv-programmed-sha3-256-v1", "unknown", "", " bfv-affine-v1", "BFV-AFFINE-V1")) {
            assertFailsWith<IllegalStateException> { RamLfeWireTags.parseBackend(name, "backend") }
            assertFailsWith<IllegalArgumentException> { RamLfeWireTags.backendTag(name) }
        }
        for (name in listOf("unknown", "ivm-proved", "", " signed", "Signed")) {
            assertFailsWith<IllegalStateException> { RamLfeWireTags.parseVerificationMode(name, "verification_mode") }
            assertFailsWith<IllegalArgumentException> { RamLfeWireTags.verificationModeTag(name) }
        }
    }

    @Test
    fun absentAndNonStringMetadataIsRejected() {
        for (value in listOf(null, 0, true, emptyMap<String, String>())) {
            assertFailsWith<IllegalStateException> { RamLfeWireTags.parseBackend(value, "backend") }
            assertFailsWith<IllegalStateException> { RamLfeWireTags.parseVerificationMode(value, "verification_mode") }
        }
    }

    @Test
    fun unknownNoritoDiscriminantsAreRejected() {
        for (tag in listOf(-1, 3, Int.MAX_VALUE)) {
            assertFailsWith<IllegalArgumentException> { RamLfeWireTags.backendName(tag) }
        }
        for (tag in listOf(-1, 2, Int.MAX_VALUE)) {
            assertFailsWith<IllegalArgumentException> { RamLfeWireTags.verificationModeName(tag) }
        }
    }
}
