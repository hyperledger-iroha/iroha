package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** Native-verified committed result for exactly one offline-device registration instruction. */
public final class AuthenticatedOfflineDeviceRegistrationResultV1 {
  private static final int JSON_MAX_BYTES = 128 * 1024;

  public enum TerminalState {
    APPLIED,
    ELIGIBILITY_REJECTED,
    OTHER_REJECTED
  }

  public enum EligibilityOutcome {
    DRAIN_ONLY,
    CRYPTOGRAPHICALLY_REJECTED
  }

  public enum EligibilityReason {
    CRYPTOGRAPHIC_ATTESTATION_REJECTED,
    POLICY_NOT_FRESH,
    INCOMPLETE_ATTESTED_PROPERTIES,
    UNSUPPORTED_PRE_ANDROID_12_TEE,
    VULNERABLE_FIRMWARE,
    PERMANENTLY_BLOCKED_DEVICE
  }

  private static final Set<String> EXACT_KEYS =
      Collections.unmodifiableSet(
          new HashSet<>(
              Arrays.asList(
                  "version",
                  "transaction_hash_hex",
                  "transaction_authority",
                  "block_hash_hex",
                  "result_hash_hex",
                  "committed_block_height",
                  "terminal_state",
                  "eligibility_outcome",
                  "eligibility_reason",
                  "matched_rule_ids",
                  "rejection_code",
                  "rejection_message")));
  private static final Set<String> OTHER_REJECTION_CODES =
      Collections.unmodifiableSet(
          new HashSet<>(
              Arrays.asList(
                  "account_does_not_exist",
                  "limit_check",
                  "validation",
                  "instruction_execution",
                  "ivm_execution",
                  "trigger_execution")));

  private final String transactionHashHex;
  private final String transactionAuthorityAccountId;
  private final String blockHashHex;
  private final String resultHashHex;
  private final BigInteger committedBlockHeight;
  private final TerminalState terminalState;
  private final EligibilityOutcome eligibilityOutcome;
  private final EligibilityReason eligibilityReason;
  private final List<String> matchedRuleIds;
  private final String rejectionCode;
  private final String rejectionMessage;

  private AuthenticatedOfflineDeviceRegistrationResultV1(
      final String transactionHashHex,
      final String transactionAuthorityAccountId,
      final String blockHashHex,
      final String resultHashHex,
      final BigInteger committedBlockHeight,
      final TerminalState terminalState,
      final EligibilityOutcome eligibilityOutcome,
      final EligibilityReason eligibilityReason,
      final List<String> matchedRuleIds,
      final String rejectionCode,
      final String rejectionMessage) {
    this.transactionHashHex = transactionHashHex;
    this.transactionAuthorityAccountId = transactionAuthorityAccountId;
    this.blockHashHex = blockHashHex;
    this.resultHashHex = resultHashHex;
    this.committedBlockHeight = committedBlockHeight;
    this.terminalState = terminalState;
    this.eligibilityOutcome = eligibilityOutcome;
    this.eligibilityReason = eligibilityReason;
    this.matchedRuleIds = Collections.unmodifiableList(new ArrayList<>(matchedRuleIds));
    this.rejectionCode = rejectionCode;
    this.rejectionMessage = rejectionMessage;
  }

