// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.EnumSet
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Stable outer status returned by the authenticated KAGEMUSHA V1 device bridge. */
enum class KagemushaAuthenticatedDeviceStatusV1(val code: Int) {
    SUCCESS(0),
    UNAVAILABLE(1),
    STALE_OR_CONCURRENT(2),
    BINDING_MISMATCH(3),
    TRUSTED_TIME_REJECTED(4),
    REJECTED(5),
    MISSING(6),
    CONFLICT(7),
    CORRUPT(8),
    MALFORMED_REQUEST(9),
    RECOVERY_REQUIRED(10),
}

/** Result whose successful response was admitted by the native KAGEMUSHA V1 authenticator verifier. */
class KagemushaAuthenticatedDeviceResponseV1(
    @JvmField val operation: Int,
    @JvmField val status: KagemushaAuthenticatedDeviceStatusV1,
    canonicalReply: ByteArray,
    authenticator: ByteArray,
) {
    private val reply = canonicalReply.copyOf()
    private val signature = authenticator.copyOf()

    init {
        require(operation in 1..22) { "operation is outside the frozen KAGEMUSHA V1 inventory" }
        if (status == KagemushaAuthenticatedDeviceStatusV1.SUCCESS) {
            require(reply.isNotEmpty()) { "successful device response omitted its canonical reply" }
            KagemushaP256Codec.requireRawLowSSignature(signature)
        } else {
            require(reply.isEmpty() && signature.isEmpty()) {
                "non-success device response carried unauthenticated bytes"
            }
        }
    }

    fun canonicalReply(): ByteArray = reply.copyOf()
    fun authenticator(): ByteArray = signature.copyOf()
}

/**
 * Platform bridge which verifies the complete KAGEMUSHA V1 response before returning a success.
 *
 * Operation 1 must pass `null` for [acceptedDevicePublicKey]. Operations 2 through 22 must pass
 * the exact 65-byte P-256 key pinned from that operation-1 exchange. Implementations must invoke
 * `connect_norito_kagemusha_device_response_authenticator_v1_verify`; JVM signature verification
 * is not an implementation of this boundary.
 */
interface KagemushaNativeAuthenticatedDeviceTransportV1 {
    fun hardwarePolicyId(): ByteArray
    fun qualificationReportDigest(): ByteArray

    fun executeAndVerify(
        operation: Int,
        requestId: ByteArray,
        canonicalCommand: ByteArray,
        acceptedDevicePublicKey: ByteArray?,
    ): KagemushaAuthenticatedDeviceResponseV1
}

/** A sender transition prepared under one durable native-Core operation identity. */
class KagemushaNativeSenderPreparationV1(
    operationId: ByteArray,
    @JvmField val context: KagemushaDeviceSenderWalletContextV1,
    inputsDigest: ByteArray,
) {
    private val operationIdValue = authenticatedDigest(operationId, "operationId")
    private val inputsDigestValue = authenticatedDigest(inputsDigest, "inputsDigest")
    fun operationId(): ByteArray = operationIdValue.copyOf()
    fun inputsDigest(): ByteArray = inputsDigestValue.copyOf()
}

/** Native-Core proof material admitted after the authenticated operation-5/6 reply. */
class KagemushaNativeSenderCandidateV1(
    @JvmField val preparation: KagemushaNativeSenderPreparationV1,
    @JvmField val selector: KagemushaDeviceSenderPreparationSelectorV1,
    candidateDigest: ByteArray,
    hardwareCommitAuthorization: ByteArray,
) {
    private val candidateDigestValue = authenticatedDigest(candidateDigest, "candidateDigest")
    private val commitAuthorization = boundedAuthenticatedBytes(
        hardwareCommitAuthorization,
        2 * 1024,
        "hardwareCommitAuthorization",
    )
    fun candidateDigest(): ByteArray = candidateDigestValue.copyOf()
    fun hardwareCommitAuthorization(): ByteArray = commitAuthorization.copyOf()
}

/** Native-Core lookup state for byte-identical operation-10 recovery. */
class KagemushaNativeSenderRecoveryV1(
    operationId: ByteArray,
    terminalId: ByteArray,
    @JvmField val context: KagemushaDeviceSenderWalletContextV1,
    inputsDigest: ByteArray,
) {
    private val operationIdValue = authenticatedDigest(operationId, "operationId")
    private val terminalIdValue = authenticatedDigest(terminalId, "terminalId")
    private val inputsDigestValue = authenticatedDigest(inputsDigest, "inputsDigest")
    fun operationId(): ByteArray = operationIdValue.copyOf()
    fun terminalId(): ByteArray = terminalIdValue.copyOf()
    fun inputsDigest(): ByteArray = inputsDigestValue.copyOf()
}

/** Complete immutable hardware-produced mint authorization and encrypted credit. */
class KagemushaMintConstructionBundleV1(
    canonicalAuthorization: ByteArray,
    encryptedCredit: ByteArray,
) {
    private val canonicalAuthorizationValue = canonicalAuthorization.copyOf()
    private val encryptedCreditValue = encryptedCredit.copyOf()
    @JvmField val authorization =
        KagemushaNoritoV1.decodeMintAuthorizationShapeExact(canonicalAuthorizationValue)

    init {
        KagemushaNoritoV1.encryptedCreditAadForMintShape(authorization.statement)
        KagemushaNoritoV1.decodeEncryptedCreditEnvelopeShapeExact(encryptedCreditValue)
        require(
            authorization.statement.ciphertextDigest().contentEquals(
                KagemushaNoritoV1.ciphertextDigestShape(encryptedCreditValue),
            ),
        ) { "mint encrypted credit digest mismatch" }
    }

    fun canonicalAuthorization(): ByteArray = canonicalAuthorizationValue.copyOf()
    fun encryptedCredit(): ByteArray = encryptedCreditValue.copyOf()

    /** Build the reserve-facing request without regenerating hardware-owned ciphertext. */
    fun topUpRequest(hardwareCredential: KagemushaHardwareCredentialV1): KagemushaTopUpRequestV1 {
        val statement = authorization.statement
        val context = statement.context
        require(context.hardwareCredentialId().contentEquals(hardwareCredential.credentialId()))
        require(context.hardwareProfileId().contentEquals(hardwareCredential.hardwareProfileId()))
        require(context.suiteId().contentEquals(hardwareCredential.suiteId()))
        require(context.policyEpoch == hardwareCredential.policyEpoch)
        return KagemushaTopUpRequestV1(
            version = 1,
            operationId = context.operationId(),
            issuanceCommitment = statement.issuanceCommitment(),
            creditId = statement.creditId(),
            releaseId = context.releaseId(),
            suiteId = context.suiteId(),
            vkDigest = context.vkDigest(),
            networkId = context.networkId,
            asset = context.asset,
            assetIncarnation = context.assetIncarnation,
            scale = context.scale,
            amount = context.amount,
            liabilityPoolId = context.liabilityPoolId(),
            payer = context.payer,
            recipient = context.recipient,
            hardwareCredential = hardwareCredential,
            recipientCredentialCommitment = context.recipientCredentialCommitment(),
            creditCommitment = context.creditCommitment(),
            recipientOneTimeKey = context.recipientOneTimeKey,
            encryptedCredit = encryptedCreditValue,
            artifactManifestDigest = context.artifactManifestDigest(),
            mintAuthorization = authorization,
        )
    }
}

