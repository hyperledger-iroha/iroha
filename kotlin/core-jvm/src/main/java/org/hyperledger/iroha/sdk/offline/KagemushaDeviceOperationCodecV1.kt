// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Public, non-authoritative command bodies for secure-device receiver operations 2 through 4. */
sealed class KagemushaDeviceReceiverCommandV1(
    @JvmField val operation: Int,
) {
    /** Operation 2: irreversibly stage one direct request-bound payment. */
    class Stage(
        canonicalRequest: ByteArray,
        canonicalPayment: ByteArray,
        stagingMetadata: ByteArray = byteArrayOf(),
    ) : KagemushaDeviceReceiverCommandV1(2) {
        private val request = boundedCopy(canonicalRequest, PAYMENT_REQUEST_MAX, "canonical_request")
        private val payment = boundedCopy(canonicalPayment, PAYMENT_MAX, "canonical_payment")
        private val metadata = boundedCopyAllowEmpty(
            stagingMetadata,
            INBOX_STAGING_METADATA_MAX,
            "staging_metadata",
        )

        fun canonicalRequest(): ByteArray = request.copyOf()
        fun canonicalPayment(): ByteArray = payment.copyOf()
        fun stagingMetadata(): ByteArray = metadata.copyOf()
    }

    /** Operation 3: recover a byte-identical staged credit by credit identity. */
    class RecoverStaged(creditId: ByteArray) : KagemushaDeviceReceiverCommandV1(3) {
        private val credit = nonzeroDigest(creditId, "credit_id")

        fun creditId(): ByteArray = credit.copyOf()
    }

    /** Operation 4: read at most four ordered receipts at one pinned revision. */
    class Page(
        @JvmField val snapshotRevision: BigInteger?,
        after: ByteArray?,
        @JvmField val maximumEntries: Int,
    ) : KagemushaDeviceReceiverCommandV1(4) {
        private val cursor = after?.let { nonzeroDigest(it, "after") }

        init {
            snapshotRevision?.let { requireU128(it, "snapshot_revision") }
            require(maximumEntries in 1..RECEIVER_PAGE_COUNT_MAX) {
                "maximum_entries must be between 1 and $RECEIVER_PAGE_COUNT_MAX"
            }
            require(cursor == null || snapshotRevision != null) { "after requires snapshot_revision" }
        }

        fun after(): ByteArray? = cursor?.copyOf()
    }
}

/** Public, non-authoritative command bodies for control operations 1, 11, and 13 through 22. */
sealed class KagemushaDeviceControlCommandV1(@JvmField val operation: Int) {
    /** Operation 1: read the active governed profile and credential. */
    object ReadActiveHardwareCredential : KagemushaDeviceControlCommandV1(1)

    /** Operation 11: sign a receipt for one exact, already durable public exchange. */
    class SignReceiveAcknowledgement(
        canonicalRequest: ByteArray,
        canonicalPayment: ByteArray,
        @JvmField val inboxReceipt: KagemushaInboxReceiptV1,
    ) : KagemushaDeviceControlCommandV1(11) {
        private val request = boundedCopy(canonicalRequest, PAYMENT_REQUEST_MAX, "canonical_request")
        private val payment = boundedCopy(canonicalPayment, PAYMENT_MAX, "canonical_payment")

        init {
            nonzeroDigest(inboxReceipt.creditId(), "inbox_receipt.credit_id")
            nonzeroDigest(inboxReceipt.receiptCommitment(), "inbox_receipt.receipt_commitment")
        }

        fun canonicalRequest(): ByteArray = request.copyOf()
        fun canonicalPayment(): ByteArray = payment.copyOf()
    }

    /** Operation 13: read opaque qualified time or monotonic-lease evidence. */
    object ReadTrustedTimeOrLease : KagemushaDeviceControlCommandV1(13)

    /** Operation 14: prepare a proof-bearing mint authorization. */
    class PrepareMintAuthorization(
        operationId: ByteArray,
        @JvmField val amount: BigInteger,
        payerCanonicalPayload: ByteArray,
        recipientCanonicalPayload: ByteArray,
    ) : KagemushaDeviceControlCommandV1(14) {
        private val id = nonzeroDigest(operationId, "operation_id")
        private val payer = boundedCopy(payerCanonicalPayload, ACCOUNT_MAX, "payer")
        private val recipient = boundedCopy(recipientCanonicalPayload, ACCOUNT_MAX, "recipient")

        init {
            requireU128(amount, "amount")
            require(amount.signum() != 0) { "amount must be nonzero" }
            KagemushaAccountIdV1.fromCanonicalPayload(payer)
            KagemushaAccountIdV1.fromCanonicalPayload(recipient)
        }

        fun operationId(): ByteArray = id.copyOf()
        fun payerCanonicalPayload(): ByteArray = payer.copyOf()
        fun recipientCanonicalPayload(): ByteArray = recipient.copyOf()
    }

    /** Operation 15: recover a retained mint authorization. */
    class RecoverMintAuthorization(operationId: ByteArray) : KagemushaDeviceControlCommandV1(15) {
        private val id = nonzeroDigest(operationId, "operation_id")
        fun operationId(): ByteArray = id.copyOf()
    }

    /** Operation 17: fold exactly one staged credit into the aggregate balance. */
    class FoldReceiveCredit(
        operationId: ByteArray,
        creditId: ByteArray,
    ) : KagemushaDeviceControlCommandV1(17) {
        private val id = nonzeroDigest(operationId, "operation_id")
        private val credit = nonzeroDigest(creditId, "credit_id")

        fun operationId(): ByteArray = id.copyOf()
        fun creditId(): ByteArray = credit.copyOf()
    }

    /** Operation 18: read the inclusive pending-credit watermark. */
    object ReadPendingCreditWatermark : KagemushaDeviceControlCommandV1(18)

    /** Operation 19: rotate into the next qualified hardware epoch. */
    class RotateHardwareEpoch(operationId: ByteArray) : KagemushaDeviceControlCommandV1(19) {
        private val id = nonzeroDigest(operationId, "operation_id")
        fun operationId(): ByteArray = id.copyOf()
    }

    /** Operation 20: create the unique sequence-zero aggregate under native authority. */
    class BootstrapAggregateState(operationId: ByteArray) : KagemushaDeviceControlCommandV1(20) {
        private val id = nonzeroDigest(operationId, "operation_id")
        fun operationId(): ByteArray = id.copyOf()
    }

    /** Operation 21: atomically read all host-visible wallet recovery state. */
    object RecoverWalletSnapshot : KagemushaDeviceControlCommandV1(21)

    /** Operation 22: construct and sign a positive exact-amount request using native trusted time. */
    class CreateSignedPaymentRequest(
        requestId: ByteArray,
        @JvmField val recipient: KagemushaAccountIdV1,
        @JvmField val amount: BigInteger,
        @JvmField val validityWindowMs: Long,
    ) : KagemushaDeviceControlCommandV1(22) {
        private val id = nonzeroDigest(requestId, "request_id")

        init {
            requireU128(amount, "amount")
            require(amount.signum() != 0) { "amount must be nonzero" }
            require(validityWindowMs in 1..KagemushaWireV1.REQUEST_MAX_TTL_MS) {
                "validity_window_ms must be between 1 and ${KagemushaWireV1.REQUEST_MAX_TTL_MS}"
            }
        }

        fun requestId(): ByteArray = id.copyOf()
    }
}

/** Stable hardware-lane identity encoded by the Rust `KagemushaLaneIdV1` schema. */
class KagemushaDeviceLaneIdV1(
    networkId: ByteArray,
    deviceLaneId: ByteArray,
    assetCanonicalPayload: ByteArray,
    @JvmField val scale: Int,
) {
    private val network = nonzeroDigest(networkId, "network_id")
    private val lane = nonzeroDigest(deviceLaneId, "device_lane_id")
    private val asset = KagemushaAssetDefinitionIdV1.fromCanonicalPayload(assetCanonicalPayload)

    init {
        require(scale in 0..KAGEMUSHA_ASSET_SCALE_MAX) { "scale exceeds the KAGEMUSHA V1 bound" }
    }

    fun networkId(): ByteArray = network.copyOf()
    fun deviceLaneId(): ByteArray = lane.copyOf()
    fun assetCanonicalPayload(): ByteArray = asset.canonicalPayload()
}

