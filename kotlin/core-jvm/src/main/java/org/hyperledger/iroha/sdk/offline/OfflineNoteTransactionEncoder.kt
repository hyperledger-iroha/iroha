// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.TransactionBuilder

internal const val INSTRUCTION_ISSUE_WIRE_NAME: String =
    OfflineNote.ISSUE_INSTRUCTION_SCHEMA
internal const val INSTRUCTION_REDEEM_WIRE_NAME: String =
    OfflineNote.REDEEM_INSTRUCTION_SCHEMA
internal const val INSTRUCTION_AUDIT_WIRE_NAME: String =
    OfflineNote.AUDIT_INSTRUCTION_SCHEMA

/**
 * Builds signed transactions carrying the Offline Note chain instructions
 * (`IssueOfflineNote`, `RedeemOfflineNote`, `AuditOfflineNote`).
 *
 * Each instruction is delivered to the chain as the raw model body (the output of
 * [OfflineNote.encodeIssue] / [OfflineNote.encodeRedeem] / [OfflineNote.encodeAudit] with the
 * outer model header stripped) wrapped in a canonical Norito frame whose schema hash is the
 * instruction wire name (e.g. `iroha_data_model::isi::offline::IssueOfflineNote`). The frame uses
 * the standard v1 `COMPACT_LEN` flag so the body round-trips through the chain's
 * `frame_bare_with_header_flags` decoder. The wrapped payload
 * carries one compact field-length prefix before the raw model body.
 *
 * The wrapped bytes are bound to the wire name via [InstructionBox.fromWirePayload] and placed in
 * [Executable.instructions]; the resulting [TransactionPayload] is signed via the supplied
 * [TransactionBuilder].
 *
 * [OfflineNote.redeemInstruction] and [OfflineNote.auditInstruction] call
 * [OfflineNote.Redeem.validateProofBinding] / [OfflineNote.AuditBundle.validateProofBinding]
 * internally so proof-binding mismatches surface as `IllegalArgumentException` from the build call
 * rather than producing a transaction that the ledger will later reject.
 */
class OfflineNoteTransactionEncoder(
    private val transactionBuilder: TransactionBuilder,
) {

    /** Inputs for [buildIssueOfflineNote]. */
    data class IssueOfflineNoteRequest(
        val chainId: String,
        val authority: String,
        val issue: OfflineNote.Issue,
        val ttlMs: Long? = null,
        val nonce: Int? = null,
    )

    /** Inputs for [buildRedeemOfflineNote]. */
    data class RedeemOfflineNoteRequest(
        val chainId: String,
        val authority: String,
        val redemption: OfflineNote.Redeem,
        val ttlMs: Long? = null,
        val nonce: Int? = null,
    )

    /** Inputs for [buildAuditOfflineNote]. */
    data class AuditOfflineNoteRequest(
        val chainId: String,
        val authority: String,
        val audit: OfflineNote.AuditBundle,
        val ttlMs: Long? = null,
        val nonce: Int? = null,
    )

    /** Builds and signs a transaction that carries a single `IssueOfflineNote` instruction. */
    fun buildIssueOfflineNote(
        request: IssueOfflineNoteRequest,
        signer: Signer,
        creationTimeMs: Long,
    ): SignedTransaction {
        val box = issueInstructionBox(request.issue)
        return signTransaction(
            chainId = request.chainId,
            authority = request.authority,
            creationTimeMs = creationTimeMs,
            ttlMs = request.ttlMs,
            nonce = request.nonce,
            instruction = box,
            signer = signer,
        )
    }

    /**
     * Builds and signs a transaction that carries a single `RedeemOfflineNote` instruction.
     *
     * Calls [OfflineNote.Redeem.validateProofBinding] (via [OfflineNote.redeemInstruction])
     * before encoding so a mismatched recursive proof public-inputs hash is rejected up front.
     */
    fun buildRedeemOfflineNote(
        request: RedeemOfflineNoteRequest,
        signer: Signer,
        creationTimeMs: Long,
    ): SignedTransaction {
        val box = redeemInstructionBox(request.redemption)
        return signTransaction(
            chainId = request.chainId,
            authority = request.authority,
            creationTimeMs = creationTimeMs,
            ttlMs = request.ttlMs,
            nonce = request.nonce,
            instruction = box,
            signer = signer,
        )
    }

    /**
     * Builds and signs a transaction that carries a single `AuditOfflineNote` instruction.
     *
     * Calls [OfflineNote.AuditBundle.validateProofBinding] (via [OfflineNote.auditInstruction])
     * before encoding so a mismatched recursive proof public-inputs hash is rejected up front.
     */
    fun buildAuditOfflineNote(
        request: AuditOfflineNoteRequest,
        signer: Signer,
        creationTimeMs: Long,
    ): SignedTransaction {
        val box = auditInstructionBox(request.audit)
        return signTransaction(
            chainId = request.chainId,
            authority = request.authority,
            creationTimeMs = creationTimeMs,
            ttlMs = request.ttlMs,
            nonce = request.nonce,
            instruction = box,
            signer = signer,
        )
    }

    private fun signTransaction(
        chainId: String,
        authority: String,
        creationTimeMs: Long,
        ttlMs: Long?,
        nonce: Int?,
        instruction: InstructionBox,
        signer: Signer,
    ): SignedTransaction {
        val payload = TransactionPayload(
            chainId = chainId,
            authority = authority,
            creationTimeMs = creationTimeMs,
            executable = Executable.instructions(listOf(instruction)),
            timeToLiveMs = ttlMs,
            nonce = nonce,
        )
        return transactionBuilder.encodeAndSign(payload, signer)
    }

    companion object {
        /** Wraps an Offline Note issue model body (header-stripped) as a typed instruction. */
        @JvmStatic
        fun issueInstructionBox(issue: OfflineNote.Issue): InstructionBox =
            OfflineNote.issueInstruction(issue)

        /** Wraps an Offline Note redeem model body (header-stripped) as a typed instruction. */
        @JvmStatic
        fun redeemInstructionBox(redemption: OfflineNote.Redeem): InstructionBox =
            OfflineNote.redeemInstruction(redemption)

        /** Wraps an Offline Note audit model body (header-stripped) as a typed instruction. */
        @JvmStatic
        fun auditInstructionBox(audit: OfflineNote.AuditBundle): InstructionBox =
            OfflineNote.auditInstruction(audit)
    }
}
