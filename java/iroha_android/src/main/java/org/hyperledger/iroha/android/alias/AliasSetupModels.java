package org.hyperledger.iroha.android.alias;

import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.model.NetworkId;

/** Supporting immutable DTOs shared by alias setup instructions and transaction plans. */
public final class AliasSetupModels {
  private AliasSetupModels() {}

  /** Account provisioning behavior requested by an account-alias intent. */
  public enum AccountProvisionV1 {
    EXISTING("existing"),
    CREATE("create");

    private final String wireValue;

    AccountProvisionV1(final String wireValue) {
      this.wireValue = wireValue;
    }

    /** Returns the stable wire value. */
    public String wireValue() {
      return wireValue;
    }

    /** Returns the tagged unit-variant JSON shape. */
    public Map<String, Object> toJsonMap() {
      return unitVariant("kind", wireValue);
    }
  }

  /** Whether an account alias is primary or additional. */
  public enum AccountAliasRoleV1 {
    PRIMARY("primary"),
    ADDITIONAL("additional");

    private final String wireValue;

    AccountAliasRoleV1(final String wireValue) {
      this.wireValue = wireValue;
    }

    /** Returns the stable wire value. */
    public String wireValue() {
      return wireValue;
    }

    /** Returns the tagged unit-variant JSON shape. */
    public Map<String, Object> toJsonMap() {
      return unitVariant("kind", wireValue);
    }
  }

  /** Lease terms used only when setup classifies a resource as absent. */
  public static final class AliasLeaseAcquisitionV1 extends AliasJsonValue {
    private final int termYears;
    private final Integer pricingClassHint;

    /** Constructs acquisition terms. */
    public AliasLeaseAcquisitionV1(final int termYears, final Integer pricingClassHint) {
      this.termYears = requireU8(termYears, "termYears", false);
      this.pricingClassHint =
          pricingClassHint == null
              ? null
              : requireU8(pricingClassHint, "pricingClassHint", true);
    }

    /** Returns the term in whole years. */
    public int termYears() {
      return termYears;
    }

    /** Returns the optional pricing-class hint. */
    public Integer pricingClassHint() {
      return pricingClassHint;
    }