/** Exact operation-12 release material retained by native Core. */
class KagemushaNativeOutboxReleaseV1(
    operationId: ByteArray,
    @JvmField val context: KagemushaDeviceSenderWalletContextV1,
    inputsDigest: ByteArray,
    envelopeDigest: ByteArray,
    @JvmField val inputs: KagemushaDeviceSenderPublicInputsV1,
    canonicalEnvelope: ByteArray,
    hardwareReleaseAuthorization: ByteArray,
) {
    private val operationIdValue = authenticatedDigest(operationId, "operationId")
    private val inputsDigestValue = authenticatedDigest(inputsDigest, "inputsDigest")
    private val envelopeDigestValue = authenticatedDigest(envelopeDigest, "envelopeDigest")
    private val envelope = canonicalEnvelope.copyOf()
    private val releaseAuthorization = boundedAuthenticatedBytes(
        hardwareReleaseAuthorization,
        2 * 1024,
        "hardwareReleaseAuthorization",
    )

    init { require(envelope.isNotEmpty()) { "canonicalEnvelope is empty" } }

    fun operationId(): ByteArray = operationIdValue.copyOf()
    fun inputsDigest(): ByteArray = inputsDigestValue.copyOf()
    fun envelopeDigest(): ByteArray = envelopeDigestValue.copyOf()
    fun canonicalEnvelope(): ByteArray = envelope.copyOf()
    fun hardwareReleaseAuthorization(): ByteArray = releaseAuthorization.copyOf()
}

enum class KagemushaNativeSenderKindV1 { PAYMENT, REDEMPTION }

/**
 * Audited native Core authority required by [KagemushaAuthenticatedHardwareProviderV1].
 *
 * This interface intentionally has no stock implementation. It owns durable operation IDs,
 * signed release-catalog membership, recursive proof generation/verification, sender typestate,
 * and byte-identical terminal recovery. A service loaded factory must fail closed when exactly one
 * implementation is not installed by the qualified device/runtime package.
 */
interface KagemushaNativeCoreCoordinatorV1 {
    /** Fsync the caller's already durable action binding and echo its exact non-zero ID. */
    fun reserveOperationId(operation: Int, operationId: ByteArray, publicBinding: ByteArray): ByteArray

    /** Admit the exact signed release member and bind it to the authenticated hardware tuple. */
    fun acceptQualification(
        qualification: KagemushaHardwareQualificationV1,
        hardwarePolicyDigest: ByteArray,
    )

    /** Admit one already P-256-authenticated canonical device reply into Core's typestate. */
    fun acceptAuthenticatedDeviceReply(
        operation: Int,
        requestId: ByteArray,
        canonicalCommand: ByteArray,
        canonicalReply: ByteArray,
        qualification: KagemushaHardwareQualificationV1,
    )

    fun beginSenderTransition(
        operationId: ByteArray,
        inputs: KagemushaDeviceSenderPublicInputsV1,
        qualification: KagemushaHardwareQualificationV1,
    ): KagemushaNativeSenderPreparationV1

    /** Generate and persist the actual recursive candidate before operation 7 is constructed. */
    fun provePreparedSenderTransition(
        preparation: KagemushaNativeSenderPreparationV1,
        authenticatedPreparationReply: ByteArray,
    ): KagemushaNativeSenderCandidateV1

    /** Verify operation 7/8 and construct the final proof-bearing terminal envelope. */
    fun terminalEnvelope(
        candidate: KagemushaNativeSenderCandidateV1,
        authenticatedCommitReply: ByteArray,
    ): ByteArray

    /** Expose a terminal result only after operation 9, operation 10, and wallet snapshot operation 21 agree. */
    fun acceptInstalledTerminal(
        candidate: KagemushaNativeSenderCandidateV1,
        canonicalEnvelope: ByteArray,
        authenticatedInstallReply: ByteArray,
        authenticatedInstalledReply: ByteArray,
        authenticatedWalletSnapshotReply: ByteArray,
    ): KagemushaHardwareTerminalResultV1

    fun senderRecovery(
        kind: KagemushaNativeSenderKindV1,
        terminalId: ByteArray,
        qualification: KagemushaHardwareQualificationV1,
    ): KagemushaNativeSenderRecoveryV1?

    /** Resolve an interrupted sender transition before its terminal identity was exposed. */
    fun senderRecoveryByOperationId(
        kind: KagemushaNativeSenderKindV1,
        operationId: ByteArray,
        qualification: KagemushaHardwareQualificationV1,
    ): KagemushaNativeSenderRecoveryV1?

    fun recoverTerminalEnvelope(
        recovery: KagemushaNativeSenderRecoveryV1,
        authenticatedInstalledReply: ByteArray,
    ): ByteArray

    fun outboxRelease(
        creditId: ByteArray,
        inputs: KagemushaDeviceSenderPublicInputsV1,
        canonicalPayment: ByteArray,
        terminalReceipt: KagemushaDeviceSenderTerminalReceiptV1,
        qualification: KagemushaHardwareQualificationV1,
    ): KagemushaNativeOutboxReleaseV1
}

/**
 * Authenticated, release-pinned client for every frozen KAGEMUSHA V1 operation.
 *
 * This type performs all platform-independent command and reply framing. Successful bytes cross
 * this boundary only after the platform transport's native verifier and the injected Core
 * coordinator both accept them.
 */
