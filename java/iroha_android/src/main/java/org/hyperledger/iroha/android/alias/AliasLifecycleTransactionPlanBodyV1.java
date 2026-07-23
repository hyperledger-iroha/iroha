package org.hyperledger.iroha.android.alias;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Canonical body committed by an alias lifecycle transaction plan hash. */
public final class AliasLifecycleTransactionPlanBodyV1 extends AliasJsonValue {
  /** Current body layout. */
  public static final int VERSION = 1;

  private final int version;
  private final String authority;
  private final String chainId;
  private final AliasSetupModels.AliasPlanAnchorV1 anchor;
  private final AliasLifecycleOperationV1 operation;
  private final AliasLifecyclePlanDispositionV1 disposition;
  private final AliasSetupModels.AliasFramedInstructionV1 instruction;
  private final AliasSetupModels.AliasLeaseQuoteV1 quote;
  private final List<AliasSetupModels.AliasAssetTotalV1> totalsByAsset;
  private final List<AliasSetupModels.AliasSetupDiagnosticV1> warnings;
  private final List<AliasSetupModels.AliasSetupDiagnosticV1> blockers;
  private final long validUntilMs;

  /** Constructs one exact lifecycle plan body. */
  public AliasLifecycleTransactionPlanBodyV1(
      final int version,
      final String authority,
      final String chainId,
      final AliasSetupModels.AliasPlanAnchorV1 anchor,
      final AliasLifecycleOperationV1 operation,
      final AliasLifecyclePlanDispositionV1 disposition,
      final AliasSetupModels.AliasFramedInstructionV1 instruction,
      final AliasSetupModels.AliasLeaseQuoteV1 quote,
      final List<AliasSetupModels.AliasAssetTotalV1> totalsByAsset,
      final List<AliasSetupModels.AliasSetupDiagnosticV1> warnings,
      final List<AliasSetupModels.AliasSetupDiagnosticV1> blockers,
      final long validUntilMs) {
    if (version != VERSION) throw new IllegalArgumentException("version must be " + VERSION);
    if (anchor == null || operation == null || disposition == null) {
      throw new IllegalArgumentException("anchor, operation, and disposition must not be null");
    }
    if (chainId == null || chainId.trim().isEmpty() || !chainId.equals(chainId.trim())) {
      throw new IllegalArgumentException("chainId must be non-blank without surrounding whitespace");
    }
    this.version = version;
    this.authority = AccountIdLiteral.requireCanonicalI105Address(authority, "authority");
    this.chainId = chainId;
    this.anchor = anchor;
    this.operation = operation;
    this.disposition = disposition;
    this.instruction = instruction;
    this.quote = quote;
    this.totalsByAsset = immutable(totalsByAsset, "totalsByAsset");
    this.warnings = immutable(warnings, "warnings");
    this.blockers = immutable(blockers, "blockers");
    this.validUntilMs = AliasNameSupport.requireNonNegative(validUntilMs, "validUntilMs");
  }

  public int version() { return version; }
  public String authority() { return authority; }
  public String chainId() { return chainId; }
  public AliasSetupModels.AliasPlanAnchorV1 anchor() { return anchor; }
  public AliasLifecycleOperationV1 operation() { return operation; }
  public AliasLifecyclePlanDispositionV1 disposition() { return disposition; }
  public AliasSetupModels.AliasFramedInstructionV1 instruction() { return instruction; }
  public AliasSetupModels.AliasLeaseQuoteV1 quote() { return quote; }
  public List<AliasSetupModels.AliasAssetTotalV1> totalsByAsset() { return totalsByAsset; }
  public List<AliasSetupModels.AliasSetupDiagnosticV1> warnings() { return warnings; }
  public List<AliasSetupModels.AliasSetupDiagnosticV1> blockers() { return blockers; }
  public long validUntilMs() { return validUntilMs; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("version", version);
    map.put("authority", authority);
    map.put("chain_id", chainId);
    map.put("anchor", anchor.toJsonMap());
    map.put("operation", operation.toJsonMap());
    map.put("disposition", disposition.toJsonMap());
    map.put("instruction", instruction == null ? null : instruction.toJsonMap());
    map.put("quote", quote == null ? null : quote.toJsonMap());
    map.put("totals_by_asset", jsonList(totalsByAsset));
    map.put("warnings", jsonList(warnings));
    map.put("blockers", jsonList(blockers));
    map.put("valid_until_ms", validUntilMs);
    return map;
  }

  private static <T> List<T> immutable(final List<T> values, final String field) {
    if (values == null || values.contains(null)) {
      throw new IllegalArgumentException(field + " must not contain null values");
    }
    return Collections.unmodifiableList(new ArrayList<>(values));
  }

  private static List<Map<String, Object>> jsonList(
      final List<? extends AliasJsonValue> values) {
    final List<Map<String, Object>> result = new ArrayList<>(values.size());
    for (final AliasJsonValue value : values) result.add(value.toJsonMap());
    return result;
  }
}
