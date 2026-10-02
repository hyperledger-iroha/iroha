// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.crypto.keystore

import android.security.keystore.KeyProperties
import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files
import java.nio.file.Paths
import java.security.KeyPairGenerator
import java.security.MessageDigest
import java.security.Signature
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Software crypto/shape specimens only; no native capability, hardware key or release qualification. */
class KagemushaAndroidAppSignatureOriginalV1Test {
    private fun vector(name: String): ByteArray {
        val path = sequenceOf("fixtures", "../fixtures", "../../fixtures").map {
            Paths.get(it, "offline/kagemusha_app_platform_messages_v1.tsv")
        }.first { Files.isRegularFile(it) }
        val hex = Files.readAllLines(path).first { it.startsWith("$name\t") }.substringAfter('\t')
        return hex.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    }
    private fun key() = KeyPairGenerator.getInstance("EC").apply {
        initialize(ECGenParameterSpec("secp256r1"))
    }.generateKeyPair()
    private fun verifies(public: ECPublicKey, message: ByteArray, der: ByteArray): Boolean =
        Signature.getInstance("SHA256withECDSA").run { initVerify(public); update(message); verify(der) }

    @Test fun originalAndroidEquationSignsExactWAndEWithoutChangingDer() {
        val key = key(); val public = key.public as ECPublicKey
        for ((name, purpose) in listOf(
            "w_mint_fold_9" to KagemushaAndroidAppSignaturePurposeV1.OPERATION_APPROVAL,
            "e_enrollment_android" to KagemushaAndroidAppSignaturePurposeV1.IDENTITY_ENROLLMENT_POSSESSION,
        )) {
            val original = vector(name)
            var guards = 0
            val der = signOriginalAndroidAppMessageV1(key.private, public, original, purpose) { guards++ }
            assertEquals(3, guards)
            requireOriginalP256DerV1(der)
            assertTrue(verifies(public, original, der))
            assertFalse(verifies(public, vector("s_mint_fold_9"), der))
            val changed = original.copyOf().also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() }
            assertFalse(verifies(public, changed, der))
        }
    }

    @Test fun allTenActualRustModelVectorsMatchExactKotlinFinancialAndApprovalLayout() {
        val key = key(); val public = key.public as ECPublicKey
        for ((operation, tag) in listOf("mint_fold" to 1, "send_split" to 2, "receive_fold" to 3, "redeem_split" to 4, "rotate" to 5)) {
            for (before in listOf("9", BigInteger.ONE.shiftLeft(128).subtract(BigInteger.valueOf(2)).toString())) {
                val name = operation + "_" + before; val s = vector("s_" + name); val w = vector("w_" + name)
                assertEquals(460, s.size); assertEquals(tag, s[331].toInt())
                val low = BigInteger(1, s.copyOfRange(428, 444).reversedArray())
                val high = BigInteger(1, s.copyOfRange(444, 460).reversedArray())
                assertEquals(BigInteger(before), low); assertEquals(low.add(BigInteger.ONE), high)
                assertEquals(tag == 2 || tag == 4, s.copyOfRange(364, 396).any { it != 0.toByte() })
                assertEquals(tag == 2 || tag == 4, s.copyOfRange(396, 428).any { it != 0.toByte() })
                org.hyperledger.iroha.sdk.crypto.keystore.attestation.requireKagemushaCoreSelectionFrameV1(
                    s, s.copyOfRange(219, 251), s.copyOfRange(428, 444), s.copyOfRange(444, 460))
                val sHash = MessageDigest.getInstance("SHA-256").digest(s)
                assertContentEquals(vector("s_" + name + "_sha256"), sHash)
                val start = KagemushaAndroidAppSignaturePurposeV1.OPERATION_APPROVAL.domain.toByteArray(Charsets.US_ASCII).size + 8
                val fields = listOf(ByteArray(32) { (0x20 + tag).toByte() }, ByteArray(32) { 0x22 }, vector("account_binding"),
                    ByteArray(32) { 0x24 }, ByteArray(32) { 0x25 }, ByteArray(32) { 0x26 }, sHash, ByteArray(32) { 0x28 })
                val exact = ByteBuffer.allocate(325).order(ByteOrder.LITTLE_ENDIAN)
                    .put(KagemushaAndroidAppSignaturePurposeV1.OPERATION_APPROVAL.domain.toByteArray(Charsets.US_ASCII))
                    .putLong(275).putShort(1).put(1)
                fields.forEach { exact.put(it) }; exact.putLong(1000).putLong(2000)
                assertEquals(start + 275, exact.position()); assertContentEquals(w, exact.array())
                assertContentEquals(vector("w_" + name + "_sha256"), MessageDigest.getInstance("SHA-256").digest(w))
                val der = signOriginalAndroidAppMessageV1(key.private, public, w, KagemushaAndroidAppSignaturePurposeV1.OPERATION_APPROVAL) {}
                assertTrue(verifies(public, w, der)); assertFalse(verifies(public, s, der))
            }
        }
    }

    @Test fun heldKeyCannotBeSubstitutedForAnotherNativeKeyIdentity() {
        val point = byteArrayOf(4) + ByteArray(64) { 17 }
        val keyId = MessageDigest.getInstance("SHA-256").digest(point)
        requireOriginalAppKeyBindingV1(point, keyId, point.copyOf(), keyId.copyOf())
        assertFailsWith<IllegalArgumentException> { requireOriginalAppKeyBindingV1(point, keyId,
            point.copyOf().also { it[1] = 18 }, keyId) }
        assertFailsWith<IllegalArgumentException> { requireOriginalAppKeyBindingV1(point, keyId,
            point, keyId.copyOf().also { it[0] = (it[0].toInt() xor 1).toByte() }) }
        assertFailsWith<IllegalArgumentException> { requireOriginalAppKeyBindingV1(point, ByteArray(32) { 1 },
            point, ByteArray(32) { 1 }) }
    }

    @Test fun nativePurposeAndAllFixedSelectorsMustRemainPresent() {
        val w = vector("w_mint_fold_9")
        assertFailsWith<IllegalArgumentException> { requireAppPlatformSigningMessageV1(w,
            KagemushaAndroidAppSignaturePurposeV1.IDENTITY_ENROLLMENT_POSSESSION) }
        assertFailsWith<IllegalArgumentException> { requireAppPlatformSigningMessageV1(vector("s_mint_fold_9"),
            KagemushaAndroidAppSignaturePurposeV1.OPERATION_APPROVAL) }
        val purpose = KagemushaAndroidAppSignaturePurposeV1.OPERATION_APPROVAL
        val start = purpose.domain.toByteArray().size + 8
        for (field in 0 until 8) {
            val changed = w.copyOf().also { it.fill(0, start + 3 + field * 32, start + 3 + (field + 1) * 32) }
            assertFailsWith<IllegalArgumentException> { requireAppPlatformSigningMessageV1(changed, purpose) }
        }
        for (changed in listOf(w + byteArrayOf(0), w.copyOfRange(1, w.size),
            w.copyOf().also { it[start] = 2 }, w.copyOf().also { it[start + 2] = 2 })) {
            assertFailsWith<IllegalArgumentException> { requireAppPlatformSigningMessageV1(changed, purpose) }
        }
    }

    @Test fun enrollmentNoncesAndOriginalIntervalCannotBeRenewed() {
        val purpose = KagemushaAndroidAppSignaturePurposeV1.IDENTITY_ENROLLMENT_POSSESSION
        val e = vector("e_enrollment_marker11"); val start = purpose.domain.toByteArray().size + 8
        val repeated = e.copyOf().also { it.copyInto(it, start + 67, start + 35, start + 67) }
        assertFailsWith<IllegalArgumentException> { requireAppPlatformSigningMessageV1(repeated, purpose) }
        val times = start + 3 + 11 * 32
        for ((issued, expires) in listOf(0L to 1L, 1000L to 1000L, 1000L to 999L, 1000L to 121001L)) {
            val changed = e.copyOf().also { ByteBuffer.wrap(it).order(ByteOrder.LITTLE_ENDIAN).putLong(times, issued).putLong(times + 8, expires) }
            assertFailsWith<IllegalArgumentException> { requireAppPlatformSigningMessageV1(changed, purpose) }
        }
    }

    @Test fun highSPlatformOriginalRemainsByteIdentical() {
        // A well-shaped high-S original must remain unchanged; verification may normalize separately.
        val order = BigInteger("ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551", 16)
        val high = order.subtract(BigInteger.ONE).toByteArray()
        val der = byteArrayOf(0x30, (5 + high.size).toByte(), 2, 1, 1, 2, high.size.toByte()) + high
        val retained = der.copyOf(); requireOriginalP256DerV1(der); assertContentEquals(retained, der)
        for (bad in listOf(der + byteArrayOf(0), byteArrayOf(0x30, 6, 2, 1, 0, 2, 1, 1),
            byteArrayOf(0x30, 7, 2, 2, 0, 1, 2, 1, 1), ByteArray(73))) {
            assertFailsWith<IllegalArgumentException> { requireOriginalP256DerV1(bad) }
        }
    }

    @Test fun staleOwnerOrWrongHeldPublicKeyCannotPublishGeneratedEvidence() {
        val key = key(); val w = vector("w_mint_fold_9"); var guards = 0
        assertFailsWith<IllegalStateException> {
            signOriginalAndroidAppMessageV1(key.private, key.public as ECPublicKey, w,
                KagemushaAndroidAppSignaturePurposeV1.OPERATION_APPROVAL) {
                guards++; check(guards < 3) { "original native owner changed" }
            }
        }
        assertEquals(3, guards)
        assertFailsWith<IllegalStateException> {
            signOriginalAndroidAppMessageV1(key.private, key().public as ECPublicKey, w,
                KagemushaAndroidAppSignaturePurposeV1.OPERATION_APPROVAL) {}
        }
    }

    @Test fun signingCopiesNativeMessageBeforeGuardCallbacksCanMutateProjection() {
        val key = key(); val public = key.public as ECPublicKey
        val message = vector("w_mint_fold_9"); val retained = message.copyOf()
        val der = signOriginalAndroidAppMessageV1(key.private, public, message,
            KagemushaAndroidAppSignaturePurposeV1.OPERATION_APPROVAL) { message[message.lastIndex] = 1 }
        assertTrue(verifies(public, retained, der)); assertFalse(verifies(public, message, der))
    }

    /** Synthetic purpose2 cash specimen derived from the maintained Rust model vector.
     * Public W/S correlation is data only and cannot authorize hardware signing or Native cash.
     */
    private fun ordinaryPreparationSpecimen(): Pair<ByteArray, ByteArray> {
        val w = vector("w_send_split_9")
        val s = vector("s_send_split_9")
        // The ordinary credential/enrollment identity is common to W and S.
        w.copyInto(s, 155, 213, 245)
        s.fill(0, 364, 428) // Before candidate/terminal selection, both commitments are absent.
        w[52] = 2
        MessageDigest.getInstance("SHA-256").digest(s).copyInto(w, 245)
        val binding = org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryCashApprovalOriginalBindingV1(
            w.copyOfRange(53,85), w.copyOfRange(117,149), w.copyOfRange(149,181),
            w.copyOfRange(181,213), w.copyOfRange(213,245), w.copyOfRange(277,309), s)
        val projected = org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryCashApprovalProjectionV1
            .requirePreparation(w, s, binding)
        assertContentEquals(w, projected.signingBytes())
        assertContentEquals(s, projected.selectionBytes())
        return w to s
    }

    @Test fun ordinaryPreparationSignsExactPurposeTwoAndCannotUseAnotherSignerPurpose() {
        val (w,s) = ordinaryPreparationSpecimen()
        val purpose = KagemushaAndroidAppSignaturePurposeV1.ORDINARY_PREPARATION_APPROVAL
        val key = key(); val public = key.public as ECPublicKey
        var guards = 0
        val der = signOriginalAndroidAppMessageV1(key.private, public, w, purpose) { guards++ }
        assertEquals(3, guards); requireOriginalP256DerV1(der)
        assertTrue(verifies(public, w, der)); assertFalse(verifies(public, s, der))
        for (other in listOf(KagemushaAndroidAppSignaturePurposeV1.OPERATION_APPROVAL,
            KagemushaAndroidAppSignaturePurposeV1.ORDINARY_BOOTSTRAP_APPROVAL,
            KagemushaAndroidAppSignaturePurposeV1.IDENTITY_ENROLLMENT_POSSESSION)) {
            assertFailsWith<IllegalArgumentException> { requireAppPlatformSigningMessageV1(w, other) }
        }
        assertFailsWith<IllegalArgumentException> {
            requireAppPlatformSigningMessageV1(vector("w_send_split_9"), purpose)
        }
        assertFailsWith<IllegalArgumentException> {
            requireAppPlatformSigningMessageV1(vector("e_enrollment_android"), purpose)
        }
        requireAppPlatformSigningMessageV1(vector("w_send_split_9"),
            KagemushaAndroidAppSignaturePurposeV1.OPERATION_APPROVAL)
        requireAppPlatformSigningMessageV1(vector("w_send_split_9"),
            KagemushaAndroidAppSignaturePurposeV1.ORDINARY_BOOTSTRAP_APPROVAL)
        requireAppPlatformSigningMessageV1(vector("e_enrollment_android"),
            KagemushaAndroidAppSignaturePurposeV1.IDENTITY_ENROLLMENT_POSSESSION)
    }

    @Test fun ordinaryPreparationKeepsExactFramingAllSelectorsAndBoundedOriginalInterval() {
        val (w,_) = ordinaryPreparationSpecimen()
        val purpose = KagemushaAndroidAppSignaturePurposeV1.ORDINARY_PREPARATION_APPROVAL
        val start = purpose.domain.toByteArray(Charsets.US_ASCII).size + 8
        for (field in 0 until 8) {
            val changed = w.copyOf().also { it.fill(0, start + 3 + field*32, start + 3 + (field+1)*32) }
            assertFailsWith<IllegalArgumentException> { requireAppPlatformSigningMessageV1(changed, purpose) }
        }
        for (tag in listOf(0,1,3,255)) {
            val changed = w.copyOf().also { it[start+2] = tag.toByte() }
            assertFailsWith<IllegalArgumentException> { requireAppPlatformSigningMessageV1(changed, purpose) }
        }
        for (changed in listOf(w+byteArrayOf(0), w.copyOf(w.size-1),
            w.copyOf().also { it[0] = 0 }, w.copyOf().also { it[start-8] = 0 },
            w.copyOf().also { it[start] = 2 })) {
            assertFailsWith<IllegalArgumentException> { requireAppPlatformSigningMessageV1(changed, purpose) }
        }
        for ((issued,expires) in listOf(0L to 1L, 1000L to 1000L, 1000L to 999L, 1000L to 121001L)) {
            val changed = w.copyOf().also { ByteBuffer.wrap(it).order(ByteOrder.LITTLE_ENDIAN)
                .putLong(309,issued).putLong(317,expires) }
            assertFailsWith<IllegalArgumentException> { requireAppPlatformSigningMessageV1(changed, purpose) }
        }
    }

    @Test fun teeOnlyNativePolicyDoesNotBroadenToStrongBoxOrSoftware() {
        fun admitted(level: Int, policy: KagemushaAndroidAppKeyHardwarePolicyV1) =
            requirePersistentHardwareAppKeyV1(level, KeyProperties.ORIGIN_GENERATED, KeyProperties.PURPOSE_SIGN,
                setOf(KeyProperties.DIGEST_SHA256), KeyProperties.UNRESTRICTED_USAGE_COUNT, false, policy)
        admitted(KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT, KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY)
        assertFailsWith<IllegalStateException> { admitted(KeyProperties.SECURITY_LEVEL_STRONGBOX, KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY) }
        assertFailsWith<IllegalStateException> { admitted(KeyProperties.SECURITY_LEVEL_SOFTWARE, KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY) }
        assertFailsWith<IllegalStateException> { admitted(KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT, KagemushaAndroidAppKeyHardwarePolicyV1.STRONGBOX_ONLY) }
    }
}
