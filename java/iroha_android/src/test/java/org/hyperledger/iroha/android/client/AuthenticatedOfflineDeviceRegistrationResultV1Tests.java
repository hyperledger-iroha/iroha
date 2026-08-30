// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertThrows;

import java.nio.charset.StandardCharsets;
import java.util.List;
import org.junit.Test;

public final class AuthenticatedOfflineDeviceRegistrationResultV1Tests {
  private static final String TRANSACTION_HASH = "ab".repeat(32);
  private static final String BLOCK_HASH = "cd".repeat(32);
  private static final String RESULT_HASH = "ef".repeat(32);

  @Test
  public void parsesClosedAppliedAndTypedRejectionShapes() {
    final AuthenticatedOfflineDeviceRegistrationResultV1 applied = parse(fields(
        "\"terminal_state\":\"applied\"," +
            "\"eligibility_outcome\":null," +
            "\"eligibility_reason\":null," +
            "\"matched_rule_ids\":[]," +
            "\"rejection_code\":null," +
            "\"rejection_message\":null"));
    assertEquals(
        AuthenticatedOfflineDeviceRegistrationResultV1.TerminalState.APPLIED,
        applied.terminalState());
    assertNull(applied.eligibilityOutcome());
    assertEquals("18446744073709551615", applied.committedBlockHeight().toString());

    final AuthenticatedOfflineDeviceRegistrationResultV1 drainOnly = parse(fields(
        "\"terminal_state\":\"eligibility_rejected\"," +
            "\"eligibility_outcome\":\"drain_only\"," +
            "\"eligibility_reason\":\"vulnerable_firmware\"," +
            "\"matched_rule_ids\":[\"samsung-2021-keymaster\",\"samsung-cve-2026-21046\"]," +
            "\"rejection_code\":\"offline_device_eligibility\"," +
            "\"rejection_message\":\"reviewed safe floor is not satisfied\""));
    assertEquals(
        AuthenticatedOfflineDeviceRegistrationResultV1.TerminalState.ELIGIBILITY_REJECTED,
        drainOnly.terminalState());
    assertEquals(
        AuthenticatedOfflineDeviceRegistrationResultV1.EligibilityOutcome.DRAIN_ONLY,
        drainOnly.eligibilityOutcome());
    assertEquals(
        AuthenticatedOfflineDeviceRegistrationResultV1.EligibilityReason.VULNERABLE_FIRMWARE,
        drainOnly.eligibilityReason());
    assertEquals(
        List.of("samsung-2021-keymaster", "samsung-cve-2026-21046"),
        drainOnly.matchedRuleIds());

    final AuthenticatedOfflineDeviceRegistrationResultV1 cryptographic = parse(fields(
        "\"terminal_state\":\"eligibility_rejected\"," +
            "\"eligibility_outcome\":\"cryptographically_rejected\"," +
            "\"eligibility_reason\":\"cryptographic_attestation_rejected\"," +
            "\"matched_rule_ids\":[]," +
            "\"rejection_code\":\"offline_device_eligibility\"," +
            "\"rejection_message\":\"verified boot was not authenticated\""));
    assertEquals(
        AuthenticatedOfflineDeviceRegistrationResultV1.EligibilityOutcome
            .CRYPTOGRAPHICALLY_REJECTED,
        cryptographic.eligibilityOutcome());
  }

  @Test
  public void rejectsContradictoryTerminalAndDecisionShapes() {
    assertRejected(fields(
        "\"terminal_state\":\"applied\"," +
            "\"eligibility_outcome\":null," +
            "\"eligibility_reason\":null," +
            "\"matched_rule_ids\":[]," +
            "\"rejection_code\":\"validation\"," +
            "\"rejection_message\":\"contradiction\""));
    assertRejected(fields(
        "\"terminal_state\":\"eligibility_rejected\"," +
            "\"eligibility_outcome\":\"drain_only\"," +
            "\"eligibility_reason\":\"vulnerable_firmware\"," +
            "\"matched_rule_ids\":[]," +
            "\"rejection_code\":\"offline_device_eligibility\"," +
            "\"rejection_message\":\"missing governed rule\""));
    assertRejected(fields(
        "\"terminal_state\":\"eligibility_rejected\"," +
            "\"eligibility_outcome\":\"cryptographically_rejected\"," +
            "\"eligibility_reason\":\"policy_not_fresh\"," +
            "\"matched_rule_ids\":[]," +
            "\"rejection_code\":\"offline_device_eligibility\"," +
            "\"rejection_message\":\"crossed outcome and reason\""));
    assertRejected(fields(
        "\"terminal_state\":\"other_rejected\"," +
            "\"eligibility_outcome\":\"drain_only\"," +
            "\"eligibility_reason\":\"policy_not_fresh\"," +
            "\"matched_rule_ids\":[]," +
            "\"rejection_code\":\"validation\"," +
            "\"rejection_message\":\"untyped rejection\""));
    assertRejected(fields(
        "\"terminal_state\":\"other_rejected\"," +
            "\"eligibility_outcome\":null," +
            "\"eligibility_reason\":null," +
            "\"matched_rule_ids\":[]," +
            "\"rejection_code\":\"unknown\"," +
            "\"rejection_message\":\"unknown rejection class\""));
  }

  @Test
  public void rejectsNoncanonicalFieldsAndRuleSets() {
    final String applied = fields(
        "\"terminal_state\":\"applied\"," +
            "\"eligibility_outcome\":null," +
            "\"eligibility_reason\":null," +
            "\"matched_rule_ids\":[]," +
            "\"rejection_code\":null," +
            "\"rejection_message\":null");
    assertRejected(applied.replace("\"version\":1", "\"version\":1,\"extra\":true"));
    assertRejected(applied.replace(TRANSACTION_HASH, TRANSACTION_HASH.toUpperCase()));
    assertRejected(applied.replace("18446744073709551615", "01"));
    assertRejected(applied.replace("18446744073709551615", "18446744073709551616"));

    final String duplicateRules = fields(
        "\"terminal_state\":\"eligibility_rejected\"," +
            "\"eligibility_outcome\":\"drain_only\"," +
            "\"eligibility_reason\":\"vulnerable_firmware\"," +
            "\"matched_rule_ids\":[\"same\",\"same\"]," +
            "\"rejection_code\":\"offline_device_eligibility\"," +
            "\"rejection_message\":\"duplicate rule\"");
    assertRejected(duplicateRules);
    assertThrows(
        IllegalStateException.class,
        () ->
            AuthenticatedOfflineDeviceRegistrationResultV1.parseNativeJson(
                new byte[] {(byte) 0xc3, (byte) 0x28}));
  }

  private static AuthenticatedOfflineDeviceRegistrationResultV1 parse(final String json) {
    return AuthenticatedOfflineDeviceRegistrationResultV1.parseNativeJson(
        json.getBytes(StandardCharsets.UTF_8));
  }

  private static void assertRejected(final String json) {
    assertThrows(IllegalStateException.class, () -> parse(json));
  }

  private static String fields(final String terminalFields) {
    return "{" +
        "\"version\":1," +
        "\"transaction_hash_hex\":\"" + TRANSACTION_HASH + "\"," +
        "\"transaction_authority\":\"canonical-i105-authority\"," +
        "\"block_hash_hex\":\"" + BLOCK_HASH + "\"," +
        "\"result_hash_hex\":\"" + RESULT_HASH + "\"," +
        "\"committed_block_height\":\"18446744073709551615\"," +
        terminalFields +
        "}";
  }
}
