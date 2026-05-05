package org.hyperledger.iroha.android.model.instructions;

import java.util.Map;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.junit.Test;

/** Regression coverage for Android RWA instruction builders. */
public final class RwaInstructionBuilderTests {

  @Test
  public void registerRwaPreservesRawJsonPayload() {
    final RegisterRwaInstruction instruction =
        RegisterRwaInstruction.builder().setRwaJson("{\"domain\":\"commodities\"}").build();

    assert instruction.kind() == InstructionKind.CUSTOM : "register rwa should be custom";
    assert "{\"domain\":\"commodities\"}".equals(instruction.rwaJson()) : "payload mismatch";
    assert "RegisterRwa".equals(instruction.toArguments().get("action")) : "action mismatch";
    assert "{\"domain\":\"commodities\"}".equals(instruction.toArguments().get("rwa"))
        : "rwa payload mismatch";
    assert instruction.equals(RegisterRwaInstruction.fromArguments(instruction.toArguments()))
        : "roundtrip mismatch";
  }

  @Test
  public void transferRwaUsesCanonicalArgumentSchema() {
    final String source = TestAccountIds.ed25519Authority(0x41);
    final String destination = TestAccountIds.ed25519Authority(0x42);
    final TransferRwaInstruction instruction =
        TransferRwaInstruction.builder()
            .setSourceAccountId(source)
            .setRwaId("lot-001$commodities")
            .setQuantity("42")
            .setDestinationAccountId(destination)
            .build();

    final Map<String, String> arguments = instruction.toArguments();
    assert instruction.kind() == InstructionKind.CUSTOM : "transfer rwa should be custom";
    assert "TransferRwa".equals(arguments.get("action")) : "action mismatch";
    assert source.equals(arguments.get("source")) : "source mismatch";
    assert "lot-001$commodities".equals(arguments.get("rwa")) : "rwa mismatch";
    assert "42".equals(arguments.get("quantity")) : "quantity mismatch";
    assert destination.equals(arguments.get("destination")) : "destination mismatch";
    assert instruction.equals(TransferRwaInstruction.fromArguments(arguments))
        : "roundtrip mismatch";
  }

  @Test
  public void setRwaControlsPreservesRawPolicyJson() {
    final SetRwaControlsInstruction instruction =
        SetRwaControlsInstruction.builder()
            .setRwaId("lot-002$commodities")
            .setControlsJson("{\"freeze_enabled\":true}")
            .build();

    assert instruction.kind() == InstructionKind.CUSTOM : "set controls should be custom";
    assert "SetRwaControls".equals(instruction.toArguments().get("action")) : "action mismatch";
    assert "lot-002$commodities".equals(instruction.toArguments().get("rwa")) : "rwa mismatch";
    assert "{\"freeze_enabled\":true}".equals(instruction.toArguments().get("controls"))
        : "controls mismatch";
    assert instruction.equals(SetRwaControlsInstruction.fromArguments(instruction.toArguments()))
        : "roundtrip mismatch";
  }

  @Test
  public void remainingRwaOperationsRoundTrip() {
    final MergeRwasInstruction merge =
        MergeRwasInstruction.builder().setMergeJson("{\"parents\":[]}").build();
    final RedeemRwaInstruction redeem =
        RedeemRwaInstruction.builder().setRwaId("lot-004$commodities").setQuantity("2").build();
    final FreezeRwaInstruction freeze =
        FreezeRwaInstruction.builder().setRwaId("lot-004$commodities").build();
    final UnfreezeRwaInstruction unfreeze =
        UnfreezeRwaInstruction.builder().setRwaId("lot-004$commodities").build();
    final HoldRwaInstruction hold =
        HoldRwaInstruction.builder().setRwaId("lot-004$commodities").setQuantity(3).build();
    final ReleaseRwaInstruction release =
        ReleaseRwaInstruction.builder().setRwaId("lot-004$commodities").setQuantity("1").build();
    final ForceTransferRwaInstruction forceTransfer =
        ForceTransferRwaInstruction.builder()
            .setRwaId("lot-004$commodities")
            .setQuantity("1")
            .setDestinationAccountId(TestAccountIds.ed25519Authority(0x43))
            .build();

    assert "MergeRwas".equals(merge.toArguments().get("action")) : "merge action mismatch";
    assert merge.equals(MergeRwasInstruction.fromArguments(merge.toArguments()))
        : "merge roundtrip mismatch";

    assert "RedeemRwa".equals(redeem.toArguments().get("action")) : "redeem action mismatch";
    assert redeem.equals(RedeemRwaInstruction.fromArguments(redeem.toArguments()))
        : "redeem roundtrip mismatch";

    assert "FreezeRwa".equals(freeze.toArguments().get("action")) : "freeze action mismatch";
    assert freeze.equals(FreezeRwaInstruction.fromArguments(freeze.toArguments()))
        : "freeze roundtrip mismatch";

    assert "UnfreezeRwa".equals(unfreeze.toArguments().get("action"))
        : "unfreeze action mismatch";
    assert unfreeze.equals(UnfreezeRwaInstruction.fromArguments(unfreeze.toArguments()))
        : "unfreeze roundtrip mismatch";

    assert "HoldRwa".equals(hold.toArguments().get("action")) : "hold action mismatch";
    assert "3".equals(hold.toArguments().get("quantity")) : "hold quantity mismatch";
    assert hold.equals(HoldRwaInstruction.fromArguments(hold.toArguments()))
        : "hold roundtrip mismatch";

    assert "ReleaseRwa".equals(release.toArguments().get("action"))
        : "release action mismatch";
    assert release.equals(ReleaseRwaInstruction.fromArguments(release.toArguments()))
        : "release roundtrip mismatch";

    assert "ForceTransferRwa".equals(forceTransfer.toArguments().get("action"))
        : "force transfer action mismatch";
    assert forceTransfer.equals(
        ForceTransferRwaInstruction.fromArguments(forceTransfer.toArguments()))
        : "force transfer roundtrip mismatch";
  }

  @Test
  public void setAndRemoveKeyValueAcceptRwaTargets() {
    final SetKeyValueInstruction setInstruction =
        SetKeyValueInstruction.builder()
            .setRwaId("lot-003$commodities")
            .setKey("serial")
            .setValue("ABC-123")
            .build();
    final RemoveKeyValueInstruction removeInstruction =
        RemoveKeyValueInstruction.builder()
            .setRwaId("lot-003$commodities")
            .setKey("serial")
            .build();

    assert setInstruction.target() == SetKeyValueInstruction.Target.RWA : "set target mismatch";
    assert "SetRwaKeyValue".equals(setInstruction.toArguments().get("action"))
        : "set action mismatch";
    assert "lot-003$commodities".equals(setInstruction.toArguments().get("rwa"))
        : "set rwa mismatch";

    assert removeInstruction.target() == RemoveKeyValueInstruction.Target.RWA
        : "remove target mismatch";
    assert "RemoveRwaKeyValue".equals(removeInstruction.toArguments().get("action"))
        : "remove action mismatch";
    assert "lot-003$commodities".equals(removeInstruction.toArguments().get("rwa"))
        : "remove rwa mismatch";
  }
}