/** Release and asset-incarnation scope encoded by Rust `KagemushaStateContextV1`. */
class KagemushaDeviceStateContextV1(
    @JvmField val protocolVersion: Int,
    suiteId: ByteArray,
    vkDigest: ByteArray,
    releaseId: ByteArray,
    assetIncarnation: ByteArray,
    hardwareProfileId: ByteArray,
    @JvmField val policyEpoch: Long,
) {
    private val suite = nonzeroDigest(suiteId, "suite_id")
    private val vk = nonzeroDigest(vkDigest, "vk_digest")
    private val release = nonzeroDigest(releaseId, "release_id")
    private val incarnation = exactCopy(assetIncarnation, 32, "asset_incarnation")
    private val profile = nonzeroDigest(hardwareProfileId, "hardware_profile_id")

    init {
        require(protocolVersion == VERSION) { "protocol_version must be 1" }
        require(policyEpoch != 0L) { "policy_epoch must be nonzero" }
        require(incarnation[31].toInt() and 1 == 1) { "asset_incarnation is not a marked hash" }
    }

    fun suiteId(): ByteArray = suite.copyOf()
    fun vkDigest(): ByteArray = vk.copyOf()
    fun releaseId(): ByteArray = release.copyOf()
    fun assetIncarnation(): ByteArray = incarnation.copyOf()
    fun hardwareProfileId(): ByteArray = profile.copyOf()
}

/** Full-width hardware epoch encoded by Rust `HardwareEpochV1`. */
class KagemushaDeviceHardwareEpochV1(
    @JvmField val generation: BigInteger,
    epochId: ByteArray,
) {
    private val epoch = nonzeroDigest(epochId, "epoch_id")

    init {
        requireU128(generation, "generation")
        require(generation.signum() != 0) { "generation must be nonzero" }
    }

    fun epochId(): ByteArray = epoch.copyOf()
}

/** Native key-reference and governed hardware-policy identity. */
class KagemushaDevicePolicyBindingV1(
    deviceKeyReference: ByteArray,
    hardwarePolicyId: ByteArray,
) {
    private val key = nonzeroDigest(deviceKeyReference, "device_key_reference")
    private val policy = nonzeroDigest(hardwarePolicyId, "hardware_policy_id")

    fun deviceKeyReference(): ByteArray = key.copyOf()
    fun hardwarePolicyId(): ByteArray = policy.copyOf()
}

/** Identity selectors authenticated by the native sender-wallet session. */
class KagemushaDeviceSenderWalletContextV1(
    @JvmField val lane: KagemushaDeviceLaneIdV1,
    @JvmField val release: KagemushaDeviceStateContextV1,
    credentialId: ByteArray,
    @JvmField val hardwareEpoch: KagemushaDeviceHardwareEpochV1,
    @JvmField val devicePolicyBinding: KagemushaDevicePolicyBindingV1,
) {
    private val credential = nonzeroDigest(credentialId, "credential_id")
    fun credentialId(): ByteArray = credential.copyOf()
}

/** Public inputs fixed before an outgoing operation is prepared. Variant order is wire order. */
sealed class KagemushaDeviceSenderPublicInputsV1(@JvmField val ordinal: Int) {
    /** Peer payment with the exact signed receiver exchange. */
    class SendSplit(canonicalRequest: ByteArray) : KagemushaDeviceSenderPublicInputsV1(0) {
        private val request = boundedCopy(canonicalRequest, PAYMENT_REQUEST_MAX, "request")

        fun canonicalRequest(): ByteArray = request.copyOf()
    }

    /** Chain-facing redemption with a positive amount and canonical beneficiary. */
    class RedeemSplit(
        @JvmField val amount: BigInteger,
        beneficiaryCanonicalPayload: ByteArray,
    ) : KagemushaDeviceSenderPublicInputsV1(1) {
        private val beneficiary = boundedCopy(beneficiaryCanonicalPayload, ACCOUNT_MAX, "beneficiary")

        init {
            requireU128(amount, "amount")
            require(amount.signum() != 0) { "amount must be nonzero" }
            KagemushaAccountIdV1.fromCanonicalPayload(beneficiary)
        }

        fun beneficiaryCanonicalPayload(): ByteArray = beneficiary.copyOf()
    }
}

/** Exact retained preparation selector. */
class KagemushaDeviceSenderPreparationSelectorV1(
    inputsDigest: ByteArray,
    preparationId: ByteArray,
) {
    private val inputs = nonzeroDigest(inputsDigest, "inputs_digest")
    private val preparation = nonzeroDigest(preparationId, "preparation_id")

    fun inputsDigest(): ByteArray = inputs.copyOf()
    fun preparationId(): ByteArray = preparation.copyOf()
}

/** Lookup or stable-revision page selector. Variant order is wire order. */
sealed class KagemushaDeviceSenderRecoverySelectorV1(@JvmField val ordinal: Int) {
    class Lookup(inputsDigest: ByteArray) : KagemushaDeviceSenderRecoverySelectorV1(0) {
        private val inputs = nonzeroDigest(inputsDigest, "inputs_digest")
        fun inputsDigest(): ByteArray = inputs.copyOf()
    }

    class Page(
        @JvmField val snapshotRevision: BigInteger?,
        after: ByteArray?,
        @JvmField val maximumEntries: Int,
    ) : KagemushaDeviceSenderRecoverySelectorV1(1) {
        private val cursor = after?.let { nonzeroDigest(it, "after") }

        init {
            snapshotRevision?.let { requireU128(it, "snapshot_revision") }
            require(maximumEntries in 1..SENDER_PAGE_COUNT_MAX) {
                "maximum_entries must be between 1 and $SENDER_PAGE_COUNT_MAX"
            }
            require(cursor == null || snapshotRevision != null) {
                "after requires snapshot_revision"
            }
        }

        fun after(): ByteArray? = cursor?.copyOf()
    }
}

/** Sender operation-specific command body. Variant ordinal is not the ABI operation code. */
sealed class KagemushaDeviceSenderCommandBodyV1(
    @JvmField val ordinal: Int,
    @JvmField val operation: Int,
) {
    class Prepare(@JvmField val inputs: KagemushaDeviceSenderPublicInputsV1) :
        KagemushaDeviceSenderCommandBodyV1(0, 5)

    class RecoverPrepared(inputsDigest: ByteArray) : KagemushaDeviceSenderCommandBodyV1(1, 6) {
        private val inputs = nonzeroDigest(inputsDigest, "inputs_digest")
        fun inputsDigest(): ByteArray = inputs.copyOf()
    }

    class Commit(
        @JvmField val selector: KagemushaDeviceSenderPreparationSelectorV1,
        candidateDigest: ByteArray,
    ) : KagemushaDeviceSenderCommandBodyV1(2, 7) {
        private val candidate = nonzeroDigest(candidateDigest, "candidate_digest")
        fun candidateDigest(): ByteArray = candidate.copyOf()
    }

    class RecoverTerminal(inputsDigest: ByteArray) : KagemushaDeviceSenderCommandBodyV1(3, 8) {
        private val inputs = nonzeroDigest(inputsDigest, "inputs_digest")
        fun inputsDigest(): ByteArray = inputs.copyOf()
    }

    class Install(
        @JvmField val selector: KagemushaDeviceSenderPreparationSelectorV1,
        candidateDigest: ByteArray,
        @JvmField val inputs: KagemushaDeviceSenderPublicInputsV1,
        canonicalEnvelope: ByteArray,
    ) : KagemushaDeviceSenderCommandBodyV1(4, 9) {
        private val candidate = nonzeroDigest(candidateDigest, "candidate_digest")
        private val envelope = boundedCopy(canonicalEnvelope, TERMINAL_ENVELOPE_MAX, "envelope")
        fun candidateDigest(): ByteArray = candidate.copyOf()
        fun canonicalEnvelope(): ByteArray = envelope.copyOf()
    }

    class RecoverInstalled(@JvmField val selector: KagemushaDeviceSenderRecoverySelectorV1) :
        KagemushaDeviceSenderCommandBodyV1(5, 10)

    class Release(
        inputsDigest: ByteArray,
        envelopeDigest: ByteArray,
        @JvmField val inputs: KagemushaDeviceSenderPublicInputsV1,
        canonicalEnvelope: ByteArray,
        canonicalAcknowledgement: ByteArray,
    ) : KagemushaDeviceSenderCommandBodyV1(6, 12) {
        private val inputsHash = nonzeroDigest(inputsDigest, "inputs_digest")
        private val envelopeHash = nonzeroDigest(envelopeDigest, "envelope_digest")
        private val envelope = boundedCopy(canonicalEnvelope, TERMINAL_ENVELOPE_MAX, "envelope")
        private val acknowledgement = boundedCopy(
            canonicalAcknowledgement,
            ACKNOWLEDGEMENT_MAX,
            "acknowledgement",
        )

        fun inputsDigest(): ByteArray = inputsHash.copyOf()
        fun envelopeDigest(): ByteArray = envelopeHash.copyOf()
        fun canonicalEnvelope(): ByteArray = envelope.copyOf()
        fun canonicalAcknowledgement(): ByteArray = acknowledgement.copyOf()
    }
}

