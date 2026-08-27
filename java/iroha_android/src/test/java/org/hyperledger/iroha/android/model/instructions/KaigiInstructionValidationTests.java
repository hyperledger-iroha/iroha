package org.hyperledger.iroha.android.model.instructions;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

public final class KaigiInstructionValidationTests {

  private KaigiInstructionValidationTests() {}

  public static void main(final String[] args) {
    usageDurationAboveSignedIntRangeRoundTrips();
    fullWidthUnsignedFieldsRoundTripThroughSignedJvmCarriers();
    rejectsOversizedAndSparseRelayManifestHopIndices();
    typedParsersRequireExactActionDiscriminator();
    parsersRejectUnknownFieldsAndRebuildImmutableCanonicalMaps();
    relayHealthReportsValidateStatusNotesAndCanonicalMaps();
    unitEnumPayloadsAndCallIdentifiersRejectMalformedState();
    rejectsInvalidRelayHpkeKey();
    rejectsInvalidRelayManifestHpkeKey();
    rejectsInvalidRelayManifestParse();
    rejectsInvalidCreateRelayManifestHpkeKey();
    rejectsInvalidJoinProofBase64();
    rejectsInvalidLeaveProofBase64();
    rejectsInvalidUsageProofBase64();
    rejectsZeroAndMissingBandwidthClass();
    rejectsStructurallyInvalidRelayManifests();
    relayManifestsAcceptEightHopsAndRejectNineInBuildersAndParsers();
    relayHpkeKeysAccept4096DecodedBytesAndReject4097();
    preservesLedgerSafePrivacyFields();
    rejectsClearPrivacyIdentityHints();
    System.out.println("[IrohaAndroid] KaigiInstructionValidationTests passed.");
  }

  private static void usageDurationAboveSignedIntRangeRoundTrips() {
    final long durationMs = 3_000_000_000L;
    final RecordKaigiUsageInstruction instruction =
        RecordKaigiUsageInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setDurationMs(durationMs)
            .build();

    final RecordKaigiUsageInstruction decoded =
        RecordKaigiUsageInstruction.fromArguments(instruction.toArguments());

    assertEquals(durationMs, decoded.durationMs());
    assertEquals(instruction, decoded);
  }

  private static void fullWidthUnsignedFieldsRoundTripThroughSignedJvmCarriers() {
    final long u64Max = -1L;
    final int u32Max = -1;
    final String u64MaxText = "18446744073709551615";
    final String u32MaxText = "4294967295";

    final CreateKaigiInstruction create =
        CreateKaigiInstruction.builder()
            .setCallId("wonderland", "unsigned-boundary")
            .setHost("host")
            .setMaxParticipants(u32Max)
            .setGasRatePerMinute(u64Max)
            .setScheduledStartMs(u64Max)
            .build();
    assertEquals(u32MaxText, create.toArguments().get("max_participants"));
    assertEquals(u64MaxText, create.toArguments().get("gas_rate_per_minute"));
    assertEquals(u64MaxText, create.toArguments().get("scheduled_start_ms"));
    assertEquals(create, CreateKaigiInstruction.fromArguments(create.toArguments()));

    final EndKaigiInstruction end =
        EndKaigiInstruction.builder()
            .setCallId("wonderland", "unsigned-boundary")
            .setEndedAtMs(u64Max)
            .build();
    assertEquals(u64MaxText, end.toArguments().get("ended_at_ms"));
    assertEquals(end, EndKaigiInstruction.fromArguments(end.toArguments()));

    final RecordKaigiUsageInstruction usage =
        RecordKaigiUsageInstruction.builder()
            .setCallId("wonderland", "unsigned-boundary")
            .setDurationMs(u64Max)
            .setBilledGas(u64Max)
            .build();
    assertEquals(u64MaxText, usage.toArguments().get("duration_ms"));
    assertEquals(u64MaxText, usage.toArguments().get("billed_gas"));
    assertEquals(usage, RecordKaigiUsageInstruction.fromArguments(usage.toArguments()));

    final ReportKaigiRelayHealthInstruction health =
        ReportKaigiRelayHealthInstruction.builder()
            .setCallId("wonderland", "unsigned-boundary")
            .setRelayId("relay")
            .setStatus(ReportKaigiRelayHealthInstruction.Status.HEALTHY)
            .setReportedAtMs(u64Max)
            .build();
    assertEquals(u64MaxText, health.toArguments().get("reported_at_ms"));
    assertEquals(
        health, ReportKaigiRelayHealthInstruction.fromArguments(health.toArguments()));

    final SetKaigiRelayManifestInstruction manifest =
        SetKaigiRelayManifestInstruction.builder()
            .setCallId("wonderland", "unsigned-boundary")
            .setRelayManifestExpiryMs(u64Max)
            .addRelayManifestHop("relay-alpha", key(1), 1)
            .addRelayManifestHop("relay-beta", key(2), 1)
            .addRelayManifestHop("relay-gamma", key(3), 1)
            .build();
    assertEquals(u64MaxText, manifest.toArguments().get("relay_manifest.expiry_ms"));
    assertEquals(manifest, SetKaigiRelayManifestInstruction.fromArguments(manifest.toArguments()));

    assertThrows(
        () ->
            RecordKaigiUsageInstruction.fromArguments(
                withArgument(usage.toArguments(), "duration_ms", "01")),
        "expected non-canonical duration to throw");
    assertThrows(
        () ->
            RecordKaigiUsageInstruction.fromArguments(
                withArgument(
                    usage.toArguments(), "billed_gas", "18446744073709551616")),
        "expected overflowing u64 to throw");
    assertThrows(
        () ->
            CreateKaigiInstruction.fromArguments(
                withArgument(create.toArguments(), "scheduled_start_ms", "")),
        "expected blank optional timestamp to throw");
    assertThrows(
        () ->
            CreateKaigiInstruction.fromArguments(
                withArgument(create.toArguments(), "max_participants", "4294967296")),
        "expected overflowing u32 to throw");
  }