class KagemushaAuthenticatedDeviceClientV1(
    private val transport: KagemushaNativeAuthenticatedDeviceTransportV1,
    internal val core: KagemushaNativeCoreCoordinatorV1,
) {
    private val lock = ReentrantLock(true)
    @Volatile private var session: Session? = null

    private class Session(
        val qualification: KagemushaHardwareQualificationV1,
        val responseKey: ByteArray,
    )

    fun qualification(): KagemushaHardwareQualificationV1 = lock.withLock {
        session?.qualification ?: qualifyLocked().qualification
    }

    internal fun invalidateQualification() = lock.withLock {
        session?.responseKey?.fill(0)
        session = null
    }

    internal fun control(
        command: KagemushaDeviceControlCommandV1,
        requestId: ByteArray,
    ): AuthenticatedCall = lock.withLock {
        val canonical = KagemushaDeviceOperationCodecV1.encodeControlCommand(command)
        KagemushaDeviceOperationCodecV1.decodeControlCommand(command.operation, requestId, canonical)
        executeLocked(command.operation, requestId, canonical, ReplyLane.CONTROL)
    }

    internal fun receiver(
        command: KagemushaDeviceReceiverCommandV1,
        requestId: ByteArray,
    ): AuthenticatedCall = lock.withLock {
        val canonical = KagemushaDeviceOperationCodecV1.encodeReceiverCommand(command)
        KagemushaDeviceOperationCodecV1.decodeReceiverCommand(command.operation, requestId, canonical)
        executeLocked(command.operation, requestId, canonical, ReplyLane.RECEIVER)
    }

    internal fun sender(command: KagemushaDeviceSenderCommandV1): AuthenticatedCall = lock.withLock {
        val requestId = command.operationId()
        val canonical = KagemushaDeviceOperationCodecV1.encodeSenderCommand(command)
        KagemushaDeviceOperationCodecV1.decodeSenderCommand(command.operation, requestId, canonical)
        executeLocked(command.operation, requestId, canonical, ReplyLane.SENDER)
    }

    internal fun mintStage(
        requestId: ByteArray,
        canonicalCommand: ByteArray,
    ): AuthenticatedCall = lock.withLock {
        KagemushaNoritoV1.decodeDeviceMintStageCommandShapeExact(canonicalCommand)
        executeLocked(16, requestId, canonicalCommand, ReplyLane.MINT)
    }

    private fun qualifyLocked(): Session {
        val operation = 1
        val requestId = core.reserveOperationId(operation, byteArrayOf(operation.toByte()))
        val command = KagemushaDeviceOperationCodecV1.encodeControlCommand(
            KagemushaDeviceControlCommandV1.ReadActiveHardwareCredential,
        )
        val response = transport.executeAndVerify(operation, requestId, command, null)
        require(response.operation == operation) { "device response substituted operation 1" }
        require(response.status == KagemushaAuthenticatedDeviceStatusV1.SUCCESS) {
            "hardware qualification failed with ${response.status}"
        }
        KagemushaP256Codec.requireRawLowSSignature(response.authenticator())
        val reply = KagemushaDeviceOperationCodecV1.decodeControlReplyAfterAuthentication(
            operation,
            response.canonicalReply(),
        )
        val decoded = decodeQualificationPayload(reply.payload())
        val capabilityPolicy = authenticatedDigest(transport.hardwarePolicyId(), "hardwarePolicyId")
        val capabilityQualification = authenticatedDigest(
            transport.qualificationReportDigest(),
            "qualificationReportDigest",
        )
        require(decoded.hardwarePolicyDigest.contentEquals(capabilityPolicy))
        require(decoded.profile.qualificationReportDigest().contentEquals(capabilityQualification))
        require(decoded.profile.hardwareProfileId().contentEquals(decoded.credential.hardwareProfileId()))
        val responseKey = KagemushaP256Codec.requireUncompressedPublicKey(
            decoded.credential.devicePublicKey.sec1Bytes(),
        )
        val qualification = KagemushaHardwareQualificationV1(
            KagemushaWireV1.WIRE_VERSION,
            decoded.profile,
            decoded.credential,
            decoded.releaseId,
            decoded.hardwarePolicyDigest,
            decoded.coreAuthorizationKeyReference,
            EnumSet.allOf(KagemushaHardwareCapabilityV1::class.java),
        )
        qualification.requireProductionReady()
        core.acceptQualification(qualification, decoded.hardwarePolicyDigest)
        core.acceptAuthenticatedDeviceReply(
            operation,
            requestId,
            command,
            reply.canonicalArchive(),
            qualification,
        )
        return Session(qualification, responseKey).also { session = it }
    }

    private fun executeLocked(
        operation: Int,
        requestId: ByteArray,
        command: ByteArray,
        lane: ReplyLane,
    ): AuthenticatedCall {
        val accepted = session ?: qualifyLocked()
        val key = accepted.responseKey.copyOf()
        val response = try {
            transport.executeAndVerify(operation, requestId, command, key)
        } finally {
            key.fill(0)
        }
        require(response.operation == operation) { "device response substituted operation $operation" }
        if (response.status != KagemushaAuthenticatedDeviceStatusV1.SUCCESS) {
            return AuthenticatedCall(operation, response.status, command, null)
        }
        KagemushaP256Codec.requireRawLowSSignature(response.authenticator())
        val archive = response.canonicalReply()
        val reply = when (lane) {
            ReplyLane.CONTROL -> KagemushaDeviceOperationCodecV1
                .decodeControlReplyAfterAuthentication(operation, archive)
            ReplyLane.RECEIVER -> KagemushaDeviceOperationCodecV1
                .decodeReceiverReplyAfterAuthentication(operation, archive)
            ReplyLane.SENDER -> KagemushaDeviceOperationCodecV1
                .decodeSenderReplyAfterAuthentication(operation, requestId, archive)
            ReplyLane.MINT -> null
        }
        if (lane == ReplyLane.MINT) {
            KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(archive)
        }
        core.acceptAuthenticatedDeviceReply(
            operation,
            requestId,
            command,
            archive,
            accepted.qualification,
        )
        return AuthenticatedCall(operation, response.status, command, reply, archive)
    }

    private enum class ReplyLane { CONTROL, RECEIVER, SENDER, MINT }
}

internal class AuthenticatedCall(
    val operation: Int,
    val status: KagemushaAuthenticatedDeviceStatusV1,
    canonicalCommand: ByteArray,
    val reply: KagemushaDeviceAuthenticatedReplyV1?,
    canonicalReply: ByteArray? = null,
) {
    private val command = canonicalCommand.copyOf()
    private val archive = canonicalReply?.copyOf() ?: reply?.canonicalArchive()
    fun canonicalCommand(): ByteArray = command.copyOf()
    fun canonicalReply(): ByteArray = checkNotNull(archive).copyOf()
}

