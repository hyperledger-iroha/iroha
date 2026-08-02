// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.consensus;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.NativeAmxParticipantApplication;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.NativeAmxParticipantApplicationState;
import org.hyperledger.iroha.sdk.consensus.NativeAmxV2;
import org.junit.Test;

/** Shared grouped Native AMX v2 fixture-consumption tests. */
public final class NativeAmxV2GroupedFixtureTests {
  private static final BigInteger U64_MAX =
      BigInteger.ONE.shiftLeft(Long.SIZE).subtract(BigInteger.ONE);

  @Test
  public void rustOwnedGroupedGoldenIsConsumable() throws Exception {
    final Map<String, Object> fixture = fixture();
    assertEquals("iroha-native-amx-v2-grouped", string(fixture, "format"));
    assertEquals(1L, number(fixture, "fixture_version"));
    assertEquals("iroha_data_model::block::consensus", string(fixture, "rust_owner"));

    final Map<String, Object> golden = object(fixture, "golden");
    final Map<String, Object> groupWire = object(golden, "receipt_group");
    final NativeAmxV2Models.ReceiptGroup group =
        NativeAmxV2Models.parseReceiptGroup(groupWire);
    assertEquals(BigInteger.valueOf(42L), group.blockHeight());
    assertEquals(BigInteger.valueOf(11L), group.dataspaceId());
    assertEquals(2L, group.transactionCount());
    final List<Object> sourceOrder = array(golden, "ordered_source_ids");
    final List<String> actualSources = new ArrayList<>();
    final NativeAmxV2Models.Leg firstLeg = group.receipts().get(0).legs().get(0);
    assertEquals(
        "hash:33F884E54077B6570826E5DB30B64CEA24B8B559C057F152848E4D1DE7FE8041#6EF8",
        firstLeg.participantProposal().descriptor().validatorSetHash().value());
    assertEquals(
        "hash:568077DEBB5ECE0F6655571DBD81F8B8935CA5FB064F6B74864B4F58F3CB1A33#E6A5",
        firstLeg.participantProposal().descriptor().descriptorHash().value());
    assertEquals(
        "hash:AAC0F352914C21699F3F8D571196C9A5DFCAA9EF1272A7DEFA7FFD35A93C21AD#8B3F",
        firstLeg.participantProposal().proposalHash().value());
    assertEquals(
        "hash:C6B18DBE6BEC468DB021B79604233F3CB9E2D6CDF3384C491CE7A6DA89747825#9D72",
        firstLeg.participantSettlementHash().value());
    final NativeAmxV2Models.Leg remoteLeg = group.receipts().get(0).legs().get(1);
    assertEquals(
        "hash:40C7FCA7AA143B323B473A9958B96F49896C03C3547B83DD340FAE2FC1A85D29#B452",
        remoteLeg.participantSettlementHash().value());
    assertTrue(
        NativeAmxV2Models.isCanonicalBlsNormalPeerId(
            firstLeg.participantProposal().descriptor().validatorSet().get(0)));
    for (final NativeAmxV2Models.Receipt receipt : group.receipts()) {
      actualSources.add(receipt.sourceId().value());
      assertEquals(2, receipt.legs().size());
      assertEquals(BigInteger.valueOf(11L), receipt.dataspaceId());
      assertEquals(BigInteger.valueOf(40L), receipt.authorityContextHeight());
      assertEquals(BigInteger.valueOf(9L), receipt.laneBlockView());
      for (final NativeAmxV2Models.Leg leg : receipt.legs()) {
        assertEquals(NativeAmxV2Models.Phase.PREPARE, leg.prepareQc().body().phase());
        assertEquals(NativeAmxV2Models.Phase.COMMIT, leg.commitQc().body().phase());
        assertEquals(BigInteger.valueOf(6L), leg.prepareQc().body().round().view());
        assertEquals(
            BigInteger.valueOf(9L), leg.prepareQc().body().coordinatorLaneBlockView());
        assertEquals(96, leg.prepareQc().aggregateSignature().size());
      }
    }
    assertEquals(sourceOrder, actualSources);

    final Map<String, Object> diagnostics = object(golden, "expected_diagnostics");
    assertEquals(groupWire, object(array(diagnostics, "lane_settlement_commitments").get(0)));
    final Map<String, Object> row =
        object(array(diagnostics, "native_amx_participant_applications").get(0));
    final NativeAmxParticipantApplication application =
        new NativeAmxParticipantApplication(
            number(row, "lane_id"),
            unsigned64(row, "dataspace_id"),
            string(row, "lane_incarnation"),
            unsigned64(row, "participant_height"),
            unsigned64(row, "participant_view"),
            unsigned64(row, "predecessor_height"),
            optionalString(row, "predecessor_descriptor_hash"),
            string(row, "descriptor_hash"),
            string(row, "proposal_hash"),
            string(row, "settlement_hash"),
            number(row, "source_count"),
            optionalUnsigned64(row, "application_block_height"),
            optionalString(row, "application_block_hash"),
            NativeAmxParticipantApplicationState.fromWireName(string(row, "state")));
    assertEquals(2L, application.sourceCount());
    assertEquals(
        NativeAmxParticipantApplicationState.DURABLY_APPLIED, application.state());
    validateApplicationEvidence(fixture);
  }