  private static void rejectsOversizedAndSparseRelayManifestHopIndices() {
    final Map<String, String> oversized = new HashMap<>();
    oversized.put("action", "SetKaigiRelayManifest");
    oversized.put("call.domain_id", "wonderland");
    oversized.put("call.call_name", "weekly-sync");
    oversized.put("relay_manifest.expiry_ms", "100");
    oversized.put("relay_manifest.hop." + Integer.MAX_VALUE + ".relay_id", "relay-alpha");
    assertThrows(
        () -> SetKaigiRelayManifestInstruction.fromArguments(oversized),
        "expected oversized relay manifest hop index to throw");

    final Map<String, String> sparse = new HashMap<>();
    sparse.put("action", "SetKaigiRelayManifest");
    sparse.put("call.domain_id", "wonderland");
    sparse.put("call.call_name", "weekly-sync");
    sparse.put("relay_manifest.expiry_ms", "100");
    sparse.put("relay_manifest.hop.0.relay_id", "relay-alpha");
    sparse.put("relay_manifest.hop.0.hpke_public_key", key(1));
    sparse.put("relay_manifest.hop.0.weight", "1");
    sparse.put("relay_manifest.hop.2.relay_id", "relay-gamma");
    sparse.put("relay_manifest.hop.2.hpke_public_key", key(3));
    sparse.put("relay_manifest.hop.2.weight", "1");
    assertThrows(
        () -> SetKaigiRelayManifestInstruction.fromArguments(sparse),
        "expected sparse relay manifest hop indices to throw");

    final Map<String, String> nonCanonical = new HashMap<>();
    nonCanonical.put("action", "SetKaigiRelayManifest");
    nonCanonical.put("call.domain_id", "wonderland");
    nonCanonical.put("call.call_name", "weekly-sync");
    nonCanonical.put("relay_manifest.expiry_ms", "100");
    nonCanonical.put("relay_manifest.hop.00.relay_id", "relay-alpha");
    assertThrows(
        () -> SetKaigiRelayManifestInstruction.fromArguments(nonCanonical),
        "expected non-canonical relay manifest hop index to throw");
  }

  private static void typedParsersRequireExactActionDiscriminator() {
    final RecordKaigiUsageInstruction usage =
        RecordKaigiUsageInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setDurationMs(1)
            .build();
    assertThrows(
        () ->
            RecordKaigiUsageInstruction.fromArguments(
                withArgument(usage.toArguments(), "action", "EndKaigi")),
        "expected mismatched action to throw");
    final Map<String, String> missingAction = new HashMap<>(usage.toArguments());
    missingAction.remove("action");
    assertThrows(
        () -> RecordKaigiUsageInstruction.fromArguments(missingAction),
        "expected missing action to throw");
  }

