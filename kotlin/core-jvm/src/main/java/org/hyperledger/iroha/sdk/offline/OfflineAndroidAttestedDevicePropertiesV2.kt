// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

/** Hardware boundary authenticated by an Android Key Attestation leaf. */
internal enum class OfflineAndroidDeviceSecurityLevelV2(internal val noritoDiscriminant: Long) {
    TRUSTED_ENVIRONMENT(0),
    STRONG_BOX(1),
}

/**
 * Exact ABI22 snapshot of Android build, patch, and verified-boot properties.
 *
 * These values are deliberately absent from the pre-key challenge: KeyMint reveals them only
 * after key generation. Core compares this submitted snapshot with the certificate chain before
 * admitting the registration.
 */
internal class OfflineAndroidAttestedDevicePropertiesV2(
    val version: Int,
    val attestationVersion: Long,
    val keymintVersion: Long,
    val securityLevel: OfflineAndroidDeviceSecurityLevelV2,
    val brand: String,
    val device: String,
    val product: String,
    val manufacturer: String,
    val model: String,
    val osVersion: Long,
    val osPatchLevel: Long,
    val vendorPatchLevel: Long,
    val bootPatchLevel: Long,
    verifiedBootKey: ByteArray,
    verifiedBootHash: ByteArray,
) {
    private val verifiedBootKeySnapshot = verifiedBootKey.copyOf()
    private val verifiedBootHashSnapshot = verifiedBootHash.copyOf()

    val verifiedBootKey: ByteArray get() = verifiedBootKeySnapshot.copyOf()
    val verifiedBootHash: ByteArray get() = verifiedBootHashSnapshot.copyOf()

    init {
        require(version == VERSION) { "Android attested-device property version must be exactly 2" }
        for ((value, field) in listOf(
            attestationVersion to "attestation_version",
            keymintVersion to "keymint_version",
            osVersion to "os_version",
            osPatchLevel to "os_patch_level",
            vendorPatchLevel to "vendor_patch_level",
            bootPatchLevel to "boot_patch_level",
        )) {
            require(value in 1..U32_MAX) { "$field must be a positive u32" }
        }
        for ((value, field) in listOf(
            brand to "brand",
            device to "device",
            product to "product",
            manufacturer to "manufacturer",
            model to "model",
        )) {
            requireCanonicalProperty(value, field)
        }
        require(verifiedBootKeySnapshot.size in 1..MAX_VERIFIED_BOOT_KEY_BYTES) {
            "verified_boot_key must contain 1..$MAX_VERIFIED_BOOT_KEY_BYTES bytes"
        }
        require(verifiedBootHashSnapshot.size == 32 && !allZero(verifiedBootHashSnapshot)) {
            "verified_boot_hash must be one non-zero 32-byte value"
        }
    }

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is OfflineAndroidAttestedDevicePropertiesV2 &&
            version == other.version &&
            attestationVersion == other.attestationVersion &&
            keymintVersion == other.keymintVersion &&
            securityLevel == other.securityLevel &&
            brand == other.brand &&
            device == other.device &&
            product == other.product &&
            manufacturer == other.manufacturer &&
            model == other.model &&
            osVersion == other.osVersion &&
            osPatchLevel == other.osPatchLevel &&
            vendorPatchLevel == other.vendorPatchLevel &&
            bootPatchLevel == other.bootPatchLevel &&
            verifiedBootKeySnapshot.contentEquals(other.verifiedBootKeySnapshot) &&
            verifiedBootHashSnapshot.contentEquals(other.verifiedBootHashSnapshot)

    override fun hashCode(): Int {
        var result = listOf(
            version,
            attestationVersion,
            keymintVersion,
            securityLevel,
            brand,
            device,
            product,
            manufacturer,
            model,
            osVersion,
            osPatchLevel,
            vendorPatchLevel,
            bootPatchLevel,
        ).hashCode()
        result = 31 * result + verifiedBootKeySnapshot.contentHashCode()
        result = 31 * result + verifiedBootHashSnapshot.contentHashCode()
        return result
    }

    companion object {
        const val VERSION: Int = 2
        const val MAX_PROPERTY_UTF8_BYTES: Int = 128
        const val MAX_VERIFIED_BOOT_KEY_BYTES: Int = 1_024
        internal const val U32_MAX: Long = 0xffff_ffffL

        private fun requireCanonicalProperty(value: String, field: String) {
            require(
                value.isNotEmpty() &&
                    value == value.trim() &&
                    value.toByteArray(Charsets.UTF_8).size <= MAX_PROPERTY_UTF8_BYTES &&
                    value.all { it.code in 0x20..0x7e },
            ) {
                "$field must be canonical non-empty printable ASCII within " +
                    "$MAX_PROPERTY_UTF8_BYTES bytes"
            }
        }

        private fun allZero(value: ByteArray): Boolean {
            var aggregate = 0
            value.forEach { aggregate = aggregate or it.toInt() }
            return aggregate == 0
        }
    }
}
