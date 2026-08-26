package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.junit.Test;

/** Kotlin-mirror parity tests for the strict Parliament V1 HTTP contract. */
public final class ParliamentApiV1Tests {
  private static final String ATTEMPT_ID = "ab".repeat(32);
  private static final String PROPOSAL_ID = "cd".repeat(32);
  private static final String CONTRACT_ADDRESS =
      "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw";

  @Test
  public void sharedFixturePinsEveryPublicTransitionAutomaticOutcomeAndResultRoot()
      throws Exception {
    final Map<String, Object> fixture = objectValue(Files.readAllBytes(fixturePath()));
    assertEquals("iroha.governance.parliament.api_fixture.v1", fixture.get("schema"));
    assertEquals(1L, fixture.get("api_version"));

    final Map<String, Object> routes = objectValue(fixture.get("routes"));
    assertEquals(ParliamentApiV1.ATTEMPT_DRAFT_PATH, routes.get("attempt_draft"));
    assertEquals(ParliamentApiV1.ATTEMPT_READ_PATH, routes.get("attempt_read"));
    assertEquals(
        ParliamentApiV1.TIMED_OVN_CASTING_CONTEXT_READ_PATH,
        routes.get("timed_ovn_casting_context_read"));
    assertEquals(
        ParliamentApiV1.TIMED_OVN_CASTING_PROOF_PATH,
        routes.get("timed_ovn_casting_proof"));
    assertEquals(
        ParliamentApiV1.TLE_RELEASE_CONTEXT_READ_PATH,
        routes.get("tle_release_context_read"));
    assertEquals(
        ParliamentApiV1.TLE_PARTIAL_RELEASE_PATH, routes.get("tle_partial_release"));
    assertEquals(ParliamentApiV1.TRANSITION_DRAFT_PATH, routes.get("transition_draft"));
    final Map<String, Object> wireIds = objectValue(fixture.get("wire_ids"));
    assertEquals(ParliamentApiV1.ATTEMPT_CREATE_WIRE_ID, wireIds.get("attempt_create"));
    assertEquals(ParliamentApiV1.TRANSITION_SUBMIT_WIRE_ID, wireIds.get("transition_submit"));
    assertEquals(ParliamentApiV1.PROPOSAL_KINDS, fixture.get("proposal_kinds"));
    final Map<String, Object> limits = objectValue(fixture.get("limits"));
    assertEquals(
        ParliamentApiV1.MAX_STATE_BYTES,
        ((Number) limits.get("attempt_state_bytes")).intValue());

    final List<?> transitions = (List<?>) fixture.get("public_transitions");
    assertEquals(ParliamentApiV1.PUBLIC_TRANSITIONS.size(), transitions.size());
    for (int i = 0; i < transitions.size(); i++) {
      final ParliamentApiV1.TransitionLayout layout =
          ParliamentApiV1.PUBLIC_TRANSITIONS.get(i);
      final Map<String, Object> entry = objectValue(transitions.get(i));
      assertEquals(layout.noritoIndex, ((Number) entry.get("norito_index")).intValue());
      assertEquals(String.format("%02x", layout.noritoIndex), entry.get("norito_prefix_hex"));
      assertEquals(layout.jsonTag, entry.get("json_tag"));
      assertEquals(
          layout.jsonPayloadRequired ? "required" : "forbidden",
          entry.get("json_payload"));
      assertEquals(layout.eventKindIndex, ((Number) entry.get("event_kind_index")).intValue());

      final String transitionJson =
          layout.jsonPayloadRequired
              ? "{\"transition\":\"" + layout.jsonTag + "\",\"payload\":{}}"
              : "{\"transition\":\"" + layout.jsonTag + "\"}";
      ParliamentApiV1.transitionDraftRequestJson(ATTEMPT_ID, bytes(transitionJson));
    }

    final List<?> outcomes = (List<?>) fixture.get("automatic_execution_outcomes");
    assertEquals(ParliamentApiV1.AUTOMATIC_EXECUTION_OUTCOMES.size(), outcomes.size());
    for (int i = 0; i < outcomes.size(); i++) {
      final ParliamentApiV1.AutomaticOutcomeLayout layout =
          ParliamentApiV1.AUTOMATIC_EXECUTION_OUTCOMES.get(i);
      final Map<String, Object> entry = objectValue(outcomes.get(i));
      assertEquals(layout.noritoIndex, ((Number) entry.get("norito_index")).intValue());
      assertEquals(layout.jsonTag, entry.get("json_tag"));
      assertEquals(layout.eventKind, entry.get("event_kind"));
      assertEquals(layout.eventKindIndex, ((Number) entry.get("event_kind_index")).intValue());
    }

    final Map<String, Object> digestDomains = objectValue(fixture.get("digest_domains"));
    assertEquals(
        ParliamentApiV1.PUBLIC_TRANSITION_DIGEST_DOMAIN,
        digestDomains.get("public_transition"));
    assertEquals(
        ParliamentApiV1.AUTOMATIC_OUTCOME_DIGEST_DOMAIN,
        digestDomains.get("automatic_execution_outcome"));
    final Map<String, String> fixtureRootDomains = new LinkedHashMap<>();
    for (final Object raw : (List<?>) fixture.get("certificate_result_roots")) {
      final Map<String, Object> entry = objectValue(raw);
      fixtureRootDomains.put((String) entry.get("name"), (String) entry.get("domain"));
    }
    assertEquals(ParliamentApiV1.CERTIFICATE_RESULT_ROOT_DOMAINS, fixtureRootDomains);

    final Map<String, Object> binding = objectValue(fixture.get("certificate_body_binding"));
    assertEquals(
        ParliamentApiV1.CERTIFICATE_BODY_BINDING_NORITO_FIELDS,
        binding.get("norito_field_order"));
    final Map<String, Object> publicFinding =
        objectValue(binding.get("public_nonbinding_body"));
    assertEquals(
        ParliamentApiV1.PUBLIC_FINDING_CERTIFICATE_NORITO_FIELDS,
        publicFinding.get("public_finding_norito_field_order"));
    assertEquals("ceil(2 * original_seats / 3)", publicFinding.get("quorum"));
    assertEquals(
        "strictly increasing distinct nonzero assignment ids",
        publicFinding.get("endorsing_assignments"));
    assertEquals("endorsing_assignments.length == quorum", publicFinding.get("endorsements"));
    final Map<String, Object> privateJury = objectValue(binding.get("private_jury"));
    assertEquals("forbidden", privateJury.get("public_finding"));
    assertEquals("ballot.tally.original_seats", privateJury.get("original_seats"));
    final List<?> noResultKinds = (List<?>) fixture.get("no_result_kinds");
    for (int i = 0; i < noResultKinds.size(); i++) {
      assertEquals(
          ParliamentApiV1.NO_RESULT_KINDS.get(i).jsonTag,
          objectValue(noResultKinds.get(i)).get("json_tag"));
    }
    assertEquals(
        ParliamentApiV1.BODY_STATE_FIELDS,
        objectValue(fixture.get("attempt_read_body_state")).get("json_fields"));
    assertEquals(
        List.of(
            "version",
            "key_session_id",
            "network_id",
            "roster_hash",
            "committee_size",
            "threshold",
            "generator_h",
            "generator_v",
            "qualified_dealers",
            "qualified_dealer_commitments",
            "dkg_event_hash",
            "group_public_key",
            "public_shares",
            "transcript_hash"),
        objectValue(fixture.get("tle_release_context")).get("transcript_public_state_fields"));
    assertEquals(
        9,
        ((List<?>) objectValue(fixture.get("tle_partial_release")).get("response_fields")).size());
  }

