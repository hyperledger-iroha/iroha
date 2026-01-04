package org.hyperledger.iroha.android.model.instructions;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.ActivateRuntimeUpgradeInstruction;
import org.hyperledger.iroha.android.model.instructions.ApprovePinManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.BindManifestAliasInstruction;
import org.hyperledger.iroha.android.model.instructions.CancelRuntimeUpgradeInstruction;
import org.hyperledger.iroha.android.model.instructions.CastPlainBallotInstruction;
import org.hyperledger.iroha.android.model.instructions.CastZkBallotInstruction;
import org.hyperledger.iroha.android.model.instructions.CreateKaigiInstruction;
import org.hyperledger.iroha.android.model.instructions.EnactReferendumInstruction;
import org.hyperledger.iroha.android.model.instructions.EndKaigiInstruction;
import org.hyperledger.iroha.android.model.instructions.ExpireSpaceDirectoryManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.FinalizeReferendumInstruction;
import org.hyperledger.iroha.android.model.instructions.UpgradeInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterPeerWithPopInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterPipelineTriggerInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterPrecommitTriggerInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterRoleInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterTimeTriggerInstruction;
import org.hyperledger.iroha.android.model.instructions.JoinKaigiInstruction;
import org.hyperledger.iroha.android.model.instructions.LeaveKaigiInstruction;
import org.hyperledger.iroha.android.model.instructions.RecordCapacityTelemetryInstruction;
import org.hyperledger.iroha.android.model.instructions.RecordKaigiUsageInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterKaigiRelayInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterOfflineAllowanceInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterCapacityDeclarationInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterCapacityDisputeInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterPinManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.RetirePinManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.CompleteReplicationOrderInstruction;
import org.hyperledger.iroha.android.model.instructions.IssueReplicationOrderInstruction;
import org.hyperledger.iroha.android.model.instructions.RecordReplicationReceiptInstruction;
import org.hyperledger.iroha.android.model.instructions.MultisigRegisterInstruction;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction;
import org.hyperledger.iroha.android.model.instructions.RevokeSpaceDirectoryManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.PersistCouncilForEpochInstruction;
import org.hyperledger.iroha.android.model.instructions.ProposeRuntimeUpgradeInstruction;
import org.hyperledger.iroha.android.model.instructions.ProposeDeployContractInstruction;
import org.hyperledger.iroha.android.model.instructions.PublishSpaceDirectoryManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterVerifyingKeyInstruction;
import org.hyperledger.iroha.android.model.instructions.SetKaigiRelayManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.UpdateVerifyingKeyInstruction;
import org.hyperledger.iroha.android.model.instructions.UpsertProviderCreditInstruction;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyRecordDescription;
import org.hyperledger.iroha.android.multisig.MultisigSpec;

/** Convenience builders for common strongly typed instructions. */
public final class InstructionBuilders {

  private InstructionBuilders() {}

  public static InstructionBox registerDomain(final String domain) {
    return InstructionBox.of(registerDomainTemplate(domain));
  }

  public static InstructionBox registerAccount(final String account) {
    return InstructionBox.of(registerAccountTemplate(account));
  }

  public static InstructionBox registerMultisig(final String accountId, final MultisigSpec spec) {
    return InstructionBox.of(registerMultisigTemplate(accountId, spec));
  }

  public static InstructionBox registerAssetDefinition(final String assetDefinitionId) {
    return InstructionBox.of(registerAssetDefinitionTemplate(assetDefinitionId));
  }

  public static InstructionBox registerPeerWithPop(
      final String peerPublicKey, final byte[] proofOfPossession) {
    return InstructionBox.of(registerPeerWithPopTemplate(peerPublicKey, proofOfPossession));
  }

  public static InstructionBox registerRole(
      final String roleId, final String ownerAccountId, final List<String> permissions) {
    return InstructionBox.of(registerRoleTemplate(roleId, ownerAccountId, permissions));
  }

  public static InstructionBox registerNft(
      final String nftId, final Map<String, String> metadata) {
    return InstructionBox.of(registerNftTemplate(nftId, metadata));
  }

