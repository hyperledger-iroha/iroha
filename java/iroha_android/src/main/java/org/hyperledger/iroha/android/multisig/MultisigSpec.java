package org.hyperledger.iroha.android.multisig;

import java.util.Collections;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.TreeMap;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Builder and helpers for multisig specifications and TTL preview/enforcement. */
public final class MultisigSpec {

  private static final int MAX_SIGNATORIES = 0xFF;
  private static final int MAX_WEIGHT = 0xFF;
  private static final int MAX_QUORUM = 0xFFFF;

  private final Map<String, Integer> signatories;
  private final int quorum;
  private final long transactionTtlMs;

  private MultisigSpec(final Builder builder) {
    this.signatories = Collections.unmodifiableMap(new LinkedHashMap<>(builder.signatories));
    this.quorum = Objects.requireNonNull(builder.quorum, "quorum");
    this.transactionTtlMs = Objects.requireNonNull(builder.transactionTtlMs, "transactionTtlMs");
  }

  public static Builder builder() {
    return new Builder();
  }

  public Map<String, Integer> signatories() {
    return signatories;
  }

  public int quorum() {
    return quorum;
  }

  public long transactionTtlMs() {
    return transactionTtlMs;
  }

  public MultisigProposalTtlPreview previewProposalExpiry(final Long requestedTtlMs, final long nowMs) {
    final long normalizedNow = Math.max(0L, nowMs);
    final long cap = transactionTtlMs;
    final long requested = requestedTtlMs == null ? cap : normalizeTtl(requestedTtlMs, "requestedTtlMs");
    final boolean wasCapped = requested > cap;
    final long effective = wasCapped ? cap : requested;
    final long expiresAt = safeExpiry(normalizedNow, effective);
    return new MultisigProposalTtlPreview(effective, cap, expiresAt, wasCapped);
  }

  public MultisigProposalTtlPreview enforceProposalTtl(final Long requestedTtlMs, final long nowMs) {
    if (requestedTtlMs != null && requestedTtlMs > transactionTtlMs) {
      throw new IllegalArgumentException(
          "Requested multisig TTL "
              + requestedTtlMs
              + " ms exceeds the policy cap "
              + transactionTtlMs
              + " ms; choose a value at or below the cap.");
    }
    return previewProposalExpiry(requestedTtlMs, nowMs);
  }

  public String toJson() {
    return toJson(false);
  }

  public String toJson(final boolean prettyPrinted) {
    final StringBuilder builder = new StringBuilder();
    final String newline = prettyPrinted ? "\n" : "";
    final String indent = prettyPrinted ? "  " : "";
    final String innerIndent = prettyPrinted ? "    " : "";

    builder.append("{").append(newline);
    builder.append(indent).append("\"signatories\": {");

    final Iterator<Map.Entry<String, Integer>> iterator = new TreeMap<>(signatories).entrySet().iterator();
    boolean first = true;
    while (iterator.hasNext()) {
      final Map.Entry<String, Integer> entry = iterator.next();
      if (!first) {
        builder.append(",");
      }
      builder.append(newline)
          .append(innerIndent)
          .append("\"")
          .append(entry.getKey())
          .append("\": ")
          .append(entry.getValue());
      first = false;
    }
    builder.append(newline).append(indent).append("},").append(newline);
    builder.append(indent).append("\"quorum\": ").append(quorum).append(",").append(newline);
    builder.append(indent).append("\"transaction_ttl_ms\": ").append(transactionTtlMs).append(newline);
    builder.append("}");
    return builder.toString();
  }

  private static long normalizeTtl(final long ttlMs, final String context) {
    if (ttlMs <= 0) {
      throw new IllegalArgumentException(context + " must be greater than zero");
    }
    return ttlMs;
  }

  private static int normalizeWeight(final int weight) {
    if (weight <= 0) {
      throw new IllegalArgumentException("weight must be greater than zero");
    }
    if (weight > MAX_WEIGHT) {
      throw new IllegalArgumentException("weight must fit in an unsigned byte (max 255)");
    }
    return weight;
  }

  private static int normalizeQuorum(final int quorumValue) {
    if (quorumValue <= 0) {
      throw new IllegalArgumentException("quorum must be greater than zero");
    }
    if (quorumValue > MAX_QUORUM) {
      throw new IllegalArgumentException("quorum must fit in an unsigned 16-bit integer");
    }
    return quorumValue;
  }

  private static String normalizeAccountId(final String accountId) {
    return AccountIdLiteral.extractI105Address(accountId);
  }

  private static long safeExpiry(final long nowMs, final long ttlMs) {
    try {
      return Math.addExact(nowMs, ttlMs);
    } catch (ArithmeticException ex) {
      return Long.MAX_VALUE;
    }
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof MultisigSpec other)) {
      return false;
    }
    return quorum == other.quorum
        && transactionTtlMs == other.transactionTtlMs
        && signatories.equals(other.signatories);
  }

  @Override
  public int hashCode() {
    return Objects.hash(signatories, quorum, transactionTtlMs);
  }

  public static final class Builder {
    private final Map<String, Integer> signatories = new LinkedHashMap<>();
    private Integer quorum;
    private Long transactionTtlMs;

    public Builder setQuorum(final int quorum) {
      this.quorum = normalizeQuorum(quorum);
      return this;
    }

    public Builder setTransactionTtlMs(final long ttlMs) {
      this.transactionTtlMs = normalizeTtl(ttlMs, "transactionTtlMs");
      return this;
    }

    public Builder addSignatory(final String accountId, final int weight) {
      final String normalizedId = normalizeAccountId(accountId);
      final int normalizedWeight = normalizeWeight(weight);
      signatories.put(normalizedId, normalizedWeight);
      return this;
    }

    public Builder removeSignatory(final String accountId) {
      final String normalizedId = normalizeAccountId(accountId);
      signatories.remove(normalizedId);
      return this;
    }

    public MultisigSpec build() {
      if (quorum == null) {
        throw new IllegalStateException("quorum must be set before building a multisig spec");
      }
      if (transactionTtlMs == null) {
        throw new IllegalStateException("transactionTtlMs must be set before building a multisig spec");
      }
      if (signatories.isEmpty()) {
        throw new IllegalStateException("multisig specs require at least one signatory");
      }
      if (signatories.size() > MAX_SIGNATORIES) {
        throw new IllegalStateException("multisig specs support at most 255 signatories");
      }

      long totalWeight = 0;
      for (int weight : signatories.values()) {
        totalWeight += weight;
      }
      if (totalWeight < quorum) {
        throw new IllegalStateException("quorum " + quorum + " exceeds total signatory weight " + totalWeight);
      }
      return new MultisigSpec(this);
    }
  }
}
