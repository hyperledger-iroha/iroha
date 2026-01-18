package org.hyperledger.iroha.android.model;

import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.instructions.ApprovePinManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.BindManifestAliasInstruction;
import org.hyperledger.iroha.android.model.instructions.BurnAssetInstruction;
import org.hyperledger.iroha.android.model.instructions.BurnTriggerRepetitionsInstruction;
import org.hyperledger.iroha.android.model.instructions.CastPlainBallotInstruction;
import org.hyperledger.iroha.android.model.instructions.CastZkBallotInstruction;
import org.hyperledger.iroha.android.model.instructions.CreateKaigiInstruction;
import org.hyperledger.iroha.android.model.instructions.EnactReferendumInstruction;
import org.hyperledger.iroha.android.model.instructions.EndKaigiInstruction;
import org.hyperledger.iroha.android.model.instructions.CompleteReplicationOrderInstruction;
import org.hyperledger.iroha.android.model.instructions.ExpireSpaceDirectoryManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.ExecuteTriggerInstruction;
import org.hyperledger.iroha.android.model.instructions.ActivateRuntimeUpgradeInstruction;
import org.hyperledger.iroha.android.model.instructions.FinalizeReferendumInstruction;
import org.hyperledger.iroha.android.model.instructions.IssueReplicationOrderInstruction;
import org.hyperledger.iroha.android.model.instructions.GrantPermissionInstruction;
import org.hyperledger.iroha.android.model.instructions.GrantRoleInstruction;
import org.hyperledger.iroha.android.model.instructions.GrantRolePermissionInstruction;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
import org.hyperledger.iroha.android.model.instructions.JoinKaigiInstruction;
import org.hyperledger.iroha.android.model.instructions.LeaveKaigiInstruction;
import org.hyperledger.iroha.android.model.instructions.LogInstruction;
import org.hyperledger.iroha.android.model.instructions.MintAssetInstruction;
import org.hyperledger.iroha.android.model.instructions.MintTriggerRepetitionsInstruction;
import org.hyperledger.iroha.android.model.instructions.PersistCouncilForEpochInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterAccountInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterAssetDefinitionInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterCapacityDeclarationInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterCapacityDisputeInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterDomainInstruction;
import org.hyperledger.iroha.android.model.instructions.CancelRuntimeUpgradeInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterKaigiRelayInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterOfflineAllowanceInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterPinManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterNftInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterPeerWithPopInstruction;
import org.hyperledger.iroha.android.model.instructions.MultisigRegisterInstruction;
import org.hyperledger.iroha.android.model.instructions.RecordCapacityTelemetryInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterPipelineTriggerInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterPrecommitTriggerInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterRoleInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterTimeTriggerInstruction;
import org.hyperledger.iroha.android.model.instructions.RecordKaigiUsageInstruction;
import org.hyperledger.iroha.android.model.instructions.RecordReplicationReceiptInstruction;
import org.hyperledger.iroha.android.model.instructions.RetirePinManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.RevokeSpaceDirectoryManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.RevokePermissionInstruction;
import org.hyperledger.iroha.android.model.instructions.RevokeRoleInstruction;
import org.hyperledger.iroha.android.model.instructions.RevokeRolePermissionInstruction;
import org.hyperledger.iroha.android.model.instructions.PublishSpaceDirectoryManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.TransferNftInstruction;
import org.hyperledger.iroha.android.model.instructions.RemoveAssetKeyValueInstruction;
import org.hyperledger.iroha.android.model.instructions.RemoveKeyValueInstruction;
import org.hyperledger.iroha.android.model.instructions.SetParameterInstruction;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction;
import org.hyperledger.iroha.android.model.instructions.TransferAssetDefinitionInstruction;
import org.hyperledger.iroha.android.model.instructions.TransferAssetInstruction;
import org.hyperledger.iroha.android.model.instructions.TransferDomainInstruction;
import org.hyperledger.iroha.android.model.instructions.SetAssetKeyValueInstruction;
import org.hyperledger.iroha.android.model.instructions.SetKeyValueInstruction;
import org.hyperledger.iroha.android.model.instructions.SetKaigiRelayManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.UnregisterInstruction;
import org.hyperledger.iroha.android.model.instructions.UpgradeInstruction;
import org.hyperledger.iroha.android.model.instructions.ProposeRuntimeUpgradeInstruction;
import org.hyperledger.iroha.android.model.instructions.ProposeDeployContractInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterVerifyingKeyInstruction;
import org.hyperledger.iroha.android.model.instructions.UpdateVerifyingKeyInstruction;
import org.hyperledger.iroha.android.model.instructions.UpsertProviderCreditInstruction;