/** High-level offline wallet provider backed only by authenticated KAGEMUSHA V1 and native Core. */
class KagemushaAuthenticatedHardwareProviderV1(
    private val client: KagemushaAuthenticatedDeviceClientV1,
) : KagemushaHardwareProviderV1 {
    private val lock = ReentrantLock(true)

    constructor(
        transport: KagemushaNativeAuthenticatedDeviceTransportV1,
        core: KagemushaNativeCoreCoordinatorV1,
    ) : this(KagemushaAuthenticatedDeviceClientV1(transport, core))

    override fun qualification(): KagemushaHardwareQualificationV1 = client.qualification()

    override fun recover(): KagemushaHardwareRecoveryV1 = lock.withLock {
        val call = control(KagemushaDeviceControlCommandV1.RecoverWalletSnapshot, freshId(21))
        val reader = payloadReader(call, 21)
        val aggregate = reader.optionVector(768)
        val journal = reader.u128Field()
        val pending = reader.u128Field()
        val retry = reader.u128Field()
        reader.finish()
        aggregate?.let(KagemushaNoritoV1::decodeAggregateStateShapeExact)
        KagemushaHardwareRecoveryV1(aggregate, journal, pending, retry)
    }

    override fun bootstrapState(): ByteArray = lock.withLock {
        val id = freshId(20)
        val call = control(KagemushaDeviceControlCommandV1.BootstrapAggregateState(id), id)
        payloadReader(call, 20).singleVector(768).also(KagemushaNoritoV1::decodeAggregateStateShapeExact)
    }

    override fun reservePaymentRequestOperationId(
        operationId: ByteArray,
        recipientAccount: ByteArray,
        amount: BigInteger,
        validityWindowMillis: Long,
    ): ByteArray = lock.withLock {
        val id = authenticatedDigest(operationId, "operationId")
        val command = KagemushaDeviceControlCommandV1.CreateSignedPaymentRequest(
            id, KagemushaAccountIdV1.fromCanonicalPayload(recipientAccount), amount, validityWindowMillis
        )
        client.core.reserveOperationId(22, id, KagemushaDeviceOperationCodecV1.encodeControlCommand(command))
    }

    override fun createPaymentRequest(
        operationId: ByteArray,
        recipientAccount: ByteArray,
        amount: BigInteger,
        validityWindowMillis: Long,
    ): ByteArray = lock.withLock {
        val recipient = KagemushaAccountIdV1.fromCanonicalPayload(recipientAccount)
        val id = authenticatedDigest(operationId, "operationId")
        val command = KagemushaDeviceControlCommandV1.CreateSignedPaymentRequest(
            id,
            recipient,
            amount,
            validityWindowMillis,
        )
        val canonical = payloadReader(control(command, id), 22).singleVector(928)
        val request = KagemushaNoritoV1.decodePaymentRequestShapeExact(canonical)
        require(request.requestId().contentEquals(id) && request.recipient == recipient)
        require(request.amount == amount)
        require(java.lang.Long.compareUnsigned(request.expiresAtMs - request.issuedAtMs, validityWindowMillis) == 0)
        require(request.releaseId().contentEquals(qualification().releaseId()))
        canonical
    }

    override fun stagePayment(
        canonicalRequest: ByteArray,
        canonicalPayment: ByteArray,
    ): KagemushaHardwarePaymentStageV1 = lock.withLock {
        val request = KagemushaNoritoV1.decodePaymentRequestShapeExact(canonicalRequest)
        val payment = KagemushaNoritoV1.decodePaymentShapeExact(canonicalPayment, request)
        val creditId = payment.output.creditId()
        val recovery = client.receiver(
            KagemushaDeviceReceiverCommandV1.RecoverStaged(creditId),
            creditId,
        )
        val disposition: KagemushaHardwareStageDispositionV1
        val receipt: KagemushaInboxReceiptV1
        if (recovery.status == KagemushaAuthenticatedDeviceStatusV1.SUCCESS) {
            disposition = KagemushaHardwareStageDispositionV1.EXACT_DUPLICATE
            receipt = parseStagedReply(recovery, 3, canonicalRequest, canonicalPayment, payment)
        } else {
            require(recovery.status == KagemushaAuthenticatedDeviceStatusV1.MISSING) {
                "operation 3 failed with ${recovery.status}"
            }
            val staged = client.receiver(
                KagemushaDeviceReceiverCommandV1.Stage(canonicalRequest, canonicalPayment),
                creditId,
            )
            disposition = KagemushaHardwareStageDispositionV1.STAGED
            receipt = parseStagedReply(
                requireSuccess(staged),
                2,
                canonicalRequest,
                canonicalPayment,
                payment,
            )
        }
        val acknowledgement = payloadReader(
            control(
                KagemushaDeviceControlCommandV1.SignReceiveAcknowledgement(
                    canonicalRequest,
                    canonicalPayment,
                    receipt,
                ),
                creditId,
            ),
            11,
        ).singleVector(256)
        KagemushaNoritoV1.decodeAcknowledgementShapeExact(
            acknowledgement,
            request,
            payment,
        ).also {
            require(it.inboxReceipt.creditId().contentEquals(creditId))
            require(it.inboxReceipt.receiptCommitment().contentEquals(receipt.receiptCommitment()))
        }
        KagemushaHardwarePaymentStageV1(
            disposition,
            creditId,
            acknowledgement,
        )
    }

    override fun stageMintCredit(
        canonicalAuthorization: ByteArray,
        canonicalMintCredit: ByteArray,
    ): KagemushaHardwareMintStageV1 = lock.withLock {
        val authorization = KagemushaNoritoV1.decodeMintAuthorizationShapeExact(canonicalAuthorization)
        val commandModel = KagemushaDeviceMintStageCommandV1(
            KagemushaWireV1.WIRE_VERSION,
            canonicalAuthorization,
            canonicalMintCredit,
        )
        val canonicalCommand = KagemushaNoritoV1.encodeDeviceMintStageCommandShape(commandModel)
        val id = authorization.statement.context.operationId()
        val call = requireSuccess(client.mintStage(id, canonicalCommand))
        val result = KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(
            call.canonicalReply(),
            commandModel,
        )
        val disposition = when (result.disposition) {
            KagemushaDeviceMintStageResultV1.STAGED -> KagemushaHardwareStageDispositionV1.STAGED
            else -> KagemushaHardwareStageDispositionV1.EXACT_DUPLICATE
        }
        KagemushaHardwareMintStageV1(disposition, result.creditId())
    }

    override fun selectPendingCredit(
        watermark: KagemushaPendingCreditWatermarkV1?,
        target: KagemushaPendingCreditTargetV1,
    ): KagemushaPendingCreditSelectionV1 = lock.withLock {
        val reader = payloadReader(
            control(
                KagemushaDeviceControlCommandV1.ReadPendingCreditWatermark(watermark, target),
                freshId(18),
            ),
            18,
        )
        val returnedWatermark = decodePendingCreditWatermarkReply(reader.field())
        val next = reader.optionPendingCreditSelector()
        reader.finish()
        watermark?.let {
            require(it.sameAs(returnedWatermark)) { "pending-credit watermark changed within one pass" }
        }
        KagemushaPendingCreditSelectionV1(returnedWatermark, next)
    }

    override fun journalRevision(): BigInteger = recover().journalRevision

    override fun foldPendingCredit(
        selector: KagemushaPendingCreditSelectorV1,
    ): KagemushaHardwareReceiveFoldV1 = lock.withLock {
        val credit = authenticatedDigest(selector.creditId(), "creditId")
        val binding = byteArrayOf(selector.kind.ordinal.toByte()) + credit
        val id = client.core.reserveOperationId(17, binding)
        val call = client.control(
            KagemushaDeviceControlCommandV1.FoldReceiveCredit(id, selector),
            id,
        )
        val reader = payloadReader(requireSuccess(call), 17)
        require(reader.pendingCreditKindField() == selector.kind)
        require(reader.digestField().contentEquals(credit))
        val aggregate = reader.vectorField(768)
        reader.finish()
        KagemushaNoritoV1.decodeAggregateStateShapeExact(aggregate)
        KagemushaHardwareReceiveFoldV1(aggregate, selector)
    }

    override fun reservePaymentOperationId(operationId: ByteArray, canonicalRequest: ByteArray): ByteArray = lock.withLock {
        KagemushaNoritoV1.decodePaymentRequestShapeExact(canonicalRequest)
        client.core.reserveOperationId(5, authenticatedDigest(operationId, "operationId"), canonicalRequest)
    }

    override fun commitPayment(
        operationId: ByteArray,
        canonicalRequest: ByteArray,
    ): KagemushaHardwareTerminalResultV1 = lock.withLock {
        KagemushaNoritoV1.decodePaymentRequestShapeExact(canonicalRequest)
        commitSender(operationId, KagemushaDeviceSenderPublicInputsV1.SendSplit(canonicalRequest))
    }

    override fun recoverPayment(creditId: ByteArray): ByteArray? = lock.withLock {
        recoverTerminal(KagemushaNativeSenderKindV1.PAYMENT, creditId)
    }

    override fun recoverPaymentByOperationId(
        operationId: ByteArray,
        canonicalRequest: ByteArray,
    ): ByteArray? = lock.withLock {
        val request = KagemushaNoritoV1.decodePaymentRequestShapeExact(canonicalRequest)
        val recovery = client.core.senderRecoveryByOperationId(
            KagemushaNativeSenderKindV1.PAYMENT,
            authenticatedDigest(operationId, "operationId"),
            qualification(),
        ) ?: return@withLock null
        val envelope = recoverTerminal(recovery)
        val payment = KagemushaNoritoV1.decodePaymentShapeExact(envelope, request)
        require(payment.output.creditId().contentEquals(recovery.terminalId())) {
            "recovered payment credit ID mismatch"
        }
        envelope
    }

    override fun recordAcknowledgement(
        creditId: ByteArray,
        canonicalRequest: ByteArray,
        canonicalPayment: ByteArray,
        canonicalAcknowledgement: ByteArray,
    ) = lock.withLock {
        val request = KagemushaNoritoV1.decodePaymentRequestShapeExact(canonicalRequest)
        val payment = KagemushaNoritoV1.decodePaymentShapeExact(canonicalPayment, request)
        require(payment.output.creditId().contentEquals(creditId))
        KagemushaNoritoV1.decodeAcknowledgementShapeExact(
            canonicalAcknowledgement,
            request,
            payment,
        )
        val inputs = KagemushaDeviceSenderPublicInputsV1.SendSplit(canonicalRequest)
        val terminalReceipt = KagemushaDeviceSenderTerminalReceiptV1.PaymentAcknowledgement(
            canonicalAcknowledgement,
        )
        val qualified = qualification()
        val release = client.core.outboxRelease(
            creditId,
            inputs,
            canonicalPayment,
            terminalReceipt,
            qualified,
        )
        require(
            release.context.devicePolicyBinding.hardwarePolicyId()
                .contentEquals(qualified.hardwarePolicyDigest()),
        ) { "outbox release hardware-policy scope mismatch" }
        require(
            release.context.coreAuthorizationKeyReference()
                .contentEquals(qualified.coreAuthorizationKeyReference()),
        ) { "outbox release Core authorization key mismatch" }
        val command = KagemushaDeviceSenderCommandV1(
            operation = 12,
            operationId = release.operationId(),
            context = release.context,
            body = KagemushaDeviceSenderCommandBodyV1.Release(
                release.inputsDigest(),
                release.envelopeDigest(),
                release.inputs,
                release.canonicalEnvelope(),
                terminalReceipt,
                release.hardwareReleaseAuthorization(),
            ),
        )
        requireSuccess(client.sender(command))
        Unit
    }

    override fun reserveRedemptionOperationId(
        operationId: ByteArray,
        amount: BigInteger,
        beneficiaryAccount: ByteArray,
    ): ByteArray = lock.withLock {
        client.core.reserveOperationId(5, authenticatedDigest(operationId, "operationId"), unsigned128(amount) + beneficiaryAccount)
    }

    override fun commitRedemption(
        operationId: ByteArray,
        amount: BigInteger,
        beneficiaryAccount: ByteArray,
    ): KagemushaHardwareTerminalResultV1 = lock.withLock {
        commitSender(
            operationId,
            KagemushaDeviceSenderPublicInputsV1.RedeemSplit(amount, beneficiaryAccount),
        )
    }

    override fun recoverRedemption(redemptionId: ByteArray): ByteArray? = lock.withLock {
        recoverTerminal(KagemushaNativeSenderKindV1.REDEMPTION, redemptionId)
    }

    override fun recoverRedemptionByOperationId(operationId: ByteArray): ByteArray? = lock.withLock {
        val recovery = client.core.senderRecoveryByOperationId(
            KagemushaNativeSenderKindV1.REDEMPTION,
            authenticatedDigest(operationId, "operationId"),
            qualification(),
        ) ?: return@withLock null
        val envelope = recoverTerminal(recovery)
        val voucher = KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(envelope)
        require(voucher.statement.redemptionId().contentEquals(recovery.terminalId())) {
            "recovered redemption ID mismatch"
        }
        envelope
    }

    override fun rotateHardwareEpoch(): ByteArray = lock.withLock {
        val id = freshId(19)
        val aggregate = payloadReader(
            control(KagemushaDeviceControlCommandV1.RotateHardwareEpoch(id), id),
            19,
        ).singleVector(768)
        KagemushaNoritoV1.decodeAggregateStateShapeExact(aggregate)
        client.invalidateQualification()
        client.qualification()
        aggregate
    }

    /** Operation 13 exposes qualified trusted-time evidence. */
    fun readTrustedTimeOrLease(): ByteArray = lock.withLock {
        requireSuccess(
            client.control(KagemushaDeviceControlCommandV1.ReadTrustedTimeOrLease, freshId(13)),
        ).canonicalReply()
    }

    override fun reserveMintOperationId(
        operationId: ByteArray,
        amount: BigInteger,
        payerAccount: ByteArray,
        recipientAccount: ByteArray,
    ): ByteArray = lock.withLock {
        val binding = unsigned128(amount) + payerAccount + recipientAccount
        client.core.reserveOperationId(14, authenticatedDigest(operationId, "operationId"), binding)
    }

    /** Operation 14 prepares one proof-bearing authorization plus its exact encrypted credit. */
    override fun prepareMintConstructionBundle(
        operationId: ByteArray,
        amount: BigInteger,
        payerAccount: ByteArray,
        recipientAccount: ByteArray,
    ): KagemushaMintConstructionBundleV1 = lock.withLock {
        val id = authenticatedDigest(operationId, "operationId")
        val command = KagemushaDeviceControlCommandV1.PrepareMintAuthorization(
            id,
            amount,
            payerAccount,
            recipientAccount,
        )
        val reader = payloadReader(control(command, id), 14)
        val authorization = reader.vectorField(7_936)
        val encryptedCredit = reader.vectorField(KagemushaWireV1.MAXIMUM_ENCRYPTED_CREDIT_BYTES)
        reader.finish()
        val bundle = KagemushaMintConstructionBundleV1(authorization, encryptedCredit)
        require(bundle.authorization.statement.context.operationId().contentEquals(id))
        bundle
    }

    /** Recover the complete operation-14 bundle byte-identically through operation 15. */
    override fun recoverMintConstructionBundle(
        operationId: ByteArray,
    ): KagemushaMintConstructionBundleV1? = lock.withLock {
        val id = authenticatedDigest(operationId, "operationId")
        val call = client.control(
            KagemushaDeviceControlCommandV1.RecoverMintAuthorization(id),
            id,
        )
        if (call.status == KagemushaAuthenticatedDeviceStatusV1.MISSING) return@withLock null
        val reader = payloadReader(requireSuccess(call), 15)
        val authorization = reader.vectorField(7_936)
        val encryptedCredit = reader.vectorField(KagemushaWireV1.MAXIMUM_ENCRYPTED_CREDIT_BYTES)
        reader.finish()
        val bundle = KagemushaMintConstructionBundleV1(authorization, encryptedCredit)
        require(bundle.authorization.statement.context.operationId().contentEquals(id))
        bundle
    }

    private fun commitSender(
        operationId: ByteArray,
        inputs: KagemushaDeviceSenderPublicInputsV1,
    ): KagemushaHardwareTerminalResultV1 {
        val operationId = authenticatedDigest(operationId, "operationId")
        val qualified = qualification()
        val preparation = client.core.beginSenderTransition(operationId, inputs, qualified)
        require(preparation.operationId().contentEquals(operationId)) {
            "native Core substituted sender operation ID"
        }
        require(
            preparation.context.devicePolicyBinding.hardwarePolicyId()
                .contentEquals(qualified.hardwarePolicyDigest()),
        ) { "sender preparation hardware-policy scope mismatch" }
        require(
            preparation.context.coreAuthorizationKeyReference()
                .contentEquals(qualified.coreAuthorizationKeyReference()),
        ) { "sender preparation Core authorization key mismatch" }
        val preparedCommand = KagemushaDeviceSenderCommandV1(
            operation = 5,
            operationId = operationId,
            context = preparation.context,
            body = KagemushaDeviceSenderCommandBodyV1.Prepare(inputs),
        )
        var prepared = client.sender(preparedCommand)
        if (prepared.status == KagemushaAuthenticatedDeviceStatusV1.RECOVERY_REQUIRED ||
            prepared.status == KagemushaAuthenticatedDeviceStatusV1.STALE_OR_CONCURRENT
        ) {
            prepared = client.sender(
                KagemushaDeviceSenderCommandV1(
                    operation = 6,
                    operationId = operationId,
                    context = preparation.context,
                    body = KagemushaDeviceSenderCommandBodyV1.RecoverPrepared(
                        preparation.inputsDigest(),
                    ),
                ),
            )
        }
        requireSuccess(prepared)
        val candidate = client.core.provePreparedSenderTransition(
            preparation,
            prepared.canonicalReply(),
        )
        var committed = client.sender(
            KagemushaDeviceSenderCommandV1(
                operation = 7,
                operationId = operationId,
                context = preparation.context,
                body = KagemushaDeviceSenderCommandBodyV1.Commit(
                    candidate.selector,
                    candidate.candidateDigest(),
                    candidate.hardwareCommitAuthorization(),
                ),
            ),
        )
        if (committed.status == KagemushaAuthenticatedDeviceStatusV1.RECOVERY_REQUIRED ||
            committed.status == KagemushaAuthenticatedDeviceStatusV1.STALE_OR_CONCURRENT
        ) {
            committed = client.sender(
                KagemushaDeviceSenderCommandV1(
                    operation = 8,
                    operationId = operationId,
                    context = preparation.context,
                    body = KagemushaDeviceSenderCommandBodyV1.RecoverTerminal(
                        preparation.inputsDigest(),
                    ),
                ),
            )
        }
        requireSuccess(committed)
        val envelope = client.core.terminalEnvelope(candidate, committed.canonicalReply())
        require(envelope.isNotEmpty()) { "native Core returned an empty terminal envelope" }
        val install = requireSuccess(
            client.sender(
                KagemushaDeviceSenderCommandV1(
                    operation = 9,
                    operationId = operationId,
                    context = preparation.context,
                    body = KagemushaDeviceSenderCommandBodyV1.Install(
                        candidate.selector,
                        candidate.candidateDigest(),
                        inputs,
                        envelope,
                    ),
                ),
            ),
        )
        val installed = requireSuccess(
            client.sender(
                KagemushaDeviceSenderCommandV1(
                    operation = 10,
                    operationId = operationId,
                    context = preparation.context,
                    body = KagemushaDeviceSenderCommandBodyV1.RecoverInstalled(
                        KagemushaDeviceSenderRecoverySelectorV1.Lookup(preparation.inputsDigest()),
                    ),
                ),
            ),
        )
        val snapshot = control(KagemushaDeviceControlCommandV1.RecoverWalletSnapshot, freshId(21))
        return client.core.acceptInstalledTerminal(
            candidate,
            envelope,
            install.canonicalReply(),
            installed.canonicalReply(),
            snapshot.canonicalReply(),
        )
    }

    private fun recoverTerminal(kind: KagemushaNativeSenderKindV1, id: ByteArray): ByteArray? {
        val expected = authenticatedDigest(id, "terminalId")
        val recovery = client.core.senderRecovery(kind, expected, qualification()) ?: return null
        require(recovery.terminalId().contentEquals(expected)) { "native Core substituted terminal ID" }
        return recoverTerminal(recovery)
    }

    private fun recoverTerminal(recovery: KagemushaNativeSenderRecoveryV1): ByteArray {
        val call = client.sender(
            KagemushaDeviceSenderCommandV1(
                operation = 10,
                operationId = recovery.operationId(),
                context = recovery.context,
                body = KagemushaDeviceSenderCommandBodyV1.RecoverInstalled(
                    KagemushaDeviceSenderRecoverySelectorV1.Lookup(recovery.inputsDigest()),
                ),
            ),
        )
        require(call.status != KagemushaAuthenticatedDeviceStatusV1.MISSING) {
            "native Core indexed a missing terminal"
        }
        requireSuccess(call)
        return client.core.recoverTerminalEnvelope(recovery, call.canonicalReply()).also {
            require(it.isNotEmpty()) { "native Core recovered an empty terminal envelope" }
        }
    }

    private fun control(
        command: KagemushaDeviceControlCommandV1,
        requestId: ByteArray,
    ): AuthenticatedCall = requireSuccess(client.control(command, requestId))

    private fun freshId(operation: Int): ByteArray =
        client.core.reserveOperationId(operation, byteArrayOf(operation.toByte()))
}