/** Canonical sender command shared by KAGEMUSHA V1 operations 5 through 10 and 12. */
class KagemushaDeviceSenderCommandV1(
    @JvmField val version: Int = VERSION,
    @JvmField val operation: Int,
    operationId: ByteArray,
    @JvmField val context: KagemushaDeviceSenderWalletContextV1,
    @JvmField val body: KagemushaDeviceSenderCommandBodyV1,
) {
    private val id = nonzeroDigest(operationId, "operation_id")

    init {
        require(version == VERSION) { "version must be 1" }
        require(operation == body.operation) { "operation does not match sender command body" }
    }

    fun operationId(): ByteArray = id.copyOf()
}

/**
 * Canonical Norito codecs for the public KAGEMUSHA V1 secure-device command bodies.
 *
 * This object performs bounded canonical serialization only. It is not a secure-device provider,
 * does not authenticate Core state, and does not expose reply decoding. The reply parser remains
 * internal until a native response-authenticator verifier can run before the parsed value escapes.
 */
object KagemushaDeviceOperationCodecV1 {
    const val CONTROL_READ_COMMAND_MAX_BYTES = 256
    const val CONTROL_ACKNOWLEDGEMENT_COMMAND_MAX_BYTES = 12 * 1024
    const val CONTROL_MINT_COMMAND_MAX_BYTES = 2 * 1024
    const val CONTROL_FOLD_COMMAND_MAX_BYTES = 256
    const val CONTROL_QUALIFICATION_REPLY_MAX_BYTES = 2 * 1024
    const val CONTROL_ACKNOWLEDGEMENT_REPLY_MAX_BYTES = 2 * 1024
    const val CONTROL_EVIDENCE_REPLY_MAX_BYTES = 512
    const val CONTROL_MINT_REPLY_MAX_BYTES = 12 * 1024
    const val CONTROL_FOLD_REPLY_MAX_BYTES = 2 * 1024
    const val CONTROL_WATERMARK_REPLY_MAX_BYTES = 256
    const val CONTROL_ROTATION_REPLY_MAX_BYTES = 2 * 1024
    const val CONTROL_WALLET_SNAPSHOT_REPLY_MAX_BYTES = 2 * 1024
    const val CONTROL_PAYMENT_REQUEST_COMMAND_MAX_BYTES = 2 * 1024
    const val CONTROL_PAYMENT_REQUEST_REPLY_MAX_BYTES = 2 * 1024
    const val RECEIVER_STAGE_COMMAND_MAX_BYTES = 16 * 1024
    const val RECEIVER_RECOVERY_COMMAND_MAX_BYTES = 512
    const val RECEIVER_STAGED_REPLY_MAX_BYTES = 64 * 1024
    const val RECEIVER_PAGE_REPLY_MAX_BYTES = 64 * 1024
    const val SENDER_COMMAND_MAX_BYTES = 16 * 1024
    const val SENDER_REPLY_MAX_BYTES = 64 * 1024

    /** Encode one operation-specific control command using its frozen Rust schema name. */
    @JvmStatic
    fun encodeControlCommand(value: KagemushaDeviceControlCommandV1): ByteArray {
        val payload = when (value) {
            KagemushaDeviceControlCommandV1.ReadActiveHardwareCredential ->
                fields(u16(VERSION), u8(value.operation))
            is KagemushaDeviceControlCommandV1.SignReceiveAcknowledgement -> fields(
                u16(VERSION),
                u8(value.operation),
                vectorBytes(value.canonicalRequest()),
                vectorBytes(value.canonicalPayment()),
                inboxReceipt(value.inboxReceipt),
            )
            KagemushaDeviceControlCommandV1.ReadTrustedTimeOrLease ->
                fields(u16(VERSION), u8(value.operation))
            is KagemushaDeviceControlCommandV1.PrepareMintAuthorization -> fields(
                u16(VERSION),
                u8(value.operation),
                value.operationId(),
                u128(value.amount),
                value.payerCanonicalPayload(),
                value.recipientCanonicalPayload(),
            )
            is KagemushaDeviceControlCommandV1.RecoverMintAuthorization -> fields(
                u16(VERSION), u8(value.operation), value.operationId(),
            )
            is KagemushaDeviceControlCommandV1.FoldReceiveCredit -> fields(
                u16(VERSION),
                u8(value.operation),
                value.operationId(),
                value.creditId(),
            )
            KagemushaDeviceControlCommandV1.ReadPendingCreditWatermark ->
                fields(u16(VERSION), u8(value.operation))
            is KagemushaDeviceControlCommandV1.RotateHardwareEpoch -> fields(
                u16(VERSION), u8(value.operation), value.operationId(),
            )
            is KagemushaDeviceControlCommandV1.BootstrapAggregateState -> fields(
                u16(VERSION), u8(value.operation), value.operationId(),
            )
            KagemushaDeviceControlCommandV1.RecoverWalletSnapshot ->
                fields(u16(VERSION), u8(value.operation))
            is KagemushaDeviceControlCommandV1.CreateSignedPaymentRequest -> fields(
                u16(VERSION),
                u8(value.operation),
                value.requestId(),
                value.recipient.canonicalPayload(),
                u128(value.amount),
                u64(value.validityWindowMs),
            )
        }
        val descriptor = controlCommandDescriptor(value.operation)
        return frame(descriptor.schema, descriptor.alignment, payload, descriptor.maximum)
    }

    /** Decode one exact control command and enforce its outer request-identity binding. */
    @JvmStatic
    fun decodeControlCommand(
        operation: Int,
        requestId: ByteArray,
        bytes: ByteArray,
    ): KagemushaDeviceControlCommandV1 {
        val expectedId = nonzeroDigest(requestId, "request_id")
        val descriptor = controlCommandDescriptor(operation)
        val reader = DeviceReader(unframe(bytes, descriptor))
        val version = reader.u16Field()
        val carriedOperation = reader.u8Field()
        require(version == VERSION && carriedOperation == operation) { "control command binding mismatch" }
        val value = when (operation) {
            1 -> KagemushaDeviceControlCommandV1.ReadActiveHardwareCredential
            11 -> KagemushaDeviceControlCommandV1.SignReceiveAcknowledgement(
                reader.byteVectorField(PAYMENT_REQUEST_MAX),
                reader.byteVectorField(PAYMENT_MAX),
                decodeInboxReceipt(reader.field()),
            )
            13 -> KagemushaDeviceControlCommandV1.ReadTrustedTimeOrLease
            14 -> KagemushaDeviceControlCommandV1.PrepareMintAuthorization(
                reader.exactField(32),
                reader.u128Field(),
                reader.field(ACCOUNT_MAX),
                reader.field(ACCOUNT_MAX),
            )
            15 -> KagemushaDeviceControlCommandV1.RecoverMintAuthorization(reader.exactField(32))
            17 -> KagemushaDeviceControlCommandV1.FoldReceiveCredit(
                reader.exactField(32), reader.exactField(32),
            )
            18 -> KagemushaDeviceControlCommandV1.ReadPendingCreditWatermark
            19 -> KagemushaDeviceControlCommandV1.RotateHardwareEpoch(reader.exactField(32))
            20 -> KagemushaDeviceControlCommandV1.BootstrapAggregateState(reader.exactField(32))
            21 -> KagemushaDeviceControlCommandV1.RecoverWalletSnapshot
            22 -> KagemushaDeviceControlCommandV1.CreateSignedPaymentRequest(
                reader.exactField(32),
                KagemushaAccountIdV1.fromCanonicalPayload(reader.field(ACCOUNT_MAX)),
                reader.u128Field(),
                reader.u64Field(),
            )
            else -> error("unreachable")
        }
        reader.finish()
        when (value) {
            is KagemushaDeviceControlCommandV1.SignReceiveAcknowledgement ->
                require(value.inboxReceipt.creditId().contentEquals(expectedId)) {
                    "inbox_receipt.credit_id does not match request_id"
                }
            is KagemushaDeviceControlCommandV1.PrepareMintAuthorization ->
                require(value.operationId().contentEquals(expectedId)) { "operation_id does not match request_id" }
            is KagemushaDeviceControlCommandV1.RecoverMintAuthorization ->
                require(value.operationId().contentEquals(expectedId)) { "operation_id does not match request_id" }
            is KagemushaDeviceControlCommandV1.FoldReceiveCredit ->
                require(value.operationId().contentEquals(expectedId)) { "operation_id does not match request_id" }
            is KagemushaDeviceControlCommandV1.RotateHardwareEpoch ->
                require(value.operationId().contentEquals(expectedId)) { "operation_id does not match request_id" }
            is KagemushaDeviceControlCommandV1.BootstrapAggregateState ->
                require(value.operationId().contentEquals(expectedId)) { "operation_id does not match request_id" }
            is KagemushaDeviceControlCommandV1.CreateSignedPaymentRequest ->
                require(value.requestId().contentEquals(expectedId)) { "request_id does not match request_id" }
            else -> Unit
        }
        require(encodeControlCommand(value).contentEquals(bytes)) { "control command is not canonical" }
        return value
    }