  @Test
  public void rustOwnedNegativeCorpusIsConsumable() throws Exception {
    final Map<String, Object> canonical = fixture();
    final List<Object> controls = array(canonical, "negative_controls");
    final Set<String> identifiers = new HashSet<>();
    for (final Object controlValue : controls) {
      identifiers.add(string(object(controlValue), "id"));
    }
    assertTrue(
        identifiers.containsAll(
            java.util.Arrays.asList(
                "coherent_forged_validator_set_hash",
                "coherent_stale_descriptor_hash",
                "coherent_stale_proposal_hash",
                "coherent_stale_settlement_hash",
                "coherent_duplicate_validator_set",
                "coherent_over_quorum_requirement",
                "non_canonical_validator_peer_id",
                "execution_commitment_merge_carrier_wrong_version",
                "execution_commitment_missing_merge_carrier_field")));
    assertFalse(
        NativeAmxV2Models.isCanonicalBlsNormalPeerId(
            "ea0130"
                + "000000000000000000000000000000000000000000000000"
                + "000000000000000000000000000000000000000000000000"));
    for (final Object controlValue : controls) {
      final Map<String, Object> document = fixture();
      final Map<String, Object> control = object(controlValue);
      final String identifier = string(control, "id");
      assertEquals("reject", string(control, "expectation"));
      for (final Object mutation : array(control, "mutations")) {
        applyMutation(document, object(mutation));
      }
      if ("application_evidence".equals(string(control, "validator"))) {
        assertThrows(
            identifier,
            IllegalArgumentException.class,
            () -> validateApplicationEvidence(document));
        continue;
      }
      assertEquals("receipt_group", string(control, "validator"));
      final Map<String, Object> group =
          object(object(document, "golden"), "receipt_group");
      assertThrows(
          identifier,
          IllegalArgumentException.class,
          () -> NativeAmxV2Models.parseReceiptGroup(group));
    }
  }

  @Test
  public void mixedRoleParticipantExposesDeferredAnchorValidation() throws Exception {
    final Map<String, Object> document = fixture();
    final NativeAmxV2Models.ReceiptGroup group =
        NativeAmxV2Models.parseReceiptGroup(
            object(object(document, "golden"), "receipt_group"));
    NativeAmxV2Models.Leg remote = null;
    for (final NativeAmxV2Models.Leg leg : group.receipts().get(0).legs()) {
      if (leg.laneId() == 8L) {
        remote = leg;
      }
    }
    assertTrue(remote != null);
    assertFalse(
        NativeAmxV2Models.requiresMixedRoleAnchorValidation(
            remote.participantProposal().descriptor(),
            remote.prepareQc().body().transactionEntrypointHash().value()));
    assertTrue(
        NativeAmxV2Models.requiresMixedRoleAnchorValidation(
            remote.participantProposal().descriptor(),
            "hash:07BAE6F998F2D195BD9481ADDFB26789F771FDD7F6BB476A9C3157F70FB85AB7#9781"));
  }

  @Test
  public void javaParserPreservesCompleteNativeUnsigned64Tokens() throws Exception {
    final Map<String, Object> document = fixtureWithEpochToken(U64_MAX.toString());
    final NativeAmxV2Models.ReceiptGroup group =
        NativeAmxV2Models.parseReceiptGroup(
            object(object(document, "golden"), "receipt_group"));

    for (final NativeAmxV2Models.Receipt receipt : group.receipts()) {
      for (final NativeAmxV2Models.Leg leg : receipt.legs()) {
        assertEquals(U64_MAX, leg.prepareQc().body().epoch());
        assertEquals(U64_MAX, leg.commitQc().body().epoch());
      }
    }
  }