private fun requireSuccess(call: AuthenticatedCall): AuthenticatedCall {
    require(call.status == KagemushaAuthenticatedDeviceStatusV1.SUCCESS) {
        "device operation ${call.operation} failed with ${call.status}"
    }
    return call
}

private fun payloadReader(call: AuthenticatedCall, operation: Int): AuthenticatedReplyReader {
    requireSuccess(call)
    val reader = AuthenticatedReplyReader(checkNotNull(call.reply).payload())
    require(reader.u16Field() == KagemushaWireV1.WIRE_VERSION && reader.u8Field() == operation) {
        "authenticated reply binding mismatch"
    }
    return reader
}

private fun parseStagedReply(
    call: AuthenticatedCall,
    operation: Int,
    requestBytes: ByteArray,
    paymentBytes: ByteArray,
    payment: KagemushaPaymentV1,
): KagemushaInboxReceiptV1 {
    val top = payloadReader(call, operation)
    require(top.u128Field().signum() != 0) { "staged inbox revision must be nonzero" }
    val record = AuthenticatedReplyReader(top.field())
    top.finish()
    require(record.vectorField(928).contentEquals(requestBytes))
    require(record.vectorField(7_552).contentEquals(paymentBytes))
    record.vectorField(1_024) // Authenticated transport metadata; empty is valid.
    val receipt = decodeInboxReceipt(record.field())
    record.finish()
    require(receipt.creditId().contentEquals(payment.output.creditId()))
    return receipt
}