  public static MultisigRegisterInstruction registerMultisigTemplate(
      final String accountId, final MultisigSpec spec) {
    return MultisigRegisterInstruction.builder()
        .setAccountId(accountId)
        .setSpec(Objects.requireNonNull(spec, "spec"))
        .build();
  }

  public static InstructionBox registerPinManifest(
      final RegisterPinManifestInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox registerCapacityDeclaration(
      final RegisterCapacityDeclarationInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox registerCapacityDispute(
      final RegisterCapacityDisputeInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox registerOfflineAllowance(
      final RegisterOfflineAllowanceInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox setPricingSchedule(
      final SetPricingScheduleInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox upsertProviderCredit(
      final UpsertProviderCreditInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox approvePinManifest(
      final ApprovePinManifestInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox retirePinManifest(
      final RetirePinManifestInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox bindManifestAlias(
      final BindManifestAliasInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox publishSpaceDirectoryManifest(
      final PublishSpaceDirectoryManifestInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox revokeSpaceDirectoryManifest(
      final RevokeSpaceDirectoryManifestInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox expireSpaceDirectoryManifest(
      final ExpireSpaceDirectoryManifestInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox issueReplicationOrder(
      final IssueReplicationOrderInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox completeReplicationOrder(
      final CompleteReplicationOrderInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox recordReplicationReceipt(
      final RecordReplicationReceiptInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox recordCapacityTelemetry(
      final RecordCapacityTelemetryInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox proposeDeployContract(
      final ProposeDeployContractInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox castZkBallot(final CastZkBallotInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox castPlainBallot(final CastPlainBallotInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox enactReferendum(final EnactReferendumInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox finalizeReferendum(
      final FinalizeReferendumInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox persistCouncilForEpoch(
      final PersistCouncilForEpochInstruction instruction) {
    return InstructionBox.of(Objects.requireNonNull(instruction, "instruction"));
  }

  public static InstructionBox proposeRuntimeUpgrade(final byte[] manifestBytes) {
    return InstructionBox.of(
        ProposeRuntimeUpgradeInstruction.builder().setManifestBytes(manifestBytes).build());
  }

  public static InstructionBox activateRuntimeUpgrade(final String idHex) {
    return InstructionBox.of(
        ActivateRuntimeUpgradeInstruction.builder().setIdHex(idHex).build());
  }

  public static InstructionBox cancelRuntimeUpgrade(final String idHex) {
    return InstructionBox.of(
        CancelRuntimeUpgradeInstruction.builder().setIdHex(idHex).build());
  }

  public static InstructionBox registerPipelineTrigger(
      final String triggerId,
      final String authority,
      final List<InstructionBox> instructions,
      final RegisterPipelineTriggerInstruction.PipelineFilter filter) {
    return registerPipelineTrigger(triggerId, authority, instructions, filter, null, null);
  }

  public static InstructionBox registerPipelineTrigger(
      final String triggerId,
      final String authority,
      final List<InstructionBox> instructions,
      final RegisterPipelineTriggerInstruction.PipelineFilter filter,
      final Integer repeats,
      final Map<String, String> metadata) {
    return InstructionBox.of(
        registerPipelineTriggerTemplate(triggerId, authority, instructions, filter, repeats, metadata));
  }

  public static InstructionBox registerTimeTrigger(
      final String triggerId,
      final String authority,
      final List<InstructionBox> instructions,
      final long startMs) {
    return registerTimeTrigger(triggerId, authority, instructions, startMs, null, null, null);
  }

  public static InstructionBox registerTimeTrigger(
      final String triggerId,
      final String authority,
      final List<InstructionBox> instructions,
      final long startMs,
      final Long periodMs,
      final Integer repeats,
      final Map<String, String> metadata) {
    return InstructionBox.of(
        registerTimeTriggerTemplate(
            triggerId, authority, instructions, startMs, periodMs, repeats, metadata));
  }

  public static InstructionBox registerPrecommitTrigger(
      final String triggerId,
      final String authority,
      final List<InstructionBox> instructions) {
    return registerPrecommitTrigger(triggerId, authority, instructions, null, null);
  }

  public static InstructionBox registerPrecommitTrigger(
      final String triggerId,
      final String authority,
      final List<InstructionBox> instructions,
      final Integer repeats,
      final Map<String, String> metadata) {
    return InstructionBox.of(
        registerPrecommitTriggerTemplate(triggerId, authority, instructions, repeats, metadata));
  }

  public static InstructionBox registerVerifyingKey(
      final String backend,
      final String name,
      final VerifyingKeyRecordDescription record) {
    return InstructionBox.of(
        RegisterVerifyingKeyInstruction.builder()
            .setBackend(backend)
            .setName(name)
            .setRecord(record)
            .build());
  }

  public static InstructionBox updateVerifyingKey(
      final String backend,
      final String name,
      final VerifyingKeyRecordDescription record) {
    return InstructionBox.of(
        UpdateVerifyingKeyInstruction.builder()
            .setBackend(backend)
            .setName(name)
            .setRecord(record)
            .build());
  }

  public static InstructionBox executeTrigger(final String triggerId) {
    return InstructionBox.of(executeTriggerTemplate(triggerId));
  }

  public static InstructionBox executeTrigger(final String triggerId, final String authorityOverride) {
    return InstructionBox.of(executeTriggerTemplate(triggerId, authorityOverride));
  }

  public static InstructionBox unregisterPeer(final String peerId) {
    return InstructionBox.of(unregisterPeerTemplate(peerId));
  }

  public static InstructionBox unregisterDomain(final String domainId) {
    return InstructionBox.of(unregisterDomainTemplate(domainId));
  }

  public static InstructionBox unregisterAccount(final String accountId) {
    return InstructionBox.of(unregisterAccountTemplate(accountId));
  }

  public static InstructionBox unregisterAssetDefinition(final String assetDefinitionId) {
    return InstructionBox.of(unregisterAssetDefinitionTemplate(assetDefinitionId));
  }

  public static InstructionBox unregisterNft(final String nftId) {
    return InstructionBox.of(unregisterNftTemplate(nftId));
  }

  public static InstructionBox unregisterRole(final String roleId) {
    return InstructionBox.of(unregisterRoleTemplate(roleId));
  }

  public static InstructionBox unregisterTrigger(final String triggerId) {
    return InstructionBox.of(unregisterTriggerTemplate(triggerId));
  }

  public static InstructionBox grantRole(final String destinationAccountId, final String roleId) {
    return InstructionBox.of(
        GrantRoleInstruction.builder()
            .setDestinationAccountId(destinationAccountId)
            .setRoleId(roleId)
            .build());
  }

  public static InstructionBox revokeRole(final String destinationAccountId, final String roleId) {
    return InstructionBox.of(
        RevokeRoleInstruction.builder()
            .setDestinationAccountId(destinationAccountId)
            .setRoleId(roleId)
            .build());
  }

  public static InstructionBox grantPermission(
      final String destinationAccountId, final String permissionName) {
    return InstructionBox.of(
        GrantPermissionInstruction.builder()
            .setDestinationId(destinationAccountId)
            .setPermissionName(permissionName)
            .build());
  }

  public static InstructionBox revokePermission(
      final String destinationAccountId, final String permissionName) {
    return InstructionBox.of(
        RevokePermissionInstruction.builder()
            .setDestinationId(destinationAccountId)
            .setPermissionName(permissionName)
            .build());
  }

  public static InstructionBox grantRolePermission(
      final String destinationRoleId, final String permissionName) {
    return grantRolePermission(destinationRoleId, permissionName, null);
  }

  public static InstructionBox grantRolePermission(
      final String destinationRoleId, final String permissionName, final String permissionPayload) {
    return InstructionBox.of(
        GrantRolePermissionInstruction.builder()
            .setDestinationRoleId(destinationRoleId)
            .setPermissionName(permissionName)
            .setPermissionPayload(permissionPayload)
            .build());
  }

  public static InstructionBox revokeRolePermission(
      final String destinationRoleId, final String permissionName) {
    return revokeRolePermission(destinationRoleId, permissionName, null);
  }

  public static InstructionBox revokeRolePermission(
      final String destinationRoleId, final String permissionName, final String permissionPayload) {
    return InstructionBox.of(
        RevokeRolePermissionInstruction.builder()
            .setDestinationRoleId(destinationRoleId)
            .setPermissionName(permissionName)
            .setPermissionPayload(permissionPayload)
            .build());
  }

  public static InstructionBox mintAsset(final String assetId, final String quantity) {
    return InstructionBox.of(
        MintAssetInstruction.builder().setAssetId(assetId).setQuantity(quantity).build());
  }

  public static InstructionBox mintTriggerRepetitions(final String triggerId, final int repetitions) {
    return InstructionBox.of(
        MintTriggerRepetitionsInstruction.builder()
            .setTriggerId(triggerId)
            .setRepetitions(repetitions)
            .build());
  }

  public static InstructionBox burnAsset(final String assetId, final String quantity) {
    return InstructionBox.of(
        BurnAssetInstruction.builder().setAssetId(assetId).setQuantity(quantity).build());
  }

  public static InstructionBox burnTriggerRepetitions(final String triggerId, final int repetitions) {
    return InstructionBox.of(
        BurnTriggerRepetitionsInstruction.builder()
            .setTriggerId(triggerId)
            .setRepetitions(repetitions)
            .build());
  }

  public static InstructionBox transferAsset(
      final String assetId, final String quantity, final String destinationAccountId) {
    return InstructionBox.of(
        transferAssetTemplate(assetId, quantity, destinationAccountId));
  }

  public static InstructionBox transferDomain(
      final String sourceAccountId, final String domainId, final String destinationAccountId) {
    return InstructionBox.of(
        transferDomainTemplate(sourceAccountId, domainId, destinationAccountId));
  }

  public static InstructionBox transferAssetDefinition(
      final String sourceAccountId,
      final String assetDefinitionId,
      final String destinationAccountId) {
    return InstructionBox.of(
        transferAssetDefinitionTemplate(sourceAccountId, assetDefinitionId, destinationAccountId));
  }

  public static InstructionBox transferNft(
      final String sourceAccountId, final String nftId, final String destinationAccountId) {
    return InstructionBox.of(
        transferNftTemplate(sourceAccountId, nftId, destinationAccountId));
  }

  public static InstructionTemplate transferAssetTemplate(
      final String assetId, final String quantity, final String destinationAccountId) {
    return TransferAssetInstruction.builder()
        .setAssetId(assetId)
        .setQuantity(quantity)
        .setDestinationAccountId(destinationAccountId)
        .build();
  }

  public static InstructionTemplate transferDomainTemplate(
      final String sourceAccountId, final String domainId, final String destinationAccountId) {
    return TransferDomainInstruction.builder()
        .setSourceAccountId(sourceAccountId)
        .setDomainId(domainId)
        .setDestinationAccountId(destinationAccountId)
        .build();
  }

  public static InstructionTemplate transferAssetDefinitionTemplate(
      final String sourceAccountId,
      final String assetDefinitionId,
      final String destinationAccountId) {
    return TransferAssetDefinitionInstruction.builder()
        .setSourceAccountId(sourceAccountId)
        .setAssetDefinitionId(assetDefinitionId)
        .setDestinationAccountId(destinationAccountId)
        .build();
  }

  public static InstructionTemplate transferNftTemplate(
      final String sourceAccountId, final String nftId, final String destinationAccountId) {
    return TransferNftInstruction.builder()
        .setSourceAccountId(sourceAccountId)
        .setNftId(nftId)
        .setDestinationAccountId(destinationAccountId)
        .build();
  }

  public static InstructionBox createKaigi(
      final String domainId,
      final String callName,
      final String hostAccountId,
      final long gasRatePerMinute) {
    return InstructionBox.of(
        createKaigiTemplate(domainId, callName, hostAccountId, gasRatePerMinute));
  }

  public static InstructionBox createKaigi(final CreateKaigiInstruction instruction) {
    return InstructionBox.of(instruction);
  }

  public static CreateKaigiInstruction createKaigiTemplate(
      final String domainId,
      final String callName,
      final String hostAccountId,
      final long gasRatePerMinute) {
    return CreateKaigiInstruction.builder()
        .setCallId(domainId, callName)
        .setHost(hostAccountId)
        .setGasRatePerMinute(gasRatePerMinute)
        .build();
  }

  public static InstructionBox joinKaigi(
      final String domainId, final String callName, final String participantAccountId) {
    return InstructionBox.of(joinKaigiTemplate(domainId, callName, participantAccountId));
  }

  public static JoinKaigiInstruction joinKaigiTemplate(
      final String domainId, final String callName, final String participantAccountId) {
    return JoinKaigiInstruction.builder()
        .setCallId(domainId, callName)
        .setParticipant(participantAccountId)
        .build();
  }

  public static InstructionBox leaveKaigi(
      final String domainId, final String callName, final String participantAccountId) {
    return InstructionBox.of(leaveKaigiTemplate(domainId, callName, participantAccountId));
  }

  public static LeaveKaigiInstruction leaveKaigiTemplate(
      final String domainId, final String callName, final String participantAccountId) {
    return LeaveKaigiInstruction.builder()
        .setCallId(domainId, callName)
        .setParticipant(participantAccountId)
        .build();
  }

  public static InstructionBox endKaigi(final String domainId, final String callName) {
    return InstructionBox.of(endKaigiTemplate(domainId, callName));
  }

  public static InstructionBox endKaigi(
      final String domainId, final String callName, final Long endedAtMs) {
    return InstructionBox.of(endKaigiTemplate(domainId, callName, endedAtMs));
  }

  public static EndKaigiInstruction endKaigiTemplate(
      final String domainId, final String callName) {
    return EndKaigiInstruction.builder().setCallId(domainId, callName).build();
  }

  public static EndKaigiInstruction endKaigiTemplate(
      final String domainId, final String callName, final Long endedAtMs) {
    return EndKaigiInstruction.builder().setCallId(domainId, callName).setEndedAtMs(endedAtMs).build();
  }

  public static InstructionBox recordKaigiUsage(
      final String domainId,
      final String callName,
      final long durationMs,
      final long billedGas) {
    return InstructionBox.of(
        recordKaigiUsageTemplate(domainId, callName, durationMs, billedGas));
  }

  public static RecordKaigiUsageInstruction recordKaigiUsageTemplate(
      final String domainId,
      final String callName,
      final long durationMs,
      final long billedGas) {
    return RecordKaigiUsageInstruction.builder()
        .setCallId(domainId, callName)
        .setDurationMs(durationMs)
        .setBilledGas(billedGas)
        .build();
  }

  public static InstructionBox registerKaigiRelay(
      final String relayId, final String hpkePublicKeyBase64, final int bandwidthClass) {
    return InstructionBox.of(
        registerKaigiRelayTemplate(relayId, hpkePublicKeyBase64, bandwidthClass));
  }

  public static RegisterKaigiRelayInstruction registerKaigiRelayTemplate(
      final String relayId, final String hpkePublicKeyBase64, final int bandwidthClass) {
    return RegisterKaigiRelayInstruction.builder()
        .setRelayId(relayId)
        .setHpkePublicKeyBase64(hpkePublicKeyBase64)
        .setBandwidthClass(bandwidthClass)
        .build();
  }

  public static InstructionBox registerKaigiRelay(
      final String relayId, final byte[] hpkePublicKey, final int bandwidthClass) {
    return InstructionBox.of(
        RegisterKaigiRelayInstruction.builder()
            .setRelayId(relayId)
            .setHpkePublicKey(hpkePublicKey)
            .setBandwidthClass(bandwidthClass)
            .build());
  }

  public static InstructionBox clearKaigiRelayManifest(
      final String domainId, final String callName) {
    return InstructionBox.of(clearKaigiRelayManifestTemplate(domainId, callName));
  }

  public static SetKaigiRelayManifestInstruction clearKaigiRelayManifestTemplate(
      final String domainId, final String callName) {
    return SetKaigiRelayManifestInstruction.builder().setCallId(domainId, callName).build();
  }

  public static InstructionBox setKaigiRelayManifest(
      final SetKaigiRelayManifestInstruction instruction) {
    return InstructionBox.of(instruction);
  }

  public static InstructionBox log(final String level, final String message) {
    return InstructionBox.of(LogInstruction.builder().setLevel(level).setMessage(message).build());
  }

  public static InstructionBox upgradeExecutor(final byte[] bytecode) {
    return InstructionBox.of(upgradeExecutorTemplate(bytecode));
  }

  public static InstructionBox custom(final InstructionKind kind, final Map<String, String> arguments) {
    return InstructionBox.builder().setKind(kind).setArguments(arguments).build();
  }

  public static InstructionBox custom(final String name, final Map<String, String> arguments) {
    final Map<String, String> copy = new LinkedHashMap<>(arguments);
    copy.putIfAbsent("action", name);
    return InstructionBox.builder().setName(name).setArguments(copy).build();
  }

  public static InstructionTemplate registerDomainTemplate(final String domain) {
    return RegisterDomainInstruction.builder().setDomainName(domain).build();
  }

  public static InstructionTemplate registerAccountTemplate(final String account) {
    return RegisterAccountInstruction.builder().setAccountId(account).build();
  }

  public static InstructionTemplate registerAssetDefinitionTemplate(final String assetDefinitionId) {
    return RegisterAssetDefinitionInstruction.builder().setAssetDefinitionId(assetDefinitionId).build();
  }

  public static InstructionTemplate registerPeerWithPopTemplate(
      final String peerPublicKey, final byte[] proofOfPossession) {
    return RegisterPeerWithPopInstruction.builder()
        .setPeerPublicKey(peerPublicKey)
        .setProofOfPossession(proofOfPossession)
        .build();
  }

  public static InstructionTemplate registerRoleTemplate(
      final String roleId, final String ownerAccountId, final List<String> permissions) {
    return RegisterRoleInstruction.builder()
        .setRoleId(roleId)
        .setOwnerAccountId(ownerAccountId)
        .setPermissions(permissions)
        .build();
  }

  public static InstructionTemplate registerNftTemplate(
      final String nftId, final Map<String, String> metadata) {
    return RegisterNftInstruction.builder()
        .setNftId(nftId)
        .setMetadata(metadata)
        .build();
  }

  public static InstructionTemplate registerPipelineTriggerTemplate(
      final String triggerId,
      final String authority,
      final List<InstructionBox> instructions,
      final RegisterPipelineTriggerInstruction.PipelineFilter filter) {
    return registerPipelineTriggerTemplate(triggerId, authority, instructions, filter, null, null);
  }

  public static InstructionTemplate registerPipelineTriggerTemplate(
      final String triggerId,
      final String authority,
      final List<InstructionBox> instructions,
      final RegisterPipelineTriggerInstruction.PipelineFilter filter,
      final Integer repeats,
      final Map<String, String> metadata) {
    final RegisterPipelineTriggerInstruction.Builder builder =
        RegisterPipelineTriggerInstruction.builder()
            .setTriggerId(triggerId)
            .setAuthority(authority)
            .setFilter(filter)
            .setInstructions(instructions);
    if (repeats != null) {
      builder.setRepeats(repeats);
    }
    if (metadata != null) {
      builder.setMetadata(metadata);
    }
    return builder.build();
  }

  public static InstructionTemplate registerTimeTriggerTemplate(
      final String triggerId,
      final String authority,
      final List<InstructionBox> instructions,
      final long startMs) {
    return registerTimeTriggerTemplate(triggerId, authority, instructions, startMs, null, null, null);
  }

  public static InstructionTemplate registerTimeTriggerTemplate(
      final String triggerId,
      final String authority,
      final List<InstructionBox> instructions,
      final long startMs,
      final Long periodMs,
      final Integer repeats,
      final Map<String, String> metadata) {
    final RegisterTimeTriggerInstruction.Builder builder =
        RegisterTimeTriggerInstruction.builder()
            .setTriggerId(triggerId)
            .setAuthority(authority)
            .setStartMs(startMs)
            .setInstructions(instructions);
    if (periodMs != null) {
      builder.setPeriodMs(periodMs);
    }
    if (repeats != null) {
      builder.setRepeats(repeats);
    }
    if (metadata != null) {
      builder.setMetadata(metadata);
    }
    return builder.build();
  }

  public static InstructionTemplate registerPrecommitTriggerTemplate(
      final String triggerId,
      final String authority,
      final List<InstructionBox> instructions) {
    return registerPrecommitTriggerTemplate(triggerId, authority, instructions, null, null);
  }

  public static InstructionTemplate registerPrecommitTriggerTemplate(
      final String triggerId,
      final String authority,
      final List<InstructionBox> instructions,
      final Integer repeats,
      final Map<String, String> metadata) {
    final RegisterPrecommitTriggerInstruction.Builder builder =
        RegisterPrecommitTriggerInstruction.builder()
            .setTriggerId(triggerId)
            .setAuthority(authority)
            .setInstructions(instructions);
    if (repeats != null) {
      builder.setRepeats(repeats);
    }
    if (metadata != null) {
      builder.setMetadata(metadata);
    }
    return builder.build();
  }

  public static InstructionTemplate executeTriggerTemplate(final String triggerId) {
    return executeTriggerTemplate(triggerId, null);
  }

  public static InstructionTemplate executeTriggerTemplate(
      final String triggerId, final String authorityOverride) {
    return ExecuteTriggerInstruction.builder()
        .setTriggerId(triggerId)
        .setAuthorityOverride(authorityOverride)
        .build();
  }

  public static InstructionTemplate upgradeExecutorTemplate(final byte[] bytecode) {
    return UpgradeInstruction.builder().setBytecode(bytecode).build();
  }

  public static InstructionTemplate unregisterPeerTemplate(final String peerId) {
    return UnregisterInstruction.builder().setPeerId(peerId).build();
  }

  public static InstructionTemplate unregisterDomainTemplate(final String domainId) {
    return UnregisterInstruction.builder().setDomainId(domainId).build();
  }

  public static InstructionTemplate unregisterAccountTemplate(final String accountId) {
    return UnregisterInstruction.builder().setAccountId(accountId).build();
  }

  public static InstructionTemplate unregisterAssetDefinitionTemplate(final String assetDefinitionId) {
    return UnregisterInstruction.builder().setAssetDefinitionId(assetDefinitionId).build();
  }

  public static InstructionTemplate unregisterNftTemplate(final String nftId) {
    return UnregisterInstruction.builder().setNftId(nftId).build();
  }

  public static InstructionTemplate unregisterRoleTemplate(final String roleId) {
    return UnregisterInstruction.builder().setRoleId(roleId).build();
  }

  public static InstructionTemplate unregisterTriggerTemplate(final String triggerId) {
    return UnregisterInstruction.builder().setTriggerId(triggerId).build();
  }

  public static InstructionBox setDomainKeyValue(
      final String domainId, final String key, final String value) {
    return InstructionBox.of(setDomainKeyValueTemplate(domainId, key, value));
  }

  public static InstructionBox setAccountKeyValue(
      final String accountId, final String key, final String value) {
    return InstructionBox.of(setAccountKeyValueTemplate(accountId, key, value));
  }

  public static InstructionBox setAssetDefinitionKeyValue(
      final String assetDefinitionId, final String key, final String value) {
    return InstructionBox.of(
        setAssetDefinitionKeyValueTemplate(assetDefinitionId, key, value));
  }

  public static InstructionBox setAssetKeyValue(
      final String assetId, final String key, final String value) {
    return InstructionBox.of(setAssetKeyValueTemplate(assetId, key, value));
  }

  public static InstructionBox setNftKeyValue(
      final String nftId, final String key, final String value) {
    return InstructionBox.of(setNftKeyValueTemplate(nftId, key, value));
  }

  public static InstructionBox setTriggerKeyValue(
      final String triggerId, final String key, final String value) {
    return InstructionBox.of(setTriggerKeyValueTemplate(triggerId, key, value));
  }

  public static InstructionBox removeDomainKeyValue(final String domainId, final String key) {
    return InstructionBox.of(removeDomainKeyValueTemplate(domainId, key));
  }

  public static InstructionBox removeAccountKeyValue(final String accountId, final String key) {
    return InstructionBox.of(removeAccountKeyValueTemplate(accountId, key));
  }

  public static InstructionBox removeAssetDefinitionKeyValue(
      final String assetDefinitionId, final String key) {
    return InstructionBox.of(removeAssetDefinitionKeyValueTemplate(assetDefinitionId, key));
  }

  public static InstructionBox removeAssetKeyValue(final String assetId, final String key) {
    return InstructionBox.of(removeAssetKeyValueTemplate(assetId, key));
  }

  public static InstructionBox setParameter(final String parameterJson) {
    return InstructionBox.of(setParameterTemplate(parameterJson));
  }

  public static InstructionBox removeNftKeyValue(final String nftId, final String key) {
    return InstructionBox.of(removeNftKeyValueTemplate(nftId, key));
  }

  public static InstructionBox removeTriggerKeyValue(final String triggerId, final String key) {
    return InstructionBox.of(removeTriggerKeyValueTemplate(triggerId, key));
  }

  public static SetKeyValueInstruction setDomainKeyValueTemplate(
      final String domainId, final String key, final String value) {
    return SetKeyValueInstruction.builder()
        .setDomainId(domainId)
        .setKey(key)
        .setValue(value)
        .build();
  }

  public static SetKeyValueInstruction setAccountKeyValueTemplate(
      final String accountId, final String key, final String value) {
    return SetKeyValueInstruction.builder()
        .setAccountId(accountId)
        .setKey(key)
        .setValue(value)
        .build();
  }

  public static SetKeyValueInstruction setAssetDefinitionKeyValueTemplate(
      final String assetDefinitionId, final String key, final String value) {
    return SetKeyValueInstruction.builder()
        .setAssetDefinitionId(assetDefinitionId)
        .setKey(key)
        .setValue(value)
        .build();
  }

  public static SetAssetKeyValueInstruction setAssetKeyValueTemplate(
      final String assetId, final String key, final String value) {
    return SetAssetKeyValueInstruction.builder()
        .setAssetId(assetId)
        .setKey(key)
        .setValue(value)
        .build();
  }

  public static SetKeyValueInstruction setNftKeyValueTemplate(
      final String nftId, final String key, final String value) {
    return SetKeyValueInstruction.builder()
        .setNftId(nftId)
        .setKey(key)
        .setValue(value)
        .build();
  }

  public static SetKeyValueInstruction setTriggerKeyValueTemplate(
      final String triggerId, final String key, final String value) {
    return SetKeyValueInstruction.builder()
        .setTriggerId(triggerId)
        .setKey(key)
        .setValue(value)
        .build();
  }

  public static RemoveKeyValueInstruction removeDomainKeyValueTemplate(
      final String domainId, final String key) {
    return RemoveKeyValueInstruction.builder().setDomainId(domainId).setKey(key).build();
  }

  public static RemoveKeyValueInstruction removeAccountKeyValueTemplate(
      final String accountId, final String key) {
    return RemoveKeyValueInstruction.builder().setAccountId(accountId).setKey(key).build();
  }

  public static RemoveKeyValueInstruction removeAssetDefinitionKeyValueTemplate(
      final String assetDefinitionId, final String key) {
    return RemoveKeyValueInstruction.builder()
        .setAssetDefinitionId(assetDefinitionId)
        .setKey(key)
        .build();
  }

  public static RemoveAssetKeyValueInstruction removeAssetKeyValueTemplate(
      final String assetId, final String key) {
    return RemoveAssetKeyValueInstruction.builder().setAssetId(assetId).setKey(key).build();
  }

  public static RemoveKeyValueInstruction removeNftKeyValueTemplate(
      final String nftId, final String key) {
    return RemoveKeyValueInstruction.builder().setNftId(nftId).setKey(key).build();
  }

  public static RemoveKeyValueInstruction removeTriggerKeyValueTemplate(
      final String triggerId, final String key) {
    return RemoveKeyValueInstruction.builder().setTriggerId(triggerId).setKey(key).build();
  }

  public static SetParameterInstruction setParameterTemplate(final String parameterJson) {
    return SetParameterInstruction.builder().setParameterJson(parameterJson).build();
  }
}
