// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import android.os.Bundle
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.ByteArrayInputStream
import java.io.File
import java.math.BigInteger
import java.security.PrivateKey
import java.security.SecureRandom
import java.security.Signature
import java.security.cert.CertificateFactory
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith

/** Isolated physical-device boundary for the future recursive backend.
 * This test has no account, ledger, wallet balance, enrollment or Advance grant.
 * It retains only a fresh test-app hardware key so a host-generated exact receipt
 * can be signed by the same nonexportable key in a later invocation.
 */
@RunWith(AndroidJUnit4::class)
class KagemushaSpartanHardwareProbeTest {
    private fun ByteArray.hex() = joinToString("") { "%02x".format(it.toInt() and 255) }
    private fun unhex(s: String): ByteArray {
        require(s.length % 2 == 0 && s.matches(Regex("[0-9a-f]+")))
        return s.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    }
    private fun rawSignature(der: ByteArray): ByteArray {
        var i = 0
        fun next() = der[i++].toInt() and 255
        require(next() == 48 && next() == der.size - 2 && next() == 2)
        val rn = next(); val r = BigInteger(1, der.copyOfRange(i, i + rn)); i += rn
        require(next() == 2)
        val sn = next(); var s = BigInteger(1, der.copyOfRange(i, i + sn)); i += sn
        require(i == der.size)
        val n = BigInteger("ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551", 16)
        require(r.signum() > 0 && r < n && s.signum() > 0 && s < n)
        if (s > n.shiftRight(1)) s = n - s
        fun scalar(x: BigInteger) = x.toByteArray().takeLast(32).toByteArray().let { ByteArray(32 - it.size) + it }
        return scalar(r) + scalar(s)
    }
    @Test fun retainedHardwareKeySignsExactNativeMessage() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val context = instrumentation.targetContext
        require(context.packageName.startsWith("org.hyperledger.iroha.kagemusha.enrollmentprobe.")) {
            "This probe requires its isolated disposable test application ID"
        }
        val arguments = InstrumentationRegistry.getArguments()
        val slotFile = File(context.filesDir, "spartan-fixture-slot.bin")
        val keyStore = KagemushaWalletAndroidSystemKeyStoreV1()
        val adapter = KagemushaWalletAndroidPlatformAdapterV1(KagemushaWalletAndroidSystemEnvironmentV1(context), keyStore)
        val random = SecureRandom()
        val slot = if (slotFile.exists()) slotFile.readBytes() else ByteArray(32).also {
            random.nextBytes(it)
            val challenge = ByteArray(32).also(random::nextBytes)
            val created = adapter.keyGenerate(it, challenge, KAGEMUSHA_WALLET_ANDROID_KEY_PROFILE_TAG_V1)
            assertTrue("Actual hardware generation failed: $created", created is KagemushaWalletAndroidKeyGenerationV1.Generated)
            File(context.filesDir, "spartan-fixture-attestation-challenge.bin").writeBytes(challenge)
            slotFile.writeBytes(it)
        }
        val key = keyStore.getKey(kagemushaWalletAndroidAliasV1(slot)) as PrivateKey
        assertNull("Payment private key must be nonexportable", key.encoded)
        val facts = keyStore.facts(key)
        assertTrue(facts.insideSecureHardware)
        assertTrue("Software-only key is forbidden", facts.securityLevel == 1 || facts.securityLevel == 2)
        val publicKey = (adapter.keyProbe(slot) as KagemushaWalletAndroidKeyProbeV1.Present).publicKeySec1()
        val chain = (adapter.attestationChain(slot) as KagemushaWalletAndroidAttestationChainV1.Present).certificatesDer()
        val leaf = CertificateFactory.getInstance("X.509").generateCertificate(ByteArrayInputStream(chain.first()))
        val message = arguments.getString("spartanMessage")?.let(::unhex) ?: ByteArray(32).also(random::nextBytes)
        require(message.size == 32)
        val der = (adapter.keySign(slot, message) as KagemushaWalletAndroidSignatureV1.Der).der()
        assertTrue(Signature.getInstance("SHA256withECDSA").run { initVerify(leaf.publicKey); update(message); verify(der) })
        val output = JSONObject().put("schema", "kagemusha-spartan-hardware-probe-v1")
            .put("package", context.packageName).put("public_key_sec1", publicKey.hex())
            .put("message", message.hex()).put("signature_raw_low_s", rawSignature(der).hex())
            .put("signature_der", der.hex()).put("security_level", facts.securityLevel)
            .put("nonexportable", true).put("attestation_challenge", File(context.filesDir, "spartan-fixture-attestation-challenge.bin").readBytes().hex())
            .put("certificate_chain_der", JSONArray(chain.map { it.hex() }))
        File(context.filesDir, "spartan-hardware-probe.json").writeText(output.toString(2))
        instrumentation.sendStatus(0, Bundle().apply { putString("spartanHardwareProbe", output.toString()) })
    }
}