  static AuthenticatedOfflineDeviceRegistrationResultV1 parseNativeJson(final byte[] payload) {
    if (payload == null || payload.length == 0 || payload.length > JSON_MAX_BYTES) {
      throw new IllegalStateException("native registration-result JSON violates its byte bound");
    }
    final String json = new String(payload, StandardCharsets.UTF_8);
    if (!Arrays.equals(payload, json.getBytes(StandardCharsets.UTF_8))) {
      throw new IllegalStateException("native registration-result JSON is not exact UTF-8");
    }
    final Object parsed = JsonParser.parse(json);
    if (!(parsed instanceof Map)) {
      throw new IllegalStateException("native registration-result JSON must be an object");
    }
    final Map<?, ?> fields = (Map<?, ?>) parsed;
    if (!new HashSet<>(fields.keySet()).equals(EXACT_KEYS)
        || JsonNumbers.asInt(fields.get("version"), "registrationResult.version") != 1) {
      throw new IllegalStateException("native registration-result JSON has an invalid field set");
    }
    final TerminalState terminal = terminalState(requireString(fields, "terminal_state", 64));
    final String outcomeText = optionalString(fields, "eligibility_outcome", 64);
    final String reasonText = optionalString(fields, "eligibility_reason", 128);
    final EligibilityOutcome outcome = outcomeText == null ? null : eligibilityOutcome(outcomeText);
    final EligibilityReason reason = reasonText == null ? null : eligibilityReason(reasonText);
    final List<String> rules = requireRules(fields.get("matched_rule_ids"));
    final String code = optionalString(fields, "rejection_code", 128);
    final String message = optionalString(fields, "rejection_message", 1_024);
    validateTerminalShape(terminal, outcome, reason, rules, code, message);
    final String heightText = requireString(fields, "committed_block_height", 20);
    if (!heightText.matches("[1-9][0-9]{0,19}")) {
      throw new IllegalStateException("committed block height must be canonical positive decimal");
    }
    final BigInteger height;
    try {
      height = new BigInteger(heightText);
    } catch (final NumberFormatException error) {
      throw new IllegalStateException("committed block height is invalid", error);
    }
    if (height.signum() <= 0 || height.bitLength() > 64) {
      throw new IllegalStateException("committed block height must be a positive u64");
    }
    return new AuthenticatedOfflineDeviceRegistrationResultV1(
        requireHash(fields, "transaction_hash_hex"),
        requireString(fields, "transaction_authority", 16 * 1024),
        requireHash(fields, "block_hash_hex"),
        requireHash(fields, "result_hash_hex"),
        height,
        terminal,
        outcome,
        reason,
        rules,
        code,
        message);
  }

  private static void validateTerminalShape(
      final TerminalState terminal,
      final EligibilityOutcome outcome,
      final EligibilityReason reason,
      final List<String> rules,
      final String code,
      final String message) {
    if (terminal == TerminalState.APPLIED) {
      if (outcome != null || reason != null || !rules.isEmpty() || code != null || message != null) {
        throw new IllegalStateException("applied registration result carries rejection fields");
      }
      return;
    }
    if (code == null || message == null) {
      throw new IllegalStateException("rejected registration result omits authenticated reason");
    }
    if (terminal == TerminalState.OTHER_REJECTED) {
      if (outcome != null
          || reason != null
          || !rules.isEmpty()
          || !OTHER_REJECTION_CODES.contains(code)) {
        throw new IllegalStateException("untyped registration rejection carries eligibility state");
      }
      return;
    }
    if (!"offline_device_eligibility".equals(code) || outcome == null || reason == null) {
      throw new IllegalStateException("typed registration rejection has an invalid code or decision");
    }
    final boolean valid =
        (outcome == EligibilityOutcome.CRYPTOGRAPHICALLY_REJECTED
                && reason == EligibilityReason.CRYPTOGRAPHIC_ATTESTATION_REJECTED
                && rules.isEmpty())
            || (outcome == EligibilityOutcome.DRAIN_ONLY
                && ((reason == EligibilityReason.POLICY_NOT_FRESH
                        || reason == EligibilityReason.INCOMPLETE_ATTESTED_PROPERTIES
                        || reason == EligibilityReason.UNSUPPORTED_PRE_ANDROID_12_TEE)
                    && rules.isEmpty()
                    || (reason == EligibilityReason.VULNERABLE_FIRMWARE
                            || reason == EligibilityReason.PERMANENTLY_BLOCKED_DEVICE)
                        && !rules.isEmpty()));
    if (!valid) {
      throw new IllegalStateException("typed registration rejection has an invalid decision shape");
    }
  }