  @Test
  public void attemptBuilderAdmitsExactlyTheSevenFirstReleaseProposalKinds() {
    for (final String kind : ParliamentApiV1.PROPOSAL_KINDS) {
      final Map<String, Object> request =
          objectValue(
              ParliamentApiV1.attemptDraftRequestJson(
                  proposal(kind), 0));
      assertEquals(kind, objectValue(request.get("proposal")).get("kind"));
    }
    assertEquals(7, ParliamentApiV1.PROPOSAL_KINDS.size());

    final Map<String, Object> fullU64Policy = validProposal("ValidationFeePolicy");
    final Map<String, Object> fullU64PolicyValue =
        objectValue(objectValue(fullU64Policy.get("payload")).get("policy"));
    fullU64PolicyValue.put("policy_version", "18446744073709551615");
    fullU64PolicyValue.put("previous_policy_hash", repeatedNumbers(32, 1));
    fullU64PolicyValue.put("effective_from_height", "18446744073709551615");
    ParliamentApiV1.Proposal.fromJson(encode(fullU64Policy));

    for (final String kind :
        List.of(
            "ProposeDeployContract",
            "EnactReferendum",
            "FinalizeReferendum",
            "deploy_contract",
            "runtimeUpgrade",
            "Unknown")) {
      assertThrows(
          IllegalArgumentException.class,
          () ->
              ParliamentApiV1.attemptDraftRequestJson(
                  ParliamentApiV1.Proposal.fromJson(
                      bytes("{\"kind\":\"" + kind + "\",\"payload\":{}}")),
                  0));
    }
    assertThrows(
        IllegalArgumentException.class,
        () ->
            ParliamentApiV1.Proposal.fromJson(
                bytes("{\"proposal_kind\":\"RuntimeUpgrade\",\"payload\":{}}")));
  }

  @Test
  public void proposalAdmissionRejectsMissingUnknownAndMalformedNestedFields() {
    final Map<String, Object> runtime = validProposal("RuntimeUpgrade");
    objectValue(objectValue(runtime.get("payload")).get("manifest")).remove("provenance");
    assertThrows(
        IllegalArgumentException.class,
        () -> ParliamentApiV1.Proposal.fromJson(encode(runtime)));

    final Map<String, Object> provider = validProposal("SorafsProviderGovernance");
    objectValue(objectValue(provider.get("payload")).get("action"))
        .put("legacy_owner", account(9));
    assertThrows(
        IllegalArgumentException.class,
        () -> ParliamentApiV1.Proposal.fromJson(encode(provider)));

    final Map<String, Object> malformedProvider =
        validProposal("SorafsProviderGovernance");
    objectValue(
            objectValue(objectValue(malformedProvider.get("payload")).get("action"))
                .get("value"))
        .put("provider_id", repeatedNumbers(32, 1));
    assertThrows(
        IllegalArgumentException.class,
        () -> ParliamentApiV1.Proposal.fromJson(encode(malformedProvider)));

    final Map<String, Object> malformedAccount =
        validProposal("MusubiRegistryGovernance");
    objectValue(objectValue(malformedAccount.get("payload")).get("value"))
        .put("owners", Arrays.asList("alice@legacy"));
    assertThrows(
        IllegalArgumentException.class,
        () -> ParliamentApiV1.Proposal.fromJson(encode(malformedAccount)));

    final Map<String, Object> impreciseHeight = validProposal("RuntimeUpgrade");
    objectValue(objectValue(impreciseHeight.get("payload")).get("manifest"))
        .put("start_height", new BigInteger("9007199254740992"));
    assertThrows(
        IllegalArgumentException.class,
        () -> ParliamentApiV1.Proposal.fromJson(encode(impreciseHeight)));
  }