  @Test
  public void javaParserRejectsInvalidNativeUnsigned64Tokens() throws Exception {
    final String[] rejected = {
      U64_MAX.add(BigInteger.ONE).toString(),
      "-1",
      "1.0",
      "1e0",
      "\"" + U64_MAX + "\""
    };
    for (final String token : rejected) {
      final Map<String, Object> document = fixtureWithEpochToken(token);
      final Map<String, Object> group =
          object(object(document, "golden"), "receipt_group");
      assertThrows(
          token,
          IllegalArgumentException.class,
          () -> NativeAmxV2Models.parseReceiptGroup(group));
    }
    assertThrows(IllegalStateException.class, () -> fixtureWithEpochToken("01"));
  }

  private static void validateApplicationEvidence(final Map<String, Object> document) {
    final Map<String, Object> golden = object(document, "golden");
    final Map<String, Object> group = object(golden, "receipt_group");
    final Map<String, Object> evidence = object(golden, "application_evidence");
    final Map<String, Object> execution = object(evidence, "execution_commitment");
    final List<Object> artifacts = array(evidence, "manifest_artifacts");
    require(number(execution, "native_amx_application_manifest_version") == 1L);
    require(execution.containsKey("merge_carrier"));
    final Object rawMergeCarrier = execution.get("merge_carrier");
    require(rawMergeCarrier instanceof Map);
    final Map<String, Object> mergeCarrier = object(rawMergeCarrier);
    require(
        mergeCarrier
            .keySet()
            .equals(new HashSet<>(Arrays.asList("version", "entry_hash"))));
    final Object mergeCarrierVersion = mergeCarrier.get("version");
    require(
        mergeCarrierVersion instanceof BigInteger
            || mergeCarrierVersion instanceof Byte
            || mergeCarrierVersion instanceof Short
            || mergeCarrierVersion instanceof Integer
            || mergeCarrierVersion instanceof Long);
    require(new BigInteger(mergeCarrierVersion.toString()).equals(BigInteger.ONE));
    require(mergeCarrier.get("entry_hash") instanceof String);
    new NativeAmxV2.ConsensusHash(string(mergeCarrier, "entry_hash"));
    require(
        number(execution, "native_amx_application_manifest_count") == artifacts.size()
            && artifacts.size() == 1);
    final Map<String, Object> artifact = object(artifacts.get(0));
    final Map<String, Object> leaf = object(artifact, "leaf");
    final Map<String, Object> proof = object(artifact, "proof");
    require(number(artifact, "version") == 1L && number(leaf, "version") == 1L);
    require(number(artifact, "leaf_index") == 0L && number(proof, "leaf_index") == 0L);
    require(array(proof, "audit_path").isEmpty());
    require(number(artifact, "manifest_leaf_count") == 1L);
    require(
        Objects.equals(
            artifact.get("manifest_root"),
            execution.get("native_amx_application_manifest_root")));
    require(Objects.equals(artifact.get("manifest_root"), artifact.get("leaf_hash")));
    require(
        Objects.equals(
            leaf.get("executed_block_wire_hash"), execution.get("executed_block_wire_hash")));
    require(unsigned64(execution, "executed_block_wire_len").equals(BigInteger.valueOf(49)));
    require(number(leaf, "predecessor_height") + 1L == number(leaf, "participant_height"));

    final Map<String, Object> active =
        object(array(evidence, "active_lane_incarnations").get(0));
    require(array(evidence, "active_lane_incarnations").size() == 1);
    require(Objects.equals(active.get("lane_id"), leaf.get("lane_id")));
    require(Objects.equals(active.get("dataspace_id"), leaf.get("dataspace_id")));
    require(Objects.equals(active.get("lane_incarnation"), leaf.get("lane_incarnation")));
    require(
        !Objects.equals(leaf.get("lane_id"), group.get("lane_id"))
            || !Objects.equals(leaf.get("dataspace_id"), group.get("dataspace_id")));

    final List<Object> members = array(leaf, "members");
    final List<Object> receipts = array(group, "native_amx_receipts");
    require(!members.isEmpty() && members.size() <= 4096 && members.size() == receipts.size());
    final List<Object> memberSources = new ArrayList<>();
    final List<Object> receiptSources = new ArrayList<>();
    long previousIndex = -1L;
    for (final Object memberValue : members) {
      final Map<String, Object> member = object(memberValue);
      memberSources.add(member.get("source_id"));
      final long entrypointIndex = number(member, "entrypoint_index");
      require(entrypointIndex > previousIndex);
      previousIndex = entrypointIndex;
    }
    for (final Object receiptValue : receipts) {
      receiptSources.add(object(receiptValue).get("source_id"));
    }
    require(memberSources.equals(receiptSources));
    require(new HashSet<>(memberSources).size() == memberSources.size());
    final Set<Object> carrierEntrypoints =
        new HashSet<>(array(evidence, "carrier_entrypoint_hashes"));

    for (int index = 0; index < receipts.size(); index++) {
      final Map<String, Object> receipt = object(receipts.get(index));
      final Map<String, Object> member = object(members.get(index));
      Map<String, Object> matchingLeg = null;
      for (final Object legValue : array(receipt, "legs")) {
        final Map<String, Object> candidate = object(legValue);
        if (Objects.equals(candidate.get("lane_id"), leaf.get("lane_id"))
            && Objects.equals(candidate.get("dataspace_id"), leaf.get("dataspace_id"))) {
          require(matchingLeg == null);
          matchingLeg = candidate;
        }
      }
      require(matchingLeg != null);
      final Map<String, Object> proposal = object(matchingLeg, "participant_proposal");
      final Map<String, Object> descriptor = object(proposal, "descriptor");
      require(
          Objects.equals(descriptor.get("lane_incarnation"), leaf.get("lane_incarnation")));
      require(
          Objects.equals(descriptor.get("lane_block_height"), leaf.get("participant_height")));
      require(Objects.equals(descriptor.get("lane_block_view"), leaf.get("participant_view")));
      require(
          Objects.equals(
              descriptor.get("previous_lane_block_height"), leaf.get("predecessor_height")));
      require(
          Objects.equals(
              descriptor.get("previous_lane_block_descriptor_hash"),
              leaf.get("predecessor_descriptor_hash")));
      require(Objects.equals(descriptor.get("descriptor_hash"), leaf.get("descriptor_hash")));
      require(Objects.equals(proposal.get("proposal_hash"), leaf.get("proposal_hash")));
      require(
          Objects.equals(matchingLeg.get("participant_settlement_hash"), leaf.get("settlement_hash")));
      final Map<String, Object> body =
          object(object(matchingLeg, "prepare_qc"), "body");
      require(Objects.equals(body.get("source_id"), member.get("source_id")));
      require(
          Objects.equals(body.get("tx_entrypoint_hash"), member.get("entrypoint_hash")));
      require(
          array(descriptor, "accepted_candidate_indices").contains(member.get("entrypoint_index")));
      require(
          carrierEntrypoints.containsAll(array(descriptor, "accepted_transaction_hashes")));
    }

    final Map<String, Object> row =
        object(
            array(object(golden, "expected_diagnostics"), "native_amx_participant_applications")
                .get(0));
    final String[] identityFields = {
      "lane_id",
      "dataspace_id",
      "lane_incarnation",
      "participant_height",
      "participant_view",
      "predecessor_height",
      "predecessor_descriptor_hash",
      "descriptor_hash",
      "proposal_hash",
      "settlement_hash",
      "application_block_height",
      "application_block_hash"
    };
    for (final String field : identityFields) {
      require(Objects.equals(row.get(field), leaf.get(field)));
    }
    require(number(row, "source_count") == members.size());
  }