private fun stagedRecordCreditId(payload: ByteArray): ByteArray {
    val record = AuthenticatedReplyReader(payload)
    record.vectorField(928)
    record.vectorField(7_552)
    record.vectorField(1_024)
    val creditId = decodeInboxReceipt(record.field()).creditId()
    record.finish()
    return creditId
}

private fun decodeInboxReceipt(payload: ByteArray): KagemushaInboxReceiptV1 {
    val reader = AuthenticatedReplyReader(payload)
    val receipt = KagemushaInboxReceiptV1(
        reader.u16Field(),
        reader.digestField(),
        reader.digestField(),
    )
    reader.finish()
    require(receipt.version == KagemushaWireV1.WIRE_VERSION)
    return receipt
}

private fun decodePendingCreditWatermarkReply(
    payload: ByteArray,
): KagemushaPendingCreditWatermarkV1 {
    val reader = AuthenticatedReplyReader(payload)
    return KagemushaPendingCreditWatermarkV1(
        reader.u128Field(), reader.digestField(), reader.u128Field(),
    ).also { reader.finish() }
}

private data class DecodedQualification(
    val releaseId: ByteArray,
    val hardwarePolicyDigest: ByteArray,
    val coreAuthorizationKeyReference: ByteArray,
    val profile: KagemushaHardwareProfileV1,
    val credential: KagemushaHardwareCredentialV1,
)

