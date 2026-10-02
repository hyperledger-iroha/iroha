// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import org.hyperledger.iroha.sdk.offline.probe.KagemushaTestnetStateProofObservationV1

/** Detached, non-authorizing original paired State proof from one retained Core operation. */
class KagemushaOutgoingStateProofArchivesV1 internal constructor(
    operationId: ByteArray,
    publicInputsArchive: ByteArray,
    pairedProofArchive: ByteArray,
) {
    private val operation = operationId.copyOf()
    private val publicInputs = publicInputsArchive.copyOf()
    private val proof = pairedProofArchive.copyOf()

    init {
        require(operation.size == 32 && operation.any { it != 0.toByte() })
        require(publicInputs.size in 1..4 * 1024)
        require(proof.size in 1..6_528)
    }

    fun operationId(): ByteArray = operation.copyOf()
    fun publicInputsArchive(): ByteArray = publicInputs.copyOf()
    fun pairedProofArchive(): ByteArray = proof.copyOf()

    /** Verify this exact pair with the independently installed, non-authorizing testnet observer. */
    fun observeWith(observer: KagemushaTestnetStateProofObservationV1): ByteArray =
        observer.observeStateProof(publicInputs, proof)
}

/**
 * Typed coordinator backed exclusively by the qualified native JNI implementation.
 *
 * Public archive and identity checks provide defense in depth. Native Core remains responsible
 * for authenticated hardware replies, proof generation, release admission, durable journal lookup,
 * historical authority, and commit ordering. No serialized projection supplies those capabilities.
 */
