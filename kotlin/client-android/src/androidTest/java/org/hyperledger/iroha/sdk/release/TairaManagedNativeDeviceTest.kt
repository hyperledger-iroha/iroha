// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.release

import androidx.test.ext.junit.runners.AndroidJUnit4
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressErrorCode
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.crypto.NativeSignerBridge
import org.hyperledger.iroha.sdk.crypto.SigningAlgorithm
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** Actual packaged native primitives; no provider substitution, skip or fallback. */
@RunWith(AndroidJUnit4::class)
class TairaManagedNativeDeviceTest {
    @Test fun exactAbiSignsAndVerifiesWithNativeAlgorithms() {
        val selector = "${TairaQualificationArguments.MANAGED_CLASS}/" +
            "exactAbiSignsAndVerifiesWithNativeAlgorithms"
        TairaQualificationEvidence.current(selector).arguments.requireNative()
        for (algorithm in SigningAlgorithm.values()) {
            // Disposable public test vectors, unrelated to any owner or wallet authority.
            val seed = IrohaHash.prehash("taira-sdk-native-test/${algorithm.name}/one"
                .toByteArray(StandardCharsets.US_ASCII))
            val otherSeed = IrohaHash.prehash("taira-sdk-native-test/${algorithm.name}/two"
                .toByteArray(StandardCharsets.US_ASCII))
            var privateKey = ByteArray(0)
            var otherPrivateKey = ByteArray(0)
            var signature = ByteArray(0)
            try {
                val (derivedPrivate, publicKey) = NativeSignerBridge.keypairFromSeed(algorithm, seed)
                privateKey = derivedPrivate
                val (foreignPrivate, foreignPublic) =
                    NativeSignerBridge.keypairFromSeed(algorithm, otherSeed)
                otherPrivateKey = foreignPrivate
                assertArrayEquals("Native key derivation disagrees for ${algorithm.name}", publicKey,
                    NativeSignerBridge.publicKeyFromPrivate(algorithm, privateKey))
                val message = IrohaHash.prehash("taira-sdk-native-signature/${algorithm.name}/one"
                    .toByteArray(StandardCharsets.US_ASCII))
                val changedMessage = IrohaHash.prehash("taira-sdk-native-signature/${algorithm.name}/two"
                    .toByteArray(StandardCharsets.US_ASCII))
                signature = NativeSignerBridge.signDetached(algorithm, privateKey, message)
                assertTrue("Empty native signature for ${algorithm.name}", signature.isNotEmpty())
                assertTrue("Native signature did not verify for ${algorithm.name}",
                    NativeSignerBridge.verifyDetached(algorithm, publicKey, message, signature))
                assertFalse("Changed native payload verified for ${algorithm.name}",
                    NativeSignerBridge.verifyDetached(algorithm, publicKey, changedMessage, signature))
                assertFalse("Foreign native key verified for ${algorithm.name}",
                    NativeSignerBridge.verifyDetached(algorithm, foreignPublic, message, signature))
                if (algorithm == SigningAlgorithm.ML_DSA) {
                    assertFalse("Malformed ML-DSA signature verified",
                        NativeSignerBridge.verifyDetached(
                            algorithm, publicKey, message, signature.copyOf(signature.size - 1)))
                    assertFalse("All-zero ML-DSA signature verified",
                        NativeSignerBridge.verifyDetached(
                            algorithm, publicKey, message, ByteArray(signature.size)))
                }
            } finally {
                seed.fill(0); otherSeed.fill(0); privateKey.fill(0)
                otherPrivateKey.fill(0); signature.fill(0)
            }
        }
    }

    @Test fun nativeAccountCodecRejectsWrongNetworkAndMalformedInput() {
        val selector = "${TairaQualificationArguments.MANAGED_CLASS}/" +
            "nativeAccountCodecRejectsWrongNetworkAndMalformedInput"
        TairaQualificationEvidence.current(selector).arguments.requireNative()
        val seed = ByteArray(32) { (it + 37).toByte() }
        var privateKey = ByteArray(0)
        try {
            val (derivedPrivate, publicKey) =
                NativeSignerBridge.keypairFromSeed(SigningAlgorithm.ED25519, seed)
            privateKey = derivedPrivate
            // Every positive constructor reaches the mandatory Rust canonical-account validator.
            val address = AccountAddress.fromAccount(publicKey, "ed25519")
            val literal = address.toI105(TAIRA_DISCRIMINANT)
            assertArrayEquals(address.canonicalBytes,
                AccountAddress.fromI105(literal, TAIRA_DISCRIMINANT).canonicalBytes)
            assertEquals(literal,
                AccountAddress.fromCanonicalBytes(address.canonicalBytes).toI105(TAIRA_DISCRIMINANT))
            reject(AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX) {
                AccountAddress.fromI105(literal, TAIRA_DISCRIMINANT + 1)
            }
            reject(AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES) {
                AccountAddress.fromCanonicalBytes(address.canonicalBytes + byteArrayOf(0))
            }
            reject(AccountAddressErrorCode.INVALID_PUBLIC_KEY) {
                AccountAddress.fromAccount(ByteArray(32), "ed25519")
            }
            reject(null) { AccountAddress.fromI105("not-an-I105-address", TAIRA_DISCRIMINANT) }
        } finally {
            seed.fill(0); privateKey.fill(0)
        }
    }

    private fun reject(expected: AccountAddressErrorCode?, operation: () -> Unit) {
        try {
            operation()
            throw AssertionError("Malformed or wrong-network account was accepted")
        } catch (failure: AccountAddressException) {
            assertFalse("Missing JNI must never count as malformed-input rejection",
                failure.code == AccountAddressErrorCode.NATIVE_BRIDGE_UNAVAILABLE)
            if (expected != null) assertEquals(expected, failure.code)
        }
    }

    companion object { private const val TAIRA_DISCRIMINANT = 369 }
}