    /** Encode exactly one receiver command under its operation-specific Rust schema. */
    @JvmStatic
    fun encodeReceiverCommand(value: KagemushaDeviceReceiverCommandV1): ByteArray {
        val payload = when (value) {
            is KagemushaDeviceReceiverCommandV1.Stage -> fields(
                u16(VERSION),
                u8(value.operation),
                vectorBytes(value.canonicalRequest()),
                vectorBytes(value.canonicalPayment()),
                vectorBytes(value.stagingMetadata()),
            )
            is KagemushaDeviceReceiverCommandV1.RecoverStaged -> fields(
                u16(VERSION),
                u8(value.operation),
                value.creditId(),
            )
            is KagemushaDeviceReceiverCommandV1.Page -> fields(
                u16(VERSION),
                u8(value.operation),
                option(value.snapshotRevision?.let(::u128)),
                option(value.after()),
                u16(value.maximumEntries),
            )
        }
        val descriptor = receiverCommandDescriptor(value.operation)
        return frame(descriptor.schema, descriptor.alignment, payload, descriptor.maximum)
    }

    /** Decode one exact receiver command and reject schema, suffix and noncanonical changes. */
    @JvmStatic
    fun decodeReceiverCommand(
        operation: Int,
        requestId: ByteArray,
        bytes: ByteArray,
    ): KagemushaDeviceReceiverCommandV1 {
        val expectedId = nonzeroDigest(requestId, "request_id")
        val descriptor = receiverCommandDescriptor(operation)
        val payload = unframe(bytes, descriptor)
        val reader = DeviceReader(payload)
        val version = reader.u16Field()
        val carriedOperation = reader.u8Field()
        require(version == VERSION && carriedOperation == operation) { "receiver command binding mismatch" }
        val value = when (operation) {
            2 -> KagemushaDeviceReceiverCommandV1.Stage(
                reader.byteVectorField(PAYMENT_REQUEST_MAX),
                reader.byteVectorField(PAYMENT_MAX),
                reader.byteVectorField(INBOX_STAGING_METADATA_MAX, allowEmpty = true),
            )
            3 -> KagemushaDeviceReceiverCommandV1.RecoverStaged(reader.exactField(32))
            4 -> KagemushaDeviceReceiverCommandV1.Page(
                reader.optionField { it.u128Raw() },
                reader.optionField { it.exactRaw(32) },
                reader.u16Field(),
            )
            else -> error("unreachable")
        }
        reader.finish()
        when (value) {
            is KagemushaDeviceReceiverCommandV1.Stage -> {
                val request = KagemushaNoritoV1.decodePaymentRequestShapeExact(value.canonicalRequest())
                val payment = KagemushaNoritoV1.decodePaymentShapeExact(value.canonicalPayment(), request)
                require(payment.output.creditId().contentEquals(expectedId)) {
                    "payment credit_id does not match request_id"
                }
            }
            is KagemushaDeviceReceiverCommandV1.RecoverStaged ->
                require(value.creditId().contentEquals(expectedId)) {
                    "credit_id does not match request_id"
                }
            is KagemushaDeviceReceiverCommandV1.Page -> Unit
        }
        require(encodeReceiverCommand(value).contentEquals(bytes)) { "receiver command is not canonical" }
        return value
    }

    /** Encode the shared canonical sender command schema. */
    @JvmStatic
    fun encodeSenderCommand(value: KagemushaDeviceSenderCommandV1): ByteArray {
        val payload = fields(
            u16(value.version),
            u8(value.operation),
            value.operationId(),
            senderContext(value.context),
            senderCommandBody(value.body),
        )
        return frame(SENDER_COMMAND_SCHEMA, 16, payload, SENDER_COMMAND_MAX_BYTES)
    }

    /** Decode one exact sender command and bind its outer operation and request identity. */
    @JvmStatic
    fun decodeSenderCommand(
        operation: Int,
        requestId: ByteArray,
        bytes: ByteArray,
    ): KagemushaDeviceSenderCommandV1 {
        val expectedId = nonzeroDigest(requestId, "request_id")
        val payload = unframe(
            bytes,
            DeviceArchiveDescriptor(SENDER_COMMAND_SCHEMA, 16, SENDER_COMMAND_MAX_BYTES),
        )
        val reader = DeviceReader(payload)
        val version = reader.u16Field()
        val carriedOperation = reader.u8Field()
        val operationId = reader.exactField(32)
        val context = decodeSenderContext(reader.field())
        val body = decodeSenderCommandBody(reader.field())
        reader.finish()
        require(version == VERSION && carriedOperation == operation && body.operation == operation) {
            "sender command operation mismatch"
        }
        require(operationId.contentEquals(expectedId)) { "sender command request identity mismatch" }
        val value = KagemushaDeviceSenderCommandV1(
            version,
            operation,
            operationId,
            context,
            body,
        )
        require(encodeSenderCommand(value).contentEquals(bytes)) { "sender command is not canonical" }
        return value
    }

    private fun receiverCommandDescriptor(operation: Int): DeviceArchiveDescriptor = when (operation) {
        2 -> DeviceArchiveDescriptor(RECEIVER_STAGE_SCHEMA, 8, RECEIVER_STAGE_COMMAND_MAX_BYTES)
        3 -> DeviceArchiveDescriptor(RECEIVER_RECOVER_STAGED_SCHEMA, 2, RECEIVER_RECOVERY_COMMAND_MAX_BYTES)
        4 -> DeviceArchiveDescriptor(RECEIVER_PAGE_SCHEMA, 16, RECEIVER_RECOVERY_COMMAND_MAX_BYTES)
        else -> throw IllegalArgumentException("unsupported receiver operation: $operation")
    }

    private fun controlCommandDescriptor(operation: Int): DeviceArchiveDescriptor = when (operation) {
        1 -> DeviceArchiveDescriptor(CONTROL_READ_CREDENTIAL_SCHEMA, 2, CONTROL_READ_COMMAND_MAX_BYTES)
        11 -> DeviceArchiveDescriptor(CONTROL_SIGN_ACK_SCHEMA, 8, CONTROL_ACKNOWLEDGEMENT_COMMAND_MAX_BYTES)
        13 -> DeviceArchiveDescriptor(CONTROL_READ_TIME_SCHEMA, 2, CONTROL_READ_COMMAND_MAX_BYTES)
        14 -> DeviceArchiveDescriptor(CONTROL_PREPARE_MINT_SCHEMA, 16, CONTROL_MINT_COMMAND_MAX_BYTES)
        15 -> DeviceArchiveDescriptor(CONTROL_RECOVER_MINT_SCHEMA, 2, CONTROL_READ_COMMAND_MAX_BYTES)
        17 -> DeviceArchiveDescriptor(CONTROL_FOLD_RECEIVE_SCHEMA, 16, CONTROL_FOLD_COMMAND_MAX_BYTES)
        18 -> DeviceArchiveDescriptor(CONTROL_READ_WATERMARK_SCHEMA, 2, CONTROL_READ_COMMAND_MAX_BYTES)
        19 -> DeviceArchiveDescriptor(CONTROL_ROTATE_EPOCH_SCHEMA, 2, CONTROL_READ_COMMAND_MAX_BYTES)
        20 -> DeviceArchiveDescriptor(CONTROL_BOOTSTRAP_SCHEMA, 2, CONTROL_READ_COMMAND_MAX_BYTES)
        21 -> DeviceArchiveDescriptor(CONTROL_RECOVER_WALLET_SCHEMA, 2, CONTROL_READ_COMMAND_MAX_BYTES)
        22 -> DeviceArchiveDescriptor(
            CONTROL_CREATE_REQUEST_SCHEMA,
            16,
            CONTROL_PAYMENT_REQUEST_COMMAND_MAX_BYTES,
        )
        else -> throw IllegalArgumentException("unsupported control operation: $operation")
    }