  private static void parsersRejectUnknownFieldsAndRebuildImmutableCanonicalMaps() {
    final String rawHash = hash(1);
    final Map<String, String> shuffled = new LinkedHashMap<>();
    shuffled.put("metadata.z", "last");
    shuffled.put("commitment.commitment", rawHash);
    shuffled.put("host", "host");
    shuffled.put("call.call_name", "weekly-sync");
    shuffled.put("call.domain_id", "wonderland");
    shuffled.put("action", "CreateKaigi");
    shuffled.put("metadata.a", "first");

    final CreateKaigiInstruction parsedCreate =
        CreateKaigiInstruction.fromArguments(shuffled);
    assertEquals(
        Arrays.asList(
            "action",
            "call.domain_id",
            "call.call_name",
            "host",
            "gas_rate_per_minute",
            "metadata.a",
            "metadata.z",
            "privacy.mode",
            "room_policy.policy",
            "commitment.commitment"),
        new ArrayList<>(parsedCreate.toArguments().keySet()));
    assertEquals(
        KaigiInstructionUtils.canonicalizeHash(rawHash),
        parsedCreate.toArguments().get("commitment.commitment"));
    final String lowercaseLiteral =
        KaigiInstructionUtils.canonicalizeHash(hash(0xAB)).toLowerCase(Locale.ROOT);
    final CreateKaigiInstruction parsedLowercaseLiteral =
        CreateKaigiInstruction.fromArguments(
            withArgument(shuffled, "commitment.commitment", lowercaseLiteral));
    assertEquals(
        KaigiInstructionUtils.canonicalizeHash(hash(0xAB)),
        parsedLowercaseLiteral.toArguments().get("commitment.commitment"));
    assertUnsupportedThrows(
        () -> parsedCreate.toArguments().put("host", "changed"),
        "expected canonical argument map to be immutable");
    assertUnsupportedThrows(
        () -> parsedCreate.metadata().put("new", "value"),
        "expected metadata map to be immutable");

    final List<KaigiInstructionUtils.RelayManifestHop> mutableHops = new ArrayList<>();
    mutableHops.add(new KaigiInstructionUtils.RelayManifestHop("relay-a", key(1), 1));
    mutableHops.add(new KaigiInstructionUtils.RelayManifestHop("relay-b", key(2), 1));
    mutableHops.add(new KaigiInstructionUtils.RelayManifestHop("relay-c", key(3), 1));
    final KaigiInstructionUtils.RelayManifest snapshot =
        new KaigiInstructionUtils.RelayManifest(100L, mutableHops);
    mutableHops.clear();
    assertEquals(3, snapshot.hops().size());
    assertUnsupportedThrows(
        () -> snapshot.hops().clear(),
        "expected relay manifest hops to be immutable");

    final List<Map<String, String>> arguments =
        Arrays.asList(
            parsedCreate.toArguments(),
            JoinKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("participant")
                .build()
                .toArguments(),
            LeaveKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("participant")
                .build()
                .toArguments(),
            EndKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .build()
                .toArguments(),
            RecordKaigiUsageInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setDurationMs(1)
                .build()
                .toArguments(),
            RegisterKaigiRelayInstruction.builder()
                .setRelayId("relay")
                .setHpkePublicKeyBase64(key(1))
                .setBandwidthClass(1)
                .build()
                .toArguments(),
            UnregisterKaigiRelayInstruction.builder()
                .setRelayId("relay")
                .build()
                .toArguments(),
            SetKaigiRelayManifestInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .build()
                .toArguments(),
            ReportKaigiRelayHealthInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setRelayId("relay")
                .setStatus(ReportKaigiRelayHealthInstruction.Status.HEALTHY)
                .setReportedAtMs(1)
                .build()
                .toArguments());
    final List<java.util.function.Consumer<Map<String, String>>> parsers =
        Arrays.asList(
            value -> CreateKaigiInstruction.fromArguments(value),
            value -> JoinKaigiInstruction.fromArguments(value),
            value -> LeaveKaigiInstruction.fromArguments(value),
            value -> EndKaigiInstruction.fromArguments(value),
            value -> RecordKaigiUsageInstruction.fromArguments(value),
            value -> RegisterKaigiRelayInstruction.fromArguments(value),
            value -> UnregisterKaigiRelayInstruction.fromArguments(value),
            value -> SetKaigiRelayManifestInstruction.fromArguments(value),
            value -> ReportKaigiRelayHealthInstruction.fromArguments(value));
    for (int index = 0; index < arguments.size(); index++) {
      final Map<String, String> unknown =
          withArgument(arguments.get(index), "unknown", "value");
      final java.util.function.Consumer<Map<String, String>> parser = parsers.get(index);
      assertThrows(
          () -> parser.accept(unknown),
          "expected parser to reject an unknown instruction argument");
    }
    assertThrows(
        () ->
            CreateKaigiInstruction.fromArguments(
                withArgument(shuffled, "metadata.", "malformed")),
        "expected malformed metadata key to throw");
    assertThrows(
        () ->
            CreateKaigiInstruction.fromArguments(
                withArgument(shuffled, "commitment.commitment", "")),
        "expected blank hash field to throw");
    assertThrows(
        () ->
            RegisterKaigiRelayInstruction.builder()
                .setRelayId("relay")
                .setHpkePublicKeyBase64("AQ")
                .setBandwidthClass(1),
        "expected non-canonical base64 to throw");
    final UnregisterKaigiRelayInstruction unregistration =
        UnregisterKaigiRelayInstruction.builder().setRelayId("relay").build();
    assertEquals(
        unregistration,
        UnregisterKaigiRelayInstruction.fromArguments(unregistration.toArguments()));
    assertThrows(
        () -> UnregisterKaigiRelayInstruction.builder().setRelayId(" "),
        "expected blank relay id to throw");
  }