private fun decodeQualificationPayload(payload: ByteArray): DecodedQualification {
    val reader = AuthenticatedReplyReader(payload)
    require(reader.u16Field() == 1 && reader.u8Field() == 1)
    val release = reader.digestField()
    val policy = reader.digestField()
    val coreAuthorizationKeyReference = reader.digestField()
    val profilePayload = reader.field(512)
    val credentialPayload = reader.field(768)
    reader.finish()
    return DecodedQualification(
        release,
        policy,
        coreAuthorizationKeyReference,
        KagemushaNoritoV1.decodeHardwareProfileShapeExact(
            nestedModelArchive(HARDWARE_PROFILE_SCHEMA, profilePayload),
        ),
        KagemushaNoritoV1.decodeHardwareCredentialShapeExact(
            nestedModelArchive(HARDWARE_CREDENTIAL_SCHEMA, credentialPayload),
        ),
    )
}

private fun nestedModelArchive(schema: String, payload: ByteArray): ByteArray =
    NoritoCodec.encode(payload, schema, RAW_NORITO_BYTES)

private val RAW_NORITO_BYTES = object : TypeAdapter<ByteArray> {
    override fun encode(encoder: NoritoEncoder, value: ByteArray) = encoder.writeBytes(value)
    override fun decode(decoder: NoritoDecoder): ByteArray = decoder.readBytes(decoder.remaining())
}