    internal fun decodeControlReplyAfterAuthentication(
        operation: Int,
        bytes: ByteArray,
    ): KagemushaDeviceAuthenticatedReplyV1 {
        val descriptor = when (operation) {
            1 -> DeviceArchiveDescriptor(CONTROL_QUALIFICATION_REPLY_SCHEMA, 8, CONTROL_QUALIFICATION_REPLY_MAX_BYTES)
            11 -> DeviceArchiveDescriptor(CONTROL_ACK_REPLY_SCHEMA, 8, CONTROL_ACKNOWLEDGEMENT_REPLY_MAX_BYTES)
            13 -> DeviceArchiveDescriptor(CONTROL_TIME_REPLY_SCHEMA, 8, CONTROL_EVIDENCE_REPLY_MAX_BYTES)
            14, 15 -> DeviceArchiveDescriptor(CONTROL_MINT_REPLY_SCHEMA, 8, CONTROL_MINT_REPLY_MAX_BYTES)
            17 -> DeviceArchiveDescriptor(CONTROL_FOLD_REPLY_SCHEMA, 16, CONTROL_FOLD_REPLY_MAX_BYTES)
            18 -> DeviceArchiveDescriptor(CONTROL_WATERMARK_REPLY_SCHEMA, 16, CONTROL_WATERMARK_REPLY_MAX_BYTES)
            19 -> DeviceArchiveDescriptor(CONTROL_ROTATION_REPLY_SCHEMA, 8, CONTROL_ROTATION_REPLY_MAX_BYTES)
            20 -> DeviceArchiveDescriptor(CONTROL_BOOTSTRAP_REPLY_SCHEMA, 8, CONTROL_ROTATION_REPLY_MAX_BYTES)
            21 -> DeviceArchiveDescriptor(
                CONTROL_WALLET_SNAPSHOT_REPLY_SCHEMA,
                16,
                CONTROL_WALLET_SNAPSHOT_REPLY_MAX_BYTES,
            )
            22 -> DeviceArchiveDescriptor(
                CONTROL_SIGNED_REQUEST_REPLY_SCHEMA,
                8,
                CONTROL_PAYMENT_REQUEST_REPLY_MAX_BYTES,
            )
            else -> throw IllegalArgumentException("unsupported control operation: $operation")
        }
        val reply = decodeAuthenticatedReply(operation, bytes, descriptor)
        validateControlReplyPayload(operation, reply.payload())
        return reply
    }

    /** Decode operation 1's nested qualification models after the outer reply was authenticated. */
    internal fun decodeQualificationReplyAfterAuthentication(
        reply: KagemushaDeviceAuthenticatedReplyV1,
    ): KagemushaDeviceQualificationReplyV1 {
        require(reply.operation == 1) { "qualification reply must use operation 1" }
        val reader = DeviceReader(reply.payload())
        require(reader.u16Field() == VERSION && reader.u8Field() == 1) {
            "qualification reply binding mismatch"
        }
        val releaseId = reader.exactField(32)
        val hardwarePolicyDigest = reader.exactField(32)
        val profile = KagemushaNoritoV1.decodeHardwareProfileShapeExact(
            frame(HARDWARE_PROFILE_SCHEMA, 8, reader.field(512), 512),
        )
        val credential = KagemushaNoritoV1.decodeHardwareCredentialShapeExact(
            frame(HARDWARE_CREDENTIAL_SCHEMA, 8, reader.field(768), 768),
        )
        reader.finish()
        return KagemushaDeviceQualificationReplyV1(
            releaseId,
            hardwarePolicyDigest,
            profile,
            credential,
        )
    }

    internal fun decodeReceiverReplyAfterAuthentication(
        operation: Int,
        bytes: ByteArray,
    ): KagemushaDeviceAuthenticatedReplyV1 {
        val descriptor = when (operation) {
            2, 3 -> DeviceArchiveDescriptor(RECEIVER_STAGED_REPLY_SCHEMA, 16, RECEIVER_STAGED_REPLY_MAX_BYTES)
            4 -> DeviceArchiveDescriptor(RECEIVER_PAGE_REPLY_SCHEMA, 16, RECEIVER_PAGE_REPLY_MAX_BYTES)
            else -> throw IllegalArgumentException("unsupported receiver operation: $operation")
        }
        val reply = decodeAuthenticatedReply(operation, bytes, descriptor)
        validateReceiverReplyPayload(operation, reply.payload())
        return reply
    }

    internal fun decodeSenderReplyAfterAuthentication(
        operation: Int,
        requestId: ByteArray,
        bytes: ByteArray,
    ): KagemushaDeviceAuthenticatedReplyV1 {
        val reply = decodeAuthenticatedReply(
            operation,
            bytes,
            DeviceArchiveDescriptor(SENDER_REPLY_SCHEMA, 16, SENDER_REPLY_MAX_BYTES),
        )
        val reader = DeviceReader(reply.payload())
        require(reader.u16Field() == VERSION) { "sender reply version mismatch" }
        require(reader.u8Field() == operation) { "sender reply operation mismatch" }
        require(reader.exactField(32).contentEquals(nonzeroDigest(requestId, "request_id"))) {
            "sender reply request identity mismatch"
        }
        decodeSenderContext(reader.field())
        reader.u128Field()
        validateSenderReplyBody(reader.field())
        reader.finish()
        return reply
    }

    private fun decodeAuthenticatedReply(
        operation: Int,
        bytes: ByteArray,
        descriptor: DeviceArchiveDescriptor,
    ): KagemushaDeviceAuthenticatedReplyV1 {
        val payload = unframe(bytes, descriptor)
        val top = DeviceReader(payload)
        require(top.u16Field() == VERSION) { "reply version mismatch" }
        require(top.u8Field() == operation) { "reply operation mismatch" }
        // The remainder is parsed by schema-specific checks where required and retained exactly.
        while (top.hasRemaining()) top.field()
        top.finish()
        require(frame(descriptor.schema, descriptor.alignment, payload, descriptor.maximum).contentEquals(bytes)) {
            "reply is not canonically framed"
        }
        return KagemushaDeviceAuthenticatedReplyV1(operation, bytes, payload)
    }

    private fun validateControlReplyPayload(operation: Int, payload: ByteArray) {
        val reader = DeviceReader(payload)
        require(reader.u16Field() == VERSION && reader.u8Field() == operation) {
            "control reply binding mismatch"
        }
        when (operation) {
            1 -> {
                reader.exactField(32)
                reader.exactField(32)
                require(reader.field(512).isNotEmpty()) { "empty hardware profile" }
                require(reader.field(768).isNotEmpty()) { "empty hardware credential" }
            }
            11 -> reader.byteVectorField(ACKNOWLEDGEMENT_MAX)
            13 -> validateCommitEvidence(reader.field())
            14, 15 -> reader.byteVectorField(MINT_AUTHORIZATION_MAX)
            17 -> {
                reader.exactField(32)
                reader.byteVectorField(AGGREGATE_STATE_MAX)
            }
            18 -> reader.u128Field()
            19, 20 -> reader.byteVectorField(AGGREGATE_STATE_MAX)
            21 -> {
                reader.optionField { it.byteVectorRaw(AGGREGATE_STATE_MAX) }
                reader.u128Field()
                reader.u128Field()
                reader.u128Field()
            }
            22 -> reader.byteVectorField(PAYMENT_REQUEST_MAX)
            else -> error("unreachable")
        }
        reader.finish()
    }

    private fun validateCommitEvidence(payload: ByteArray) {
        val reader = DeviceReader(payload)
        reader.enumTag(2)
        val statement = DeviceReader(reader.field())
        statement.exactField(32)
        statement.finish()
        reader.finish()
    }

    private fun validateReceiverReplyPayload(operation: Int, payload: ByteArray) {
        val reader = DeviceReader(payload)
        require(reader.u16Field() == VERSION && reader.u8Field() == operation) {
            "receiver reply binding mismatch"
        }
        when (operation) {
            2, 3 -> {
                reader.u128Field()
                validateStagedReceipt(reader.field())
            }
            4 -> {
                reader.u128Field()
                reader.itemVectorField(RECEIVER_PAGE_COUNT_MAX) { validateStagedReceipt(it) }
                reader.optionField { it.exactRaw(32); Unit }
            }
            else -> error("unreachable")
        }
        reader.finish()
    }

    private fun validateStagedReceipt(payload: ByteArray) {
        val reader = DeviceReader(payload)
        reader.byteVectorField(PAYMENT_REQUEST_MAX)
        reader.byteVectorField(PAYMENT_MAX)
        reader.byteVectorField(INBOX_STAGING_METADATA_MAX, allowEmpty = true)
        decodeInboxReceipt(reader.field())
        reader.finish()
    }

    private fun senderContext(value: KagemushaDeviceSenderWalletContextV1): ByteArray = fields(
        lane(value.lane),
        stateContext(value.release),
        value.credentialId(),
        hardwareEpoch(value.hardwareEpoch),
        policyBinding(value.devicePolicyBinding),
    )

    private fun inboxReceipt(value: KagemushaInboxReceiptV1): ByteArray = fields(
        u16(value.version), value.creditId(), value.receiptCommitment(),
    )

    private fun decodeInboxReceipt(payload: ByteArray): KagemushaInboxReceiptV1 {
        val reader = DeviceReader(payload)
        val value = KagemushaInboxReceiptV1(
            reader.u16Field(), reader.exactField(32), reader.exactField(32),
        )
        reader.finish()
        return value
    }

