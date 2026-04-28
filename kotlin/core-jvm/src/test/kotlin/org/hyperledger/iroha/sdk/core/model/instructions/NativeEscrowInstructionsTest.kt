package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.escrow.AssetEscrowStatus
import org.hyperledger.iroha.sdk.core.model.escrow.NativeEscrowPermissions
import kotlin.test.Test
import kotlin.test.assertEquals

class NativeEscrowInstructionsTest {

    @Test
    fun `open asset escrow uses native escrow argument schema`() {
        val instruction = OpenAssetEscrowInstruction(
            escrowId = "escrow-hash",
            assetDefinition = "xor#wonderland",
            amount = "42.5",
            evidenceHashes = listOf("hash-a", "hash-b"),
        )

        assertEquals(InstructionKind.CUSTOM, instruction.kind)
        assertEquals("OpenAssetEscrow", instruction.arguments["action"])
        assertEquals("escrow-hash", instruction.arguments["escrow_id"])
        assertEquals("xor#wonderland", instruction.arguments["asset_definition"])
        assertEquals("42.5", instruction.arguments["amount"])
        assertEquals("hash-a,hash-b", instruction.arguments["evidence_hashes"])
        assertEquals(instruction, OpenAssetEscrowInstruction.fromArguments(instruction.arguments))
    }

    @Test
    fun `simple escrow lifecycle operations roundtrip`() {
        val accept = AcceptAssetEscrowInstruction("escrow-hash")
        val markPaid = MarkEscrowPaymentSentInstruction("escrow-hash")
        val release = ReleaseAssetEscrowInstruction("escrow-hash")
        val cancel = CancelAssetEscrowInstruction("escrow-hash")

        assertEquals("AcceptAssetEscrow", accept.arguments["action"])
        assertEquals(accept, AcceptAssetEscrowInstruction.fromArguments(accept.arguments))
        assertEquals("MarkEscrowPaymentSent", markPaid.arguments["action"])
        assertEquals(markPaid, MarkEscrowPaymentSentInstruction.fromArguments(markPaid.arguments))
        assertEquals("ReleaseAssetEscrow", release.arguments["action"])
        assertEquals(release, ReleaseAssetEscrowInstruction.fromArguments(release.arguments))
        assertEquals("CancelAssetEscrow", cancel.arguments["action"])
        assertEquals(cancel, CancelAssetEscrowInstruction.fromArguments(cancel.arguments))
    }

    @Test
    fun `dispute operations carry split and evidence`() {
        val dispute = OpenEscrowDisputeInstruction(
            escrowId = "escrow-hash",
            evidenceHashes = listOf("party-evidence"),
        )
        val resolve = ResolveEscrowDisputeInstruction(
            escrowId = "escrow-hash",
            buyerAmount = "30",
            sellerAmount = "12",
            evidenceHashes = listOf("judgement"),
        )

        assertEquals("party-evidence", dispute.arguments["evidence_hashes"])
        assertEquals(dispute, OpenEscrowDisputeInstruction.fromArguments(dispute.arguments))
        assertEquals("ResolveEscrowDispute", resolve.arguments["action"])
        assertEquals("30", resolve.arguments["buyer_amount"])
        assertEquals("12", resolve.arguments["seller_amount"])
        assertEquals("judgement", resolve.arguments["evidence_hashes"])
        assertEquals(resolve, ResolveEscrowDisputeInstruction.fromArguments(resolve.arguments))
    }

    @Test
    fun `anonymous escrow operations carry shielded proof material`() {
        val open = OpenAnonymousAssetEscrowInstruction(
            escrowId = "anonymous-escrow",
            assetDefinition = "xor#wonderland",
            fundingNullifiers = listOf("n1", "n2"),
            escrowCommitment = "escrow-note",
            proof = "proof-envelope",
            rootHint = "root",
            evidenceHashes = listOf("receipt"),
        )
        val release = ReleaseAnonymousAssetEscrowInstruction(
            escrowId = "anonymous-escrow",
            escrowNullifiers = listOf("escrow-nullifier"),
            buyerOutputCommitments = listOf("buyer-note"),
            proof = "release-proof",
        )
        val resolve = ResolveAnonymousEscrowDisputeInstruction(
            escrowId = "anonymous-escrow",
            escrowNullifiers = listOf("escrow-nullifier"),
            buyerOutputCommitments = listOf("buyer-note"),
            sellerOutputCommitments = listOf("seller-note"),
            proof = "resolve-proof",
            evidenceHashes = listOf("judgement"),
        )

        assertEquals("OpenAnonymousAssetEscrow", open.arguments["action"])
        assertEquals("n1,n2", open.arguments["funding_nullifiers"])
        assertEquals("escrow-note", open.arguments["escrow_commitment"])
        assertEquals("proof-envelope", open.arguments["proof"])
        assertEquals("root", open.arguments["root_hint"])
        assertEquals("receipt", open.arguments["evidence_hashes"])
        assertEquals(open, OpenAnonymousAssetEscrowInstruction.fromArguments(open.arguments))
        assertEquals("buyer-note", release.arguments["buyer_output_commitments"])
        assertEquals(release, ReleaseAnonymousAssetEscrowInstruction.fromArguments(release.arguments))
        assertEquals("seller-note", resolve.arguments["seller_output_commitments"])
        assertEquals("judgement", resolve.arguments["evidence_hashes"])
        assertEquals(resolve, ResolveAnonymousEscrowDisputeInstruction.fromArguments(resolve.arguments))
    }

    @Test
    fun `native escrow status and permission constants match wire names`() {
        assertEquals(AssetEscrowStatus.PAYMENT_SENT, AssetEscrowStatus.fromWireName("PaymentSent"))
        assertEquals("CanResolveEscrowDispute", NativeEscrowPermissions.CAN_RESOLVE_ESCROW_DISPUTE)
    }
}
