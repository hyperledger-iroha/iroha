package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.escrow.AssetEscrowStatus
import org.hyperledger.iroha.sdk.core.model.escrow.NativeEscrowPermissions
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

class NativeEscrowInstructionsTest {

    @Test
    fun `native escrow amounts are canonical quantities`() {
        listOf(" ", "+1", "01", "1e0", "-1", "1.0", "1.2300").forEach { amount ->
            assertFailsWith<IllegalArgumentException> {
                OpenAssetEscrowInstruction("escrow", "asset", amount)
            }
            assertFailsWith<IllegalArgumentException> {
                ResolveEscrowDisputeInstruction("escrow", amount, "0")
            }
        }
        assertFailsWith<IllegalArgumentException> {
            OpenAssetEscrowInstruction.fromArguments(
                mapOf("escrow_id" to "escrow", "asset_definition" to "asset", "amount" to "1.0"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ResolveEscrowDisputeInstruction.fromArguments(
                mapOf("escrow_id" to "escrow", "buyer_amount" to "1", "seller_amount" to "0.0"),
            )
        }

        val quantity = KotodamaQuantity.parseCanonical("1.25")
        assertEquals("1.25", OpenAssetEscrowInstruction("escrow", "asset", quantity).amount)
        assertEquals(
            listOf("1.25", "1.25"),
            ResolveEscrowDisputeInstruction("escrow", quantity, quantity).let {
                listOf(it.buyerAmount, it.sellerAmount)
            },
        )
    }

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
    fun `native escrow status and permission constants match wire names`() {
        assertEquals(AssetEscrowStatus.PAYMENT_SENT, AssetEscrowStatus.fromWireName("PaymentSent"))
        assertEquals("CanResolveEscrowDispute", NativeEscrowPermissions.CAN_RESOLVE_ESCROW_DISPUTE)
    }
}
