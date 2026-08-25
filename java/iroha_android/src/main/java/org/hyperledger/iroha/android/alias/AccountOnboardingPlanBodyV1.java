package org.hyperledger.iroha.android.alias;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.model.NetworkId;

/** Canonical body signed by a stateless sponsored-onboarding receipt. */
public final class AccountOnboardingPlanBodyV1 extends AliasJsonValue {
  /** Current receipt body layout. */
  public static final int VERSION = 1;

  private final int version;
  private final AccountOnboardingPlanRequestV1 request;
  private final String authority;
  private final NetworkId networkId;
  private final AliasSetupModels.AliasPlanAnchorV1 anchor;
  private final AliasSetupModels.AliasPlanResourceV1 resource;
  private final AliasSetupModels.AliasLeaseAcquisitionV1 acquisition;
  private final AliasQuoteGuardV1 quoteGuard;
  private final List<AliasSetupModels.AliasFramedInstructionV1> instructions;
  private final AliasSetupModels.AliasFramedInstructionV1 ownerAutoRenewInstruction;
  private final long validUntilMs;

  /** Constructs one exact canonical receipt body. */
  public AccountOnboardingPlanBodyV1(
      final int version,
      final AccountOnboardingPlanRequestV1 request,
      final String authority,
      final NetworkId networkId,
      final AliasSetupModels.AliasPlanAnchorV1 anchor,
      final AliasSetupModels.AliasPlanResourceV1 resource,
      final AliasSetupModels.AliasLeaseAcquisitionV1 acquisition,
      final AliasQuoteGuardV1 quoteGuard,
      final List<AliasSetupModels.AliasFramedInstructionV1> instructions,
      final AliasSetupModels.AliasFramedInstructionV1 ownerAutoRenewInstruction,
      final long validUntilMs) {
    if (version != VERSION) throw new IllegalArgumentException("version must be " + VERSION);
    if (request == null
        || anchor == null
        || resource == null
        || acquisition == null
        || quoteGuard == null
        || instructions == null
        || instructions.contains(null)) {
      throw new IllegalArgumentException("onboarding receipt body fields must not be null");
    }
    this.version = version;
    this.request = request;
    this.authority = AccountIdLiteral.requireCanonicalI105Address(authority, "authority");
    this.networkId = java.util.Objects.requireNonNull(networkId, "networkId");
    this.anchor = anchor;
    this.resource = resource;
    this.acquisition = acquisition;
    this.quoteGuard = quoteGuard;
    this.instructions = Collections.unmodifiableList(new ArrayList<>(instructions));
    this.ownerAutoRenewInstruction = ownerAutoRenewInstruction;
    this.validUntilMs = AliasNameSupport.requireNonNegative(validUntilMs, "validUntilMs");
  }

  public int version() { return version; }
  public AccountOnboardingPlanRequestV1 request() { return request; }
  public String authority() { return authority; }
  public NetworkId networkId() { return networkId; }
  public AliasSetupModels.AliasPlanAnchorV1 anchor() { return anchor; }
  public AliasSetupModels.AliasPlanResourceV1 resource() { return resource; }
  public AliasSetupModels.AliasLeaseAcquisitionV1 acquisition() { return acquisition; }
  public AliasQuoteGuardV1 quoteGuard() { return quoteGuard; }
  public List<AliasSetupModels.AliasFramedInstructionV1> instructions() { return instructions; }
  public AliasSetupModels.AliasFramedInstructionV1 ownerAutoRenewInstruction() {
    return ownerAutoRenewInstruction;
  }
  public long validUntilMs() { return validUntilMs; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("version", version);
    map.put("request", request.toJsonMap());
    map.put("authority", authority);
    map.put("network_id", networkId.noritoJsonLiteral());
    map.put("anchor", anchor.toJsonMap());
    map.put("resource", resource.toJsonMap());
    map.put("acquisition", acquisition.toJsonMap());
    map.put("quote_guard", quoteGuard.toJsonMap());
    final List<Map<String, Object>> frames = new ArrayList<>(instructions.size());
    for (final AliasSetupModels.AliasFramedInstructionV1 frame : instructions) {
      frames.add(frame.toJsonMap());
    }
    map.put("instructions", frames);
    map.put(
        "owner_auto_renew_instruction",
        ownerAutoRenewInstruction == null ? null : ownerAutoRenewInstruction.toJsonMap());
    map.put("valid_until_ms", validUntilMs);
    return map;
  }
}
