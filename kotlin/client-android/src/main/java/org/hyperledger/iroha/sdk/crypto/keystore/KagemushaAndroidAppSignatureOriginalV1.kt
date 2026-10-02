// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.crypto.keystore

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import java.security.PrivateKey
import java.security.Signature
import java.security.interfaces.ECPublicKey
import org.hyperledger.iroha.sdk.offline.KagemushaNativePreparedAppApprovalV1

/** Internal framing checks do not produce a native prepared capability or monetary authority. */
internal enum class KagemushaAndroidAppSignaturePurposeV1(val domain: String, val bodyBytes: Int, val fields: Int, val signedPurpose: Int = 1) {
    ORDINARY_MINT_PRE_DEBIT_APPROVAL("iroha:kagemusha:v1:ordinary-mint-pre-debit-approval\u0000", 210, 6),
    ORDINARY_INTEGRITY_REFRESH_POSSESSION("iroha:kagemusha:v1:play-integrity-refresh-possession\u0000", 0, 0),
    FIRST_DEVICE_HARDWARE_POSSESSION("iroha:kagemusha:v1:hardware-evidence-possession\u0000", 308, 7),
    OPERATION_APPROVAL("iroha:kagemusha:v1:app-operation-approval\u0000", 275, 8),
    ORDINARY_INCOMING_TERMINAL_APPROVAL("iroha:kagemusha:v1:app-operation-approval\u0000", 275, 8),
    ORDINARY_BOOTSTRAP_APPROVAL("iroha:kagemusha:v1:app-operation-approval\u0000", 275, 8),
    ORDINARY_CASH_TERMINAL_APPROVAL("iroha:kagemusha:v1:app-operation-approval\u0000", 275, 8),
    IDENTITY_ENROLLMENT_POSSESSION("iroha:kagemusha:v1:app-enrollment-possession\u0000", 371, 11),
    // Native cash selection must correlate exact W/S before selecting this private purpose.
    ORDINARY_PREPARATION_APPROVAL("iroha:kagemusha:v1:app-operation-approval\u0000", 275, 8, 2),
}

internal fun requireAppPlatformSigningMessageV1(original: ByteArray, purpose: KagemushaAndroidAppSignaturePurposeV1) {
    if(purpose == KagemushaAndroidAppSignaturePurposeV1.ORDINARY_MINT_PRE_DEBIT_APPROVAL) {
        org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryMintApprovalProjectionV1.requireOriginal(original);return
    }
    if(purpose == KagemushaAndroidAppSignaturePurposeV1.ORDINARY_INTEGRITY_REFRESH_POSSESSION) {
        val outer=purpose.domain.toByteArray(Charsets.US_ASCII)
        val inner="iroha:kagemusha:v1:play-integrity-refresh-challenge\u0000".toByteArray(Charsets.US_ASCII)
        val width=inner.size+8+450
        require(original.size==outer.size+8+width && original.copyOfRange(0,outer.size).contentEquals(outer))
        require(ByteBuffer.wrap(original,outer.size,8).order(ByteOrder.LITTLE_ENDIAN).long==width.toLong())
        val body=outer.size+8
        require(original.copyOfRange(body,body+inner.size).contentEquals(inner) &&
            ByteBuffer.wrap(original,body+inner.size,8).order(ByteOrder.LITTLE_ENDIAN).long==450L)
        val raw=original.copyOfRange(body+inner.size+8,original.size)+ByteArray(64)
        val shape=org.hyperledger.iroha.sdk.offline.KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(raw)
        require(shape.possessionSigningBytes().contentEquals(original));return
    }
    val domain = purpose.domain.toByteArray(Charsets.US_ASCII)
    require(original.size == domain.size + 8 + purpose.bodyBytes &&
        original.copyOfRange(0, domain.size).contentEquals(domain)) { "Native app signing purpose or message length differs" }
    val body = domain.size + 8
    val reader = ByteBuffer.wrap(original).order(ByteOrder.LITTLE_ENDIAN)
    require(reader.getLong(domain.size) == purpose.bodyBytes.toLong() && reader.getShort(body).toInt() == 1 &&
        original[body + 2] == purpose.signedPurpose.toByte()) { "Native app signing version or fixed body differs" }
    repeat(purpose.fields) { field ->
        val start = body + 3 + field * 32
        require(original.copyOfRange(start, start + 32).any { it != 0.toByte() }) { "Native app signing selector is absent" }
    }
    if (purpose == KagemushaAndroidAppSignaturePurposeV1.IDENTITY_ENROLLMENT_POSSESSION) {
        require(!MessageDigest.isEqual(original.copyOfRange(body + 35, body + 67),
            original.copyOfRange(body + 67, body + 99))) { "Native enrollment nonces must differ" }
    }
    var times = body + 3 + purpose.fields * 32
    if (purpose == KagemushaAndroidAppSignaturePurposeV1.FIRST_DEVICE_HARDWARE_POSSESSION) {
        val point = original.copyOfRange(times, times + 65)
        val keyId = original.copyOfRange(body + 3 + 5 * 32, body + 3 + 6 * 32)
        requireOriginalAppKeyBindingV1(point, keyId, point, keyId)
        times += 65
    }
    val issued = reader.getLong(times).toULong()
    val expires = reader.getLong(times + 8).toULong()
    val maximumLifetime = if (purpose == KagemushaAndroidAppSignaturePurposeV1.ORDINARY_PREPARATION_APPROVAL) 10_000uL else 120_000uL
    require(issued != 0uL && expires > issued && expires - issued <= maximumLifetime) { "Native app signing interval differs" }
}