class KagemushaNativeCoreCoordinatorAdapterV1 private constructor(
    private val bridge: KagemushaCoreCoordinatorBridgeV1,
) : KagemushaNativeCoreCoordinatorV1, AutoCloseable {
    private val enrollmentPhases = KagemushaNativeEnrollmentPhasesV1(bridge)
    private val recoveredEnrollment = KagemushaNativeRecoveredEnrollmentV1(bridge)
    private val appIdentity = KagemushaNativeAppApprovalCoordinatorV1(bridge)

    /** Reuse this coordinator's sole native owner for bounded initial-enrollment phases. */
    fun initialEnrollment(): KagemushaNativeEnrollmentPhasesV1 = enrollmentPhases

    /** Authenticate the original enrolled native owner after process restart. */
    fun recoveredEnrollment(): KagemushaNativeRecoveredEnrollmentV1 = recoveredEnrollment

    /** Use the same actual native owner for non-monetary app approval and enrollment possession. */
    fun appIdentityOperations(): KagemushaNativeAppApprovalCoordinatorV1 = appIdentity

    /** Bind a separate full-original current FI transport to this sole opened descriptor.
     * This returns data transport only; Bootstrap and fields never establish money readiness.
     */
    fun ordinaryCurrentControlTransportBinding(): KagemushaOrdinaryCurrentControlTransportBindingV1 =
        KagemushaOrdinaryCurrentControlTransportBindingV1(bridge)

    /** Revoke this native owner during logout or account switch. A new open needs a new process. */
    override fun close() {
        enrollmentPhases.revokeLocal()
        recoveredEnrollment.revokeLocal()
        bridge.close()
    }

    override fun authenticatedHardwarePolicy(): KagemushaAuthenticatedHardwarePolicyV1 {
        val fields = bridge.invoke(KagemushaCoreCoordinatorMethodV1.AUTHENTICATED_HARDWARE_POLICY, emptyList())
        return KagemushaAuthenticatedHardwarePolicyV1(fields[0], fields[1], fields[2])
    }

    override fun stageIncomingOriginal(kind: KagemushaIncomingStageKindV1, creditId: ByteArray): ByteArray =
        bridge.invoke(KagemushaCoreCoordinatorMethodV1.STAGE_INCOMING_ORIGINAL,
            listOf(u32(kind.code), creditId)).single()

    override fun prepareIncomingFold(selector: KagemushaPendingCreditSelectorV1): KagemushaNativeIncomingFoldPreparationV1 {
        val response = bridge.invoke(KagemushaCoreCoordinatorMethodV1.PREPARE_INCOMING_FOLD,
            listOf(u32(selector.kind.ordinal), selector.creditId()))
        return KagemushaNativeIncomingFoldPreparationV1(selector.kind, response[0], response[1], response[2],
            response[3], response[4], response[5], response[6], BigInteger(1, response[7].reversedArray()),
            response[8], response[9])
    }

    override fun completeIncomingFold(preparation: KagemushaNativeIncomingFoldPreparationV1,
        evidence: KagemushaIncomingFoldEvidenceV1): ByteArray {
        preparation.requireEvidence(evidence)
        return bridge.invoke(KagemushaCoreCoordinatorMethodV1.COMPLETE_INCOMING_FOLD,
            listOf(preparation.historyOperationId(), preparation.canonicalPairedProof(),
                evidence.canonicalHardwareTransitionCertificate(), evidence.deviceRootSelectionSignature())).single()
    }

    override fun beginObservation(operation: Int, canonicalCommand: ByteArray): ByteArray =
        bridge.invoke(KagemushaCoreCoordinatorMethodV1.BEGIN_OBSERVATION,
            listOf(u32(operation), canonicalCommand)).single().also {
            KagemushaDeviceOperationCodecV1.decodeControlCommand(operation, it, canonicalCommand)
        }

    /**
     * Admit the original signed op-1 report under the live phase-one native owner. This supplies
     * enrollment evidence and does not create a monetary hardware wallet or bypass its readiness.
     */
    fun prepareInitialEnrollmentQualification(selection: KagemushaNativeEnrollmentPhasesV1.Selection,
        device: KagemushaDeviceLifecycleBridgeV1, guard: () -> Unit): KagemushaPreEnrollmentDeviceQualificationV1 {
        guard()
        check(enrollmentPhases.recoverExactSelection(selection.accountI105) === selection) {
            "The original enrollment selection is no longer live"
        }
        val transport = KagemushaAndroidAuthenticatedDeviceTransportV1(device)
        val command = KagemushaDeviceOperationCodecV1.encodeControlCommand(KagemushaDeviceControlCommandV1.ReadActiveHardwareCredential)
        val nonce = beginObservation(1, command)
        guard()
        val response = transport.executeAndVerify(1, nonce, command, null)
        guard()
        val report = KagemushaPreEnrollmentDeviceQualificationV1.decodeAfterDeviceAuthentication(response,
            transport.hardwarePolicyId(), transport.qualificationReportDigest())
        same(report.releaseId(), selection.releaseId(), "enrollment release")
        same(report.profile.hardwareProfileId(), selection.profileId(), "enrollment profile")
        val fields = listOf(u32(KagemushaWireV1.WIRE_VERSION), report.releaseId(),
            KagemushaNoritoV1.encodeHardwareProfileShape(report.profile),
            KagemushaNoritoV1.encodeHardwareCredentialShape(report.credential), u32(0xffff))
        guard()
        bridge.invoke(KagemushaCoreCoordinatorMethodV1.ACCEPT_QUALIFICATION,
            fields + listOf(report.hardwarePolicyDigest()))
        guard()
        bridge.invoke(KagemushaCoreCoordinatorMethodV1.ACCEPT_AUTHENTICATED_REPLY,
            listOf(u32(1), nonce, command, report.canonicalControlReply(), report.authenticator()) + fields)
        guard()
        return report
    }

    /** Copy the original retained outgoing proof into an unqualified testnet observer input. */
    fun exportOutgoingStateProof(operationId: ByteArray): KagemushaOutgoingStateProofArchivesV1 {
        val id = operationId.copyOf()
        val response = bridge.invoke(
            KagemushaCoreCoordinatorMethodV1.EXPORT_OUTGOING_STATE_PROOF,
            listOf(id),
        )
        return KagemushaOutgoingStateProofArchivesV1(response[0], response[1], response[2])
    }

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
        originalResponse: ByteArray?,
    ) {
        val fields = listOf(u32(operation), requestId, canonicalCommand, canonicalReply, responseAuthenticator) + qualificationFields(qualification)
        val request = if (operation == 12) fields + listOf(requireNotNull(originalResponse) {
            "release acceptance requires its original signed response"
        }.copyOf()) else fields
        bridge.invoke(KagemushaCoreCoordinatorMethodV1.ACCEPT_AUTHENTICATED_REPLY, request)
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

    override fun terminalEnvelope(candidate: KagemushaNativeSenderCandidateV1, originalCommitResponseFrame: ByteArray): ByteArray =
        bridge.invoke(KagemushaCoreCoordinatorMethodV1.BUILD_TERMINAL_ENVELOPE,
            listOf(KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(candidate), originalCommitResponseFrame))
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
        val installedPolicy = authenticatedHardwarePolicy()
        same(state.releaseId(), installedPolicy.releaseId(), "terminal native installed release")
        same(state.hardwarePolicyId(), installedPolicy.providerPolicyRoot(), "terminal native provider registry root")
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
        val policy = authenticatedHardwarePolicy().also { it.requireQualification(qualification) }
        same(context.devicePolicyBinding.hardwarePolicyId(), policy.providerPolicyRoot(), "preparation policy registry root")
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
