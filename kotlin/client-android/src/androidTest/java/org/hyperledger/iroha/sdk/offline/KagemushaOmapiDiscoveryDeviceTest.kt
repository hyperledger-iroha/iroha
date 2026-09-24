package org.hyperledger.iroha.sdk.offline

import android.os.Build
import android.os.Bundle
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class KagemushaOmapiDiscoveryDeviceTest {
    @Test
    fun discoveryAlwaysReachesATerminalBridgeWithinItsBound() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val required = requiredPixel6EseProfile(InstrumentationRegistry.getArguments())
        if (required != null) {
            assertTrue(
                "The provisioned Pixel 6 eSE gate must run on Google/oriole hardware",
                Build.MANUFACTURER.equals("Google", ignoreCase = true) && Build.DEVICE == "oriole",
            )
        }

        val executor = Executors.newSingleThreadExecutor()
        try {
            val bridge = KagemushaOmapiDeviceLifecycleV1.openAsync(
                instrumentation.targetContext,
                executor,
                configuration = required?.configuration ?: KagemushaOmapiDeviceLifecycleV1.Configuration(),
                discoveryTimeoutMillis = KagemushaOmapiDeviceLifecycleV1.DEFAULT_DISCOVERY_TIMEOUT_MILLIS,
            ).get(15, TimeUnit.SECONDS)

            assertNotNull(bridge.availability)
            if (required == null) {
                if (Build.MANUFACTURER.equals("Google", ignoreCase = true) && Build.DEVICE == "oriole") {
                    Log.i("IrohaKagemushaOmapiProbe", "Pixel 6 applet discovery: ${bridge.availability}")
                }
                return
            }

            assertEquals(
                "Pinned Pixel 6 embedded reader/AID did not admit the complete ABI-23 capability frame",
                KagemushaDeviceLifecycleBridgeV1.Availability.AVAILABLE,
                bridge.availability,
            )
            val capabilities = bridge.capabilities()
            assertNotNull("Available bridge must retain exact capabilities", capabilities)
            assertArrayEquals(
                "Hardware policy ID differs from the provisioned profile",
                required.hardwarePolicyId,
                capabilities!!.hardwarePolicyId(),
            )
            assertArrayEquals(
                "Qualification report digest differs from the provisioned profile",
                required.qualificationReportDigest,
                capabilities.qualificationReportDigest(),
            )
            Log.i("IrohaKagemushaOmapiProbe", "Pixel 6 pinned applet capability admission passed")
        } finally {
            executor.shutdownNow()
        }
    }

    @Test
    fun requiredProfileRejectsMissingOrMalformedPins() {
        assertNull(requiredPixel6EseProfile(Bundle()))

        val valid = profileArguments()
        val profile = requiredPixel6EseProfile(valid)
        assertNotNull(profile)
        assertArrayEquals(
            KagemushaOmapiDeviceLifecycleV1.defaultAppletAid(),
            profile!!.configuration.appletAid,
        )

        val invalid = listOf(
            Bundle(valid).apply { putString(ARG_REQUIRE_PROFILE, "yes") },
            Bundle(valid).apply { remove(ARG_READER) },
            Bundle(valid).apply { putString(ARG_READER, "SIM1") },
            Bundle(valid).apply { putString(ARG_AID, "00") },
            Bundle(valid).apply { putString(ARG_AID, "F04F444A524E000Z") },
            Bundle(valid).apply { putString(ARG_POLICY_ID, "00") },
            Bundle(valid).apply { putString(ARG_POLICY_ID, "00".repeat(32)) },
            Bundle(valid).apply { putString(ARG_REPORT_DIGEST, "00") },
            Bundle(valid).apply { putString(ARG_REPORT_DIGEST, "11".repeat(32)) },
            Bundle(valid).apply { putString(ARG_REQUIRE_PROFILE, "false") },
        )
        for (arguments in invalid) {
            try {
                requiredPixel6EseProfile(arguments)
                fail("Malformed required Pixel 6 eSE profile was accepted")
            } catch (_: IllegalArgumentException) {
                // An invalid pin must fail the gate, not silently run diagnostics.
            }
        }
    }

    private fun profileArguments(): Bundle = Bundle().apply {
        putString(ARG_REQUIRE_PROFILE, "true")
        putString(ARG_READER, "eSE1")
        putString(ARG_AID, "F04F444A524E0001")
        putString(ARG_POLICY_ID, "11".repeat(32))
        putString(ARG_REPORT_DIGEST, "22".repeat(32))
    }

    private fun requiredPixel6EseProfile(arguments: Bundle): RequiredPixel6EseProfile? {
        val requireProfile = arguments.getString(ARG_REQUIRE_PROFILE) ?: "false"
        require(requireProfile == "true" || requireProfile == "false") {
            "$ARG_REQUIRE_PROFILE must be exactly true or false"
        }
        if (requireProfile == "false") {
            require(listOf(ARG_READER, ARG_AID, ARG_POLICY_ID, ARG_REPORT_DIGEST).none { arguments.containsKey(it) }) {
                "Pixel 6 eSE pins require $ARG_REQUIRE_PROFILE=true"
            }
            return null
        }

        fun argument(name: String): String = arguments.getString(name)
            ?: throw IllegalArgumentException("Required Pixel 6 eSE pin $name is missing")

        val configuration = KagemushaOmapiDeviceLifecycleV1.Configuration(
            readerName = argument(ARG_READER),
            appletAid = decodeHexPin(argument(ARG_AID), ARG_AID, 5, 16),
        )
        val hardwarePolicyId = decodeHexPin(argument(ARG_POLICY_ID), ARG_POLICY_ID, 32, 32)
        val qualificationReportDigest = decodeHexPin(argument(ARG_REPORT_DIGEST), ARG_REPORT_DIGEST, 32, 32)
        require(hardwarePolicyId.any { it != 0.toByte() } &&
            qualificationReportDigest.any { it != 0.toByte() } &&
            !hardwarePolicyId.contentEquals(qualificationReportDigest)) {
            "Pixel 6 eSE policy and qualification pins must be non-zero and distinct"
        }
        return RequiredPixel6EseProfile(
            configuration,
            hardwarePolicyId,
            qualificationReportDigest,
        )
    }

    private fun decodeHexPin(value: String, name: String, minimumBytes: Int, maximumBytes: Int): ByteArray {
        require(value.length % 2 == 0 && value.length / 2 in minimumBytes..maximumBytes &&
            value.all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }) {
            "$name must be $minimumBytes..$maximumBytes bytes of ASCII hexadecimal"
        }
        return ByteArray(value.length / 2) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private class RequiredPixel6EseProfile(
        val configuration: KagemushaOmapiDeviceLifecycleV1.Configuration,
        val hardwarePolicyId: ByteArray,
        val qualificationReportDigest: ByteArray,
    )

    private companion object {
        const val ARG_REQUIRE_PROFILE = "kagemusha.requireProvisionedPixel6Ese"
        const val ARG_READER = "kagemusha.pixel6EseReader"
        const val ARG_AID = "kagemusha.pixel6EseAid"
        const val ARG_POLICY_ID = "kagemusha.pixel6EseHardwarePolicyId"
        const val ARG_REPORT_DIGEST = "kagemusha.pixel6EseQualificationReportDigest"
    }
}