  private static void relayHealthReportsValidateStatusNotesAndCanonicalMaps() {
    final String maxNotes = repeat("\uD83D\uDE00", 512);
    final ReportKaigiRelayHealthInstruction report =
        ReportKaigiRelayHealthInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setRelayId("relay")
            .setStatus(ReportKaigiRelayHealthInstruction.Status.DEGRADED)
            .setReportedAtMs(-1L)
            .setNotes(maxNotes)
            .build();

    assertEquals(
        Arrays.asList(
            "action",
            "call.domain_id",
            "call.call_name",
            "relay_id",
            "status",
            "reported_at_ms",
            "notes"),
        new ArrayList<>(report.toArguments().keySet()));
    assertEquals("Degraded", report.toArguments().get("status"));
    assertEquals("18446744073709551615", report.toArguments().get("reported_at_ms"));
    assertEquals(
        report, ReportKaigiRelayHealthInstruction.fromArguments(report.toArguments()));
    assertUnsupportedThrows(
        () -> report.toArguments().put("status", "Healthy"),
        "expected relay health arguments to be immutable");

    final ReportKaigiRelayHealthInstruction zeroTimestamp =
        ReportKaigiRelayHealthInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setRelayId("relay")
            .setStatus(ReportKaigiRelayHealthInstruction.Status.HEALTHY)
            .setReportedAtMs(0)
            .build();
    assertEquals("0", zeroTimestamp.toArguments().get("reported_at_ms"));

    final ReportKaigiRelayHealthInstruction emptyNotes =
        ReportKaigiRelayHealthInstruction.fromArguments(
            withArgument(report.toArguments(), "notes", ""));
    assertEquals("", emptyNotes.notes());
    assertEquals(true, emptyNotes.toArguments().containsKey("notes"));

    assertThrows(
        () ->
            ReportKaigiRelayHealthInstruction.fromArguments(
                withArgument(report.toArguments(), "status", "degraded")),
        "expected non-canonical relay health status to throw");
    assertThrows(
        () ->
            ReportKaigiRelayHealthInstruction.fromArguments(
                withArgument(report.toArguments(), "reported_at_ms", "01")),
        "expected non-canonical relay health timestamp to throw");
    final Map<String, String> missingRelay = new HashMap<>(report.toArguments());
    missingRelay.remove("relay_id");
    assertThrows(
        () -> ReportKaigiRelayHealthInstruction.fromArguments(missingRelay),
        "expected missing relay ID to throw");
    assertThrows(
        () ->
            ReportKaigiRelayHealthInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setRelayId("relay")
                .setStatus(ReportKaigiRelayHealthInstruction.Status.UNAVAILABLE)
                .setReportedAtMs(1)
                .setNotes(repeat("x", 513)),
        "expected oversized relay health notes to throw");
    assertThrows(
        () ->
            ReportKaigiRelayHealthInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setRelayId("relay")
                .setStatus(ReportKaigiRelayHealthInstruction.Status.UNAVAILABLE)
                .setReportedAtMs(1)
                .setNotes("\uD800"),
        "expected unpaired UTF-16 surrogate in relay health notes to throw");
  }

  private static void unitEnumPayloadsAndCallIdentifiersRejectMalformedState() {
    assertThrows(
        () -> CreateKaigiInstruction.builder().setCallId("", "weekly-sync"),
        "expected blank call domain to throw");
    assertThrows(
        () -> CreateKaigiInstruction.builder().setCallId("wonderland", " "),
        "expected blank call name to throw");
    assertThrows(
        () ->
            CreateKaigiInstruction.builder()
                .setPrivacyMode("Transparent", "unexpected"),
        "expected unit privacy mode state to throw");
    assertThrows(
        () ->
            CreateKaigiInstruction.builder()
                .setRoomPolicy("Public", "unexpected"),
        "expected unit room policy state to throw");

    final Map<String, String> create =
        CreateKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setHost("host")
            .build()
            .toArguments();
    assertThrows(
        () ->
            CreateKaigiInstruction.fromArguments(
                withArgument(create, "privacy.state", "unexpected")),
        "expected parsed unit privacy mode state to throw");
    assertThrows(
        () ->
            CreateKaigiInstruction.fromArguments(
                withArgument(create, "room_policy.state", "unexpected")),
        "expected parsed unit room policy state to throw");
  }

  private static void rejectsInvalidRelayHpkeKey() {
    assertThrows(
        () ->
            RegisterKaigiRelayInstruction.builder()
                .setRelayId("relay-alpha@wonderland")
                .setHpkePublicKeyBase64("not!base64")
                .setBandwidthClass(1)
                .build(),
        "expected invalid hpke public key base64 to throw");
  }

  private static void rejectsInvalidRelayManifestHpkeKey() {
    assertThrows(
        () ->
            SetKaigiRelayManifestInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .addRelayManifestHop("relay-alpha@wonderland", "not!base64", 7)
                .build(),
        "expected invalid relay manifest hpke key to throw");
  }

  private static void rejectsInvalidRelayManifestParse() {
    final Map<String, String> args = new HashMap<>();
    args.put("action", "SetKaigiRelayManifest");
    args.put("call.domain_id", "wonderland");
    args.put("call.call_name", "weekly-sync");
    args.put("relay_manifest.hop.0.relay_id", "relay-alpha@wonderland");
    args.put("relay_manifest.hop.0.hpke_public_key", "not!base64");
    args.put("relay_manifest.hop.0.weight", "1");

    assertThrows(
        () -> SetKaigiRelayManifestInstruction.fromArguments(args),
        "expected invalid relay manifest parse to throw");
  }

