package org.hyperledger.iroha.android.model;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Required signature-bound choice of fee payer, charge maxima, and executable gas bound. */
public abstract class FeePaymentIntent {
  private final List<FeeChargeLimit> chargeLimits;
  private final Long gasLimit;

  private FeePaymentIntent(final List<FeeChargeLimit> chargeLimits, final Long gasLimit) {
    Objects.requireNonNull(chargeLimits, "chargeLimits");
    this.chargeLimits = Collections.unmodifiableList(new ArrayList<>(chargeLimits));
    this.gasLimit = gasLimit;
    if (gasLimit != null && gasLimit.longValue() <= 0L) {
      throw new IllegalArgumentException("gasLimit must be positive when present");
    }
    int previous = -1;
    for (final FeeChargeLimit limit : this.chargeLimits) {
      final int current = Objects.requireNonNull(limit, "chargeLimit").kind().ordinal();
      if (current <= previous) {
        throw new IllegalArgumentException(
            "chargeLimits must be unique and ordered nexus before pipeline gas");
      }
      previous = current;
    }
  }

  public List<FeeChargeLimit> chargeLimits() { return chargeLimits; }
  public Long gasLimit() { return gasLimit; }

  /** Authority-paid intent. */
  public static final class Authority extends FeePaymentIntent {
    private Authority(final List<FeeChargeLimit> chargeLimits, final Long gasLimit) {
      super(chargeLimits, gasLimit);
    }

    @Override
    public boolean equals(final Object other) {
      if (this == other) return true;
      if (!(other instanceof Authority)) return false;
      final Authority that = (Authority) other;
      return chargeLimits().equals(that.chargeLimits()) && Objects.equals(gasLimit(), that.gasLimit());
    }

    @Override
    public int hashCode() { return Objects.hash(chargeLimits(), gasLimit()); }
  }

  /** Exact sponsor-program revision intent. */
  public static final class Sponsor extends FeePaymentIntent {
    private final FeeSponsorProgramId programId;
    private final long programRevision;

    private Sponsor(
        final FeeSponsorProgramId programId,
        final long programRevision,
        final List<FeeChargeLimit> chargeLimits,
        final Long gasLimit) {
      super(chargeLimits, gasLimit);
      this.programId = Objects.requireNonNull(programId, "programId");
      if (programRevision <= 0L) {
        throw new IllegalArgumentException("programRevision must be positive");
      }
      this.programRevision = programRevision;
    }

    public FeeSponsorProgramId programId() { return programId; }
    public long programRevision() { return programRevision; }

    @Override
    public boolean equals(final Object other) {
      if (this == other) return true;
      if (!(other instanceof Sponsor)) return false;
      final Sponsor that = (Sponsor) other;
      return programRevision == that.programRevision
          && programId.equals(that.programId)
          && chargeLimits().equals(that.chargeLimits())
          && Objects.equals(gasLimit(), that.gasLimit());
    }

    @Override
    public int hashCode() {
      return Objects.hash(programId, programRevision, chargeLimits(), gasLimit());
    }
  }

  public static FeePaymentIntent authority(final List<FeeChargeLimit> chargeLimits) {
    return authority(chargeLimits, null);
  }

  public static FeePaymentIntent authority(
      final List<FeeChargeLimit> chargeLimits, final Long gasLimit) {
    return new Authority(chargeLimits, gasLimit);
  }

  public static FeePaymentIntent sponsor(
      final FeeSponsorProgramId programId,
      final long programRevision,
      final List<FeeChargeLimit> chargeLimits) {
    return sponsor(programId, programRevision, chargeLimits, null);
  }

  public static FeePaymentIntent sponsor(
      final FeeSponsorProgramId programId,
      final long programRevision,
      final List<FeeChargeLimit> chargeLimits,
      final Long gasLimit) {
    return new Sponsor(programId, programRevision, chargeLimits, gasLimit);
  }

  /** True only when a quote preserves the exact payer, revision, and signed gas bound. */
  public final boolean hasSamePayerAndGasBound(final FeePaymentIntent other) {
    Objects.requireNonNull(other, "other");
    if (!Objects.equals(gasLimit, other.gasLimit)) return false;
    if (this instanceof Authority && other instanceof Authority) return true;
    if (this instanceof Sponsor && other instanceof Sponsor) {
      final Sponsor left = (Sponsor) this;
      final Sponsor right = (Sponsor) other;
      return left.programId.equals(right.programId)
          && left.programRevision == right.programRevision;
    }
    return false;
  }

  /** Exact Norito JSON object used by Torii request bodies and native bridges. */
  public final Map<String, Object> toJsonMap() {
    final Map<String, Object> value = new LinkedHashMap<>();
    if (this instanceof Sponsor) {
      final Sponsor sponsor = (Sponsor) this;
      final Map<String, Object> programId = new LinkedHashMap<>();
      programId.put("sponsor", sponsor.programId.sponsor());
      programId.put("name", sponsor.programId.name());
      value.put("program_id", programId);
      value.put("program_revision", sponsor.programRevision);
    }
    final List<Map<String, Object>> limits = new ArrayList<>();
    for (final FeeChargeLimit limit : chargeLimits) {
      final Map<String, Object> kind = new LinkedHashMap<>();
      kind.put("kind", limit.kind() == FeeChargeKind.NEXUS ? "nexus" : "pipeline_gas");
      kind.put("value", null);
      final Map<String, Object> item = new LinkedHashMap<>();
      item.put("kind", kind);
      item.put("asset_definition_id", limit.assetDefinitionId());
      item.put("max_amount", limit.maxAmount());
      limits.add(item);
    }
    value.put("charge_limits", limits);
    value.put("gas_limit", gasLimit);
    final Map<String, Object> out = new LinkedHashMap<>();
    out.put("payer", this instanceof Authority ? "authority" : "sponsor");
    out.put("value", value);
    return out;
  }
}
