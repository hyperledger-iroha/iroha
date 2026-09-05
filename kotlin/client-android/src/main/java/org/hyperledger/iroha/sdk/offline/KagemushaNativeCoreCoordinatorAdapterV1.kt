// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger

/**
 * Typed coordinator backed exclusively by the qualified native JNI implementation.
 *
 * Public archive and identity checks provide defense in depth. Native Core remains responsible
 * for authenticated hardware replies, proof generation, release admission, durable journal lookup,
 * historical authority, and commit ordering. No serialized projection supplies those capabilities.
 */
class KagemushaNativeCoreCoordinatorAdapterV1 private constructor(
    private val bridge: KagemushaCoreCoordinatorBridgeV1,
) : KagemushaNativeCoreCoordinatorV1 {
    override fun reserveOperationId(operation: Int, operationId: ByteArray, publicBinding: ByteArray): ByteArray =
        bridge.invoke(KagemushaCoreCoordinatorMethodV1.RESERVE_OPERATION_ID,
            listOf(u32(operation), operationId, publicBinding)).single()

    override fun acceptQualification(qualification: KagemushaHardwareQualificationV1, hardwarePolicyDigest: ByteArray) {
        same(hardwarePolicyDigest, qualification.hardwarePolicyDigest(), "qualification policy")
        bridge.invoke(KagemushaCoreCoordinatorMethodV1.ACCEPT_QUALIFICATION,
            qualificationFields(qualification) + listOf(hardwarePolicyDigest))
    }

    override fun acceptAuthenticatedDeviceReply(
        operation: Int, requestId: ByteArray, canonicalCommand: ByteArray, canonicalReply: ByteArray,
        responseAuthenticator: ByteArray,
        qualification: KagemushaHardwareQualificationV1,
    ) {
        bridge.invoke(KagemushaCoreCoordinatorMethodV1.ACCEPT_AUTHENTICATED_REPLY,
            listOf(u32(operation), requestId, canonicalCommand, canonicalReply, responseAuthenticator) + qualificationFields(qualification))
    }

    override fun beginSenderTransition(
        operationId: ByteArray, inputs: KagemushaDeviceSenderPublicInputsV1,
        qualification: KagemushaHardwareQualificationV1,
    ): KagemushaNativeSenderPreparationV1 {
        val id = operationId.copyOf()
        val response = bridge.invoke(KagemushaCoreCoordinatorMethodV1.BEGIN_SENDER_TRANSITION,
            listOf(id) + inputFields(inputs) + qualificationFields(qualification))
        val preparation = KagemushaCoreCoordinatorArchiveV1.decodePreparationShapeExact(response[1])
        same(preparation.operationId(), id, "preparation operation")
        bindCurrent(preparation.context, qualification)
        bindInputs(preparation, inputs)
        return preparation
    }

    override fun provePreparedSenderTransition(
        preparation: KagemushaNativeSenderPreparationV1, authenticatedPreparationReply: ByteArray,
    ): KagemushaNativeSenderCandidateV1 {
        val archive = KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(preparation)
        val response = bridge.invoke(KagemushaCoreCoordinatorMethodV1.PROVE_PREPARED_SENDER_TRANSITION,
            listOf(archive, authenticatedPreparationReply))
        return KagemushaCoreCoordinatorArchiveV1.decodeCandidateShapeExact(response.single()).also {
            same(KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(it.preparation), archive, "candidate preparation")
        }
    }

    override fun terminalEnvelope(candidate: KagemushaNativeSenderCandidateV1, authenticatedCommitReply: ByteArray): ByteArray =
        bridge.invoke(KagemushaCoreCoordinatorMethodV1.BUILD_TERMINAL_ENVELOPE,
            listOf(KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(candidate), authenticatedCommitReply))
            .single().also { KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(it) }

    override fun acceptInstalledTerminal(
        candidate: KagemushaNativeSenderCandidateV1, canonicalEnvelope: ByteArray,
        authenticatedInstallReply: ByteArray, authenticatedInstalledReply: ByteArray,
        authenticatedWalletSnapshotReply: ByteArray,
    ): KagemushaHardwareTerminalResultV1 {
        val response = bridge.invoke(KagemushaCoreCoordinatorMethodV1.ACCEPT_INSTALLED_TERMINAL,
            listOf(KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(candidate), canonicalEnvelope,
                authenticatedInstallReply, authenticatedInstalledReply, authenticatedWalletSnapshotReply))
        val state = KagemushaNoritoV1.decodeAggregateStateShapeExact(response[1])
        val context = candidate.preparation.context
        same(state.releaseId(), context.release.releaseId(), "terminal release")
        same(state.networkId.bytes(), context.lane.networkId(), "terminal network")
        same(state.laneId(), context.lane.deviceLaneId(), "terminal lane")
        same(state.asset.canonicalPayload(), context.lane.assetCanonicalPayload(), "terminal asset")
        same(state.assetIncarnation.bytes(), context.release.assetIncarnation(), "terminal asset incarnation")
        require(state.scale == context.lane.scale) { "terminal scale mismatch" }
        same(state.hardwareEpochId(), context.hardwareEpoch.epochId(), "terminal epoch")
        same(state.keyReference(), context.devicePolicyBinding.deviceKeyReference(), "terminal device key")
        same(state.hardwarePolicyId(), context.devicePolicyBinding.hardwarePolicyId(), "terminal hardware policy")
        same(state.liabilityPoolId(), KagemushaNoritoV1.liabilityPoolId(
            state.networkId, state.asset, state.assetIncarnation), "terminal liability pool")
        return KagemushaHardwareTerminalResultV1(response[0], response[1])
    }

    override fun senderRecovery(
        kind: KagemushaNativeSenderKindV1, terminalId: ByteArray, qualification: KagemushaHardwareQualificationV1,
    ): KagemushaNativeSenderRecoveryV1? = recover(0, kind, terminalId, qualification)

    override fun senderRecoveryByOperationId(
        kind: KagemushaNativeSenderKindV1, operationId: ByteArray, qualification: KagemushaHardwareQualificationV1,
    ): KagemushaNativeSenderRecoveryV1? = recover(1, kind, operationId, qualification)

    private fun recover(
        selector: Int, kind: KagemushaNativeSenderKindV1, id: ByteArray, qualification: KagemushaHardwareQualificationV1,
    ): KagemushaNativeSenderRecoveryV1? {
        val response = bridge.invoke(KagemushaCoreCoordinatorMethodV1.RECOVER_SENDER,
            listOf(byteArrayOf(selector.toByte()), id, u32(kind.ordinal)) + qualificationFields(qualification))
        if (response.isEmpty()) return null
        return KagemushaCoreCoordinatorArchiveV1.decodeRecoveryShapeExact(response[2]).also {
            same(it.operationId(), response[0], "recovery operation")
            same(it.terminalId(), response[1], "recovery terminal")
            bindRetained(it.context, qualification)
        }
    }

    override fun recoverTerminalEnvelope(recovery: KagemushaNativeSenderRecoveryV1, authenticatedInstalledReply: ByteArray): ByteArray =
        bridge.invoke(KagemushaCoreCoordinatorMethodV1.RECOVER_TERMINAL_ENVELOPE,
            listOf(KagemushaCoreCoordinatorArchiveV1.encodeRecoveryShape(recovery), authenticatedInstalledReply))
            .single().also { KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(it) }

    override fun outboxRelease(
        creditId: ByteArray, inputs: KagemushaDeviceSenderPublicInputsV1, canonicalPayment: ByteArray,
        terminalReceipt: KagemushaDeviceSenderTerminalReceiptV1, qualification: KagemushaHardwareQualificationV1,
    ): KagemushaNativeOutboxReleaseV1 {
        require(inputs.ordinal == terminalReceipt.ordinal) { "receipt sender kind mismatch" }
        val envelope = canonicalPayment.copyOf()
        val receipt = when (terminalReceipt) {
            is KagemushaDeviceSenderTerminalReceiptV1.PaymentAcknowledgement -> {
                val request = KagemushaNoritoV1.decodePaymentRequestShapeExact(
                    (inputs as KagemushaDeviceSenderPublicInputsV1.SendSplit).canonicalRequest())
                val payment = KagemushaNoritoV1.decodePaymentShapeExact(envelope, request)
                same(payment.output.creditId(), creditId, "release credit")
                val bytes = terminalReceipt.canonicalAcknowledgement()
                KagemushaNoritoV1.decodeAcknowledgementShapeExact(bytes, request, payment)
                bytes
            }
            is KagemushaDeviceSenderTerminalReceiptV1.RedemptionSettlement -> {
                val voucher = KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(envelope)
                val redemption = inputs as KagemushaDeviceSenderPublicInputsV1.RedeemSplit
                require(voucher.statement.amount == redemption.amount) { "release redemption amount mismatch" }
                same(voucher.statement.beneficiary.canonicalPayload(), redemption.beneficiaryCanonicalPayload(), "release beneficiary")
                same(voucher.statement.redemptionId(), creditId, "voucher redemption")
                same(voucher.statement.terminalNullifier(), terminalReceipt.receipt.terminalNullifier(), "receipt nullifier")
                same(voucher.statement.lifecycle.networkId.bytes(), terminalReceipt.receipt.networkId(), "receipt voucher network")
                same(terminalReceipt.receipt.redemptionId(), creditId, "release redemption")
                same(terminalReceipt.receipt.envelopeDigest(),
                    KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(envelope), "receipt envelope")
                KagemushaCoreCoordinatorArchiveV1.encodeRedemptionReceiptShape(terminalReceipt.receipt)
            }
        }
        val response = bridge.invoke(KagemushaCoreCoordinatorMethodV1.RELEASE_OUTBOX,
            listOf(creditId) + inputFields(inputs) + listOf(envelope, u32(terminalReceipt.ordinal) + receipt) +
                qualificationFields(qualification))
        val preparation = KagemushaCoreCoordinatorArchiveV1.decodePreparationShapeExact(response[1])
        same(preparation.operationId(), response[0], "release operation")
        bindRetained(preparation.context, qualification)
        bindInputs(preparation, inputs)
        same(response[2], KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(envelope), "release envelope digest")
        if (terminalReceipt is KagemushaDeviceSenderTerminalReceiptV1.RedemptionSettlement) {
            same(terminalReceipt.receipt.operationId(), preparation.operationId(), "receipt operation")
            same(terminalReceipt.receipt.networkId(), preparation.context.lane.networkId(), "receipt network")
        }
        return KagemushaNativeOutboxReleaseV1(preparation.operationId(), preparation.context,
            preparation.inputsDigest(), response[2], inputs, response[3], response[4])
    }

    private fun qualificationFields(value: KagemushaHardwareQualificationV1): List<ByteArray> {
        value.requireProductionReady()
        return listOf(u32(value.protocolVersion), value.releaseId(),
            KagemushaNoritoV1.encodeHardwareProfileShape(value.profile),
            KagemushaNoritoV1.encodeHardwareCredentialShape(value.credential), u32(0xffff))
    }

    private fun inputFields(value: KagemushaDeviceSenderPublicInputsV1): List<ByteArray> = when (value) {
        is KagemushaDeviceSenderPublicInputsV1.SendSplit -> listOf(u32(0), value.canonicalRequest())
        is KagemushaDeviceSenderPublicInputsV1.RedeemSplit -> {
            val amount = value.amount.toByteArray().reversedArray().copyOf(16)
            listOf(u32(1), amount, value.beneficiaryCanonicalPayload())
        }
    }

    private fun bindInputs(preparation: KagemushaNativeSenderPreparationV1, inputs: KagemushaDeviceSenderPublicInputsV1) {
        same(preparation.inputsDigest(), KagemushaCoreCoordinatorArchiveV1.inputsDigestShape(
            preparation.operationId(), preparation.context, inputs), "sender public inputs")
    }

    private fun bindRetained(context: KagemushaDeviceSenderWalletContextV1, qualification: KagemushaHardwareQualificationV1) {
        val credential = qualification.credential
        same(context.lane.networkId(), credential.networkId.bytes(), "retained network")
        same(context.lane.deviceLaneId(), credential.laneCommitment(), "retained lane")
        val generation = BigInteger(java.lang.Long.toUnsignedString(credential.hardwareEpochGeneration))
        require(context.hardwareEpoch.generation <= generation) { "retained epoch is from the future" }
        if (context.hardwareEpoch.generation == generation) {
            same(context.hardwareEpoch.epochId(), credential.hardwareEpochId(), "retained epoch")
        }
        // Historical release, suite, credential and policy may rotate. Native Core authenticates
        // the retained creation record and binds the asset/incarnation to its current wallet.
    }

    private fun bindCurrent(context: KagemushaDeviceSenderWalletContextV1, qualification: KagemushaHardwareQualificationV1) {
        bindRetained(context, qualification)
        val credential = qualification.credential
        same(context.credentialId(), credential.credentialId(), "preparation credential")
        same(context.release.releaseId(), qualification.releaseId(), "preparation release")
        same(context.release.hardwareProfileId(), qualification.profile.hardwareProfileId(), "preparation profile")
        same(context.release.suiteId(), credential.suiteId(), "preparation suite")
        require(context.release.policyEpoch == credential.policyEpoch) { "preparation policy epoch mismatch" }
        require(context.hardwareEpoch.generation == BigInteger(java.lang.Long.toUnsignedString(credential.hardwareEpochGeneration))) {
            "preparation hardware generation mismatch"
        }
        same(context.devicePolicyBinding.deviceKeyReference(), credential.deviceKeyReference(), "preparation device key")
        same(context.devicePolicyBinding.hardwarePolicyId(), qualification.hardwarePolicyDigest(), "preparation policy")
        same(context.coreAuthorizationKeyReference(), qualification.coreAuthorizationKeyReference(), "preparation Core key")
    }

    private fun same(actual: ByteArray, expected: ByteArray, label: String) {
        require(actual.contentEquals(expected)) { "$label mismatch" }
    }

    private fun u32(value: Int): ByteArray = KagemushaCoreCoordinatorFrameV1.u32(value)

    companion object {
        /** Open the real native ABI; absent JNI or qualified backend fails closed. */
        @JvmStatic fun open(storagePath: String): KagemushaNativeCoreCoordinatorAdapterV1 =
            KagemushaNativeCoreCoordinatorAdapterV1(KagemushaCoreCoordinatorBridgeV1.open(storagePath))

        internal fun openEndpoint(storagePath: String, endpoint: KagemushaCoreCoordinatorEndpointV1): KagemushaNativeCoreCoordinatorAdapterV1 =
            KagemushaNativeCoreCoordinatorAdapterV1(KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath, endpoint))
    }
}
