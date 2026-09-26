// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import android.util.Base64
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.json.JSONObject
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** Exact issuer-to-JNI conformance on an installed phone; no KeyMint authority is granted. */
@RunWith(AndroidJUnit4::class)
class KagemushaSignedAppPreparationDeviceTest {
    @Test
    fun independentlyIssuedAndroidPreparationVerifiesBeforeKeyMint() {
        val assets = InstrumentationRegistry.getInstrumentation().context.assets
        val fixture = JSONObject(assets.open("kagemusha_signed_app_preparation_android_v1.json")
            .bufferedReader(Charsets.UTF_8).use { it.readText() })
        require(fixture.getString("schema") == "iroha.kagemusha.app-preparation.android.v1")
        val policy = Base64.decode(fixture.getString("canonicalPolicyBase64"), Base64.DEFAULT)
        val policyPin = hex32(fixture.getString("policySha256Hex"))
        val token = Base64.decode(fixture.getString("signedPreparationBase64"), Base64.DEFAULT)
        val selection = KagemushaNativeEnrollmentPhasesV1.Selection(
            fixture.getString("accountI105"),
            ByteArray(32) { 8 },
            hex32(fixture.getString("clientNonceHex")),
            hex32(fixture.getString("releaseIdHex")),
            hex32(fixture.getString("profileIdHex")),
            hex32(fixture.getString("laneIdHex")),
            ByteArray(32) { 9 },
            ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(1).array(),
        )
        val verifier = KagemushaSignedAppPreparationV1.open(policy, policyPin)
        val now = fixture.getLong("trustedNowMs")
        assertArrayEquals(hex32(fixture.getString("serverNonceHex")),
            verifier.verifyForKeyMint(selection, token, now))

        val changedRelease = token.copyOf().also { it[81] = (it[81].toInt() xor 1).toByte() }
        assertTrue(runCatching { verifier.verifyForKeyMint(selection, changedRelease, now) }
            .exceptionOrNull() is IllegalStateException)
        val changedSignature = token.copyOf().also { it[209] = (it[209].toInt() xor 1).toByte() }
        assertTrue(runCatching { verifier.verifyForKeyMint(selection, changedSignature, now) }
            .exceptionOrNull() is IllegalStateException)
        assertTrue(runCatching { verifier.verifyForKeyMint(selection, token, 121_000) }
            .exceptionOrNull() is IllegalStateException)
        assertTrue(runCatching { KagemushaSignedAppPreparationV1.open(policy, ByteArray(32)) }
            .exceptionOrNull() is IllegalArgumentException)
    }

    private fun hex32(value: String): ByteArray {
        require(value.matches(Regex("[0-9a-f]{64}")))
        return ByteArray(32) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }
}