    private fun lane(value: KagemushaDeviceLaneIdV1): ByteArray = fields(
        value.networkId(),
        value.deviceLaneId(),
        value.assetCanonicalPayload(),
        u32(value.scale),
    )

    private fun stateContext(value: KagemushaDeviceStateContextV1): ByteArray = fields(
        u16(value.protocolVersion),
        value.suiteId(),
        value.vkDigest(),
        value.releaseId(),
        fields(value.assetIncarnation()),
        value.hardwareProfileId(),
        u64(value.policyEpoch),
    )

    private fun hardwareEpoch(value: KagemushaDeviceHardwareEpochV1): ByteArray = fields(
        u128(value.generation), value.epochId(),
    )

    private fun policyBinding(value: KagemushaDevicePolicyBindingV1): ByteArray = fields(
        value.deviceKeyReference(), value.hardwarePolicyId(),
    )

    private fun senderInputs(value: KagemushaDeviceSenderPublicInputsV1): ByteArray = when (value) {
        is KagemushaDeviceSenderPublicInputsV1.SendSplit -> enumPayload(
            value.ordinal,
            vectorBytes(value.canonicalRequest()),
        )
        is KagemushaDeviceSenderPublicInputsV1.RedeemSplit -> enumPayload(
            value.ordinal,
            u128(value.amount),
            value.beneficiaryCanonicalPayload(),
        )
    }

    private fun preparationSelector(value: KagemushaDeviceSenderPreparationSelectorV1): ByteArray =
        fields(value.inputsDigest(), value.preparationId())

    private fun recoverySelector(value: KagemushaDeviceSenderRecoverySelectorV1): ByteArray = when (value) {
        is KagemushaDeviceSenderRecoverySelectorV1.Lookup -> enumPayload(
            value.ordinal, value.inputsDigest(),
        )
        is KagemushaDeviceSenderRecoverySelectorV1.Page -> enumPayload(
            value.ordinal,
            option(value.snapshotRevision?.let(::u128)),
            option(value.after()),
            u16(value.maximumEntries),
        )
    }

    private fun senderCommandBody(value: KagemushaDeviceSenderCommandBodyV1): ByteArray = when (value) {
        is KagemushaDeviceSenderCommandBodyV1.Prepare -> enumPayload(value.ordinal, senderInputs(value.inputs))
        is KagemushaDeviceSenderCommandBodyV1.RecoverPrepared -> enumPayload(value.ordinal, value.inputsDigest())
        is KagemushaDeviceSenderCommandBodyV1.Commit -> enumPayload(
            value.ordinal, preparationSelector(value.selector), value.candidateDigest(),
        )
        is KagemushaDeviceSenderCommandBodyV1.RecoverTerminal -> enumPayload(value.ordinal, value.inputsDigest())
        is KagemushaDeviceSenderCommandBodyV1.Install -> enumPayload(
            value.ordinal,
            preparationSelector(value.selector),
            value.candidateDigest(),
            senderInputs(value.inputs),
            vectorBytes(value.canonicalEnvelope()),
        )
        is KagemushaDeviceSenderCommandBodyV1.RecoverInstalled -> enumPayload(
            value.ordinal, recoverySelector(value.selector),
        )
        is KagemushaDeviceSenderCommandBodyV1.Release -> enumPayload(
            value.ordinal,
            value.inputsDigest(),
            value.envelopeDigest(),
            senderInputs(value.inputs),
            vectorBytes(value.canonicalEnvelope()),
            vectorBytes(value.canonicalAcknowledgement()),
        )
    }

    private fun decodeSenderContext(payload: ByteArray): KagemushaDeviceSenderWalletContextV1 {
        val reader = DeviceReader(payload)
        val lane = decodeLane(reader.field())
        val release = decodeStateContext(reader.field())
        val credential = reader.exactField(32)
        val epoch = decodeHardwareEpoch(reader.field())
        val policy = decodePolicyBinding(reader.field())
        reader.finish()
        return KagemushaDeviceSenderWalletContextV1(lane, release, credential, epoch, policy)
    }

    private fun decodeLane(payload: ByteArray): KagemushaDeviceLaneIdV1 {
        val reader = DeviceReader(payload)
        val value = KagemushaDeviceLaneIdV1(
            reader.exactField(32),
            reader.exactField(32),
            reader.field(ACCOUNT_MAX),
            reader.u32Field(),
        )
        reader.finish()
        return value
    }

    private fun decodeStateContext(payload: ByteArray): KagemushaDeviceStateContextV1 {
        val reader = DeviceReader(payload)
        val protocol = reader.u16Field()
        val suite = reader.exactField(32)
        val vk = reader.exactField(32)
        val release = reader.exactField(32)
        val incarnationReader = DeviceReader(reader.field())
        val incarnation = incarnationReader.exactField(32)
        incarnationReader.finish()
        val profile = reader.exactField(32)
        val policyEpoch = reader.u64Field()
        reader.finish()
        return KagemushaDeviceStateContextV1(
            protocol,
            suite,
            vk,
            release,
            incarnation,
            profile,
            policyEpoch,
        )
    }

    private fun decodeHardwareEpoch(payload: ByteArray): KagemushaDeviceHardwareEpochV1 {
        val reader = DeviceReader(payload)
        val value = KagemushaDeviceHardwareEpochV1(reader.u128Field(), reader.exactField(32))
        reader.finish()
        return value
    }

    private fun decodePolicyBinding(payload: ByteArray): KagemushaDevicePolicyBindingV1 {
        val reader = DeviceReader(payload)
        val value = KagemushaDevicePolicyBindingV1(reader.exactField(32), reader.exactField(32))
        reader.finish()
        return value
    }

    private fun decodeSenderInputs(payload: ByteArray): KagemushaDeviceSenderPublicInputsV1 {
        val reader = DeviceReader(payload)
        val value = when (reader.enumTag(2)) {
            0 -> KagemushaDeviceSenderPublicInputsV1.SendSplit(
                reader.byteVectorField(PAYMENT_REQUEST_MAX),
            )
            1 -> KagemushaDeviceSenderPublicInputsV1.RedeemSplit(
                reader.u128Field(),
                reader.field(ACCOUNT_MAX),
            )
            else -> error("unreachable")
        }
        reader.finish()
        return value
    }

    private fun decodePreparationSelector(payload: ByteArray): KagemushaDeviceSenderPreparationSelectorV1 {
        val reader = DeviceReader(payload)
        val value = KagemushaDeviceSenderPreparationSelectorV1(
            reader.exactField(32), reader.exactField(32),
        )
        reader.finish()
        return value
    }

    private fun decodeRecoverySelector(payload: ByteArray): KagemushaDeviceSenderRecoverySelectorV1 {
        val reader = DeviceReader(payload)
        val value = when (reader.enumTag(2)) {
            0 -> KagemushaDeviceSenderRecoverySelectorV1.Lookup(reader.exactField(32))
            1 -> KagemushaDeviceSenderRecoverySelectorV1.Page(
                reader.optionField { it.u128Raw() },
                reader.optionField { it.exactRaw(32) },
                reader.u16Field(),
            )
            else -> error("unreachable")
        }
        reader.finish()
        return value
    }

    private fun decodeSenderCommandBody(payload: ByteArray): KagemushaDeviceSenderCommandBodyV1 {
        val reader = DeviceReader(payload)
        val value = when (reader.enumTag(7)) {
            0 -> KagemushaDeviceSenderCommandBodyV1.Prepare(decodeSenderInputs(reader.field()))
            1 -> KagemushaDeviceSenderCommandBodyV1.RecoverPrepared(reader.exactField(32))
            2 -> KagemushaDeviceSenderCommandBodyV1.Commit(
                decodePreparationSelector(reader.field()),
                reader.exactField(32),
            )
            3 -> KagemushaDeviceSenderCommandBodyV1.RecoverTerminal(reader.exactField(32))
            4 -> KagemushaDeviceSenderCommandBodyV1.Install(
                decodePreparationSelector(reader.field()),
                reader.exactField(32),
                decodeSenderInputs(reader.field()),
                reader.byteVectorField(TERMINAL_ENVELOPE_MAX),
            )
            5 -> KagemushaDeviceSenderCommandBodyV1.RecoverInstalled(
                decodeRecoverySelector(reader.field()),
            )
            6 -> KagemushaDeviceSenderCommandBodyV1.Release(
                reader.exactField(32),
                reader.exactField(32),
                decodeSenderInputs(reader.field()),
                reader.byteVectorField(TERMINAL_ENVELOPE_MAX),
                reader.byteVectorField(ACKNOWLEDGEMENT_MAX),
            )
            else -> error("unreachable")
        }
        reader.finish()
        return value
    }

