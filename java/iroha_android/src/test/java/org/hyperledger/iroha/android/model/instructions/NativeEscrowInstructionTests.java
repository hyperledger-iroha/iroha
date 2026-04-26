package org.hyperledger.iroha.android.model.instructions;

/** Regression coverage for Android native escrow instruction builders. */
public final class NativeEscrowInstructionTests {

  @org.junit.Test
  public void openAssetEscrowUsesNativeEscrowArgumentSchema() {
    final NativeEscrowInstructions.OpenAssetEscrow instruction =
        NativeEscrowInstructions.openAssetEscrow(
            "escrow-hash",
            "xor#wonderland",
            "42.5",
            NativeEscrowInstructions.evidenceHashes("hash-a", "hash-b"));

    assert instruction.kind() == InstructionKind.CUSTOM : "open escrow should be custom";
    assert "OpenAssetEscrow".equals(instruction.toArguments().get("action")) : "action mismatch";
    assert "escrow-hash".equals(instruction.toArguments().get("escrow_id")) : "escrow mismatch";
    assert "xor#wonderland".equals(instruction.toArguments().get("asset_definition"))
        : "asset definition mismatch";
    assert "42.5".equals(instruction.toArguments().get("amount")) : "amount mismatch";
    assert "hash-a,hash-b".equals(instruction.toArguments().get("evidence_hashes"))
        : "evidence mismatch";
    assert instruction.equals(
        NativeEscrowInstructions.OpenAssetEscrow.fromArguments(instruction.toArguments()))
        : "roundtrip mismatch";
  }

  @org.junit.Test
  public void lifecycleOperationsRoundTrip() {
    final NativeEscrowInstructions.AcceptAssetEscrow accept =
        NativeEscrowInstructions.acceptAssetEscrow("escrow-hash");
    final NativeEscrowInstructions.MarkEscrowPaymentSent markPaid =
        NativeEscrowInstructions.markPaymentSent("escrow-hash");
    final NativeEscrowInstructions.ReleaseAssetEscrow release =
        NativeEscrowInstructions.releaseAssetEscrow("escrow-hash");
    final NativeEscrowInstructions.CancelAssetEscrow cancel =
        NativeEscrowInstructions.cancelAssetEscrow("escrow-hash");

    assert "AcceptAssetEscrow".equals(accept.toArguments().get("action"))
        : "accept action mismatch";
    assert accept.equals(NativeEscrowInstructions.AcceptAssetEscrow.fromArguments(accept.toArguments()))
        : "accept roundtrip mismatch";
    assert "MarkEscrowPaymentSent".equals(markPaid.toArguments().get("action"))
        : "mark paid action mismatch";
    assert markPaid.equals(
        NativeEscrowInstructions.MarkEscrowPaymentSent.fromArguments(markPaid.toArguments()))
        : "mark paid roundtrip mismatch";
    assert "ReleaseAssetEscrow".equals(release.toArguments().get("action"))
        : "release action mismatch";
    assert release.equals(
        NativeEscrowInstructions.ReleaseAssetEscrow.fromArguments(release.toArguments()))
        : "release roundtrip mismatch";
    assert "CancelAssetEscrow".equals(cancel.toArguments().get("action"))
        : "cancel action mismatch";
    assert cancel.equals(NativeEscrowInstructions.CancelAssetEscrow.fromArguments(cancel.toArguments()))
        : "cancel roundtrip mismatch";
  }