    @Override
    public Map<String, Object> toJsonMap() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("term_years", termYears);
      map.put("pricing_class_hint", pricingClassHint);
      return map;
    }
  }

  /** Desired state for one dataspace alias. */
  public static final class AliasDataSpaceIntentV1 extends AliasJsonValue {
    private final ResolvedDataSpaceV1 dataspace;
    private final String owner;

    /** Constructs an exact dataspace intent. */
    public AliasDataSpaceIntentV1(final ResolvedDataSpaceV1 dataspace, final String owner) {
      if (dataspace == null) throw new IllegalArgumentException("dataspace must not be null");
      this.dataspace = dataspace;
      this.owner = AccountIdLiteral.requireCanonicalI105Address(owner, "owner");
    }

    /** Returns the resolved dataspace. */
    public ResolvedDataSpaceV1 dataspace() {
      return dataspace;
    }

    /** Returns the exact owner. */
    public String owner() {
      return owner;
    }

    @Override
    public Map<String, Object> toJsonMap() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("dataspace", dataspace.toJsonMap());
      map.put("owner", owner);
      return map;
    }
  }

  /** Desired state for one domain. */
  public static final class AliasDomainIntentV1 extends AliasJsonValue {
    private final ResolvedDomainV1 domain;
    private final String owner;

    /** Constructs an exact domain intent. */
    public AliasDomainIntentV1(final ResolvedDomainV1 domain, final String owner) {
      if (domain == null) throw new IllegalArgumentException("domain must not be null");
      this.domain = domain;
      this.owner = AccountIdLiteral.requireCanonicalI105Address(owner, "owner");
    }

    /** Returns the resolved domain. */
    public ResolvedDomainV1 domain() {
      return domain;
    }

    /** Returns the exact owner. */
    public String owner() {
      return owner;
    }

    @Override
    public Map<String, Object> toJsonMap() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("domain", domain.toJsonMap());
      map.put("owner", owner);
      return map;
    }
  }

  /** Desired state for one account alias. */
  public static final class AliasAccountIntentV1 extends AliasJsonValue {
    private final ResolvedAccountAliasV1 alias;
    private final String targetAccount;
    private final AccountProvisionV1 provision;
    private final AccountAliasRoleV1 role;

    /** Constructs an exact account-alias intent. */
    public AliasAccountIntentV1(
        final ResolvedAccountAliasV1 alias,
        final String targetAccount,
        final AccountProvisionV1 provision,
        final AccountAliasRoleV1 role) {
      if (alias == null || provision == null || role == null) {
        throw new IllegalArgumentException("alias, provision, and role must not be null");
      }
      this.alias = alias;
      this.targetAccount =
          AccountIdLiteral.requireCanonicalI105Address(targetAccount, "targetAccount");
      this.provision = provision;
      this.role = role;
    }

    /** Returns the resolved alias. */
    public ResolvedAccountAliasV1 alias() {
      return alias;
    }

    /** Returns the canonical target account. */
    public String targetAccount() {
      return targetAccount;
    }

    /** Returns the provisioning behavior. */
    public AccountProvisionV1 provision() {
      return provision;
    }

    /** Returns the desired alias role. */
    public AccountAliasRoleV1 role() {
      return role;
    }

    @Override
    public Map<String, Object> toJsonMap() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("alias", alias.toJsonMap());
      map.put("target_account", targetAccount);
      map.put("provision", provision.toJsonMap());
      map.put("role", role.toJsonMap());
      return map;
    }
  }

  /** Declarative desired state for one alias/SNS resource. */
  public abstract static class AliasIntentV1 extends AliasJsonValue {
    /** Returns the stable variant name. */
    public abstract String kind();

    /** Returns dependency rank: dataspace, domain, account alias. */
    public abstract int dependencyRank();

    /** Returns the canonical resource text. */
    public abstract String resourceText();
  }

  /** Dataspace desired-state variant. */
  public static final class DataspaceIntent extends AliasIntentV1 {
    private final AliasDataSpaceIntentV1 intent;

    /** Wraps a dataspace intent. */
    public DataspaceIntent(final AliasDataSpaceIntentV1 intent) {
      if (intent == null) throw new IllegalArgumentException("intent must not be null");
      this.intent = intent;
    }

    /** Returns the exact intent payload. */
    public AliasDataSpaceIntentV1 intent() {
      return intent;
    }

    @Override
    public String kind() {
      return "dataspace";
    }

    @Override
    public int dependencyRank() {
      return 0;
    }

    @Override
    public String resourceText() {
      return intent.dataspace().canonicalName();
    }

    @Override
    public Map<String, Object> toJsonMap() {
      return variant("dataspace", "intent", intent.toJsonMap());
    }
  }

  /** Domain desired-state variant. */
  public static final class DomainIntent extends AliasIntentV1 {
    private final AliasDomainIntentV1 intent;

    /** Wraps a domain intent. */
    public DomainIntent(final AliasDomainIntentV1 intent) {
      if (intent == null) throw new IllegalArgumentException("intent must not be null");
      this.intent = intent;
    }

    /** Returns the exact intent payload. */
    public AliasDomainIntentV1 intent() {
      return intent;
    }

    @Override
    public String kind() {
      return "domain";
    }

    @Override
    public int dependencyRank() {
      return 1;
    }

    @Override
    public String resourceText() {
      return intent.domain().canonicalName();
    }

    @Override
    public Map<String, Object> toJsonMap() {
      return variant("domain", "intent", intent.toJsonMap());
    }
  }

  /** Account-alias desired-state variant. */
  public static final class AccountAliasIntent extends AliasIntentV1 {
    private final AliasAccountIntentV1 intent;

    /** Wraps an account-alias intent. */
    public AccountAliasIntent(final AliasAccountIntentV1 intent) {
      if (intent == null) throw new IllegalArgumentException("intent must not be null");
      this.intent = intent;
    }

    /** Returns the exact intent payload. */
    public AliasAccountIntentV1 intent() {
      return intent;
    }

    @Override
    public String kind() {
      return "account_alias";
    }

    @Override
    public int dependencyRank() {
      return 2;
    }

    @Override
    public String resourceText() {
      return intent.alias().canonicalName().canonicalText();
    }

    @Override
    public Map<String, Object> toJsonMap() {
      return variant("account_alias", "intent", intent.toJsonMap());
    }
  }

  /** Exact resolved resource supported by setup and lifecycle operations. */
  public abstract static class AliasTargetV1 extends AliasJsonValue {
    /** Returns the stable variant name. */
    public abstract String kind();
  }

  /** Dataspace target variant. */
  public static final class DataspaceTarget extends AliasTargetV1 {
    private final ResolvedDataSpaceV1 resource;

    /** Constructs a dataspace target. */
    public DataspaceTarget(final ResolvedDataSpaceV1 resource) {
      if (resource == null) throw new IllegalArgumentException("resource must not be null");
      this.resource = resource;
    }

    /** Returns the resolved target. */
    public ResolvedDataSpaceV1 resource() {
      return resource;
    }

    @Override
    public String kind() {
      return "dataspace";
    }

    @Override
    public Map<String, Object> toJsonMap() {
      return variant("dataspace", "resource", resource.toJsonMap());
    }
  }

  /** Domain target variant. */
  public static final class DomainTarget extends AliasTargetV1 {
    private final ResolvedDomainV1 resource;

    /** Constructs a domain target. */
    public DomainTarget(final ResolvedDomainV1 resource) {
      if (resource == null) throw new IllegalArgumentException("resource must not be null");
      this.resource = resource;
    }

    /** Returns the resolved target. */
    public ResolvedDomainV1 resource() {
      return resource;
    }

    @Override
    public String kind() {
      return "domain";
    }

    @Override
    public Map<String, Object> toJsonMap() {
      return variant("domain", "resource", resource.toJsonMap());
    }
  }

  /** Account-alias target variant. */
  public static final class AccountAliasTarget extends AliasTargetV1 {
    private final ResolvedAccountAliasV1 resource;

    /** Constructs an account-alias target. */
    public AccountAliasTarget(final ResolvedAccountAliasV1 resource) {
      if (resource == null) throw new IllegalArgumentException("resource must not be null");
      this.resource = resource;
    }

    /** Returns the resolved target. */
    public ResolvedAccountAliasV1 resource() {
      return resource;
    }

    @Override
    public String kind() {
      return "account_alias";
    }

    @Override
    public Map<String, Object> toJsonMap() {
      return variant("account_alias", "resource", resource.toJsonMap());
    }
  }

  /** Planner classification for one resource. */
  public enum AliasPlanDispositionV1 {
    NO_OP("no_op"),
    REPAIR("repair"),
    CREATE("create"),
    CONFLICT("conflict");

    private final String wireValue;

    AliasPlanDispositionV1(final String wireValue) {
      this.wireValue = wireValue;
    }

    /** Returns the stable wire value. */
    public String wireValue() {
      return wireValue;
    }

    /** Returns the tagged unit-variant JSON shape. */
    public Map<String, Object> toJsonMap() {
      return unitVariant("kind", wireValue);
    }
  }

  /** Exact lease quote attached to a create or renewal plan resource. */
  public static final class AliasLeaseQuoteV1 extends AliasJsonValue {
    private final AliasTargetV1 target;
    private final int pricingClass;
    private final String exactAmount;
    private final AliasQuoteGuardV1 guard;
    private final long expiresAtMs;
    private final long graceExpiresAtMs;
    private final long redemptionExpiresAtMs;

    /** Constructs an exact bounded lease quote. */
    public AliasLeaseQuoteV1(
        final AliasTargetV1 target,
        final int pricingClass,
        final String exactAmount,
        final AliasQuoteGuardV1 guard,
        final long expiresAtMs,
        final long graceExpiresAtMs,
        final long redemptionExpiresAtMs) {
      if (target == null || guard == null) {
        throw new IllegalArgumentException("target and guard must not be null");
      }
      this.target = target;
      this.pricingClass = requireU8(pricingClass, "pricingClass", true);
      this.exactAmount = AliasQuoteGuardV1.canonicalQuantity(exactAmount, "exactAmount");
      this.guard = guard;
      this.expiresAtMs = AliasNameSupport.requireNonNegative(expiresAtMs, "expiresAtMs");
      this.graceExpiresAtMs =
          AliasNameSupport.requireNonNegative(graceExpiresAtMs, "graceExpiresAtMs");
      this.redemptionExpiresAtMs =
          AliasNameSupport.requireNonNegative(redemptionExpiresAtMs, "redemptionExpiresAtMs");
    }

    /** Returns the exact target. */
    public AliasTargetV1 target() { return target; }

    /** Returns the selected pricing class. */
    public int pricingClass() { return pricingClass; }

    /** Returns the exact charge. */
    public String exactAmount() { return exactAmount; }

    /** Returns the quote guard. */
    public AliasQuoteGuardV1 guard() { return guard; }

    /** Returns the paid-term expiry. */
    public long expiresAtMs() { return expiresAtMs; }

    /** Returns the grace expiry. */
    public long graceExpiresAtMs() { return graceExpiresAtMs; }

    /** Returns the redemption expiry. */
    public long redemptionExpiresAtMs() { return redemptionExpiresAtMs; }

    @Override
    public Map<String, Object> toJsonMap() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("target", target.toJsonMap());
      map.put("pricing_class", pricingClass);
      map.put("exact_amount", exactAmount);
      map.put("guard", guard.toJsonMap());
      map.put("expires_at_ms", expiresAtMs);
      map.put("grace_expires_at_ms", graceExpiresAtMs);
      map.put("redemption_expires_at_ms", redemptionExpiresAtMs);
      return map;
    }
  }

  /** Planner result for one ordered resource intent. */
  public static final class AliasPlanResourceV1 extends AliasJsonValue {
    private final AliasIntentV1 intent;
    private final AliasPlanDispositionV1 disposition;
    private final AliasLeaseQuoteV1 quote;
    private final Long instructionIndex;

    /** Constructs an ordered resource plan entry. */
    public AliasPlanResourceV1(
        final AliasIntentV1 intent,
        final AliasPlanDispositionV1 disposition,
        final AliasLeaseQuoteV1 quote,
        final Long instructionIndex) {
      if (intent == null || disposition == null) {
        throw new IllegalArgumentException("intent and disposition must not be null");
      }
      if (instructionIndex != null
          && (instructionIndex.longValue() < 0 || instructionIndex.longValue() > 0xffff_ffffL)) {
        throw new IllegalArgumentException("instructionIndex must be an unsigned 32-bit integer");
      }
      this.intent = intent;
      this.disposition = disposition;
      this.quote = quote;
      this.instructionIndex = instructionIndex;
    }

    /** Returns the exact intent. */
    public AliasIntentV1 intent() { return intent; }

    /** Returns the fixed classification. */
    public AliasPlanDispositionV1 disposition() { return disposition; }

    /** Returns the optional exact quote. */
    public AliasLeaseQuoteV1 quote() { return quote; }

    /** Returns the optional matching instruction index. */
    public Long instructionIndex() { return instructionIndex; }

    @Override
    public Map<String, Object> toJsonMap() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("intent", intent.toJsonMap());
      map.put("disposition", disposition.toJsonMap());
      map.put("quote", quote == null ? null : quote.toJsonMap());
      map.put("instruction_index", instructionIndex);
      return map;
    }
  }

  /** Exact framed Norito instruction returned by the planner. */
  public static final class AliasFramedInstructionV1 extends AliasJsonValue {
    private final String wireId;
    private final byte[] framedPayload;

    /** Constructs an exact defensive copy of a planner frame. */
    public AliasFramedInstructionV1(final String wireId, final byte[] framedPayload) {
      this.wireId = AliasNameSupport.requireToken(wireId, "wireId");
      if (framedPayload == null) {
        throw new IllegalArgumentException("framedPayload must not be null");
      }
      this.framedPayload = framedPayload.clone();
    }

    /** Returns the stable instruction wire identifier. */
    public String wireId() { return wireId; }

    /** Returns a defensive copy of the exact frame. */
    public byte[] framedPayload() { return framedPayload.clone(); }

    @Override
    public Map<String, Object> toJsonMap() {
      final List<Integer> bytes = new ArrayList<>(framedPayload.length);
      for (final byte value : framedPayload) bytes.add(value & 0xff);
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("wire_id", wireId);
      map.put("framed_payload", bytes);
      return map;
    }
  }

  /** Exact total charge for one payment asset. */
  public static final class AliasAssetTotalV1 extends AliasJsonValue {
    private final String paymentAsset;
    private final String amount;

    /** Constructs one exact per-asset total. */
    public AliasAssetTotalV1(final String paymentAsset, final String amount) {
      if (!AssetDefinitionIdEncoder.isCanonicalAddress(paymentAsset)) {
        throw new IllegalArgumentException(
            "paymentAsset must use a canonical unprefixed Base58 asset-definition address");
      }
      this.paymentAsset = paymentAsset;
      this.amount = AliasQuoteGuardV1.canonicalQuantity(amount, "amount");
    }

    /** Returns the canonical payment asset. */
    public String paymentAsset() { return paymentAsset; }

    /** Returns the exact total quantity. */
    public String amount() { return amount; }

    @Override
    public Map<String, Object> toJsonMap() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("payment_asset", paymentAsset);
      map.put("amount", amount);
      return map;
    }
  }

  /** Overall setup/readiness state. */
  public enum AliasSetupStatusV1 {
    READY("ready"), PENDING("pending"), BLOCKED("blocked");

    private final String wireValue;

    AliasSetupStatusV1(final String wireValue) { this.wireValue = wireValue; }

    /** Returns the stable wire value. */
    public String wireValue() { return wireValue; }

    /** Returns the tagged unit-variant JSON shape. */
    public Map<String, Object> toJsonMap() { return unitVariant("status", wireValue); }
  }

  /** Setup/readiness validation phase. */
  public enum AliasSetupValidationPhaseV1 {
    CONFIG("config"), CATALOG("catalog"), BOOTSTRAP("bootstrap"),
    WORLD_STATE("world_state"), PLANNING("planning");

    private final String wireValue;

    AliasSetupValidationPhaseV1(final String wireValue) { this.wireValue = wireValue; }

    /** Returns the stable wire value. */
    public String wireValue() { return wireValue; }

    /** Returns the tagged unit-variant JSON shape. */
    public Map<String, Object> toJsonMap() { return unitVariant("phase", wireValue); }
  }

  /** Setup/readiness diagnostic severity. */
  public enum AliasSetupSeverityV1 {
    INFO("info"), WARNING("warning"), ERROR("error");

    private final String wireValue;

    AliasSetupSeverityV1(final String wireValue) { this.wireValue = wireValue; }

    /** Returns the stable wire value. */
    public String wireValue() { return wireValue; }

    /** Returns the tagged unit-variant JSON shape. */
    public Map<String, Object> toJsonMap() { return unitVariant("severity", wireValue); }
  }

  /** One stable, secret-free setup/readiness diagnostic. */
  public static final class AliasSetupDiagnosticV1 extends AliasJsonValue
      implements Comparable<AliasSetupDiagnosticV1> {
    private final AliasSetupValidationPhaseV1 phase;
    private final String code;
    private final AliasSetupSeverityV1 severity;
    private final String resource;
    private final String configPath;
    private final String expected;
    private final String actual;
    private final String remediation;

    /** Constructs one planner diagnostic. */
    public AliasSetupDiagnosticV1(
        final AliasSetupValidationPhaseV1 phase,
        final String code,
        final AliasSetupSeverityV1 severity,
        final String resource,
        final String configPath,
        final String expected,
        final String actual,
        final String remediation) {
      if (phase == null || severity == null) {
        throw new IllegalArgumentException("phase and severity must not be null");
      }
      this.phase = phase;
      this.code = AliasNameSupport.requireToken(code, "code");
      this.severity = severity;
      this.resource = optionalText(resource, "resource");
      this.configPath = optionalText(configPath, "configPath");
      this.expected = optionalText(expected, "expected");
      this.actual = optionalText(actual, "actual");
      this.remediation = requireText(remediation, "remediation");
    }

    String sortKey() {
      return phase.ordinal() + "\0" + code + "\0" + severity.ordinal() + "\0"
          + nullToEmpty(resource) + "\0" + nullToEmpty(configPath) + "\0"
          + nullToEmpty(expected) + "\0" + nullToEmpty(actual) + "\0" + remediation;
    }

    /** Returns the validation phase. */
    public AliasSetupValidationPhaseV1 phase() { return phase; }

    /** Returns the stable diagnostic code. */
    public String code() { return code; }

    /** Returns the diagnostic severity. */
    public AliasSetupSeverityV1 severity() { return severity; }

    /** Returns the optional canonical resource. */
    public String resource() { return resource; }

    /** Returns the optional configuration path. */
    public String configPath() { return configPath; }

    /** Returns the optional expected value. */
    public String expected() { return expected; }

    /** Returns the optional actual value. */
    public String actual() { return actual; }

    /** Returns the secret-free remediation. */
    public String remediation() { return remediation; }

    @Override
    public int compareTo(final AliasSetupDiagnosticV1 other) {
      return sortKey().compareTo(other.sortKey());
    }

    @Override
    public Map<String, Object> toJsonMap() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("phase", phase.toJsonMap());
      map.put("code", code);
      map.put("severity", severity.toJsonMap());
      map.put("resource", resource);
      map.put("config_path", configPath);
      map.put("expected", expected);
      map.put("actual", actual);
      map.put("remediation", remediation);
      return map;
    }
  }

  /** Deterministically ordered setup/readiness diagnostics. */
  public static final class AliasSetupReportV1 extends AliasJsonValue {
    /** Current report layout version. */
    public static final int VERSION = 1;

    private final AliasSetupStatusV1 status;
    private final List<AliasSetupDiagnosticV1> diagnostics;

    /** Constructs a report and canonically sorts its diagnostics. */
    public AliasSetupReportV1(
        final AliasSetupStatusV1 status,
        final List<AliasSetupDiagnosticV1> diagnostics) {
      if (status == null || diagnostics == null) {
        throw new IllegalArgumentException("status and diagnostics must not be null");
      }
      this.status = status;
      final List<AliasSetupDiagnosticV1> sorted = new ArrayList<>(diagnostics);
      Collections.sort(sorted);
      this.diagnostics = Collections.unmodifiableList(sorted);
    }

    /** Returns the overall readiness state. */
    public AliasSetupStatusV1 status() { return status; }

    /** Returns canonically ordered diagnostics. */
    public List<AliasSetupDiagnosticV1> diagnostics() { return diagnostics; }

    @Override
    public Map<String, Object> toJsonMap() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("version", VERSION);
      map.put("status", status.toJsonMap());
      final List<Map<String, Object>> encoded = new ArrayList<>(diagnostics.size());
      for (final AliasSetupDiagnosticV1 diagnostic : diagnostics) {
        encoded.add(diagnostic.toJsonMap());
      }
      map.put("diagnostics", encoded);
      return map;
    }
  }

  /** World-state anchor used to classify an alias plan. */
  public static final class AliasPlanAnchorV1 extends AliasJsonValue {
    private final long blockHeight;
    private final String blockHash;

    /** Constructs an exact plan anchor. */
    public AliasPlanAnchorV1(final long blockHeight, final String blockHash) {
      this.blockHeight = AliasNameSupport.requireNonNegative(blockHeight, "blockHeight");
      this.blockHash = AliasNameSupport.requireHash(blockHash, "blockHash");
    }

    /** Returns the block height. */
    public long blockHeight() { return blockHeight; }

    /** Returns the block hash. */
    public String blockHash() { return blockHash; }

    @Override
    public Map<String, Object> toJsonMap() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("block_height", blockHeight);
      map.put("block_hash", blockHash);
      return map;
    }
  }

  /** Canonical body committed by an alias transaction plan hash. */
  public static final class AliasTransactionPlanBodyV1 extends AliasJsonValue {
    /** Current canonical plan-body version. */
    public static final int VERSION = 1;

    private final int version;
    private final String authority;
    private final NetworkId networkId;
    private final AliasPlanAnchorV1 anchor;
    private final List<AliasPlanResourceV1> resources;
    private final List<AliasFramedInstructionV1> instructions;
    private final List<AliasAssetTotalV1> totalsByAsset;
    private final List<AliasSetupDiagnosticV1> warnings;
    private final List<AliasSetupDiagnosticV1> blockers;
    private final long validUntilMs;

    /** Constructs one immutable canonical plan body. */
    public AliasTransactionPlanBodyV1(
        final int version,
        final String authority,
        final NetworkId networkId,
        final AliasPlanAnchorV1 anchor,
        final List<AliasPlanResourceV1> resources,
        final List<AliasFramedInstructionV1> instructions,
        final List<AliasAssetTotalV1> totalsByAsset,
        final List<AliasSetupDiagnosticV1> warnings,
        final List<AliasSetupDiagnosticV1> blockers,
        final long validUntilMs) {
      if (version < 0 || version > 255 || anchor == null) {
        throw new IllegalArgumentException(
            "version must fit in an unsigned byte and anchor is required");
      }
      this.version = version;
      this.authority = AccountIdLiteral.requireCanonicalI105Address(authority, "authority");
      if (networkId == null) throw new IllegalArgumentException("networkId must not be null");
      this.networkId = networkId;
      this.anchor = anchor;
      this.resources = immutableCopy(resources, "resources");
      this.instructions = immutableCopy(instructions, "instructions");
      this.totalsByAsset = immutableCopy(totalsByAsset, "totalsByAsset");
      this.warnings = immutableCopy(warnings, "warnings");
      this.blockers = immutableCopy(blockers, "blockers");
      this.validUntilMs = AliasNameSupport.requireNonNegative(validUntilMs, "validUntilMs");
    }

    /** Returns the layout version. */
    public int version() { return version; }

    /** Returns transaction authority and lease payer. */
    public String authority() { return authority; }

    /** Returns the exact genesis-derived target network. */
    public NetworkId networkId() { return networkId; }

    /** Returns the world-state anchor. */
    public AliasPlanAnchorV1 anchor() { return anchor; }

    /** Returns ordered resource plans. */
    public List<AliasPlanResourceV1> resources() { return resources; }

    /** Returns ordered exact instructions. */
    public List<AliasFramedInstructionV1> instructions() { return instructions; }

    /** Returns canonical per-asset totals. */
    public List<AliasAssetTotalV1> totalsByAsset() { return totalsByAsset; }

    /** Returns canonical warnings. */
    public List<AliasSetupDiagnosticV1> warnings() { return warnings; }

    /** Returns canonical blockers. */
    public List<AliasSetupDiagnosticV1> blockers() { return blockers; }

    /** Returns the plan deadline. */
    public long validUntilMs() { return validUntilMs; }

    @Override
    public Map<String, Object> toJsonMap() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("version", version);
      map.put("authority", authority);
      map.put("network_id", networkId.literal());
      map.put("anchor", anchor.toJsonMap());
      map.put("resources", maps(resources));
      map.put("instructions", maps(instructions));
      map.put("totals_by_asset", maps(totalsByAsset));
      map.put("warnings", maps(warnings));
      map.put("blockers", maps(blockers));
      map.put("valid_until_ms", validUntilMs);
      return map;
    }
  }

  static AliasTargetV1 targetFor(final AliasIntentV1 intent) {
    if (intent instanceof DataspaceIntent value) {
      return new DataspaceTarget(value.intent().dataspace());
    }
    if (intent instanceof DomainIntent value) return new DomainTarget(value.intent().domain());
    if (intent instanceof AccountAliasIntent value) {
      return new AccountAliasTarget(value.intent().alias());
    }
    throw new IllegalArgumentException("unsupported alias intent");
  }

  static boolean amountWithinCap(final String exact, final String cap) {
    try {
      final BigDecimal exactValue = new BigDecimal(exact);
      final BigDecimal capValue = new BigDecimal(cap);
      return exactValue.signum() >= 0
          && capValue.signum() >= 0
          && exactValue.compareTo(capValue) <= 0;
    } catch (final NumberFormatException ignored) {
      return false;
    }
  }

  private static int requireU8(final int value, final String field, final boolean allowZero) {
    if (value < (allowZero ? 0 : 1) || value > 255) {
      throw new IllegalArgumentException(field + " must fit in an unsigned byte");
    }
    return value;
  }

  private static Map<String, Object> unitVariant(final String tag, final String value) {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put(tag, value);
    map.put("value", null);
    return map;
  }

  private static Map<String, Object> variant(
      final String kind, final String contentName, final Object content) {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("kind", kind);
    map.put(contentName, content);
    return map;
  }

  private static String optionalText(final String value, final String field) {
    return value == null ? null : requireText(value, field);
  }

  private static String requireText(final String value, final String field) {
    if (value == null || value.trim().isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(
          field + " must be non-blank without surrounding whitespace");
    }
    return value;
  }

  private static String nullToEmpty(final String value) {
    return value == null ? "" : value;
  }

  private static <T> List<T> immutableCopy(final List<T> values, final String field) {
    if (values == null || values.stream().anyMatch(value -> value == null)) {
      throw new IllegalArgumentException(field + " must not be null or contain null");
    }
    return Collections.unmodifiableList(new ArrayList<>(values));
  }

  private static List<Map<String, Object>> maps(final List<? extends AliasJsonValue> values) {
    final List<Map<String, Object>> result = new ArrayList<>(values.size());
    for (final AliasJsonValue value : values) result.add(value.toJsonMap());
    return result;
  }
}