    private fun validateSenderReplyBody(payload: ByteArray) {
        val reader = DeviceReader(payload)
        when (reader.enumTag(2)) {
            0 -> reader.optionField { item -> validateSenderRecoveryItem(item.allRaw()); Unit }
            1 -> {
                val entries = reader.itemVectorField(SENDER_PAGE_COUNT_MAX) { item ->
                    validateSenderRecoveryItem(item)
                }
                require(entries <= SENDER_PAGE_COUNT_MAX)
                reader.optionField { it.exactRaw(32); Unit }
            }
        }
        reader.finish()
    }

    private fun validateSenderRecoveryItem(payload: ByteArray) {
        val reader = DeviceReader(payload)
        validateSenderRecord(reader.field())
        reader.byteVectorField(TERMINAL_ENVELOPE_MAX, allowEmpty = true)
        reader.finish()
    }

    private fun validateSenderRecord(payload: ByteArray) {
        val reader = DeviceReader(payload)
        reader.exactField(32)
        decodeSenderContext(reader.field())
        reader.exactField(32)
        val kindReader = DeviceReader(reader.field())
        val kind = kindReader.enumTag(7)
        require(kind == 2 || kind == 4) {
            "sender record operation_kind is unsupported"
        }
        kindReader.finish()
        reader.exactField(32)
        reader.exactField(32)
        reader.exactField(32)
        val phase = reader.field()
        val phaseReader = DeviceReader(phase)
        phaseReader.enumTag(6)
        phaseReader.finish()
        reader.u128Field()
        reader.optionField { decodeSenderInputs(it.allRaw()); Unit }
        repeat(4) { reader.optionField { it.exactRaw(32); Unit } }
        reader.finish()
    }
}

/** Parsed only inside the SDK after a native response authenticator has succeeded. */
internal class KagemushaDeviceAuthenticatedReplyV1(
    @JvmField val operation: Int,
    canonicalArchive: ByteArray,
    payload: ByteArray,
) {
    private val archive = canonicalArchive.copyOf()
    private val body = payload.copyOf()
    fun canonicalArchive(): ByteArray = archive.copyOf()
    internal fun payload(): ByteArray = body.copyOf()
}

internal class KagemushaDeviceQualificationReplyV1(
    releaseId: ByteArray,
    hardwarePolicyDigest: ByteArray,
    @JvmField val profile: KagemushaHardwareProfileV1,
    @JvmField val credential: KagemushaHardwareCredentialV1,
) {
    private val release = nonzeroDigest(releaseId, "release_id")
    private val policy = nonzeroDigest(hardwarePolicyDigest, "hardware_policy_digest")
    fun releaseId(): ByteArray = release.copyOf()
    fun hardwarePolicyDigest(): ByteArray = policy.copyOf()
}

private data class DeviceArchiveDescriptor(
    val schema: String,
    val alignment: Int,
    val maximum: Int,
)

private const val VERSION = 1
private const val RECEIVER_PAGE_COUNT_MAX = 4
private const val SENDER_PAGE_COUNT_MAX = 4
private const val KAGEMUSHA_ASSET_SCALE_MAX = 28
private const val PAYMENT_REQUEST_MAX = 928
private const val PAYMENT_MAX = 7_552
private const val INBOX_STAGING_METADATA_MAX = 1_024
private const val REDEMPTION_VOUCHER_MAX = 7_936
private const val TERMINAL_ENVELOPE_MAX = REDEMPTION_VOUCHER_MAX
private const val ACKNOWLEDGEMENT_MAX = 256
private const val AGGREGATE_STATE_MAX = 768
private const val MINT_AUTHORIZATION_MAX = 7_936
private const val ACCOUNT_MAX = 512
private const val KAGEMUSHA_MODEL_SCHEMA_PREFIX =
    "iroha_data_model::kagemusha::kagemusha_v1::"
private const val HARDWARE_PROFILE_SCHEMA =
    KAGEMUSHA_MODEL_SCHEMA_PREFIX + "KagemushaHardwareProfileV1"
private const val HARDWARE_CREDENTIAL_SCHEMA =
    KAGEMUSHA_MODEL_SCHEMA_PREFIX + "KagemushaHardwareCredentialV1"

private const val CONTROL_READ_CREDENTIAL_SCHEMA =
    "iroha.kagemusha.device.v1.read-active-hardware-credential-command"
private const val CONTROL_SIGN_ACK_SCHEMA =
    "iroha.kagemusha.device.v1.sign-receive-acknowledgement-command"
private const val CONTROL_READ_TIME_SCHEMA =
    "iroha.kagemusha.device.v1.read-trusted-time-or-lease-command"
private const val CONTROL_PREPARE_MINT_SCHEMA =
    "iroha.kagemusha.device.v1.prepare-mint-authorization-command"
private const val CONTROL_RECOVER_MINT_SCHEMA =
    "iroha.kagemusha.device.v1.recover-mint-authorization-command"
private const val CONTROL_FOLD_RECEIVE_SCHEMA =
    "iroha.kagemusha.device.v1.fold-receive-credit-command"
private const val CONTROL_READ_WATERMARK_SCHEMA =
    "iroha.kagemusha.device.v1.read-pending-credit-watermark-command"
private const val CONTROL_ROTATE_EPOCH_SCHEMA =
    "iroha.kagemusha.device.v1.rotate-hardware-epoch-command"
private const val CONTROL_QUALIFICATION_REPLY_SCHEMA =
    "iroha.kagemusha.device.v1.active-hardware-credential-reply"
private const val CONTROL_ACK_REPLY_SCHEMA =
    "iroha.kagemusha.device.v1.receive-acknowledgement-reply"
private const val CONTROL_TIME_REPLY_SCHEMA =
    "iroha.kagemusha.device.v1.trusted-time-or-lease-reply"
private const val CONTROL_MINT_REPLY_SCHEMA =
    "iroha.kagemusha.device.v1.mint-authorization-reply"
private const val CONTROL_FOLD_REPLY_SCHEMA =
    "iroha.kagemusha.device.v1.fold-receive-credit-reply"
private const val CONTROL_WATERMARK_REPLY_SCHEMA =
    "iroha.kagemusha.device.v1.pending-credit-watermark-reply"
private const val CONTROL_ROTATION_REPLY_SCHEMA =
    "iroha.kagemusha.device.v1.rotate-hardware-epoch-reply"
private const val CONTROL_BOOTSTRAP_SCHEMA =
    "iroha.kagemusha.device.v1.bootstrap-aggregate-state-command"
private const val CONTROL_RECOVER_WALLET_SCHEMA =
    "iroha.kagemusha.device.v1.recover-wallet-snapshot-command"
private const val CONTROL_CREATE_REQUEST_SCHEMA =
    "iroha.kagemusha.device.v1.create-signed-payment-request-command"
private const val CONTROL_BOOTSTRAP_REPLY_SCHEMA =
    "iroha.kagemusha.device.v1.bootstrap-aggregate-state-reply"
private const val CONTROL_WALLET_SNAPSHOT_REPLY_SCHEMA =
    "iroha.kagemusha.device.v1.wallet-recovery-snapshot-reply"
private const val CONTROL_SIGNED_REQUEST_REPLY_SCHEMA =
    "iroha.kagemusha.device.v1.signed-payment-request-reply"

// Receiver names are stable explicit Rust #[norito(schema_name = ...)] identities.
private const val RECEIVER_STAGE_SCHEMA = "iroha.kagemusha.device.v1.stage-inbound-payment-command"
private const val RECEIVER_RECOVER_STAGED_SCHEMA = "iroha.kagemusha.device.v1.recover-staged-inbound-payment-command"
private const val RECEIVER_PAGE_SCHEMA = "iroha.kagemusha.device.v1.recover-inbound-inbox-page-command"
private const val RECEIVER_STAGED_REPLY_SCHEMA = "iroha.kagemusha.device.v1.staged-inbound-payment-reply"
private const val RECEIVER_PAGE_REPLY_SCHEMA = "iroha.kagemusha.device.v1.inbound-inbox-page-reply"
private const val SENDER_COMMAND_SCHEMA = "iroha.kagemusha.device.v1.sender-command"
private const val SENDER_REPLY_SCHEMA = "iroha.kagemusha.device.v1.sender-reply"

private val RAW_ADAPTER = object : TypeAdapter<ByteArray> {
    override fun encode(encoder: NoritoEncoder, value: ByteArray) = encoder.writeBytes(value)
    override fun decode(decoder: NoritoDecoder): ByteArray = decoder.readBytes(decoder.remaining())
}

