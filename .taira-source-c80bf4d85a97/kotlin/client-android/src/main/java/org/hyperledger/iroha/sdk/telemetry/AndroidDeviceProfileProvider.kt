package org.hyperledger.iroha.sdk.telemetry

import android.os.Build
import java.util.Optional

private val ENTERPRISE_MANUFACTURERS = setOf("zebra", "honeywell", "panasonic", "sonim", "datalogic", "bluebird")

/** Device-profile provider that reads `android.os.Build` fields. */
class AndroidDeviceProfileProvider private constructor() : DeviceProfileProvider {

    override fun snapshot(): Optional<DeviceProfile> {
        val fingerprint = Build.FINGERPRINT ?: ""
        val model = Build.MODEL ?: ""
        val manufacturer = Build.MANUFACTURER ?: ""
        val brand = Build.BRAND ?: ""
        val hardware = Build.HARDWARE ?: ""
        val bucket = classify(fingerprint, model, manufacturer, brand, hardware)
        return Optional.of(DeviceProfile.of(bucket))
    }

    companion object {
        private val INSTANCE = AndroidDeviceProfileProvider()

        @JvmStatic
        fun create(): DeviceProfileProvider = INSTANCE

        private fun classify(
            fingerprint: String,
            model: String,
            manufacturer: String,
            brand: String,
            hardware: String,
        ): String {
            if (isEmulator(fingerprint, model, manufacturer, brand, hardware)) return "emulator"
            if (isEnterprise(manufacturer, brand, model)) return "enterprise"
            return "consumer"
        }

        private fun isEmulator(
            fingerprint: String,
            model: String,
            manufacturer: String,
            brand: String,
            hardware: String,
        ): Boolean {
            val normalizedFingerprint = normalize(fingerprint)
            if (normalizedFingerprint.contains("generic")
                || normalizedFingerprint.contains("unknown")
                || normalizedFingerprint.contains("emulator")
            ) return true

            val normalizedModel = normalize(model)
            if (normalizedModel.contains("sdk")
                || normalizedModel.contains("emulator")
                || normalizedModel.contains("gphone")
            ) return true

            val normalizedBrand = normalize(brand)
            val normalizedManufacturer = normalize(manufacturer)
            if (normalizedBrand.startsWith("generic")
                || normalizedManufacturer.startsWith("generic")
                || normalizedManufacturer.contains("android")
            ) return true

            val normalizedHardware = normalize(hardware)
            return normalizedHardware.contains("goldfish")
                || normalizedHardware.contains("ranchu")
                || normalizedHardware.contains("emulator")
        }

        private fun isEnterprise(manufacturer: String, brand: String, model: String): Boolean {
            val normalizedManufacturer = normalize(manufacturer)
            val normalizedBrand = normalize(brand)
            if (ENTERPRISE_MANUFACTURERS.contains(normalizedManufacturer)
                || ENTERPRISE_MANUFACTURERS.contains(normalizedBrand)
            ) return true

            val normalizedModel = normalize(model)
            return normalizedModel.startsWith("tc")
                || normalizedModel.startsWith("ec")
                || normalizedModel.startsWith("et")
        }

        private fun normalize(value: String?): String =
            value?.trim()?.lowercase() ?: ""
    }
}