private const val HARDWARE_PROFILE_SCHEMA =
    "iroha_data_model::kagemusha::kagemusha_v1::KagemushaHardwareProfileV1"
private const val HARDWARE_CREDENTIAL_SCHEMA =
    "iroha_data_model::kagemusha::kagemusha_v1::KagemushaHardwareCredentialV1"

private class AuthenticatedReplyReader(private val bytes: ByteArray) {
    private var offset = 0

    fun field(maximum: Int = bytes.size): ByteArray {
        val length = compactLength()
        require(length <= maximum && length <= bytes.size - offset) { "reply field is oversized" }
        return bytes.copyOfRange(offset, offset + length).also { offset += length }
    }

    fun digestField(): ByteArray = field(32).also {
        require(it.size == 32 && it.any { byte -> byte != 0.toByte() }) { "zero or malformed digest" }
    }

    fun u8Field(): Int = field(1).also { require(it.size == 1) }[0].toInt() and 0xff

    fun u16Field(): Int = littleEndian(field(2), 2).toInt()

    fun u128Field(): BigInteger = BigInteger(1, field(16).reversedArray())

    fun vectorField(maximum: Int): ByteArray {
        val nested = AuthenticatedReplyReader(field(maximum + 8))
        val countBytes = nested.raw(8)
        val count = ByteBuffer.wrap(countBytes).order(ByteOrder.LITTLE_ENDIAN).long
        require(count >= 0 && count <= maximum.toLong()) { "reply vector is oversized" }
        return nested.raw(count.toInt()).also { nested.finish() }
    }

    fun optionVector(maximum: Int): ByteArray? {
        val nested = AuthenticatedReplyReader(field(maximum + 10))
        val tag = nested.raw(1)[0].toInt() and 0xff
        val value = when (tag) {
            0 -> null
            1 -> nested.vectorField(maximum)
            else -> throw IllegalArgumentException("invalid reply option tag")
        }
        nested.finish()
        return value
    }

    fun optionDigest(): ByteArray? {
        val nested = AuthenticatedReplyReader(field(34))
        val tag = nested.raw(1)[0].toInt() and 0xff
        val value = when (tag) {
            0 -> null
            1 -> {
                val item = AuthenticatedReplyReader(nested.field(32))
                item.digestRaw().also { item.finish() }
            }
            else -> throw IllegalArgumentException("invalid reply option tag")
        }
        nested.finish()
        return value
    }

    fun pendingCreditKindField(): KagemushaPendingCreditKindV1 {
        val nested = AuthenticatedReplyReader(field(4))
        val ordinal = ByteBuffer.wrap(nested.raw(4)).order(ByteOrder.LITTLE_ENDIAN).int
        nested.finish()
        require(ordinal in KagemushaPendingCreditKindV1.entries.indices) {
            "invalid pending-credit kind"
        }
        return KagemushaPendingCreditKindV1.entries[ordinal]
    }

    fun optionPendingCreditSelector(): KagemushaPendingCreditSelectorV1? {
        val nested = AuthenticatedReplyReader(field(80))
        val value = when (nested.raw(1)[0].toInt() and 0xff) {
            0 -> null
            1 -> {
                val item = AuthenticatedReplyReader(nested.field(48))
                KagemushaPendingCreditSelectorV1(
                    item.pendingCreditKindField(), item.digestField(),
                ).also { item.finish() }
            }
            else -> throw IllegalArgumentException("invalid pending-credit option tag")
        }
        nested.finish()
        return value
    }

    fun itemVectorFields(maximumEntries: Int): List<ByteArray> {
        val nested = AuthenticatedReplyReader(field())
        val count = ByteBuffer.wrap(nested.raw(8)).order(ByteOrder.LITTLE_ENDIAN).long
        require(count in 0..maximumEntries.toLong()) { "reply vector has too many entries" }
        val values = ArrayList<ByteArray>(count.toInt())
        repeat(count.toInt()) { values.add(nested.field()) }
        nested.finish()
        return values
    }

    private fun digestRaw(): ByteArray = raw(32).also {
        require(it.any { byte -> byte != 0.toByte() }) { "zero digest" }
    }

    fun singleVector(maximum: Int): ByteArray = vectorField(maximum).also { finish() }

    fun raw(count: Int): ByteArray {
        require(count >= 0 && count <= bytes.size - offset) { "truncated reply" }
        return bytes.copyOfRange(offset, offset + count).also { offset += count }
    }

    fun finish() = require(offset == bytes.size) { "authenticated reply has trailing bytes" }

    private fun compactLength(): Int {
        var value = 0L
        var shift = 0
        var count = 0
        while (true) {
            require(offset < bytes.size && count < 10) { "invalid compact field length" }
            val byte = bytes[offset++].toInt() and 0xff
            count++
            val payload = byte and 0x7f
            require(shift < 63 || payload == 0) { "compact field length overflow" }
            value = value or (payload.toLong() shl shift)
            if (byte and 0x80 == 0) {
                require(count == 1 || payload != 0) { "non-minimal compact field length" }
                require(value <= Int.MAX_VALUE.toLong()) { "compact field length is oversized" }
                return value.toInt()
            }
            shift += 7
        }
    }
}

private fun authenticatedDigest(value: ByteArray, field: String): ByteArray = value.copyOf().also {
    require(it.size == 32 && it.any { byte -> byte != 0.toByte() }) { "$field must be a non-zero digest" }
}

private fun boundedAuthenticatedBytes(value: ByteArray, maximum: Int, field: String): ByteArray =
    value.copyOf().also {
        require(it.isNotEmpty() && it.size <= maximum) { "$field must be non-empty and at most $maximum bytes" }
    }

private fun littleEndian(bytes: ByteArray, expected: Int): Long {
    require(bytes.size == expected)
    var value = 0L
    for (index in bytes.indices) value = value or ((bytes[index].toLong() and 0xff) shl (index * 8))
    return value
}

private fun unsigned128(value: BigInteger): ByteArray {
    require(value.signum() >= 0 && value.bitLength() <= 128)
    val bigEndian = value.toByteArray().let { if (it.size == 17 && it[0] == 0.toByte()) it.copyOfRange(1, 17) else it }
    require(bigEndian.size <= 16)
    return ByteArray(16).also { target ->
        bigEndian.reversedArray().copyInto(target)
    }
}