/**
 * Typed representation of an instruction scheduled for execution within a transaction.
 *
 * <p>The container wraps strongly typed instruction payloads where available (for example, register
 * domain/account/asset definition builders) and falls back to an opaque key/value payload when the
 * instruction has no dedicated Java binding yet. Wire-framed instruction payloads can also be
 * attached directly to preserve Norito parity when raw instruction bytes are available.
 */
public final class InstructionBox {

  private final InstructionPayload payload;

  private InstructionBox(final InstructionPayload payload) {
    this.payload = Objects.requireNonNull(payload, "payload");
  }

  /** Instruction display name (matches `InstructionType` tag). */
  public String name() {
    if (payload instanceof WirePayload wire) {
      return wire.wireName();
    }
    return kind().displayName();
  }

  public InstructionKind kind() {
    return payload.kind();
  }

  public InstructionPayload payload() {
    return payload;
  }

  /**
   * Returns Norito-ready arguments describing the instruction. The returned map is immutable and
   * safe to cache by callers.
   */
  public Map<String, String> arguments() {
    return payload.toArguments();
  }

  public Builder toBuilder() {
    return builder().setKind(kind()).setArguments(arguments());
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof InstructionBox other)) {
      return false;
    }
    return kind() == other.kind() && arguments().equals(other.arguments());
  }

  @Override
  public int hashCode() {
    return Objects.hash(kind(), arguments());
  }

  public static InstructionBox of(final InstructionPayload payload) {
    return new InstructionBox(payload);
  }

  /**
   * Builds an {@link InstructionBox} from a canonical wire identifier and its Norito-framed payload.
   *
   * <p>The payload must include the Norito header with a valid checksum.
   */
  public static InstructionBox fromWirePayload(final String wireName, final byte[] payloadBytes) {
    return new InstructionBox(new WireInstructionPayload(wireName, payloadBytes));
  }

  /**
   * Builds an {@link InstructionBox} from Norito decoded components, attempting to hydrate typed
   * payload implementations when a matching builder exists.
   */
  public static InstructionBox fromNorito(
      final InstructionKind kind, final Map<String, String> arguments) {
    return new InstructionBox(resolvePayload(kind, arguments));
  }

  public static Builder builder() {
    return new Builder();
  }

  private static InstructionPayload resolvePayload(
      final InstructionKind kind, final Map<String, String> arguments) {
    Objects.requireNonNull(kind, "kind");
    Objects.requireNonNull(arguments, "arguments");
    if (kind == InstructionKind.REGISTER) {
      final String action = arguments.get("action");
      if ("RegisterDomain".equals(action)) {
        return RegisterDomainInstruction.fromArguments(arguments);
      }
      if ("RegisterAccount".equals(action)) {
        return RegisterAccountInstruction.fromArguments(arguments);
      }
      if ("RegisterAssetDefinition".equals(action)) {
        return RegisterAssetDefinitionInstruction.fromArguments(arguments);
      }
      if ("RegisterPeerWithPop".equals(action)) {
        return RegisterPeerWithPopInstruction.fromArguments(arguments);
      }
      if ("RegisterRole".equals(action)) {
        return RegisterRoleInstruction.fromArguments(arguments);
      }
      if ("RegisterNft".equals(action)) {
        return RegisterNftInstruction.fromArguments(arguments);
      }
      if (RegisterPinManifestInstruction.ACTION.equals(action)) {
        return RegisterPinManifestInstruction.fromArguments(arguments);
      }
      if (RegisterCapacityDeclarationInstruction.ACTION.equals(action)) {
        return RegisterCapacityDeclarationInstruction.fromArguments(arguments);
      }
      if (RegisterTimeTriggerInstruction.ACTION.equals(action)) {
        return RegisterTimeTriggerInstruction.fromArguments(arguments);
      }
      if (RegisterPrecommitTriggerInstruction.ACTION.equals(action)) {
        return RegisterPrecommitTriggerInstruction.fromArguments(arguments);
      }
      if (RegisterPipelineTriggerInstruction.ACTION.equals(action)) {
        return RegisterPipelineTriggerInstruction.fromArguments(arguments);
      }
    }
    if (kind == InstructionKind.TRANSFER) {
      final String action = arguments.get("action");
      if ("TransferAsset".equals(action)) {
        return TransferAssetInstruction.fromArguments(arguments);
      }
      if ("TransferDomain".equals(action)) {
        return TransferDomainInstruction.fromArguments(arguments);
      }
      if ("TransferAssetDefinition".equals(action)) {
        return TransferAssetDefinitionInstruction.fromArguments(arguments);
      }
      if ("TransferNft".equals(action)) {
        return TransferNftInstruction.fromArguments(arguments);
      }
    }
    if (kind == InstructionKind.UNREGISTER) {
      return UnregisterInstruction.fromArguments(arguments);
    }
    if (kind == InstructionKind.MINT) {
      final String action = arguments.get("action");
      if ("MintAsset".equals(action)) {
        return MintAssetInstruction.fromArguments(arguments);
      }
      if ("MintTriggerRepetitions".equals(action)) {
        return MintTriggerRepetitionsInstruction.fromArguments(arguments);
      }
    }
    if (kind == InstructionKind.BURN) {
      final String action = arguments.get("action");
      if ("BurnAsset".equals(action)) {
        return BurnAssetInstruction.fromArguments(arguments);
      }
      if ("BurnTriggerRepetitions".equals(action)) {
        return BurnTriggerRepetitionsInstruction.fromArguments(arguments);
      }
    }
    if (kind == InstructionKind.SET_KEY_VALUE) {
      final String action = arguments.get("action");
      if ("SetAssetKeyValue".equals(action)) {
        return SetAssetKeyValueInstruction.fromArguments(arguments);
      }
      try {
        return SetKeyValueInstruction.fromArguments(arguments);
      } catch (final IllegalArgumentException ignored) {
        // fall back to custom payload below.
      }
    }
    if (kind == InstructionKind.SET_PARAMETER) {
      final String action = arguments.get("action");
      if ("SetParameter".equals(action)) {
        try {
          return SetParameterInstruction.fromArguments(arguments);
        } catch (final IllegalArgumentException ignored) {
          // fall back to custom payload below.
        }
      }
    }
    if (kind == InstructionKind.REMOVE_KEY_VALUE) {
      final String action = arguments.get("action");
      if ("RemoveAssetKeyValue".equals(action)) {
        return RemoveAssetKeyValueInstruction.fromArguments(arguments);
      }
      try {
        return RemoveKeyValueInstruction.fromArguments(arguments);
      } catch (final IllegalArgumentException ignored) {
        // fall back to custom payload below.
      }
    }
    if (kind == InstructionKind.GRANT) {
      final String action = arguments.get("action");
      if ("GrantPermission".equals(action)) {
        return GrantPermissionInstruction.fromArguments(arguments);
      }
      if ("GrantRole".equals(action)) {
        return GrantRoleInstruction.fromArguments(arguments);
      }
      if ("GrantRolePermission".equals(action)) {
        return GrantRolePermissionInstruction.fromArguments(arguments);
      }
    }
    if (kind == InstructionKind.REVOKE) {
      final String action = arguments.get("action");
      if ("RevokePermission".equals(action)) {
        return RevokePermissionInstruction.fromArguments(arguments);
      }
      if ("RevokeRole".equals(action)) {
        return RevokeRoleInstruction.fromArguments(arguments);
      }
      if ("RevokeRolePermission".equals(action)) {
        return RevokeRolePermissionInstruction.fromArguments(arguments);
      }
    }
    if (kind == InstructionKind.LOG) {
      return LogInstruction.fromArguments(arguments);
    }
    if (kind == InstructionKind.EXECUTE_TRIGGER) {
      return ExecuteTriggerInstruction.fromArguments(arguments);
    }
    if (kind == InstructionKind.UPGRADE) {
      return UpgradeInstruction.fromArguments(arguments);
    }
    if (kind == InstructionKind.CUSTOM) {
      final String action = arguments.get("action");
      if ("ProposeDeployContract".equals(action)) {
        return ProposeDeployContractInstruction.fromArguments(arguments);
      }
      if ("CastZkBallot".equals(action)) {
        return CastZkBallotInstruction.fromArguments(arguments);
      }
      if ("CastPlainBallot".equals(action)) {
        return CastPlainBallotInstruction.fromArguments(arguments);
      }
      if ("EnactReferendum".equals(action)) {
        return EnactReferendumInstruction.fromArguments(arguments);
      }
      if ("FinalizeReferendum".equals(action)) {
        return FinalizeReferendumInstruction.fromArguments(arguments);
      }
      if ("PersistCouncilForEpoch".equals(action)) {
        return PersistCouncilForEpochInstruction.fromArguments(arguments);
      }
      if (ProposeRuntimeUpgradeInstruction.ACTION.equals(action)) {
        return ProposeRuntimeUpgradeInstruction.fromArguments(arguments);
      }
      if (ActivateRuntimeUpgradeInstruction.ACTION.equals(action)) {
        return ActivateRuntimeUpgradeInstruction.fromArguments(arguments);
      }
      if (CancelRuntimeUpgradeInstruction.ACTION.equals(action)) {
        return CancelRuntimeUpgradeInstruction.fromArguments(arguments);
      }
      if ("CreateKaigi".equals(action)) {
        return CreateKaigiInstruction.fromArguments(arguments);
      }
      if ("JoinKaigi".equals(action)) {
        return JoinKaigiInstruction.fromArguments(arguments);
      }
      if ("LeaveKaigi".equals(action)) {
        return LeaveKaigiInstruction.fromArguments(arguments);
      }
      if ("EndKaigi".equals(action)) {
        return EndKaigiInstruction.fromArguments(arguments);
      }
      if ("RecordKaigiUsage".equals(action)) {
        return RecordKaigiUsageInstruction.fromArguments(arguments);
      }
      if ("SetKaigiRelayManifest".equals(action)) {
        return SetKaigiRelayManifestInstruction.fromArguments(arguments);
      }
      if ("RegisterKaigiRelay".equals(action)) {
        return RegisterKaigiRelayInstruction.fromArguments(arguments);
      }
      if ("RegisterVerifyingKey".equals(action)) {
        return RegisterVerifyingKeyInstruction.fromArguments(arguments);
      }
      if ("UpdateVerifyingKey".equals(action)) {
        return UpdateVerifyingKeyInstruction.fromArguments(arguments);
      }
      if ("IssueReplicationOrder".equals(action)) {
        return IssueReplicationOrderInstruction.fromArguments(arguments);
      }
      if ("CompleteReplicationOrder".equals(action)) {
        return CompleteReplicationOrderInstruction.fromArguments(arguments);
      }
      if ("RecordReplicationReceipt".equals(action)) {
        return RecordReplicationReceiptInstruction.fromArguments(arguments);
      }
      if ("RecordCapacityTelemetry".equals(action)) {
        return RecordCapacityTelemetryInstruction.fromArguments(arguments);
      }
      if (RegisterCapacityDisputeInstruction.ACTION.equals(action)) {
        return RegisterCapacityDisputeInstruction.fromArguments(arguments);
      }
      if (SetPricingScheduleInstruction.ACTION.equals(action)) {
        return SetPricingScheduleInstruction.fromArguments(arguments);
      }
      if (UpsertProviderCreditInstruction.ACTION.equals(action)) {
        return UpsertProviderCreditInstruction.fromArguments(arguments);
      }
      if ("PublishSpaceDirectoryManifest".equals(action)) {
        return PublishSpaceDirectoryManifestInstruction.fromArguments(arguments);
      }
      if ("RevokeSpaceDirectoryManifest".equals(action)) {
        return RevokeSpaceDirectoryManifestInstruction.fromArguments(arguments);
      }
      if ("ExpireSpaceDirectoryManifest".equals(action)) {
        return ExpireSpaceDirectoryManifestInstruction.fromArguments(arguments);
      }
      if (ApprovePinManifestInstruction.ACTION.equals(action)) {
        return ApprovePinManifestInstruction.fromArguments(arguments);
      }
      if (RetirePinManifestInstruction.ACTION.equals(action)) {
        return RetirePinManifestInstruction.fromArguments(arguments);
      }
      if (BindManifestAliasInstruction.ACTION.equals(action)) {
        return BindManifestAliasInstruction.fromArguments(arguments);
      }
      if (MultisigRegisterInstruction.ACTION.equals(action)) {
        return MultisigRegisterInstruction.fromArguments(arguments);
      }
      if (RegisterOfflineAllowanceInstruction.ACTION.equals(action)) {
        return RegisterOfflineAllowanceInstruction.fromArguments(arguments);
      }
    }
    return new CustomInstructionPayload(kind, arguments);
  }

  private static InstructionKind wireKindForName(final String wireName) {
    if (wireName == null) {
      return InstructionKind.CUSTOM;
    }
    final String normalized = wireName.toLowerCase(Locale.ROOT);
    if (normalized.startsWith("iroha.register")) {
      return InstructionKind.REGISTER;
    }
    if (normalized.startsWith("iroha.unregister")) {
      return InstructionKind.UNREGISTER;
    }
    if (normalized.startsWith("iroha.transfer")) {
      return InstructionKind.TRANSFER;
    }
    if (normalized.startsWith("iroha.mint")) {
      return InstructionKind.MINT;
    }
    if (normalized.startsWith("iroha.burn")) {
      return InstructionKind.BURN;
    }
    if (normalized.startsWith("iroha.grant")) {
      return InstructionKind.GRANT;
    }
    if (normalized.startsWith("iroha.revoke")) {
      return InstructionKind.REVOKE;
    }
    if (normalized.startsWith("iroha.set_key_value")) {
      return InstructionKind.SET_KEY_VALUE;
    }
    if (normalized.startsWith("iroha.remove_key_value")) {
      return InstructionKind.REMOVE_KEY_VALUE;
    }
    if (normalized.startsWith("iroha.set_parameter")) {
      return InstructionKind.SET_PARAMETER;
    }
    if (normalized.startsWith("iroha.execute_trigger")) {
      return InstructionKind.EXECUTE_TRIGGER;
    }
    if (normalized.startsWith("iroha.log")) {
      return InstructionKind.LOG;
    }
    if (normalized.startsWith("iroha.runtime_upgrade") || normalized.startsWith("iroha.upgrade")) {
      return InstructionKind.UPGRADE;
    }
    return InstructionKind.CUSTOM;
  }

  private static final class CustomInstructionPayload implements InstructionPayload {

    private final InstructionKind kind;
    private final Map<String, String> arguments;

    private CustomInstructionPayload(
        final InstructionKind kind, final Map<String, String> arguments) {
      this.kind = Objects.requireNonNull(kind, "kind");
      this.arguments =
          Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(arguments, "arguments")));
    }

    @Override
    public InstructionKind kind() {
      return kind;
    }

    @Override
    public Map<String, String> toArguments() {
      return arguments;
    }
  }

  public interface InstructionPayload {
    InstructionKind kind();

    Map<String, String> toArguments();
  }

  public interface WirePayload extends InstructionPayload {
    String wireName();

    byte[] payloadBytes();
  }

  private static final class WireInstructionPayload implements WirePayload {
    private static final String ARG_WIRE_NAME = "wire_name";
    private static final String ARG_PAYLOAD_BASE64 = "payload_base64";

    private final InstructionKind kind;
    private final String wireName;
    private final byte[] payloadBytes;
    private final Map<String, String> arguments;

    private WireInstructionPayload(final String wireName, final byte[] payloadBytes) {
      if (wireName == null || wireName.isBlank()) {
        throw new IllegalArgumentException("wireName must not be blank");
      }
      if (payloadBytes == null || payloadBytes.length == 0) {
        throw new IllegalArgumentException("payloadBytes must not be empty");
      }
      this.wireName = wireName;
      this.payloadBytes = payloadBytes.clone();
      this.kind = wireKindForName(wireName);
      final LinkedHashMap<String, String> args = new LinkedHashMap<>();
      args.put(ARG_WIRE_NAME, wireName);
      args.put(ARG_PAYLOAD_BASE64, Base64.getEncoder().encodeToString(this.payloadBytes));
      this.arguments = Collections.unmodifiableMap(args);
    }

    @Override
    public InstructionKind kind() {
      return kind;
    }

    @Override
    public Map<String, String> toArguments() {
      return arguments;
    }

    @Override
    public String wireName() {
      return wireName;
    }

    @Override
    public byte[] payloadBytes() {
      return payloadBytes.clone();
    }
  }

  public static final class Builder {
    private InstructionKind kind = InstructionKind.CUSTOM;
    private final Map<String, String> arguments = new LinkedHashMap<>();

    public Builder setKind(final InstructionKind kind) {
      this.kind = Objects.requireNonNull(kind, "kind");
      return this;
    }

    public Builder setName(final String name) {
      Objects.requireNonNull(name, "name");
      try {
        this.kind = InstructionKind.fromDisplayName(name);
      } catch (final IllegalArgumentException ignored) {
        this.kind = InstructionKind.CUSTOM;
      }
      arguments.putIfAbsent("action", name);
      return this;
    }

    public Builder putArgument(final String key, final String value) {
      arguments.put(Objects.requireNonNull(key, "key"), Objects.requireNonNull(value, "value"));
      return this;
    }

    public Builder setArguments(final Map<String, String> arguments) {
      this.arguments.clear();
      if (arguments != null) {
        arguments.forEach(this::putArgument);
      }
      return this;
    }

    public InstructionBox build() {
      return InstructionBox.fromNorito(kind, arguments);
    }
  }
}