  private static void require(final boolean condition) {
    if (!condition) {
      throw new IllegalArgumentException("invalid Native AMX application evidence fixture");
    }
  }

  private static void applyMutation(
      final Map<String, Object> root, final Map<String, Object> mutation) {
    final String operation = string(mutation, "op");
    final String path = string(mutation, "path");
    switch (operation) {
      case "replace" -> assign(root, path, mutation.get("value"));
      case "remove" -> remove(root, path);
      case "copy" -> {
        final String source = string(object(mutation, "value"), "from");
        assign(root, path, resolve(root, source));
      }
      case "swap" -> {
        final Map<String, Object> options = object(mutation, "value");
        final List<Object> target = list(resolve(root, path));
        final int left = (int) number(options, "left");
        final int right = (int) number(options, "right");
        final Object temporary = target.get(left);
        target.set(left, target.get(right));
        target.set(right, temporary);
      }
      case "repeat" -> {
        final Map<String, Object> options = object(mutation, "value");
        final List<Object> target = list(resolve(root, path));
        final Object source = target.get((int) number(options, "source_index"));
        final List<Object> repeated = new ArrayList<>();
        for (int index = 0; index < number(options, "count"); index++) {
          repeated.add(source);
        }
        assign(root, path, repeated);
      }
      default -> throw new AssertionError("unsupported fixture mutation " + operation);
    }
  }