  @Test
  public void transitionBuilderRejectsRemovedOutcomeTagsAndPayloadAliases() {
    for (final String tag :
        List.of(
            "ConstructCertificate",
            "MarkEnacted",
            "MarkSuperseded",
            "MarkExecutionFailed",
            "PlainBallotFallback")) {
      assertThrows(
          IllegalArgumentException.class,
          () ->
              ParliamentApiV1.transitionDraftRequestJson(
                  ATTEMPT_ID,
                  bytes("{\"transition\":\"" + tag + "\",\"payload\":{}}")));
    }
    assertThrows(
        IllegalArgumentException.class,
        () ->
            ParliamentApiV1.transitionDraftRequestJson(
                ATTEMPT_ID,
                bytes("{\"transition\":\"CompleteQualification\",\"payload\":{}}")));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            ParliamentApiV1.transitionDraftRequestJson(
                ATTEMPT_ID, bytes("{\"transition\":\"EscalateRisk\"}")));
  }

  @Test
  public void requestBuildersExposeOnlyCanonicalFieldsAndRoutes() {
    final Map<String, Object> attempt =
        objectValue(
            ParliamentApiV1.attemptDraftRequestJson(
                proposal("RuntimeUpgrade"), 7));
    assertEquals(Set.of("version", "proposal", "attempt_sequence"), attempt.keySet());
    assertEquals(1L, attempt.get("version"));
    assertEquals(7L, attempt.get("attempt_sequence"));

    final Map<String, Object> transition =
        objectValue(
            ParliamentApiV1.transitionDraftRequestJson(
                ATTEMPT_ID, bytes("{\"transition\":\"CompleteQualification\"}")));
    assertEquals(
        Set.of("version", "governance_attempt_id", "transition"), transition.keySet());
    assertEquals(ATTEMPT_ID, transition.get("governance_attempt_id"));
    assertEquals(
        "/v1/gov/parliament/attempts/" + ATTEMPT_ID,
        ParliamentApiV1.attemptReadPath(ATTEMPT_ID));
    final String ballotId = "33".repeat(32);
    assertEquals(
        "/v1/gov/parliament/ballots/" + ballotId + "/casting-context",
        ParliamentApiV1.timedOvnCastingContextReadPath(ballotId));
    assertEquals(
        "/v1/gov/parliament/ballots/" + ballotId + "/casting-proof",
        ParliamentApiV1.timedOvnCastingProofPath(ballotId));
    assertEquals(
        "/v1/gov/parliament/ballots/" + ballotId + "/release-context",
        ParliamentApiV1.tleReleaseContextReadPath(ballotId));
    assertEquals(
        "/v1/gov/parliament/ballots/" + ballotId + "/partial-release",
        ParliamentApiV1.tlePartialReleasePath(ballotId));

    assertThrows(
        IllegalArgumentException.class,
        () ->
            ParliamentApiV1.transitionDraftRequestJson(
                ATTEMPT_ID,
                bytes(
                    "{\"transition\":\"CompleteQualification\",\"authority\":\"alice\"}")));
    assertThrows(
        IllegalArgumentException.class,
        () -> ParliamentApiV1.attemptReadPath(ATTEMPT_ID.toUpperCase()));
  }

  @Test
  public void strictDraftParsersBindVersionIdentifiersDigestAndWireIds() {
    final Map<String, Object> attemptResponse = new LinkedHashMap<>();
    attemptResponse.put("version", 1);
    attemptResponse.put("proposal_content_id", PROPOSAL_ID);
    attemptResponse.put("governance_attempt_id", ATTEMPT_ID);
    attemptResponse.put(
        "tx_instructions",
        List.of(
            instruction(ParliamentApiV1.ATTEMPT_CREATE_WIRE_ID, "0102")));
    final ParliamentApiV1.AttemptDraftResponse parsedAttempt =
        ParliamentApiV1.parseAttemptDraftResponse(
            encode(attemptResponse), PROPOSAL_ID, ATTEMPT_ID);
    assertEquals(ATTEMPT_ID, parsedAttempt.governanceAttemptId);

    final byte[] digest = new byte[32];
    java.util.Arrays.fill(digest, (byte) 0x55);
    final List<Integer> digestJson = new ArrayList<>();
    for (final byte value : digest) digestJson.add(value & 0xff);
    final Map<String, Object> transitionResponse = new LinkedHashMap<>();
    transitionResponse.put("version", 1);
    transitionResponse.put("governance_attempt_id", ATTEMPT_ID);
    transitionResponse.put("transition_kind", Map.of("kind", "CompleteQualification"));
    transitionResponse.put("transition_digest", digestJson);
    transitionResponse.put(
        "tx_instructions",
        List.of(
            instruction(ParliamentApiV1.TRANSITION_SUBMIT_WIRE_ID, "0304")));
    final ParliamentApiV1.TransitionDraftResponse parsedTransition =
        ParliamentApiV1.parseTransitionDraftResponse(
            encode(transitionResponse), ATTEMPT_ID, "CompleteQualification", digest);
    assertArrayEquals(digest, parsedTransition.transitionDigest);

    final Map<String, Object> wrongVersion = new LinkedHashMap<>(attemptResponse);
    wrongVersion.put("version", 2);
    assertThrows(
        IllegalArgumentException.class,
        () ->
            ParliamentApiV1.parseAttemptDraftResponse(
                encode(wrongVersion), PROPOSAL_ID, ATTEMPT_ID));
    final Map<String, Object> unknown = new LinkedHashMap<>(attemptResponse);
    unknown.put("private_key", "secret");
    assertThrows(
        IllegalArgumentException.class,
        () ->
            ParliamentApiV1.parseAttemptDraftResponse(
                encode(unknown), PROPOSAL_ID, ATTEMPT_ID));
  }

  @Test
  public void readParserRejectsAliasesAndMismatchedState() {
    final Map<String, Object> response = readResponse();
    final ParliamentApiV1.AttemptReadResponse parsed =
        ParliamentApiV1.parseAttemptReadResponse(encode(response), ATTEMPT_ID);
    assertEquals("9", parsed.currentHeight);
    assertEquals("rules-committee", parsed.bodyStates.get(0).body);
    assertEquals(
        List.of("11".repeat(32), "12".repeat(32)),
        parsed.publicFindingBindings.get(0).endorsingAssignments);

    final Map<String, Object> alias = new LinkedHashMap<>(response);
    alias.put("statePayloadHex", alias.remove("state_payload_hex"));
    assertThrows(
        IllegalArgumentException.class,
        () -> ParliamentApiV1.parseAttemptReadResponse(encode(alias), ATTEMPT_ID));
    assertThrows(
        IllegalArgumentException.class,
        () -> ParliamentApiV1.parseAttemptReadResponse(encode(response), "66".repeat(32)));

    final Map<String, Object> barePayload = new LinkedHashMap<>(response);
    barePayload.put("state_payload_hex", "0102");
    assertThrows(
        IllegalArgumentException.class,
        () -> ParliamentApiV1.parseAttemptReadResponse(encode(barePayload), ATTEMPT_ID));

    final Map<String, Object> badChecksum = new LinkedHashMap<>(response);
    final byte[] tampered = stateFrame();
    tampered[tampered.length - 1]++;
    badChecksum.put("state_payload_hex", toHex(tampered));
    assertThrows(
        IllegalArgumentException.class,
        () -> ParliamentApiV1.parseAttemptReadResponse(encode(badChecksum), ATTEMPT_ID));

    final Map<String, Object> wrongDeadline = readResponse();
    final List<Map<String, Object>> states = maps(wrongDeadline.get("body_states"));
    states.get(0).put("public_finding_deadline_height", 9);
    wrongDeadline.put("body_states", states);
    assertThrows(
        IllegalArgumentException.class,
        () -> ParliamentApiV1.parseAttemptReadResponse(encode(wrongDeadline), ATTEMPT_ID));

    final Map<String, Object> unsorted = readResponse();
    final Map<String, Object> certificate = objectValue(unsorted.get("certificate"));
    final List<Map<String, Object>> bindings = maps(certificate.get("body_bindings"));
    final Map<String, Object> finding = objectValue(bindings.get(0).get("public_finding"));
    finding.put("endorsing_assignments", List.of("12".repeat(32), "11".repeat(32)));
    bindings.get(0).put("public_finding", finding);
    certificate.put("body_bindings", bindings);
    unsorted.put("certificate", certificate);
    assertThrows(
        IllegalArgumentException.class,
        () -> ParliamentApiV1.parseAttemptReadResponse(encode(unsorted), ATTEMPT_ID));
  }

  @Test
  public void releaseContextRequiresCompleteTranscriptAndBindsEveryPartial() throws Exception {
    final String ballotId = "33".repeat(32);
    final Map<String, Object> response = tleReleaseContextResponse();
    final ParliamentApiV1.TleReleaseContextResponse context =
        ParliamentApiV1.parseTleReleaseContextResponse(encode(response), ballotId);
    assertEquals(4, context.keySession.publicShares.size());
    assertEquals(2, context.keySession.qualifiedDealerCommitments.size());

    final Map<String, Object> partial = tlePartialReleaseResponse(response);
    final ParliamentApiV1.TlePartialReleaseShare parsedPartial =
        ParliamentApiV1.parseTlePartialReleaseResponse(
            encode(partial),
            context.keySession.keySessionId,
            context.identityDigest,
            context.keySession.committeeSize);
    assertEquals(1, parsedPartial.participantIndex);

    final Map<String, Object> missingShare = tleReleaseContextResponse();
    final Map<String, Object> missingSession =
        new LinkedHashMap<>(objectValue(missingShare.get("tle_key_session")));
    final List<?> shares = (List<?>) missingSession.get("public_shares");
    missingSession.put("public_shares", new ArrayList<>(shares.subList(0, shares.size() - 1)));
    missingShare.put("tle_key_session", missingSession);
    assertThrows(
        IllegalArgumentException.class,
        () -> ParliamentApiV1.parseTleReleaseContextResponse(encode(missingShare), ballotId));

    final Map<String, Object> wrongDigest = tleReleaseContextResponse();
    final List<Integer> wrongDigestBytes = byteValues(32, 0x88);
    wrongDigestBytes.set(0, 1);
    wrongDigest.put("identity_digest", wrongDigestBytes);
    assertThrows(
        IllegalArgumentException.class,
        () -> ParliamentApiV1.parseTleReleaseContextResponse(encode(wrongDigest), ballotId));

    final Map<String, Object> crossBound = tlePartialReleaseResponse(response);
    crossBound.put("key_session_id", "77".repeat(32));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            ParliamentApiV1.parseTlePartialReleaseResponse(
                encode(crossBound),
                context.keySession.keySessionId,
            context.identityDigest,
            4));
  }

  @Test
  public void castingContextRequiresExactPublicCorpusAndCanonicalArchive() throws Exception {
    final String ballotId = "33".repeat(32);
    final Map<String, Object> response = timedOvnCastingContextResponse();
    final ParliamentApiV1.TimedOvnCastingContextResponse context =
        ParliamentApiV1.parseTimedOvnCastingContextResponse(encode(response), ballotId);
    assertEquals(ParliamentApiV1.TimedOvnCastingPhase.Registered, context.phase);
    assertEquals(1, context.registrationRecordsHex.size());
    assertArrayEquals(bytes("NRT0"), context.archiveNorito);

    final Map<String, Object> unknown = new LinkedHashMap<>(response);
    unknown.put("seed", byteValues(32, 0x11));
    assertThrows(
        IllegalArgumentException.class,
        () -> ParliamentApiV1.parseTimedOvnCastingContextResponse(encode(unknown), ballotId));

    final Map<String, Object> emptyClosed = new LinkedHashMap<>(response);
    emptyClosed.put("phase", "RegistrationClosed");
    emptyClosed.put("registration_records_hex", List.of());
    assertThrows(
        IllegalArgumentException.class,
        () ->
            ParliamentApiV1.parseTimedOvnCastingContextResponse(
                encode(emptyClosed), ballotId));

    final Map<String, Object> emptyFrozen = new LinkedHashMap<>(response);
    emptyFrozen.put("phase", "SurvivorsFrozen");
    emptyFrozen.put("survivor_participant_hashes", List.of());
    emptyFrozen.put(
        "release_identity", tleReleaseContextResponse().get("release_identity"));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            ParliamentApiV1.parseTimedOvnCastingContextResponse(
                encode(emptyFrozen), ballotId));

    final Map<String, Object> noncanonicalArchive = new LinkedHashMap<>(response);
    noncanonicalArchive.put("archive_norito_base64", "TlJUMA");
    assertThrows(
        IllegalArgumentException.class,
        () ->
            ParliamentApiV1.parseTimedOvnCastingContextResponse(
                encode(noncanonicalArchive), ballotId));
  }

  private static Map<String, Object> tleReleaseContextResponse() throws Exception {
    final String ballotId = "33".repeat(32);
    final String bodyId = "22".repeat(32);
    final String keySessionId = "44".repeat(32);
    final List<Integer> survivorRoot = byteValues(32, 0x61);
    final List<Integer> noRecoveryRoot = byteValues(32, 0x62);
    final List<Integer> parameterHash = byteValues(32, 0x63);

    final ByteArrayOutputStream identity = new ByteArrayOutputStream();
    append(identity, bytes("iroha.parliament.tle.identity-payload.v1\0"));
    append(identity, u16(1));
    append(identity, hexBytes(ATTEMPT_ID));
    append(identity, hexBytes(bodyId));
    append(identity, hexBytes(ballotId));
    append(identity, bytesOf(survivorRoot));
    append(identity, bytesOf(noRecoveryRoot));
    append(identity, u64(40));
    append(identity, bytesOf(parameterHash));
    final byte[] identityPayload = identity.toByteArray();
    assertEquals(243, identityPayload.length);

    final Map<String, Object> keySession = new LinkedHashMap<>();
    keySession.put("version", 1);
    keySession.put("key_session_id", keySessionId);
    keySession.put("network_id", byteValues(32, 0x45));
    keySession.put("roster_hash", byteValues(32, 0x46));
    keySession.put("committee_size", 4);
    keySession.put("threshold", 2);
    keySession.put("generator_h", byteValues(96, 0x47));
    keySession.put("generator_v", byteValues(96, 0x48));
    keySession.put("qualified_dealers", List.of(1, 2));
    final List<Map<String, Object>> dealers = new ArrayList<>();
    for (final int dealerIndex : List.of(1, 2)) {
      final Map<String, Object> dealer = new LinkedHashMap<>();
      dealer.put("dealer_index", dealerIndex);
      dealer.put(
          "coefficient_commitments",
          List.of(
              byteValues(96, 0x50 + dealerIndex),
              byteValues(96, 0x60 + dealerIndex)));
      dealer.put("constant_pok_commitment", byteValues(96, 0x70 + dealerIndex));
      dealer.put("constant_pok_response", byteValues(32, 0x80 + dealerIndex));
      dealers.add(dealer);
    }
    keySession.put("qualified_dealer_commitments", dealers);
    keySession.put("dkg_event_hash", byteValues(32, 0x49));
    keySession.put("group_public_key", byteValues(96, 0x4a));
    final List<Map<String, Object>> publicShares = new ArrayList<>();
    for (int index = 1; index <= 4; index++) {
      final Map<String, Object> share = new LinkedHashMap<>();
      share.put("index", index);
      share.put("participant_hash", byteValues(32, 0x20 + index));
      share.put("public_key_share", byteValues(96, 0x30 + index));
      publicShares.add(share);
    }
    keySession.put("public_shares", publicShares);
    keySession.put("transcript_hash", byteValues(32, 0x4b));

    final ByteArrayOutputStream framed = new ByteArrayOutputStream();
    append(framed, bytes("iroha.threshold-bls.message.v1\0"));
    append(framed, bytes("iroha.threshold-bls.session.v1\0"));
    append(framed, u16(1));
    framed.write(2);
    append(framed, bytesOf((List<?>) keySession.get("network_id")));
    append(framed, hexBytes(keySessionId));
    append(framed, bytesOf((List<?>) keySession.get("roster_hash")));
    append(framed, u16(4));
    append(framed, u16(2));
    append(framed, u32(identityPayload.length));
    append(framed, identityPayload);

    final Map<String, Object> releaseIdentity = new LinkedHashMap<>();
    releaseIdentity.put("tle_key_session_id", keySessionId);
    releaseIdentity.put("governance_attempt_id", ATTEMPT_ID);
    releaseIdentity.put("body_instance_id", bodyId);
    releaseIdentity.put("ballot_attempt_id", ballotId);
    releaseIdentity.put("survivor_corpus_root", survivorRoot);
    releaseIdentity.put("no_recovery_root", noRecoveryRoot);
    releaseIdentity.put("target_finalized_height", 40);
    releaseIdentity.put("parameter_hash", parameterHash);

    final Map<String, Object> response = new LinkedHashMap<>();
    response.put("version", 1);
    response.put("current_height", 42);
    response.put("ballot_attempt_id", ballotId);
    response.put("governance_attempt_id", ATTEMPT_ID);
    response.put("body_instance_id", bodyId);
    response.put("status", Map.of("status", "Opening"));
    response.put("release_height", 40);
    response.put("opening_deadline_height", 45);
    response.put("tle_key_session", keySession);
    response.put("release_identity", releaseIdentity);
    response.put(
        "identity_digest",
        byteValues(MessageDigest.getInstance("SHA-256").digest(framed.toByteArray())));
    response.put("identity_payload_hex", toHex(identityPayload));
    return response;
  }

  private static Map<String, Object> tlePartialReleaseResponse(
      final Map<String, Object> context) {
    final Map<String, Object> session = objectValue(context.get("tle_key_session"));
    final Map<String, Object> response = new LinkedHashMap<>();
    response.put("key_session_id", session.get("key_session_id"));
    response.put("identity_digest", context.get("identity_digest"));
    response.put("participant_index", 1);
    response.put("sigma", byteValues(48, 0x91));
    response.put("proof_x", byteValues(96, 0x92));
    response.put("proof_y", byteValues(48, 0x93));
    response.put("z_s", byteValues(32, 0x94));
    response.put("z_r", byteValues(32, 0x95));
    response.put("z_u", byteValues(32, 0x96));
    return response;
  }

  private static Map<String, Object> timedOvnCastingContextResponse() throws Exception {
    final Map<String, Object> release = tleReleaseContextResponse();
    final Map<String, Object> keySession = objectValue(release.get("tle_key_session"));
    final Map<String, Object> identity = objectValue(release.get("release_identity"));

    final Map<String, Object> session = new LinkedHashMap<>();
    session.put("network_id", keySession.get("network_id"));
    session.put("proposal_content_id", PROPOSAL_ID);
    session.put("governance_attempt_id", ATTEMPT_ID);
    session.put("body_instance_id", release.get("body_instance_id"));
    session.put("ballot_attempt_id", release.get("ballot_attempt_id"));
    session.put("parameter_hash", identity.get("parameter_hash"));
    session.put("tle_key_session_id", keySession.get("key_session_id"));
    session.put("tle_key_transcript_hash", keySession.get("transcript_hash"));
    session.put("tle_master_public_key", keySession.get("group_public_key"));

    final Map<String, Object> response = new LinkedHashMap<>();
    response.put("version", 1);
    response.put("current_height", 30);
    response.put("phase", "Registered");
    response.put("session", session);
    response.put("registration_opened_at_finalized_height", 20);
    response.put("target_finalized_height", 40);
    response.put("tle_key_session", keySession);
    response.put("registration_records_hex", List.of("81".repeat(3_624)));
    response.put("survivor_participant_hashes", null);
    response.put("release_identity", null);
    response.put(
        "archive_norito_base64",
        Base64.getEncoder().encodeToString(bytes("NRT0")));
    return response;
  }

  private static Map<String, Object> readResponse() {
    final List<Integer> root = new ArrayList<>();
    for (int i = 0; i < 32; i++) root.add(0x55);
    final String id1 = "01".repeat(32);
    final String id2 = "02".repeat(32);
    final Map<String, Object> bodyState = new LinkedHashMap<>();
    bodyState.put("body", "rules-committee");
    bodyState.put("body_instance_id", id1);
    bodyState.put("status", Map.of("status", "Approved"));
    bodyState.put("public_finding_opened_at_height", 1);
    bodyState.put("public_finding_phase_blocks", 7);
    bodyState.put("public_finding_deadline_height", 8);
    bodyState.put("no_result_kind", null);
    bodyState.put("no_result_height", null);

    final Map<String, Object> publicFinding = new LinkedHashMap<>();
    publicFinding.put("endorsement_root", root);
    publicFinding.put("endorsing_assignments", List.of("11".repeat(32), "12".repeat(32)));
    publicFinding.put("endorsements", 2);
    publicFinding.put("quorum", 2);

    final Map<String, Object> binding = new LinkedHashMap<>();
    binding.put("body_instance_id", id1);
    binding.put("election_attempt_id", id2);
    binding.put("election_attempt_sequence", 0);
    binding.put("sortition_request_id", "03".repeat(32));
    binding.put("sortition_request", Map.of());
    binding.put("body", "rules-committee");
    binding.put("original_seats", 3);
    binding.put("beacon_session_id", "04".repeat(32));
    binding.put("beacon_pulse_id", "05".repeat(32));
    binding.put("roster_root", root);
    binding.put("assignment_root", root);
    binding.put("result_root", root);
    binding.put("result_height", 8);
    binding.put("public_finding", publicFinding);
    binding.put("ballot", null);

    final Map<String, Object> certificate = new LinkedHashMap<>();
    certificate.put("proposal_content_id", PROPOSAL_ID);
    certificate.put("governance_attempt_id", ATTEMPT_ID);
    certificate.put("governance_attempt_sequence", 0);
    certificate.put("risk_tier", Map.of("tier", "Standard"));
    certificate.put("body_bindings", List.of(binding));
    certificate.put("policy_version", 1);
    certificate.put("effect_preimage_hash", root);
    certificate.put(
        "expected_head", Map.of("state", "Absent", "head", Map.of("subject_id", root)));
    certificate.put("certified_at_height", 8);
    certificate.put("enact_at_height", 10);

    final Map<String, Object> response = new LinkedHashMap<>();
    response.put("version", 1);
    response.put("current_height", 9);
    response.put(
        "attempt",
        Map.of(
            "id", ATTEMPT_ID,
            "proposal_content_id", PROPOSAL_ID,
            "sequence", 0,
            "risk_tier", Map.of("tier", "Standard"),
            "stage", Map.of("stage", "Rules"),
            "status", Map.of("status", "Certified")));
    response.put("policy_version", 1);
    response.put(
        "required_bodies",
        List.of(
            Map.of(
                "body", "rules-committee",
                "decision_mode", Map.of("mode", "PublicFinding"))));
    response.put("body_states", List.of(bodyState));
    response.put("certificate", certificate);
    response.put("terminal_height", null);
    response.put("superseding_head", null);
    response.put("execution_failure_root", null);
    response.put("state_payload_hex", stateFrameHex());
    return response;
  }

  private static byte[] stateFrame() {
    final byte[] payload = new byte[] {1, 2};
    final NoritoHeader header =
        new NoritoHeader(
            filled(16, (byte) 3),
            payload.length,
            CRC64.compute(payload),
            0,
            NoritoHeader.COMPRESSION_NONE);
    final byte[] headerBytes = header.encode();
    final byte[] frame = Arrays.copyOf(headerBytes, headerBytes.length + payload.length);
    System.arraycopy(payload, 0, frame, headerBytes.length, payload.length);
    return frame;
  }

  private static String stateFrameHex() {
    return toHex(stateFrame());
  }

  private static byte[] filled(final int length, final byte value) {
    final byte[] bytes = new byte[length];
    Arrays.fill(bytes, value);
    return bytes;
  }

  private static List<Integer> byteValues(final int length, final int value) {
    final List<Integer> values = new ArrayList<>(length);
    for (int index = 0; index < length; index++) values.add(value & 0xff);
    return values;
  }

  private static List<Integer> byteValues(final byte[] bytes) {
    final List<Integer> values = new ArrayList<>(bytes.length);
    for (final byte value : bytes) values.add(value & 0xff);
    return values;
  }

  private static byte[] bytesOf(final List<?> values) {
    final byte[] bytes = new byte[values.size()];
    for (int index = 0; index < values.size(); index++) {
      bytes[index] = (byte) ((Number) values.get(index)).intValue();
    }
    return bytes;
  }

  private static byte[] hexBytes(final String value) {
    final byte[] bytes = new byte[value.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] =
          (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return bytes;
  }

  private static byte[] u16(final int value) {
    return new byte[] {(byte) (value >>> 8), (byte) value};
  }

  private static byte[] u32(final int value) {
    return new byte[] {
      (byte) (value >>> 24), (byte) (value >>> 16), (byte) (value >>> 8), (byte) value
    };
  }

  private static byte[] u64(final long value) {
    final byte[] bytes = new byte[8];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] = (byte) (value >>> (8 * (7 - index)));
    }
    return bytes;
  }

  private static void append(final ByteArrayOutputStream output, final byte[] value) {
    output.write(value, 0, value.length);
  }

  private static String toHex(final byte[] bytes) {
    final StringBuilder out = new StringBuilder(bytes.length * 2);
    for (final byte value : bytes) out.append(String.format("%02x", value & 0xff));
    return out.toString();
  }

  private static Map<String, Object> instruction(
      final String wireId, final String payloadHex) {
    final Map<String, Object> value = new LinkedHashMap<>();
    value.put("wire_id", wireId);
    value.put("payload_hex", payloadHex);
    return value;
  }

  private static ParliamentApiV1.Proposal proposal(final String kind) {
    return ParliamentApiV1.Proposal.fromJson(encode(validProposal(kind)));
  }

  private static Map<String, Object> validProposal(final String kind) {
    final Map<String, Object> payload;
    switch (kind) {
      case "DeployContract" ->
          payload =
              map(
                  "contract_address", CONTRACT_ADDRESS,
                  "code_hash", "11".repeat(32),
                  "abi_hash", "22".repeat(32),
                  "abi_version", 1,
                  "manifest_provenance", null);
      case "RuntimeUpgrade" ->
          payload =
              map(
                  "manifest",
                  map(
                      "name", "runtime-v1",
                      "description", "first release runtime",
                      "abi_version", 1,
                      "abi_hash", repeatedNumbers(32, 1),
                      "added_syscalls", new ArrayList<Object>(),
                      "added_pointer_types", new ArrayList<Object>(),
                      "start_height", 10,
                      "end_height", 20,
                      "sbom_digests", new ArrayList<Object>(),
                      "slsa_attestation", "",
                      "provenance", new ArrayList<Object>()));
      case "SccpRouteGovernance" ->
          payload =
              map(
                  "anchor",
                  map(
                      "network_id", networkId(),
                      "action",
                      map(
                          "action", "Remove",
                          "route",
                          map(
                              "lane_id", inboundLane(),
                              "route_id", "taira_bsc_xor",
                              "asset_key", "xor",
                              "revision", 1))));
      case "ValidationFeePolicy" ->
          payload =
              map(
                  "proposal_operator", account(1),
                  "policy", disabledFeePolicy(),
                  "payout_lifecycle_proposal_id", null);
      case "ValidationFeePayoutLifecycle" ->
          payload =
              map(
                  "proposal_operator", account(1),
                  "payout_binding", payoutBinding());
      case "MusubiRegistryGovernance" ->
          payload =
              map(
                  "kind", "RecoverPackageOwners",
                  "value",
                  map(
                      "package",
                      map(
                          "home_dataspace", 7,
                          "scope", map("kind", "DataspaceRoot", "value", null),
                          "name", Arrays.asList("wallet-core")),
                      "owners", Arrays.asList(account(2)),
                      "expected_revision", 1));
      case "SorafsProviderGovernance" ->
          payload =
              map(
                  "action",
                  map(
                      "action", "establish",
                      "value",
                      map(
                          "provider_id", Arrays.asList(repeatedNumbers(32, 0x45)),
                          "owner", account(3))));
      default -> throw new AssertionError("unsupported fixture kind " + kind);
    }
    return map("kind", kind, "payload", payload);
  }

  private static Map<String, Object> disabledFeePolicy() {
    return map(
        "schema_version", 1,
        "network_id", networkId(),
        "policy_version", "1",
        "previous_policy_hash", null,
        "ds_asset_id", asset(1),
        "ds_scale", 2,
        "fee", "0",
        "treasury_account_id", account(2),
        "charging_mode", map("charging_mode", "DISABLED", "value", null),
        "effective_from_height", "10",
        "expires_after_height", null,
        "exemption_classes", new ArrayList<Object>(),
        "treasury_payout_binding", null);
  }

  private static Map<String, Object> payoutBinding() {
    final List<Object> recipients = new ArrayList<>();
    for (int seed = 4; seed <= 7; seed++) {
      recipients.add(map("account_id", account(seed), "share", "0.25"));
    }
    return map(
        "contract_address", CONTRACT_ADDRESS,
        "code_hash", repeatedNumbers(32, 0x31),
        "entrypoint", "autonomous_validation_fee_tick",
        "treasury_account_id", account(2),
        "ds_asset_id", asset(1),
        "xor_asset_id", asset(2),
        "pool_vault_account_id", account(3),
        "batch_ds", "10",
        "min_xor_out", "4",
        "max_xor_out", "100",
        "recipients", recipients);
  }

  private static Map<String, Object> inboundLane() {
    return map(
        "source", map("network", "bsc_mainnet", "profile", null),
        "target", map("network", "sora_taira", "profile", null));
  }

  private static String account(final int seed) {
    try {
      return AccountAddress.fromAccount(TestEd25519Keys.publicKey(seed), "ed25519")
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new AssertionError(ex);
    }
  }

  private static String asset(final int seed) {
    final byte[] bytes = new byte[16];
    for (int index = 0; index < bytes.length; index++) bytes[index] = (byte) (seed + index);
    bytes[6] = (byte) ((bytes[6] & 0x0f) | 0x40);
    bytes[8] = (byte) ((bytes[8] & 0x3f) | 0x80);
    return AssetDefinitionIdEncoder.encodeFromBytes(bytes);
  }

  private static String networkId() {
    final byte[] bytes = new byte[32];
    Arrays.fill(bytes, (byte) 0x23);
    bytes[31] = 0x25;
    return NetworkId.fromBytes(bytes).literal();
  }

  private static List<Object> repeatedNumbers(final int size, final int value) {
    final List<Object> result = new ArrayList<>();
    for (int index = 0; index < size; index++) result.add(value);
    return result;
  }

  private static Map<String, Object> map(final Object... entries) {
    if ((entries.length & 1) != 0) throw new AssertionError("map entries must be paired");
    final Map<String, Object> result = new LinkedHashMap<>();
    for (int index = 0; index < entries.length; index += 2) {
      result.put((String) entries[index], entries[index + 1]);
    }
    return result;
  }

  private static byte[] encode(final Map<String, Object> value) {
    return JsonEncoder.encode(value).getBytes(StandardCharsets.UTF_8);
  }

  private static byte[] bytes(final String value) {
    return value.getBytes(StandardCharsets.UTF_8);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> objectValue(final byte[] bytes) {
    return (Map<String, Object>) JsonParser.parse(new String(bytes, StandardCharsets.UTF_8));
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> objectValue(final Object value) {
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Map<String, Object>> maps(final Object value) {
    final List<Map<String, Object>> copied = new ArrayList<>();
    for (final Object entry : (List<Object>) value) {
      copied.add(new LinkedHashMap<>(objectValue(entry)));
    }
    return copied;
  }

  private static Path fixturePath() {
    Path current = Paths.get("").toAbsolutePath();
    while (current != null) {
      final Path candidate = current.resolve("fixtures/governance/parliament_api_v1.json");
      if (Files.isRegularFile(candidate)) return candidate;
      current = current.getParent();
    }
    throw new AssertionError("fixtures/governance/parliament_api_v1.json was not found");
  }
}
