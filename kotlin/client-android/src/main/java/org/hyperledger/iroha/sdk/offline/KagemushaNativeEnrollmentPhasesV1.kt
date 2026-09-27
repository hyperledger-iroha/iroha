// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address

/**
 * Typed method-12 transport on the already-open process-owned native coordinator.
 *
 * A validated frame is not issuer, app, device, or monetary authority. Only the qualified
 * native backend may authenticate the signed preparation, complete qualification and issuer
 * certificate. Stock JNI has no such backend; its open fails closed. Phase 7 reads the original
 * selection only while its native owner and deadline remain live; it never selects again.
 * TODO: qualify an online replacement protocol for process-death recovery of an unexposed
 * phase-1 result. A new process cannot restore the original native deadline or kernel state.
 */
class KagemushaNativeEnrollmentPhasesV1 internal constructor(
    private val bridge: KagemushaCoreCoordinatorBridgeV1,
) {
    /** Untrusted original ticket and independently pinned native selection. */
    class Selection internal constructor(
        val accountI105: String,
        ticket: ByteArray,
        clientNonce: ByteArray,
        releaseId: ByteArray,
        profileId: ByteArray,
        laneId: ByteArray,
        ownerScope: ByteArray,
        nativeDeadlineContinuousMS: ByteArray,
    ) {
        private val ticketValue = ticket.copyOf()
        private val nonceValue = clientNonce.copyOf()
        private val releaseValue = releaseId.copyOf()
        private val profileValue = profileId.copyOf()
        private val laneValue = laneId.copyOf()
        private val ownerScopeValue = ownerScope.copyOf()
        private val nativeDeadlineValue = ByteBuffer.wrap(nativeDeadlineContinuousMS)
            .order(ByteOrder.LITTLE_ENDIAN).long.also {
                require(it > 0) { "Invalid native continuous enrollment deadline" }
            }

        fun ticket(): ByteArray = ticketValue.copyOf()
        fun clientNonce(): ByteArray = nonceValue.copyOf()
        fun releaseId(): ByteArray = releaseValue.copyOf()
        fun profileId(): ByteArray = profileValue.copyOf()
        fun laneId(): ByteArray = laneValue.copyOf()
        /** Original native attempt cache scope, not the final enrollment ID. */
        fun ownerScope(): ByteArray = ownerScopeValue.copyOf()
        /** Suspend-inclusive boot-clock expiry; native phase checks remain authoritative. */
        fun nativeDeadlineContinuousMS(): Long = nativeDeadlineValue
    }

    /** Native-checked signing projections for one complete qualified issuer challenge. */
    class AcceptedChallenge internal constructor(
        val selection: Selection,
        challengeId: ByteArray,
        signingMessage: ByteArray,
        canonicalCommand: ByteArray,
    ) {
        private val challengeValue = challengeId.copyOf()
        private val signingValue = signingMessage.copyOf()
        private val commandValue = canonicalCommand.copyOf()
        fun challengeId(): ByteArray = challengeValue.copyOf()
        fun signingMessage(): ByteArray = signingValue.copyOf()
        fun canonicalCommand(): ByteArray = commandValue.copyOf()
    }

    /** The sole native-checked proof, retained for exact finish retries. */
    class Proof internal constructor(
        val accepted: AcceptedChallenge,
        canonicalProof: ByteArray,
    ) {
        private val value = canonicalProof.copyOf()
        fun canonicalProof(): ByteArray = value.copyOf()
    }

    private var beginInvoked = false
    private var begunAccountI105: String? = null
    private var selected: Selection? = null
    private var challengeRequest: List<ByteArray>? = null
    private var accepted: AcceptedChallenge? = null
    private var proofRequest: List<ByteArray>? = null
    private var proof: Proof? = null
    private var finishRequest: List<ByteArray>? = null
    private var enrollmentId: ByteArray? = null
    private var cancelled = false
    private var cancelledSelection: Selection? = null
    private var poisoned = false

    /** Revoke cached phase authority when the parent owner closes or changes account. */
    @Synchronized
    internal fun revokeLocal() {
        cancelled = true
        poisoned = true
        selected = null
        begunAccountI105 = null
        accepted = null
        proof = null
        challengeRequest = null
        proofRequest = null
        finishRequest = null
        enrollmentId = null
        cancelledSelection = null
    }

    /** Invoke phase 1 exactly once. An uncertain return freezes selection in this process. */
    @Synchronized
    fun begin(accountI105: String): Selection {
        check(!beginInvoked && !cancelled) { "Native enrollment selection was already invoked" }
        val canonical = requireCanonicalI105Address(accountI105, "enrollment account")
        val account = canonical.toByteArray(Charsets.UTF_8)
        require(account.size <= 512) { "Enrollment account exceeds native frame bound" }
        beginInvoked = true
        begunAccountI105 = canonical
        val fields = bridge.invoke(METHOD, listOf(u32(1), account))
        return Selection(canonical, fields[0], fields[1], fields[2], fields[3], fields[4],
            fields[5], fields[6])
            .also { selected = it }
    }

    /** Recheck the exact live native selection, including after a lost phase-1 response. */
    @Synchronized
    fun recoverExactSelection(accountI105: String): Selection? {
        val canonical = requireCanonicalI105Address(accountI105, "enrollment account")
        if (!beginInvoked || cancelled || poisoned || begunAccountI105 != canonical) return null
        val fields = bridge.invoke(METHOD, listOf(u32(7), canonical.toByteArray(Charsets.UTF_8)))
        val candidate = Selection(canonical, fields[0], fields[1], fields[2], fields[3], fields[4],
            fields[5], fields[6])
        selected?.let { original ->
            if (!sameSelection(original, candidate)) {
                poisoned = true
                error("Native enrollment selection changed on exact recovery")
            }
            return original
        }
        return candidate.also { selected = it }
    }

    /**
     * Consume phase 2 with the original signed preparation, app certificate, complete device
     * qualification and canonical issuer challenge. Exact retry reuses the same request bytes.
     * This method does not create or certify a device qualification.
     */
    @Synchronized
    fun acceptQualifiedChallenge(
        selection: Selection,
        signedPreparation: ByteArray,
        appCertificate: ByteArray,
        qualification: ByteArray,
        canonicalChallenge: ByteArray,
        challengeId: ByteArray,
        accountSigningMessage: ByteArray,
        deviceRequestId: ByteArray,
        canonicalDeviceCommand: ByteArray,
        expiresAtMs: Long,
    ): AcceptedChallenge {
        requireSelection(selection)
        require(challengeId.size == 32 && challengeId.any { it != 0.toByte() } &&
            challengeId.contentEquals(deviceRequestId)) { "Issuer challenge identity changed" }
        require(accountSigningMessage.size == 32 && accountSigningMessage.any { it != 0.toByte() }) {
            "Issuer account signing message is invalid"
        }
        requireSignedPreparation(selection, signedPreparation, expiresAtMs)
        val fields = listOf(u32(2), selection.ticket(), signedPreparation.copyOf(),
            appCertificate.copyOf(), qualification.copyOf(), canonicalChallenge.copyOf(),
            challengeId.copyOf(), accountSigningMessage.copyOf(), deviceRequestId.copyOf(),
            canonicalDeviceCommand.copyOf(), u64(expiresAtMs))
        sameOrRemember(challengeRequest, fields, "Issuer challenge")
        if (challengeRequest == null) challengeRequest = copyFields(fields)
        val response = bridge.invoke(METHOD, fields)
        val result = AcceptedChallenge(selection, response[2], response[1], response[3])
        accepted?.let {
            if (!it.challengeId().contentEquals(result.challengeId()) ||
                !it.signingMessage().contentEquals(result.signingMessage()) ||
                !it.canonicalCommand().contentEquals(result.canonicalCommand())) {
                poisoned = true
                error("Native accepted challenge changed on exact retry")
            }
        }
        return (accepted ?: result).also { accepted = it }
    }

    /** Phase 3 submits one account signature and complete authenticated device frame. */
    @Synchronized
    fun prepareProof(accepted: AcceptedChallenge, accountSignature: ByteArray,
        completeDeviceResponse: ByteArray): Proof {
        requireAccepted(accepted)
        val fields = listOf(u32(3), accepted.selection.ticket(), accountSignature.copyOf(),
            completeDeviceResponse.copyOf())
        sameOrRemember(proofRequest, fields, "Possession proof")
        if (proofRequest == null) proofRequest = copyFields(fields)
        return checkedProof(accepted, bridge.invoke(METHOD, fields))
    }

    /** Phase 4 reads only the original native proof after an uncertain phase-3 dispatch. */
    @Synchronized
    fun recoverExactProof(accepted: AcceptedChallenge): Proof {
        requireAccepted(accepted)
        check(proofRequest != null) { "No original possession proof was dispatched" }
        return checkedProof(accepted, bridge.invoke(METHOD, listOf(u32(4), accepted.selection.ticket())))
    }

    /** Phase 5 authenticates one issuer certificate against the retained native proof. */
    @Synchronized
    fun complete(proof: Proof, canonicalCertificate: ByteArray): ByteArray {
        check(this.proof === proof && !cancelled && !poisoned) {
            "Enrollment proof is not the live retained native proof"
        }
        val fields = listOf(u32(5), proof.accepted.selection.ticket(), canonicalCertificate.copyOf())
        sameOrRemember(finishRequest, fields, "Issuer completion")
        if (finishRequest == null) finishRequest = copyFields(fields)
        val response = bridge.invoke(METHOD, fields)
        enrollmentId?.let { if (!it.contentEquals(response[1])) {
            poisoned = true
            error("Native enrollment identity changed")
        } }
        return response[1].copyOf().also { enrollmentId = it.copyOf() }
    }

    /** Revoke the original ticket, including after local response poisoning; only exact retry follows. */
    @Synchronized
    fun cancel(selection: Selection) {
        check(selected === selection && (!cancelled || cancelledSelection === selection)) {
            "Enrollment cancellation is not the original native ticket"
        }
        if (!cancelled) {
            cancelled = true
            cancelledSelection = selection
        }
        bridge.invoke(METHOD, listOf(u32(6), selection.ticket()))
    }

    private fun checkedProof(accepted: AcceptedChallenge, response: List<ByteArray>): Proof {
        if (!response[1].contentEquals(accepted.challengeId())) {
            poisoned = true
            error("Native proof challenge changed")
        }
        val candidate = Proof(accepted, response[2])
        proof?.let { if (!it.canonicalProof().contentEquals(candidate.canonicalProof())) {
            poisoned = true
            error("Native proof changed on exact recovery")
        } }
        return (proof ?: candidate).also { proof = it }
    }

    private fun requireSelection(value: Selection) {
        check(selected === value && !cancelled && !poisoned) {
            "Enrollment selection is not the original native ticket"
        }
    }

    private fun requireAccepted(value: AcceptedChallenge) {
        requireSelection(value.selection)
        check(accepted === value) { "Issuer challenge is not the original native selection" }
    }

    private fun requireSignedPreparation(selection: Selection, bytes: ByteArray, expiresAtMs: Long) {
        require(bytes.size == 273 && bytes[0] == 1.toByte() && expiresAtMs > 0) {
            "Signed app preparation is not the native V1 frame"
        }
        val issued = ByteBuffer.wrap(bytes, 1, 8).order(ByteOrder.LITTLE_ENDIAN).long
        val expiry = ByteBuffer.wrap(bytes, 9, 8).order(ByteOrder.LITTLE_ENDIAN).long
        // KeyMint selects its attested key after this issuer challenge; the signed key ID is zero.
        require(issued > 0 && issued < expiry && expiry - issued == 120_000L && expiry == expiresAtMs &&
            bytes.copyOfRange(17, 49).contentEquals(selection.clientNonce()) &&
            bytes.copyOfRange(81, 113).contentEquals(selection.releaseId()) &&
            bytes.copyOfRange(113, 145).contentEquals(selection.profileId()) &&
            bytes.copyOfRange(145, 177).all { it == 0.toByte() } &&
            bytes.copyOfRange(177, 209).contentEquals(selection.laneId()) &&
            bytes.copyOfRange(49, 81).any { it != 0.toByte() } &&
            !bytes.copyOfRange(49, 81).contentEquals(selection.clientNonce()) &&
            bytes.copyOfRange(209, 273).any { it != 0.toByte() }) {
            "Signed app preparation changed its native selection"
        }
    }

    private fun sameOrRemember(original: List<ByteArray>?, next: List<ByteArray>, label: String) {
        if (original != null) check(original.size == next.size && original.indices.all {
            original[it].contentEquals(next[it])
        }) { "$label changed after native dispatch" }
    }

    private fun copyFields(fields: List<ByteArray>): List<ByteArray> = fields.map(ByteArray::copyOf)
    private fun sameSelection(left: Selection, right: Selection): Boolean =
        left.accountI105 == right.accountI105 &&
            left.ticket().contentEquals(right.ticket()) &&
            left.clientNonce().contentEquals(right.clientNonce()) &&
            left.releaseId().contentEquals(right.releaseId()) &&
            left.profileId().contentEquals(right.profileId()) &&
            left.laneId().contentEquals(right.laneId()) &&
            left.ownerScope().contentEquals(right.ownerScope()) &&
            left.nativeDeadlineContinuousMS() == right.nativeDeadlineContinuousMS()
    private fun u32(value: Int): ByteArray = KagemushaCoreCoordinatorFrameV1.u32(value)
    private fun u64(value: Long): ByteArray = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()

    private companion object {
        val METHOD = KagemushaCoreCoordinatorMethodV1.INITIAL_ENROLLMENT
    }
}