/** Fixed purpose2 consumer for the opaque phase1 cash holder. The separate phase8 Bootstrap
 * type cannot enter here. Shape checks precede the durable fence; unknown outcomes never reset it.
 * This internal platform callback is shared with DATA-only unit controls, not an owner factory.
 */
internal fun approveNativeOrdinaryPreparationOriginalV1(prepared: KagemushaNativePreparedAppApprovalV1,
    signOriginal: (String, ByteArray, ByteArray, ByteArray, ByteArray, KagemushaAndroidAppKeyHardwarePolicyV1,
        KagemushaAndroidAppSignaturePurposeV1, () -> Unit) -> ByteArray): ByteArray {
    val purpose = KagemushaAndroidAppSignaturePurposeV1.ORDINARY_PREPARATION_APPROVAL
    val original = prepared.signingBytes()
    try {
        requireAppPlatformSigningMessageV1(original, purpose)
        return prepared.performPlatformSigning { alias, generationChallenge, point, keyId, message, policy, guard ->
            requireAppPlatformSigningMessageV1(message, purpose)
            check(MessageDigest.isEqual(original, message)) { "The Native-held preparation message changed" }
            signOriginal(alias, generationChallenge, point, keyId, message, policy, purpose, guard)
        }.copyOf()
    } finally { original.fill(0) }
}

/** Real terminal consumer validates the fixed purpose before Main can fsync its OS fence. */
internal fun approveNativeOrdinaryTerminalOriginalV1(prepared: org.hyperledger.iroha.sdk.offline.KagemushaNativePreparedOrdinaryTerminalApprovalV1,
    signOriginal: (String, ByteArray, ByteArray, ByteArray, ByteArray, KagemushaAndroidAppKeyHardwarePolicyV1,
        KagemushaAndroidAppSignaturePurposeV1, () -> Unit) -> ByteArray): ByteArray {
    val purpose = KagemushaAndroidAppSignaturePurposeV1.ORDINARY_CASH_TERMINAL_APPROVAL
    val original = prepared.signingBytes()
    try {
        requireAppPlatformSigningMessageV1(original, purpose)
        return prepared.performPlatformSigning { alias, challenge, point, keyId, message, policy, guard ->
            requireAppPlatformSigningMessageV1(message, purpose)
            check(MessageDigest.isEqual(original, message)) { "The Native-held terminal message changed" }
            signOriginal(alias, challenge, point, keyId, message, policy, purpose, guard)
        }.copyOf()
    } finally { original.fill(0) }
}

internal fun requireOriginalAppKeyBindingV1(point: ByteArray, keyId: ByteArray, expectedPoint: ByteArray, expectedKeyId: ByteArray) {
    require(point.size == 65 && point[0] == 4.toByte() && expectedPoint.size == 65 && expectedPoint[0] == 4.toByte() &&
        keyId.size == 32 && expectedKeyId.size == 32 &&
        MessageDigest.isEqual(point, expectedPoint) && MessageDigest.isEqual(keyId, expectedKeyId) &&
        MessageDigest.isEqual(keyId, MessageDigest.getInstance("SHA-256").digest(point))) { "The held app key differs from the native enrolled original" }
}

/** Preserve the actual platform DER, including high-S originals; Rust normalizes only to verify. */
internal fun requireOriginalP256DerV1(original: ByteArray) {
    require(original.size in 8..72 && original[0] == 0x30.toByte() &&
        (original[1].toInt() and 0xff) == original.size - 2) { "Original P-256 DER shape differs" }
    val order = BigInteger("ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551", 16)
    var offset = 2
    repeat(2) {
        require(offset + 2 <= original.size && original[offset++] == 2.toByte()) { "Original P-256 DER scalar tag differs" }
        val size = original[offset++].toInt() and 0xff
        require(size in 1..33 && size <= original.size - offset && original[offset].toInt() >= 0) { "Original P-256 DER scalar shape differs" }
        require(size == 1 || original[offset] != 0.toByte() || original[offset + 1].toInt() < 0) { "Original P-256 DER scalar is not minimal" }
        val scalar = BigInteger(1, original.copyOfRange(offset, offset + size))
        require(scalar.signum() > 0 && scalar < order) { "Original P-256 DER scalar is outside the curve order" }
        offset += size
    }
    require(offset == original.size) { "Original P-256 DER has trailing bytes" }
}

/** Private platform equation; its caller must already hold the genuine native capability. */
internal fun signOriginalAndroidAppMessageV1(key: PrivateKey, publicKey: ECPublicKey, message: ByteArray,
    purpose: KagemushaAndroidAppSignaturePurposeV1, requireCurrent: () -> Unit): ByteArray {
    val subject = message.copyOf()
    requireAppPlatformSigningMessageV1(subject, purpose)
    requireCurrent()
    val signer = Signature.getInstance("SHA256withECDSA")
    signer.initSign(key)
    signer.update(subject)
    requireCurrent()
    val original = signer.sign()
    try {
        requireOriginalP256DerV1(original)
        val verifier = Signature.getInstance("SHA256withECDSA")
        verifier.initVerify(publicKey); verifier.update(subject)
        check(verifier.verify(original)) { "Original app-key signature does not verify" }
        requireCurrent()
        return original
    } catch (failure: Throwable) {
        original.fill(0)
        throw failure
    } finally { subject.fill(0) }
}