  private static void rejectsInvalidCreateRelayManifestHpkeKey() {
    assertThrows(
        () ->
            CreateKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setHost("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
                .addRelayManifestHop("relay-alpha@wonderland", "not!base64", 7)
                .build(),
        "expected invalid create relay manifest hpke key to throw");
  }

  private static void rejectsInvalidJoinProofBase64() {
    assertThrows(
        () ->
            JoinKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
                .setProofBase64("not!base64")
                .build(),
        "expected invalid join proof base64 to throw");
  }

  private static void rejectsInvalidLeaveProofBase64() {
    assertThrows(
        () ->
            LeaveKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
                .setProofBase64("not!base64")
                .build(),
        "expected invalid leave proof base64 to throw");
  }

  private static void rejectsInvalidUsageProofBase64() {
    assertThrows(
        () ->
            RecordKaigiUsageInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setDurationMs(1)
                .setBilledGas(0)
                .setProofBase64("not!base64")
                .build(),
        "expected invalid usage proof base64 to throw");
  }

  private static void rejectsZeroAndMissingBandwidthClass() {
    assertThrows(
        () ->
            RegisterKaigiRelayInstruction.builder()
                .setRelayId("relay-alpha")
                .setHpkePublicKeyBase64(key(1))
                .setBandwidthClass(0),
        "expected zero bandwidth class to throw");
    assertStateThrows(
        () ->
            RegisterKaigiRelayInstruction.builder()
                .setRelayId("relay-alpha")
                .setHpkePublicKeyBase64(key(1))
                .build(),
        "expected missing bandwidth class to throw");
  }

  private static void rejectsStructurallyInvalidRelayManifests() {
    final SetKaigiRelayManifestInstruction valid = validSetManifestBuilder().build();
    assertEquals(valid, SetKaigiRelayManifestInstruction.fromArguments(valid.toArguments()));

    assertThrows(
        () ->
            validSetManifestBuilder()
                .setRelayManifestExpiryMs(null)
                .build(),
        "expected missing manifest expiry to throw");
    assertThrows(
        () ->
            SetKaigiRelayManifestInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setRelayManifestExpiryMs(100L)
                .addRelayManifestHop("relay-alpha", key(1), 1)
                .addRelayManifestHop("relay-beta", key(2), 1)
                .build(),
        "expected fewer than three manifest hops to throw");
    assertThrows(
        () ->
            SetKaigiRelayManifestInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setRelayManifestExpiryMs(100L)
                .addRelayManifestHop("relay-alpha", key(1), 1)
                .addRelayManifestHop("relay-alpha", key(2), 1)
                .addRelayManifestHop("relay-gamma", key(3), 1)
                .build(),
        "expected duplicate relay IDs to throw");
    assertThrows(
        () -> validSetManifestBuilder().addRelayManifestHop("relay-delta", key(4), 0),
        "expected zero manifest weight to throw");
    assertThrows(
        () ->
            CreateKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setHost("host")
                .setRelayManifestExpiryMs(100L)
                .addRelayManifestHop("relay-alpha", key(1), 1)
                .addRelayManifestHop("relay-beta", "", 1),
        "expected empty manifest key to throw");
  }

  private static void relayManifestsAcceptEightHopsAndRejectNineInBuildersAndParsers() {
    final SetKaigiRelayManifestInstruction.Builder setBuilder =
        SetKaigiRelayManifestInstruction.builder()
            .setCallId("wonderland", "eight-hop-limit")
            .setRelayManifestExpiryMs(100L);
    for (int index = 0;
        index < KaigiInstructionUtils.KAIGI_RELAY_MANIFEST_MAX_HOPS_V1;
        index++) {
      setBuilder.addRelayManifestHop("relay-" + index, key(index + 1), 1);
    }
    final SetKaigiRelayManifestInstruction setAtLimit = setBuilder.build();
    assertEquals(
        setAtLimit,
        SetKaigiRelayManifestInstruction.fromArguments(setAtLimit.toArguments()));
    assertThrows(
        () -> setBuilder.addRelayManifestHop("relay-8", key(9), 1),
        "expected a ninth SetKaigiRelayManifest builder hop to throw");

    final Map<String, String> setWithNineArguments =
        new LinkedHashMap<>(setAtLimit.toArguments());
    setWithNineArguments.put("relay_manifest.hop.8.relay_id", "relay-8");
    setWithNineArguments.put("relay_manifest.hop.8.hpke_public_key", key(9));
    setWithNineArguments.put("relay_manifest.hop.8.weight", "1");
    assertThrows(
        () -> SetKaigiRelayManifestInstruction.fromArguments(setWithNineArguments),
        "expected a ninth parsed SetKaigiRelayManifest hop to throw");

    final CreateKaigiInstruction.Builder createBuilder =
        CreateKaigiInstruction.builder()
            .setCallId("wonderland", "eight-hop-limit")
            .setHost("host")
            .setRelayManifestExpiryMs(100L);
    for (int index = 0;
        index < KaigiInstructionUtils.KAIGI_RELAY_MANIFEST_MAX_HOPS_V1;
        index++) {
      createBuilder.addRelayManifestHop("relay-" + index, key(index + 1), 1);
    }
    final CreateKaigiInstruction createAtLimit = createBuilder.build();
    assertEquals(
        createAtLimit, CreateKaigiInstruction.fromArguments(createAtLimit.toArguments()));
    assertThrows(
        () -> createBuilder.addRelayManifestHop("relay-8", key(9), 1),
        "expected a ninth CreateKaigi builder hop to throw");

    final Map<String, String> createWithNineArguments =
        new LinkedHashMap<>(createAtLimit.toArguments());
    createWithNineArguments.put("relay_manifest.hop.8.relay_id", "relay-8");
    createWithNineArguments.put("relay_manifest.hop.8.hpke_public_key", key(9));
    createWithNineArguments.put("relay_manifest.hop.8.weight", "1");
    assertThrows(
        () -> CreateKaigiInstruction.fromArguments(createWithNineArguments),
        "expected a ninth parsed CreateKaigi hop to throw");
  }

  private static void relayHpkeKeysAccept4096DecodedBytesAndReject4097() {
    final String maxKey =
        keyWithSize(KaigiInstructionUtils.KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1);
    final String oversizedKey =
        keyWithSize(KaigiInstructionUtils.KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1 + 1);

    final RegisterKaigiRelayInstruction registration =
        RegisterKaigiRelayInstruction.builder()
            .setRelayId("relay-alpha")
            .setHpkePublicKeyBase64(maxKey)
            .setBandwidthClass(1)
            .build();
    assertEquals(
        registration,
        RegisterKaigiRelayInstruction.fromArguments(registration.toArguments()));
    RegisterKaigiRelayInstruction.builder()
        .setRelayId("relay-alpha-bytes")
        .setHpkePublicKey(
            new byte[KaigiInstructionUtils.KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1])
        .setBandwidthClass(1)
        .build();
    assertThrows(
        () ->
            RegisterKaigiRelayInstruction.builder()
                .setRelayId("relay-alpha")
                .setHpkePublicKeyBase64(oversizedKey),
        "expected a 4097-byte registered relay HPKE key to throw");
    assertThrows(
        () ->
            RegisterKaigiRelayInstruction.builder()
                .setRelayId("relay-alpha")
                .setHpkePublicKey(
                    new byte[
                        KaigiInstructionUtils.KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1 + 1]),
        "expected the byte-array registered relay HPKE builder to enforce the limit");
    assertThrows(
        () ->
            RegisterKaigiRelayInstruction.fromArguments(
                withArgument(
                    registration.toArguments(), "relay.hpke_public_key", oversizedKey)),
        "expected a parsed 4097-byte registered relay HPKE key to throw");

    final SetKaigiRelayManifestInstruction manifest =
        SetKaigiRelayManifestInstruction.builder()
            .setCallId("wonderland", "hpke-key-limit")
            .setRelayManifestExpiryMs(100L)
            .addRelayManifestHop("relay-alpha", maxKey, 1)
            .addRelayManifestHop("relay-beta", key(2), 1)
            .addRelayManifestHop("relay-gamma", key(3), 1)
            .build();
    assertEquals(
        manifest, SetKaigiRelayManifestInstruction.fromArguments(manifest.toArguments()));
    assertThrows(
        () ->
            SetKaigiRelayManifestInstruction.builder()
                .setCallId("wonderland", "hpke-key-limit")
                .setRelayManifestExpiryMs(100L)
                .addRelayManifestHop("relay-alpha", oversizedKey, 1),
        "expected a 4097-byte SetKaigiRelayManifest builder key to throw");
    assertThrows(
        () ->
            SetKaigiRelayManifestInstruction.fromArguments(
                withArgument(
                    manifest.toArguments(),
                    "relay_manifest.hop.0.hpke_public_key",
                    oversizedKey)),
        "expected a parsed 4097-byte SetKaigiRelayManifest key to throw");

    final CreateKaigiInstruction create =
        CreateKaigiInstruction.builder()
            .setCallId("wonderland", "hpke-key-limit")
            .setHost("host")
            .setRelayManifestExpiryMs(100L)
            .addRelayManifestHop("relay-alpha", maxKey, 1)
            .addRelayManifestHop("relay-beta", key(2), 1)
            .addRelayManifestHop("relay-gamma", key(3), 1)
            .build();
    assertEquals(create, CreateKaigiInstruction.fromArguments(create.toArguments()));
    assertThrows(
        () ->
            CreateKaigiInstruction.builder()
                .setCallId("wonderland", "hpke-key-limit")
                .setHost("host")
                .setRelayManifestExpiryMs(100L)
                .addRelayManifestHop("relay-alpha", oversizedKey, 1),
        "expected a 4097-byte CreateKaigi builder key to throw");
    assertThrows(
        () ->
            CreateKaigiInstruction.fromArguments(
                withArgument(
                    create.toArguments(),
                    "relay_manifest.hop.0.hpke_public_key",
                    oversizedKey)),
        "expected a parsed 4097-byte CreateKaigi key to throw");
  }

  private static void preservesLedgerSafePrivacyFields() {
    final String proof = key(9);
    final CreateKaigiInstruction create =
        CreateKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setHost("host")
            .setRoomPolicy("Public")
            .setCommitment(hash(1))
            .setNullifierDigest(hash(2))
            .setNullifierIssuedAtMs(0L)
            .setRosterRoot(hash(3))
            .setProofBase64(proof)
            .build();
    final JoinKaigiInstruction join =
        JoinKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setParticipant("participant")
            .setCommitment(hash(1))
            .setNullifierDigest(hash(2))
            .setNullifierIssuedAtMs(0L)
            .setRosterRoot(hash(3))
            .setProofBase64(proof)
            .build();
    final LeaveKaigiInstruction leave =
        LeaveKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setParticipant("participant")
            .build();
    final EndKaigiInstruction end =
        EndKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setEndedAtMs(84L)
            .setCommitment(hash(1))
            .setNullifierDigest(hash(2))
            .setNullifierIssuedAtMs(0L)
            .setRosterRoot(hash(3))
            .setProofBase64(proof)
            .build();

    for (InstructionTemplate instruction :
        new InstructionTemplate[] {create, join, end}) {
      assertEquals(null, instruction.toArguments().get("commitment.alias_tag"));
      assertEquals("0", instruction.toArguments().get("nullifier.issued_at_ms"));
    }
    for (String key :
        new String[] {
          "commitment.commitment",
          "commitment.alias_tag",
          "nullifier.digest",
          "nullifier.issued_at_ms",
          "roster_root",
          "proof"
        }) {
      assertEquals(null, leave.toArguments().get(key));
    }
    assertEquals("Public", create.toArguments().get("room_policy.policy"));
    assertEquals(proof, create.toArguments().get("proof"));
    assertEquals(create, CreateKaigiInstruction.fromArguments(create.toArguments()));
    assertEquals(join, JoinKaigiInstruction.fromArguments(join.toArguments()));
    assertEquals(leave, LeaveKaigiInstruction.fromArguments(leave.toArguments()));
    assertEquals(end, EndKaigiInstruction.fromArguments(end.toArguments()));

    final CreateKaigiInstruction createWithoutIssuedAt =
        CreateKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setHost("host")
            .setNullifierDigest(hash(2))
            .build();
    final JoinKaigiInstruction joinWithoutIssuedAt =
        JoinKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setParticipant("participant")
            .setNullifierDigest(hash(2))
            .build();
    final LeaveKaigiInstruction leaveWithoutIssuedAt =
        LeaveKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setParticipant("participant")
            .build();
    final EndKaigiInstruction endWithoutIssuedAt =
        EndKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setNullifierDigest(hash(2))
            .build();
    for (InstructionTemplate instruction :
        new InstructionTemplate[] {
          createWithoutIssuedAt, joinWithoutIssuedAt, leaveWithoutIssuedAt, endWithoutIssuedAt
        }) {
      assertEquals(null, instruction.toArguments().get("nullifier.issued_at_ms"));
    }
    assertEquals(
        createWithoutIssuedAt,
        CreateKaigiInstruction.fromArguments(createWithoutIssuedAt.toArguments()));
    assertEquals(
        joinWithoutIssuedAt,
        JoinKaigiInstruction.fromArguments(joinWithoutIssuedAt.toArguments()));
    assertEquals(
        leaveWithoutIssuedAt,
        LeaveKaigiInstruction.fromArguments(leaveWithoutIssuedAt.toArguments()));
    assertEquals(
        endWithoutIssuedAt,
        EndKaigiInstruction.fromArguments(endWithoutIssuedAt.toArguments()));
  }

  private static void rejectsClearPrivacyIdentityHints() {
    assertThrows(
        () ->
            CreateKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setHost("host")
                .setCommitmentAliasTag("host-alias"),
        "expected CreateKaigi commitment alias to throw");
    assertThrows(
        () ->
            JoinKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("participant")
                .setCommitmentAliasTag("participant-alias"),
        "expected JoinKaigi commitment alias to throw");
    assertThrows(
        () ->
            LeaveKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("participant")
                .setCommitmentAliasTag("participant-alias"),
        "expected LeaveKaigi commitment alias to throw");
    assertThrows(
        () ->
            LeaveKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("participant")
                .setCommitment(hash(1)),
        "expected LeaveKaigi commitment to throw");
    assertThrows(
        () ->
            LeaveKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("participant")
                .setNullifierDigest(hash(2)),
        "expected LeaveKaigi nullifier to throw");
    assertThrows(
        () ->
            LeaveKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("participant")
                .setNullifierIssuedAtMs(0L),
        "expected LeaveKaigi nullifier timestamp to throw");
    assertThrows(
        () ->
            LeaveKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("participant")
                .setRosterRoot(hash(3)),
        "expected LeaveKaigi roster root to throw");
    assertThrows(
        () ->
            LeaveKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("participant")
                .setProofBase64(key(1)),
        "expected LeaveKaigi proof to throw");
    assertThrows(
        () ->
            EndKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setCommitmentAliasTag("host-alias"),
        "expected EndKaigi commitment alias to throw");

    assertThrows(
        () ->
            CreateKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setHost("host")
                .setNullifierIssuedAtMs(1L),
        "expected CreateKaigi nonzero nullifier time to throw");
    assertThrows(
        () ->
            JoinKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("participant")
                .setNullifierIssuedAtMs(1L),
        "expected JoinKaigi nonzero nullifier time to throw");
    assertThrows(
        () ->
            LeaveKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("participant")
                .setNullifierIssuedAtMs(1L),
        "expected LeaveKaigi nonzero nullifier time to throw");
    assertThrows(
        () ->
            EndKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setNullifierIssuedAtMs(1L),
        "expected EndKaigi nonzero nullifier time to throw");
    assertStateThrows(
        () ->
            CreateKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setHost("host")
                .setNullifierIssuedAtMs(0L)
                .build(),
        "expected CreateKaigi orphan nullifier time to throw");
    assertStateThrows(
        () ->
            JoinKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("participant")
                .setNullifierIssuedAtMs(0L)
                .build(),
        "expected JoinKaigi orphan nullifier time to throw");
    assertStateThrows(
        () ->
            EndKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setNullifierIssuedAtMs(0L)
                .build(),
        "expected EndKaigi orphan nullifier time to throw");

    final Map<String, String> create =
        CreateKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setHost("host")
            .build()
            .toArguments();
    final Map<String, String> join =
        JoinKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setParticipant("participant")
            .build()
            .toArguments();
    final Map<String, String> leave =
        LeaveKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setParticipant("participant")
            .build()
            .toArguments();
    final Map<String, String> end =
        EndKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .build()
            .toArguments();

    assertThrows(
        () ->
            CreateKaigiInstruction.fromArguments(
                withArgument(create, "commitment.alias_tag", "host-alias")),
        "expected parsed CreateKaigi commitment alias to throw");
    assertThrows(
        () ->
            JoinKaigiInstruction.fromArguments(
                withArgument(join, "commitment.alias_tag", "participant-alias")),
        "expected parsed JoinKaigi commitment alias to throw");
    assertThrows(
        () ->
            LeaveKaigiInstruction.fromArguments(
                withArgument(leave, "commitment.alias_tag", "participant-alias")),
        "expected parsed LeaveKaigi commitment alias to throw");
    assertThrows(
        () ->
            LeaveKaigiInstruction.fromArguments(
                withArgument(leave, "commitment.commitment", hash(1))),
        "expected parsed LeaveKaigi commitment to throw");
    assertThrows(
        () ->
            LeaveKaigiInstruction.fromArguments(
                withArgument(leave, "nullifier.issued_at_ms", "0")),
        "expected parsed LeaveKaigi nullifier timestamp to throw");
    assertThrows(
        () ->
            LeaveKaigiInstruction.fromArguments(
                withArgument(leave, "roster_root", hash(3))),
        "expected parsed LeaveKaigi roster root to throw");
    assertThrows(
        () ->
            LeaveKaigiInstruction.fromArguments(withArgument(leave, "proof", key(1))),
        "expected parsed LeaveKaigi proof to throw");
    assertThrows(
        () ->
            EndKaigiInstruction.fromArguments(
                withArgument(end, "commitment.alias_tag", "host-alias")),
        "expected parsed EndKaigi commitment alias to throw");
    assertThrows(
        () ->
            CreateKaigiInstruction.fromArguments(
                withArgument(create, "nullifier.issued_at_ms", "1")),
        "expected parsed CreateKaigi nonzero nullifier time to throw");
    assertThrows(
        () ->
            JoinKaigiInstruction.fromArguments(
                withArgument(join, "nullifier.issued_at_ms", "1")),
        "expected parsed JoinKaigi nonzero nullifier time to throw");
    assertThrows(
        () ->
            LeaveKaigiInstruction.fromArguments(
                withArgument(leave, "nullifier.issued_at_ms", "1")),
        "expected parsed LeaveKaigi nonzero nullifier time to throw");
    assertThrows(
        () ->
            EndKaigiInstruction.fromArguments(
                withArgument(end, "nullifier.issued_at_ms", "1")),
        "expected parsed EndKaigi nonzero nullifier time to throw");
    assertThrows(
        () ->
            CreateKaigiInstruction.fromArguments(
                withArgument(create, "nullifier.issued_at_ms", "0")),
        "expected parsed CreateKaigi orphan nullifier time to throw");
    assertThrows(
        () ->
            JoinKaigiInstruction.fromArguments(
                withArgument(join, "nullifier.issued_at_ms", "0")),
        "expected parsed JoinKaigi orphan nullifier time to throw");
    assertThrows(
        () ->
            EndKaigiInstruction.fromArguments(
                withArgument(end, "nullifier.issued_at_ms", "0")),
        "expected parsed EndKaigi orphan nullifier time to throw");
  }

  private static Map<String, String> withArgument(
      final Map<String, String> source, final String key, final String value) {
    final Map<String, String> copy = new HashMap<>(source);
    copy.put(key, value);
    return copy;
  }

  private static SetKaigiRelayManifestInstruction.Builder validSetManifestBuilder() {
    return SetKaigiRelayManifestInstruction.builder()
        .setCallId("wonderland", "weekly-sync")
        .setRelayManifestExpiryMs(100L)
        .addRelayManifestHop("relay-alpha", key(1), 1)
        .addRelayManifestHop("relay-beta", key(2), 1)
        .addRelayManifestHop("relay-gamma", key(3), 1);
  }

  private static String key(final int value) {
    return Base64.getEncoder().encodeToString(new byte[] {(byte) value});
  }

  private static String keyWithSize(final int size) {
    final byte[] key = new byte[size];
    Arrays.fill(key, (byte) 0xA5);
    return Base64.getEncoder().encodeToString(key);
  }

  private static String hash(final int value) {
    final StringBuilder result = new StringBuilder(64);
    final String octet = String.format("%02x", value & 0xFF);
    for (int index = 0; index < 32; index++) {
      result.append(octet);
    }
    return result.toString();
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder result = new StringBuilder(value.length() * count);
    for (int index = 0; index < count; index++) {
      result.append(value);
    }
    return result.toString();
  }

  private static void assertEquals(final Object expected, final Object actual) {
    if (!java.util.Objects.equals(expected, actual)) {
      throw new AssertionError("expected " + expected + " but was " + actual);
    }
  }

  private static void assertThrows(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final IllegalArgumentException expected) {
      return;
    }
    throw new AssertionError(message);
  }

  private static void assertStateThrows(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final IllegalStateException expected) {
      return;
    }
    throw new AssertionError(message);
  }

  private static void assertUnsupportedThrows(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final UnsupportedOperationException expected) {
      return;
    }
    throw new AssertionError(message);
  }
}