  private static Object resolve(final Object root, final String pointer) {
    Object target = root;
    for (final String token : pointerTokens(pointer)) {
      target =
          target instanceof List
              ? list(target).get(Integer.parseInt(token))
              : object(target).get(token);
    }
    return target;
  }

  private static void assign(final Object root, final String pointer, final Object value) {
    final List<String> tokens = pointerTokens(pointer);
    Object parent = root;
    for (int index = 0; index < tokens.size() - 1; index++) {
      final String token = tokens.get(index);
      parent =
          parent instanceof List
              ? list(parent).get(Integer.parseInt(token))
              : object(parent).get(token);
    }
    final String leaf = tokens.get(tokens.size() - 1);
    if (parent instanceof List) {
      list(parent).set(Integer.parseInt(leaf), value);
    } else {
      object(parent).put(leaf, value);
    }
  }

  private static void remove(final Object root, final String pointer) {
    final List<String> tokens = pointerTokens(pointer);
    Object parent = root;
    for (int index = 0; index < tokens.size() - 1; index++) {
      final String token = tokens.get(index);
      parent =
          parent instanceof List
              ? list(parent).get(Integer.parseInt(token))
              : object(parent).get(token);
    }
    final String leaf = tokens.get(tokens.size() - 1);
    if (parent instanceof List) {
      list(parent).remove(Integer.parseInt(leaf));
    } else {
      object(parent).remove(leaf);
    }
  }

  private static List<String> pointerTokens(final String pointer) {
    if (!pointer.startsWith("/")) {
      throw new AssertionError("fixture mutation path must be an absolute JSON pointer");
    }
    final List<String> tokens = new ArrayList<>();
    for (final String token : pointer.substring(1).split("/", -1)) {
      tokens.add(token.replace("~1", "/").replace("~0", "~"));
    }
    return tokens;
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> fixture() throws Exception {
    return (Map<String, Object>)
        JsonParser.parse(
            new String(Files.readAllBytes(fixturePath()), StandardCharsets.UTF_8));
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> fixtureWithEpochToken(final String token) throws Exception {
    final String canonical =
        new String(Files.readAllBytes(fixturePath()), StandardCharsets.UTF_8);
    final String mutated = canonical.replace("\"epoch\": 3", "\"epoch\": " + token);
    if (canonical.equals(mutated)) {
      throw new AssertionError("fixture contains no Native AMX epoch token");
    }
    return (Map<String, Object>) JsonParser.parse(mutated);
  }

  private static Path fixturePath() {
    Path current = Paths.get("").toAbsolutePath();
    while (current != null) {
      final Path candidate =
          current.resolve("fixtures/sumeragi_v2/native_amx_v2_grouped.json");
      if (Files.isRegularFile(candidate)) {
        return candidate;
      }
      current = current.getParent();
    }
    throw new AssertionError("fixtures/sumeragi_v2/native_amx_v2_grouped.json was not found");
  }

  private static String optionalString(
      final Map<String, Object> object, final String name) {
    return object.get(name) == null ? null : (String) object.get(name);
  }

  private static BigInteger optionalUnsigned64(
      final Map<String, Object> object, final String name) {
    return object.get(name) == null ? null : unsigned64(object, name);
  }

  private static String string(final Map<String, Object> object, final String name) {
    return (String) object.get(name);
  }

  private static long number(final Map<String, Object> object, final String name) {
    return ((Number) object.get(name)).longValue();
  }

  private static BigInteger unsigned64(
      final Map<String, Object> object, final String name) {
    final Object value = object.get(name);
    final BigInteger parsed;
    if (value instanceof BigInteger) {
      parsed = (BigInteger) value;
    } else if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      parsed = BigInteger.valueOf(((Number) value).longValue());
    } else {
      throw new AssertionError(name + " must be an integer");
    }
    if (parsed.signum() < 0 || parsed.compareTo(U64_MAX) > 0) {
      throw new AssertionError(name + " must fit in unsigned 64-bit range");
    }
    return parsed;
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> object(final Object value) {
    return (Map<String, Object>) value;
  }

  private static Map<String, Object> object(
      final Map<String, Object> value, final String name) {
    return object(value.get(name));
  }

  @SuppressWarnings("unchecked")
  private static List<Object> list(final Object value) {
    return (List<Object>) value;
  }

  private static List<Object> array(
      final Map<String, Object> value, final String name) {
    return list(value.get(name));
  }
}
