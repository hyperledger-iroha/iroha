// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

/** Hardware security boundary authenticated by an Android KeyDescription. */
enum class OfflineAndroidDeviceSecurityLevelV2(
    internal val noritoDiscriminant: Long,
) {
    /** Key isolated by the device trusted execution environment. */
    TRUSTED_ENVIRONMENT(0),

    /** Key isolated by a discrete StrongBox secure element. */
    STRONG_BOX(1),
    ;

    internal companion object {
        fun fromNoritoDiscriminant(value: Long): OfflineAndroidDeviceSecurityLevelV2 =
            entries.firstOrNull { it.noritoDiscriminant == value }
                ?: throw IllegalArgumentException(
                    "unknown Android device security-level discriminant: $value",
                )
    }
}

/**
 * Exact bounded SDK model of Rust `OfflineAndroidAttestedDevicePropertiesV2`.
 *
 * Empty identity strings and zero version/patch values remain representable because native
 * policy classifies otherwise authenticated but incomplete evidence as drain-only. Call
 * [isCompleteV2] before using the properties to authorize new offline activity.
 */
class OfflineAndroidAttestedDevicePropertiesV2(
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
    private val _verifiedBootKey = verifiedBootKey.copyOf()
    private val _verifiedBootHash = verifiedBootHash.copyOf()

    val verifiedBootKey: ByteArray
        get() = _verifiedBootKey.copyOf()

    val verifiedBootHash: ByteArray
        get() = _verifiedBootHash.copyOf()

    init {
        require(version == VERSION_V2) {
            "Android attested-device properties version must be exactly 2"
        }
        require(attestationVersion in 0..U32_MAX) {
            "attestation_version must fit in u32"
        }
        require(keymintVersion in 0..U32_MAX) {
            "keymint_version must fit in u32"
        }
        require(osVersion in 0..U32_MAX) {
            "os_version must fit in u32"
        }
        require(osPatchLevel in 0..U32_MAX) { "os_patch_level must fit in u32" }
        require(vendorPatchLevel in 0..U32_MAX) { "vendor_patch_level must fit in u32" }
        require(bootPatchLevel in 0..U32_MAX) { "boot_patch_level must fit in u32" }
        for ((field, value) in listOf(
            "brand" to brand,
            "device" to device,
            "product" to product,
            "manufacturer" to manufacturer,
            "model" to model,
        )) {
            requireBoundedProperty(value, field)
        }
        require(_verifiedBootKey.size <= VERIFIED_BOOT_KEY_MAX_BYTES_V2) {
            "verified_boot_key is outside the V2 protocol bound"
        }
        require(_verifiedBootHash.size == VERIFIED_BOOT_HASH_BYTES_V2) {
            "verified_boot_hash must contain exactly 32 bytes"
        }
    }

    /** Whether every property required for testnet eligibility is present and canonical. */
    fun isCompleteV2(): Boolean =
        attestationVersion > 0 &&
            keymintVersion > 0 &&
            osVersion in 1..OS_VERSION_MAX_V2 &&
            canonicalPatchMonth(osPatchLevel) &&
            canonicalPatchDate(vendorPatchLevel) &&
            canonicalPatchDate(bootPatchLevel) &&
            _verifiedBootKey.isNotEmpty() &&
            _verifiedBootHash.any { it.toInt() != 0 } &&
            listOf(brand, device, product, manufacturer, model).all { value ->
                value.isNotEmpty() && value.all { character -> character.code <= 0x7f }
            }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is OfflineAndroidAttestedDevicePropertiesV2) return false
        return version == other.version &&
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
            _verifiedBootKey.contentEquals(other._verifiedBootKey) &&
            _verifiedBootHash.contentEquals(other._verifiedBootHash)
    }

    override fun hashCode(): Int {
        var result = version
        for (value in listOf<Any>(
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
        )) {
            result = 31 * result + value.hashCode()
        }
        result = 31 * result + _verifiedBootKey.contentHashCode()
        result = 31 * result + _verifiedBootHash.contentHashCode()
        return result
    }

    companion object {
        const val VERSION_V2: Int = 2
        const val OS_VERSION_MAX_V2: Long = 999_999
        const val ATTESTED_PROPERTY_MAX_BYTES_V2: Int = 128
        const val VERIFIED_BOOT_KEY_MAX_BYTES_V2: Int = 1_024
        const val VERIFIED_BOOT_HASH_BYTES_V2: Int = 32
        private const val U32_MAX: Long = 0xffff_ffffL

        private fun requireBoundedProperty(value: String, field: String) {
            require(value.toByteArray(Charsets.UTF_8).size <= ATTESTED_PROPERTY_MAX_BYTES_V2) {
                "$field exceeds the V2 protocol bound"
            }
            require(value.none(Char::isISOControl)) {
                "$field must not contain control characters"
            }
            require(value == value.trim()) { "$field must not contain surrounding whitespace" }
        }

        private fun canonicalPatchMonth(value: Long): Boolean {
            if (value !in 0..U32_MAX) return false
            val year = value / 100
            val month = value % 100
            return year in 2_000..9_999 && month in 1..12
        }

        private fun canonicalPatchDate(value: Long): Boolean {
            if (value !in 0..U32_MAX) return false
            val year = value / 10_000
            val month = (value / 100) % 100
            val day = value % 100
            if (year !in 2_000..9_999 || month !in 1..12) return false
            val leap = year % 4 == 0L && (year % 100 != 0L || year % 400 == 0L)
            val maximumDay = when (month) {
                2L -> if (leap) 29 else 28
                4L, 6L, 9L, 11L -> 30
                else -> 31
            }
            return day in 1..maximumDay
        }
    }
}
