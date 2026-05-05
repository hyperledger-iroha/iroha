package org.hyperledger.iroha.sdk.telemetry

/** Immutable descriptor for `android.telemetry.device_profile`. */
class DeviceProfile private constructor(
    @JvmField val bucket: String,
) {

    override fun toString(): String = "DeviceProfile{bucket='$bucket'}"

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is DeviceProfile) return false
        return bucket == other.bucket
    }

    override fun hashCode(): Int = bucket.hashCode()

    companion object {
        @JvmStatic
        fun of(bucket: String): DeviceProfile {
            val normalized = bucket.trim().lowercase()
            require(normalized.isNotEmpty()) { "Device profile bucket must be non-empty" }
            return DeviceProfile(normalized)
        }
    }
}