private fun frame(schema: String, alignment: Int, payload: ByteArray, maximum: Int): ByteArray {
    val archive = NoritoCodec.encode(payload, schema, RAW_ADAPTER)
    val padding = (alignment - NoritoHeader.HEADER_LENGTH % alignment) % alignment
    val canonical = if (padding == 0) {
        archive
    } else {
        ByteArray(archive.size + padding).also { result ->
            archive.copyInto(result, endIndex = NoritoHeader.HEADER_LENGTH)
            archive.copyInto(
                result,
                destinationOffset = NoritoHeader.HEADER_LENGTH + padding,
                startIndex = NoritoHeader.HEADER_LENGTH,
            )
        }
    }
    require(canonical.isNotEmpty() && canonical.size <= maximum) { "device archive exceeds $maximum bytes" }
    return canonical
}

private fun unframe(bytes: ByteArray, descriptor: DeviceArchiveDescriptor): ByteArray {
    require(bytes.isNotEmpty() && bytes.size <= descriptor.maximum) {
        "device archive is empty or exceeds ${descriptor.maximum} bytes"
    }
    val canonical = bytes.copyOf()
    val decoded = NoritoHeader.decode(canonical, SchemaHash.hash16(descriptor.schema))
    require(decoded.header.compression == NoritoHeader.COMPRESSION_NONE) { "compressed device archive" }
    require(decoded.header.flags == NoritoHeader.COMPACT_LEN) { "noncanonical device archive flags" }
    val expectedPadding =
        (descriptor.alignment - NoritoHeader.HEADER_LENGTH % descriptor.alignment) % descriptor.alignment
    require(canonical.size == NoritoHeader.HEADER_LENGTH + expectedPadding + decoded.payload.size) {
        "wrong device archive alignment"
    }
    decoded.header.validateChecksum(decoded.payload)
    return decoded.payload
}

private fun fields(vararg values: ByteArray): ByteArray = DeviceWriter().apply {
    values.forEach(::field)
}.bytes()

private fun enumPayload(tag: Int, vararg values: ByteArray): ByteArray = DeviceWriter().apply {
    raw(u32(tag))
    values.forEach(::field)
}.bytes()

private fun option(value: ByteArray?): ByteArray = DeviceWriter().apply {
    if (value == null) {
        raw(byteArrayOf(0))
    } else {
        raw(byteArrayOf(1))
        field(value)
    }
}.bytes()

private fun vectorBytes(value: ByteArray): ByteArray = DeviceWriter().apply {
    raw(u64(value.size.toLong()))
    raw(value)
}.bytes()

private fun u8(value: Int): ByteArray = byteArrayOf(value.toByte())
private fun u16(value: Int): ByteArray = ByteBuffer.allocate(2).order(ByteOrder.LITTLE_ENDIAN)
    .putShort(value.toShort()).array()
private fun u32(value: Int): ByteArray = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN)
    .putInt(value).array()
private fun u64(value: Long): ByteArray = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN)
    .putLong(value).array()
private fun u128(value: BigInteger): ByteArray {
    requireU128(value, "u128")
    val bigEndian = value.toByteArray()
    val source = if (bigEndian.size == 17 && bigEndian[0].toInt() == 0) {
        bigEndian.copyOfRange(1, bigEndian.size)
    } else {
        bigEndian
    }
    return ByteArray(16).also { output -> source.reversedArray().copyInto(output) }
}

private class DeviceWriter {
    private val output = ArrayList<Byte>()
    fun raw(value: ByteArray) { value.forEach(output::add) }
    fun field(value: ByteArray) { length(value.size); raw(value) }
    private fun length(length: Int) {
        require(length >= 0)
        var remaining = length.toLong()
        while (remaining >= 0x80) {
            output.add(((remaining and 0x7f) or 0x80).toByte())
            remaining = remaining ushr 7
        }
        output.add(remaining.toByte())
    }
    fun bytes(): ByteArray = ByteArray(output.size) { output[it] }
}

private class DeviceReader(private val bytes: ByteArray) {
    private var offset = 0
    fun hasRemaining(): Boolean = offset < bytes.size
    fun field(maximum: Int = bytes.size): ByteArray {
        val length = length()
        require(length <= maximum && length <= bytes.size - offset) { "truncated or oversized device field" }
        return exactRaw(length)
    }
    fun exactField(width: Int): ByteArray = field(width).also {
        require(it.size == width) { "device field has the wrong width" }
    }
    fun u16Field(): Int = ByteBuffer.wrap(exactField(2)).order(ByteOrder.LITTLE_ENDIAN)
        .short.toInt() and 0xffff
    fun u8Field(): Int = exactField(1)[0].toInt() and 0xff
    fun u32Field(): Int = ByteBuffer.wrap(exactField(4)).order(ByteOrder.LITTLE_ENDIAN).int
    fun u64Field(): Long = ByteBuffer.wrap(exactField(8)).order(ByteOrder.LITTLE_ENDIAN).long
    fun u128Field(): BigInteger = BigInteger(1, exactField(16).reversedArray())
    fun u128Raw(): BigInteger = BigInteger(1, exactRaw(16).reversedArray())
    fun enumTag(variants: Int): Int {
        val tag = ByteBuffer.wrap(exactRaw(4)).order(ByteOrder.LITTLE_ENDIAN).int
        require(tag in 0 until variants) { "unknown device enum tag" }
        return tag
    }
    fun byteVectorField(maximum: Int, allowEmpty: Boolean = false): ByteArray {
        val nested = DeviceReader(field(maximum + 8))
        return nested.byteVectorRaw(maximum, allowEmpty).also { nested.finish() }
    }
    fun byteVectorRaw(maximum: Int, allowEmpty: Boolean = false): ByteArray {
        val length = u64Raw()
        require(length <= maximum.toLong() && length <= Int.MAX_VALUE.toLong()) {
            "oversized device byte vector"
        }
        val value = exactRaw(length.toInt())
        require(allowEmpty || value.isNotEmpty()) { "empty device byte vector" }
        return value
    }
    fun <T> optionField(decode: (DeviceReader) -> T): T? {
        val nested = DeviceReader(field())
        val tag = nested.exactRaw(1)[0].toInt() and 0xff
        val value = when (tag) {
            0 -> null
            1 -> {
                val item = DeviceReader(nested.field())
                val decoded = decode(item)
                item.finish()
                decoded
            }
            else -> throw IllegalArgumentException("unknown device option tag")
        }
        nested.finish()
        return value
    }
    fun itemVectorField(maximumEntries: Int, validate: (ByteArray) -> Unit): Int {
        val nested = DeviceReader(field())
        val count = nested.u64Raw()
        require(count <= maximumEntries.toLong()) { "device vector has too many entries" }
        repeat(count.toInt()) { validate(nested.field()) }
        nested.finish()
        return count.toInt()
    }
    fun exactRaw(width: Int): ByteArray {
        require(width >= 0 && width <= bytes.size - offset) { "truncated device payload" }
        return bytes.copyOfRange(offset, offset + width).also { offset += width }
    }
    fun allRaw(): ByteArray = exactRaw(bytes.size - offset)
    private fun u64Raw(): Long = ByteBuffer.wrap(exactRaw(8)).order(ByteOrder.LITTLE_ENDIAN).long.also {
        require(it >= 0) { "device length exceeds JVM range" }
    }
    private fun length(): Int {
        var result = 0L
        var shift = 0
        var count = 0
        while (count < 10) {
            val byte = exactRaw(1)[0].toInt() and 0xff
            val chunk = byte and 0x7f
            require(shift < 64 && !(shift == 63 && chunk > 1)) { "device field length overflow" }
            result = result or (chunk.toLong() shl shift)
            count += 1
            if (byte and 0x80 == 0) {
                require(count <= 5) { "device field length exceeds JVM range" }
                require(count == 1 || result >= (1L shl (7 * (count - 1)))) {
                    "nonminimal device field length"
                }
                require(result <= Int.MAX_VALUE.toLong()) { "device field length exceeds JVM range" }
                return result.toInt()
            }
            shift += 7
        }
        throw IllegalArgumentException("device field length overflow")
    }
    fun finish() { require(offset == bytes.size) { "trailing device payload bytes" } }
}

private fun exactCopy(value: ByteArray, size: Int, field: String): ByteArray {
    require(value.size == size) { "$field must be $size bytes" }
    return value.copyOf()
}

private fun nonzeroDigest(value: ByteArray, field: String): ByteArray =
    exactCopy(value, 32, field).also { require(it.any { byte -> byte.toInt() != 0 }) { "$field must be nonzero" } }

private fun boundedCopy(value: ByteArray, maximum: Int, field: String): ByteArray {
    require(value.isNotEmpty() && value.size <= maximum) { "$field is empty or oversized" }
    return value.copyOf()
}

private fun boundedCopyAllowEmpty(value: ByteArray, maximum: Int, field: String): ByteArray {
    require(value.size <= maximum) { "$field is oversized" }
    return value.copyOf()
}

private fun requireU128(value: BigInteger, field: String) {
    require(value.signum() >= 0 && value.bitLength() <= 128) { "$field is outside the u128 domain" }
}
