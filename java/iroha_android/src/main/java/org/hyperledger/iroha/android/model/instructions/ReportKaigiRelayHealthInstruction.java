package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

/** Typed builder for {@code ReportKaigiRelayHealth} instructions. */
public final class ReportKaigiRelayHealthInstruction implements InstructionTemplate {

  private static final String ACTION = "ReportKaigiRelayHealth";
  private static final int MAX_NOTES_CHARACTERS = 512;
  private static final Set<String> ALLOWED_ARGUMENTS =
      KaigiInstructionUtils.argumentSet(
          "action",
          "call.domain_id",
          "call.call_name",
          "relay_id",
          "status",
          "reported_at_ms",
          "notes");

  private final KaigiInstructionUtils.CallId callId;
  private final String relayId;
  private final Status status;
  private final long reportedAtMs;
  private final String notes;
  private final Map<String, String> arguments;

  private ReportKaigiRelayHealthInstruction(final Builder builder) {
    this.callId = builder.callId;
    this.relayId = builder.relayId;
    this.status = builder.status;
    this.reportedAtMs = builder.reportedAtMs;
    this.notes = builder.notes;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(builder.canonicalArguments()));
  }

  /** Relay health variants accepted by the Rust data model. */
  public enum Status {
    HEALTHY("Healthy"),
    DEGRADED("Degraded"),
    UNAVAILABLE("Unavailable");

    private final String wireName;

    Status(final String wireName) {
      this.wireName = wireName;
    }

    public String wireName() {
      return wireName;
    }

    public static Status fromWireName(final String value) {
      for (final Status status : values()) {
        if (status.wireName.equals(value)) {
          return status;
        }
      }
      throw new IllegalArgumentException("Unknown Kaigi relay health status: " + value);
    }
  }

  public KaigiInstructionUtils.CallId callId() {
    return callId;
  }

  public String relayId() {
    return relayId;
  }

  public Status status() {
    return status;
  }

  public long reportedAtMs() {
    return reportedAtMs;
  }

  public String notes() {
    return notes;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static ReportKaigiRelayHealthInstruction fromArguments(
      final Map<String, String> arguments) {
    KaigiInstructionUtils.requireKnownArguments(arguments, ALLOWED_ARGUMENTS);
    KaigiInstructionUtils.requireAction(arguments, ACTION);
    return builder()
        .setCallId(KaigiInstructionUtils.parseCallId(arguments, "call"))
        .setRelayId(KaigiInstructionUtils.require(arguments, "relay_id"))
        .setStatus(Status.fromWireName(KaigiInstructionUtils.require(arguments, "status")))
        .setReportedAtMs(
            KaigiInstructionUtils.parseUnsignedLong(
                KaigiInstructionUtils.require(arguments, "reported_at_ms"), "reported_at_ms"))
        .setNotes(arguments.get("notes"))
        .build();
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof ReportKaigiRelayHealthInstruction)) {
      return false;
    }
    final ReportKaigiRelayHealthInstruction other =
        (ReportKaigiRelayHealthInstruction) obj;
    return Objects.equals(callId.domainId(), other.callId.domainId())
        && Objects.equals(callId.callName(), other.callId.callName())
        && Objects.equals(relayId, other.relayId)
        && status == other.status
        && reportedAtMs == other.reportedAtMs
        && Objects.equals(notes, other.notes);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        callId.domainId(), callId.callName(), relayId, status, reportedAtMs, notes);
  }

  /** Builder for relay health report instructions. */
  public static final class Builder {
    private KaigiInstructionUtils.CallId callId;
    private String relayId;
    private Status status;
    private Long reportedAtMs;
    private String notes;

    private Builder() {}

    public Builder setCallId(final String domainId, final String callName) {
      this.callId = new KaigiInstructionUtils.CallId(domainId, callName);
      return this;
    }

    public Builder setCallId(final KaigiInstructionUtils.CallId callId) {
      this.callId = Objects.requireNonNull(callId, "callId");
      return this;
    }

    public Builder setRelayId(final String relayId) {
      if (relayId == null || relayId.trim().isEmpty()) {
        throw new IllegalArgumentException("relayId must not be blank");
      }
      this.relayId = relayId;
      return this;
    }

    public Builder setStatus(final Status status) {
      this.status = Objects.requireNonNull(status, "status");
      return this;
    }

    public Builder setStatus(final String wireName) {
      return setStatus(Status.fromWireName(wireName));
    }

    public Builder setReportedAtMs(final long reportedAtMs) {
      this.reportedAtMs = reportedAtMs;
      return this;
    }

    public Builder setNotes(final String notes) {
      validateNotes(notes);
      this.notes = notes;
      return this;
    }

    public ReportKaigiRelayHealthInstruction build() {
      if (callId == null) {
        throw new IllegalStateException("callId must be provided");
      }
      if (relayId == null) {
        throw new IllegalStateException("relayId must be provided");
      }
      if (status == null) {
        throw new IllegalStateException("status must be provided");
      }
      if (reportedAtMs == null) {
        throw new IllegalStateException("reportedAtMs must be provided");
      }
      return new ReportKaigiRelayHealthInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> result = new LinkedHashMap<>();
      result.put("action", ACTION);
      KaigiInstructionUtils.appendCallId(callId, result, "call");
      result.put("relay_id", relayId);
      result.put("status", status.wireName());
      result.put("reported_at_ms", Long.toUnsignedString(reportedAtMs));
      if (notes != null) {
        result.put("notes", notes);
      }
      return result;
    }
  }

  private static void validateNotes(final String notes) {
    if (notes != null) {
      requireWellFormedUtf16(notes);
      if (notes.codePointCount(0, notes.length()) > MAX_NOTES_CHARACTERS) {
        throw new IllegalArgumentException("relay health notes must not exceed 512 characters");
      }
    }
  }

  private static void requireWellFormedUtf16(final String value) {
    for (int index = 0; index < value.length(); index++) {
      final char current = value.charAt(index);
      if (Character.isHighSurrogate(current)) {
        if (index + 1 >= value.length()
            || !Character.isLowSurrogate(value.charAt(index + 1))) {
          throw new IllegalArgumentException(
              "relay health notes must not contain unpaired UTF-16 surrogates");
        }
        index++;
      } else if (Character.isLowSurrogate(current)) {
        throw new IllegalArgumentException(
            "relay health notes must not contain unpaired UTF-16 surrogates");
      }
    }
  }
}
