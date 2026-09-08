// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest

/**
 * Canonical Norito public projections for the native coordinator, bounded to 16 KiB.
 *
 * Shape decoding checks canonical encoding, context shape, and selector consistency. It does not
 * authenticate the embedded Core signature, establish release admission, prove a transition, or
 * recreate a verified capability. The native coordinator must resolve every projection against its
 * authenticated durable state and validate the full authorization on every invocation.
 */
object KagemushaCoreCoordinatorArchiveV1 {
    const val MAXIMUM_ARCHIVE_BYTES = 16 * 1024

    /** Public operation/input binding; this digest is a selector, never proof of preparation. */
    @JvmStatic fun inputsDigestShape(
        operationId: ByteArray,
        context: KagemushaDeviceSenderWalletContextV1,
        inputs: KagemushaDeviceSenderPublicInputsV1,
    ): ByteArray = digest("iroha:kagemusha:device:v1:sender-public-inputs",
        KagemushaDeviceOperationCodecV1.coordinatorInputPreimage(operationId, context, inputs))

    /** Exact byte binding used for retained native terminal envelopes. */
    @JvmStatic fun terminalEnvelopeDigestShape(envelope: ByteArray): ByteArray {
        require(envelope.isNotEmpty() && envelope.size <= 7_936) { "invalid terminal envelope size" }
        return digest("iroha:kagemusha:v1:terminal-envelope", envelope.copyOf())
    }

    private fun digest(domain: String, bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").run {
        update(domain.toByteArray(Charsets.US_ASCII))
        update(0.toByte())
        update(ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(bytes.size.toLong()).array())
        update(bytes)
        digest()
    }

    @JvmStatic fun encodePreparationShape(value: KagemushaNativeSenderPreparationV1): ByteArray =
        KagemushaDeviceOperationCodecV1.encodeCoordinatorPreparation(value)
    @JvmStatic fun decodePreparationShapeExact(bytes: ByteArray): KagemushaNativeSenderPreparationV1 =
        KagemushaDeviceOperationCodecV1.decodeCoordinatorPreparation(bytes)
    @JvmStatic fun encodeCandidateShape(value: KagemushaNativeSenderCandidateV1): ByteArray =
        KagemushaDeviceOperationCodecV1.encodeCoordinatorCandidate(value)
    @JvmStatic fun decodeCandidateShapeExact(bytes: ByteArray): KagemushaNativeSenderCandidateV1 =
        KagemushaDeviceOperationCodecV1.decodeCoordinatorCandidate(bytes)
    @JvmStatic fun encodeRecoveryShape(value: KagemushaNativeSenderRecoveryV1): ByteArray =
        KagemushaDeviceOperationCodecV1.encodeCoordinatorRecovery(value)
    @JvmStatic fun decodeRecoveryShapeExact(bytes: ByteArray): KagemushaNativeSenderRecoveryV1 =
        KagemushaDeviceOperationCodecV1.decodeCoordinatorRecovery(bytes)

    /** Full receipt archive; decoding does not create verified redemption-release authority. */
    @JvmStatic fun encodeRedemptionReceiptShape(value: KagemushaDeviceRedemptionTerminalReceiptV1): ByteArray =
        KagemushaDeviceOperationCodecV1.encodeCoordinatorRedemptionReceipt(value)
    @JvmStatic fun decodeRedemptionReceiptShapeExact(bytes: ByteArray): KagemushaDeviceRedemptionTerminalReceiptV1 =
        KagemushaDeviceOperationCodecV1.decodeCoordinatorRedemptionReceipt(bytes)
}
