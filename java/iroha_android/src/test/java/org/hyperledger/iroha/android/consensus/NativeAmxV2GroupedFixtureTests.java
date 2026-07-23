// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.consensus;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.NativeAmxParticipantApplication;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.NativeAmxParticipantApplicationState;
import org.junit.Test;

/** Shared grouped Native AMX v2 fixture-consumption tests. */
public final class NativeAmxV2GroupedFixtureTests {
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
    final List<Object> sourceOrder = array(golden, "ordered_source_ids");
    final List<String> actualSources = new ArrayList<>();
    for (final NativeAmxV2Models.Receipt receipt : group.receipts()) {
      actualSources.add(receipt.sourceId().value());
      assertEquals(2, receipt.legs().size());
      assertEquals(9L, receipt.laneBlockView());
      for (final NativeAmxV2Models.Leg leg : receipt.legs()) {
        assertEquals(NativeAmxV2Models.Phase.PREPARE, leg.prepareQc().body().phase());
        assertEquals(NativeAmxV2Models.Phase.COMMIT, leg.commitQc().body().phase());
        assertEquals(6L, leg.prepareQc().body().round().view());
        assertEquals(9L, leg.prepareQc().body().coordinatorLaneBlockView());
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
            number(row, "dataspace_id"),
            string(row, "lane_incarnation"),
            number(row, "participant_height"),
            number(row, "participant_view"),
            number(row, "predecessor_height"),
            optionalString(row, "predecessor_descriptor_hash"),
            string(row, "descriptor_hash"),
            string(row, "proposal_hash"),
            string(row, "settlement_hash"),
            number(row, "source_count"),
            optionalLong(row, "application_block_height"),
            optionalString(row, "application_block_hash"),
            NativeAmxParticipantApplicationState.fromWireName(string(row, "state")));
    assertEquals(2L, application.sourceCount());
    assertEquals(
        NativeAmxParticipantApplicationState.DURABLY_APPLIED, application.state());
  }

  @Test
  public void rustOwnedNegativeCorpusIsConsumable() throws Exception {
    final Map<String, Object> canonical = fixture();
    for (final Object controlValue : array(canonical, "negative_controls")) {
      final Map<String, Object> document = fixture();
      final Map<String, Object> control = object(controlValue);
      final String identifier = string(control, "id");
      assertEquals("reject", string(control, "expectation"));
      for (final Object mutation : array(control, "mutations")) {
        applyMutation(document, object(mutation));
      }
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
    final String descriptorPath =
        "/golden/receipt_group/native_amx_receipts/0/legs/1/"
            + "participant_proposal/descriptor";
    final Map<String, Object> descriptor = object(resolve(document, descriptorPath));
    final List<Object> hashes = array(descriptor, "accepted_transaction_hashes");
    final List<Object> indices = array(descriptor, "accepted_candidate_indices");
    final List<Object> deferredHashes = new ArrayList<>();
    deferredHashes.add(hashes.get(1));
    final List<Object> deferredIndices = new ArrayList<>();
    deferredIndices.add(indices.get(1));
    assign(
        document,
        descriptorPath + "/accepted_transaction_hashes",
        deferredHashes);
    assign(
        document,
        descriptorPath + "/accepted_candidate_indices",
        deferredIndices);
    final NativeAmxV2Models.ReceiptGroup group =
        NativeAmxV2Models.parseReceiptGroup(
            object(object(document, "golden"), "receipt_group"));
    NativeAmxV2Models.Leg remote = null;
    for (final NativeAmxV2Models.Leg leg : group.receipts().get(0).legs()) {
      if (leg.laneId() == 8L) {
        remote = leg;
      }
    }
    assertEquals(true, remote != null && remote.requiresMixedRoleAnchorValidation());
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

  private static Long optionalLong(final Map<String, Object> object, final String name) {
    return object.get(name) == null ? null : ((Number) object.get(name)).longValue();
  }

  private static String string(final Map<String, Object> object, final String name) {
    return (String) object.get(name);
  }

  private static long number(final Map<String, Object> object, final String name) {
    return ((Number) object.get(name)).longValue();
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