  private static List<String> requireRules(final Object value) {
    if (!(value instanceof List)) {
      throw new IllegalStateException("matched_rule_ids must be an array");
    }
    final List<?> values = (List<?>) value;
    if (values.size() > 256) {
      throw new IllegalStateException("matched_rule_ids exceeds its closed bound");
    }
    final List<String> result = new ArrayList<>(values.size());
    String previous = null;
    for (final Object entry : values) {
      if (!(entry instanceof String)) {
        throw new IllegalStateException("matched_rule_ids entries must be strings");
      }
      final String rule = requireCanonicalText((String) entry, "matched_rule_id", 128);
      if (!StandardCharsets.US_ASCII.newEncoder().canEncode(rule)
          || previous != null && previous.compareTo(rule) >= 0) {
        throw new IllegalStateException("matched_rule_ids must be sorted unique ASCII");
      }
      result.add(rule);
      previous = rule;
    }
    return result;
  }

  private static String requireHash(final Map<?, ?> fields, final String key) {
    final String value = requireString(fields, key, 64);
    if (!value.matches("[0-9a-f]{64}")) {
      throw new IllegalStateException(key + " must be an exact lowercase 32-byte hash");
    }
    return value;
  }

  private static String requireString(
      final Map<?, ?> fields, final String key, final int maximumBytes) {
    final Object value = fields.get(key);
    if (!(value instanceof String)) {
      throw new IllegalStateException(key + " must be a string");
    }
    return requireCanonicalText((String) value, key, maximumBytes);
  }

  private static String optionalString(
      final Map<?, ?> fields, final String key, final int maximumBytes) {
    final Object value = fields.get(key);
    if (value == null) {
      return null;
    }
    if (!(value instanceof String)) {
      throw new IllegalStateException(key + " must be null or a string");
    }
    return requireCanonicalText((String) value, key, maximumBytes);
  }

  private static String requireCanonicalText(
      final String value, final String field, final int maximumBytes) {
    if (value.isEmpty()
        || value.getBytes(StandardCharsets.UTF_8).length > maximumBytes
        || !value.equals(value.trim())
        || value.codePoints().anyMatch(Character::isISOControl)) {
      throw new IllegalStateException(field + " violates its closed text bound");
    }
    return value;
  }

  private static TerminalState terminalState(final String value) {
    switch (value) {
      case "applied": return TerminalState.APPLIED;
      case "eligibility_rejected": return TerminalState.ELIGIBILITY_REJECTED;
      case "other_rejected": return TerminalState.OTHER_REJECTED;
      default: throw new IllegalStateException("unknown registration terminal state");
    }
  }

  private static EligibilityOutcome eligibilityOutcome(final String value) {
    switch (value) {
      case "drain_only": return EligibilityOutcome.DRAIN_ONLY;
      case "cryptographically_rejected": return EligibilityOutcome.CRYPTOGRAPHICALLY_REJECTED;
      default: throw new IllegalStateException("unknown eligibility outcome");
    }
  }

  private static EligibilityReason eligibilityReason(final String value) {
    switch (value) {
      case "cryptographic_attestation_rejected":
        return EligibilityReason.CRYPTOGRAPHIC_ATTESTATION_REJECTED;
      case "policy_not_fresh": return EligibilityReason.POLICY_NOT_FRESH;
      case "incomplete_attested_properties":
        return EligibilityReason.INCOMPLETE_ATTESTED_PROPERTIES;
      case "unsupported_pre_android_12_tee":
        return EligibilityReason.UNSUPPORTED_PRE_ANDROID_12_TEE;
      case "vulnerable_firmware": return EligibilityReason.VULNERABLE_FIRMWARE;
      case "permanently_blocked_device": return EligibilityReason.PERMANENTLY_BLOCKED_DEVICE;
      default: throw new IllegalStateException("unknown eligibility reason");
    }
  }

  public String transactionHashHex() { return transactionHashHex; }
  public String transactionAuthorityAccountId() { return transactionAuthorityAccountId; }
  public String blockHashHex() { return blockHashHex; }
  public String resultHashHex() { return resultHashHex; }
  public BigInteger committedBlockHeight() { return committedBlockHeight; }
  public TerminalState terminalState() { return terminalState; }
  public EligibilityOutcome eligibilityOutcome() { return eligibilityOutcome; }
  public EligibilityReason eligibilityReason() { return eligibilityReason; }
  public List<String> matchedRuleIds() { return matchedRuleIds; }
  public String rejectionCode() { return rejectionCode; }
  public String rejectionMessage() { return rejectionMessage; }
}