  @org.junit.Test
  public void disputeOperationsCarrySplitAndEvidence() {
    final NativeEscrowInstructions.OpenEscrowDispute dispute =
        NativeEscrowInstructions.openEscrowDispute(
            "escrow-hash", NativeEscrowInstructions.evidenceHashes("party-evidence"));
    final NativeEscrowInstructions.ResolveEscrowDispute resolve =
        NativeEscrowInstructions.resolveEscrowDispute(
            "escrow-hash", "30", "12", NativeEscrowInstructions.evidenceHashes("judgement"));

    assert "party-evidence".equals(dispute.toArguments().get("evidence_hashes"))
        : "dispute evidence mismatch";
    assert dispute.equals(
        NativeEscrowInstructions.OpenEscrowDispute.fromArguments(dispute.toArguments()))
        : "dispute roundtrip mismatch";
    assert "ResolveEscrowDispute".equals(resolve.toArguments().get("action"))
        : "resolve action mismatch";
    assert "30".equals(resolve.toArguments().get("buyer_amount")) : "buyer split mismatch";
    assert "12".equals(resolve.toArguments().get("seller_amount")) : "seller split mismatch";
    assert "judgement".equals(resolve.toArguments().get("evidence_hashes"))
        : "resolve evidence mismatch";
    assert resolve.equals(
        NativeEscrowInstructions.ResolveEscrowDispute.fromArguments(resolve.toArguments()))
        : "resolve roundtrip mismatch";
  }

  @org.junit.Test
  public void anonymousOperationsCarryShieldedProofMaterial() {
    final NativeEscrowInstructions.AnonymousEscrowInstruction open =
        NativeEscrowInstructions.openAnonymousAssetEscrow(
            "anonymous-escrow",
            "xor#wonderland",
            NativeEscrowInstructions.evidenceHashes("n1", "n2"),
            "escrow-note",
            "proof-envelope",
            "root",
            NativeEscrowInstructions.evidenceHashes("receipt"));
    final NativeEscrowInstructions.AnonymousEscrowInstruction release =
        NativeEscrowInstructions.releaseAnonymousAssetEscrow(
            "anonymous-escrow",
            NativeEscrowInstructions.evidenceHashes("escrow-nullifier"),
            NativeEscrowInstructions.evidenceHashes("buyer-note"),
            "release-proof",
            null);
    final NativeEscrowInstructions.AnonymousEscrowInstruction resolve =
        NativeEscrowInstructions.resolveAnonymousEscrowDispute(
            "anonymous-escrow",
            NativeEscrowInstructions.evidenceHashes("escrow-nullifier"),
            NativeEscrowInstructions.evidenceHashes("buyer-note"),
            NativeEscrowInstructions.evidenceHashes("seller-note"),
            "resolve-proof",
            null,
            NativeEscrowInstructions.evidenceHashes("judgement"));

    assert "OpenAnonymousAssetEscrow".equals(open.action()) : "open action mismatch";
    assert "n1,n2".equals(open.toArguments().get("funding_nullifiers"))
        : "funding nullifiers mismatch";
    assert "escrow-note".equals(open.toArguments().get("escrow_commitment"))
        : "commitment mismatch";
    assert "proof-envelope".equals(open.toArguments().get("proof")) : "proof mismatch";
    assert "root".equals(open.toArguments().get("root_hint")) : "root hint mismatch";
    assert open.equals(
        NativeEscrowInstructions.AnonymousEscrowInstruction.fromArguments(open.toArguments()))
        : "open anonymous roundtrip mismatch";
    assert "buyer-note".equals(release.toArguments().get("buyer_output_commitments"))
        : "release output mismatch";
    assert release.equals(
        NativeEscrowInstructions.AnonymousEscrowInstruction.fromArguments(release.toArguments()))
        : "release anonymous roundtrip mismatch";
    assert "seller-note".equals(resolve.toArguments().get("seller_output_commitments"))
        : "resolve seller output mismatch";
    assert "judgement".equals(resolve.toArguments().get("evidence_hashes"))
        : "resolve evidence mismatch";
    assert resolve.equals(
        NativeEscrowInstructions.AnonymousEscrowInstruction.fromArguments(resolve.toArguments()))
        : "resolve anonymous roundtrip mismatch";
  }

  @org.junit.Test
  public void statusAndPermissionConstantsMatchWireNames() {
    assert NativeEscrowInstructions.Status.PAYMENT_SENT
        == NativeEscrowInstructions.Status.fromWireName("PaymentSent")
        : "status parse mismatch";
    assert "CanResolveEscrowDispute".equals(
        NativeEscrowInstructions.CAN_RESOLVE_ESCROW_DISPUTE)
        : "permission mismatch";
  }
}
