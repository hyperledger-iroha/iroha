package org.hyperledger.iroha.android.client;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.regex.Pattern;
import org.bouncycastle.crypto.digests.KeccakDigest;
import org.hyperledger.iroha.android.sccp.SccpLaneIdV1;
import org.hyperledger.iroha.android.sccp.SccpNetworkV1;
import org.hyperledger.iroha.android.sccp.SccpV1;

/** Strict decoders for the closed exact first-release SCCP JSON API. */
public final class SccpJsonParser {
  private static final Pattern ROUTE_KEY =
      Pattern.compile("[a-z0-9](?:[a-z0-9_-]{0,62}[a-z0-9])?");
  private static final Pattern LOWER_HASH = Pattern.compile("[0-9a-f]{64}");
  private static final Pattern PREFIXED_HASH = Pattern.compile("0x[0-9a-f]{64}");
  private static final Pattern UNSIGNED_DECIMAL = Pattern.compile("0|[1-9][0-9]*");
  private static final Pattern POSITIVE_DECIMAL = Pattern.compile("[1-9][0-9]*");

  private static final Set<String> CAPABILITY_FIELDS =
      Set.of(
          "version",
          "registry_revision",
          "registry_path",
          "message_bundle_path",
          "proof_request_path",
          "recent_messages_path",
          "registry_limits",
          "resource_limits",
          "proof_submit_path",
          "native_message_submit_path");
  private static final Set<String> CAPABILITY_REQUIRED =
      Set.of(
          "version",
          "registry_revision",
          "registry_path",
          "message_bundle_path",
          "proof_request_path",
          "recent_messages_path",
          "registry_limits",
          "resource_limits");
  private static final Set<String> REGISTRY_LIMIT_FIELDS =
      Set.of(
          "max_governed_lanes",
          "max_live_governed_routes",
          "max_live_routes_per_lane",
          "max_retained_routes_per_lane",
          "max_retained_native_trust_anchors_per_lane");
  private static final Set<String> RESOURCE_LIMIT_FIELDS =
      Set.of(
          "max_outbound_messages_per_block",
          "max_outbound_message_payload_bytes",
          "max_pending_outbound_messages",
          "max_pending_outbound_payload_bytes",
          "max_proofs_per_transaction",
          "max_proofs_per_block",
          "max_proof_bytes_per_proof",
          "max_proof_bytes_per_transaction",
          "max_proof_bytes_per_block",
          "max_native_headers_per_transaction",
          "max_native_headers_per_block",
          "max_ethereum_light_client_updates_per_transaction",
          "max_ethereum_light_client_updates_per_block",
          "max_native_header_bytes_per_transaction",
          "max_native_header_bytes_per_block",
          "max_secp256k1_recoveries_per_transaction",
          "max_secp256k1_recoveries_per_block",
          "max_bls_aggregate_checks_per_transaction",
          "max_bls_aggregate_checks_per_block",
          "max_bls_signer_contributions_per_transaction",
          "max_bls_signer_contributions_per_block",
          "max_bn254_pairing_checks_per_transaction",
          "max_bn254_pairing_checks_per_block");
  private static final Set<String> ROUTE_FIELDS =
      Set.of(
          "lane_id",
          "route_id",
          "asset_key",
          "revision",
          "activation",
          "inbound_finality_cutoff",
          "source_identity",
          "destination",
          "settlement");
  private static final Set<String> DESTINATION_FIELDS =
      Set.of(
          "token_address",
          "token_code_hash",
          "verifier_address",
          "verifier_code_hash",
          "verifying_key",
          "verifier_key_hash",
          "outbound_proof_policy",
          "route_address",
          "route_code_hash",
          "taira_to_token_multiplier");
  private static final Set<String> VERIFYING_KEY_FIELDS =
      Set.of("version", "alpha1", "beta2", "gamma2", "delta2", "ic");
  private static final List<String> IC_FIELD_ORDER =
      List.of(
          "constant",
          "signal_0",
          "signal_1",
          "signal_2",
          "signal_3",
          "signal_4",
          "signal_5",
          "signal_6",
          "signal_7",
          "signal_8",
          "signal_9",
          "signal_10");
  private static final Set<String> IC_FIELDS = new LinkedHashSet<>(IC_FIELD_ORDER);
  private static final Set<String> FINALITY_ANCHOR_FIELDS =
      Set.of(
          "version",
          "source_network",
          "protocol_version",
          "chain_id_hash",
          "checkpoint_height",
          "checkpoint_block_hash",
          "checkpoint_context_id",
          "checkpoint_finality_artifact_hash");
  private static final Set<String> PROOF_REQUEST_FIELDS =
      Set.of(
          "version",
          "backend",
          "source_network",
          "target_network",
          "public_inputs",
          "verifying_key",
          "verifier_key_hash",
          "semantic_proof_profile",
          "semantic_proof_profile_hash",
          "sora_finality_anchor",
          "sora_finality_anchor_hash",
          "bundle_bytes",
          "statement_hash",
          "destination_binding_hash",
          "route_configuration_hash",
          "request_hash");
  private static final Set<String> TRANSFER_FIELDS =
      Set.of(
          "version",
          "source_domain",
          "dest_domain",
          "nonce",
          "route_revision",
          "asset_home_domain",
          "asset_id_codec",
          "asset_id",
          "amount",
          "sender_codec",
          "sender",
          "recipient_codec",
          "recipient",
          "route_id_codec",
          "route_id");
  private static final Set<String> PROJECTION_TRANSFER_FIELDS =
      Set.of(
          "version",
          "source_domain",
          "dest_domain",
          "nonce",
          "route_revision",
          "asset_home_domain",
          "asset_id",
          "amount",
          "sender",
          "recipient",
          "route_id");
  private static final Set<String> RECENT_FIELDS =
      Set.of(
          "height",
          "commitment_index",
          "message_id_hex",
          "kind",
          "source_profile",
          "target_profile",
          "destination_binding_hash",
          "route_configuration_hash",
          "target_domain",
          "asset_id",
          "route_id",
          "recipient",
          "amount",
          "payload_projection",
          "links");
  private static final Set<String> RECENT_REQUIRED =
      Set.of(
          "height",
          "commitment_index",
          "message_id_hex",
          "kind",
          "source_profile",
          "target_profile",
          "destination_binding_hash",
          "route_configuration_hash",
          "target_domain",
          "amount",
          "payload_projection",
          "links");
  private static final Set<String> ACTIVATIONS =
      Set.of("staged", "bidirectional", "inbound_only", "paused", "retired");
  private static final String SEMANTIC_PROFILE =
      "sora_taira_finality_inclusion_groth16_bn254";
  private static final String TAIRA_XOR_ASSET_ID = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
  private static final String EVM_DESTINATION_BINDING_DOMAIN =
      "iroha:sccp:evm-destination-binding:v1";
  private static final String TRON_DESTINATION_BINDING_DOMAIN =
      "iroha:sccp:tron-destination-binding:v1";
  private static final String EVM_GROTH16_BACKEND = "evm-groth16-bn254-v1";
  private static final String TRON_GROTH16_BACKEND = "tron-groth16-bn254-v1";
  private static final String CONCRETE_ROUTE_CONFIG_DOMAIN =
      "sccp:concrete-route-config:v1";
  private static final BigInteger BN254_MODULUS =
      new BigInteger("30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47", 16);
  private static final BigInteger MAX_U32 = BigInteger.ONE.shiftLeft(32).subtract(BigInteger.ONE);
  private static final BigInteger MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  private static final BigInteger MAX_JSON_SAFE_INTEGER =
      new BigInteger("9007199254740991");
  private static final BigInteger MAX_U128 = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE);
  private static final String TAIRA_CHAIN_ID_HASH =
      upperHex(
          keccak(
              new byte[] {
                (byte) 0xfc,
                0x56,
                (byte) 0x98,
                0x4b,
                0x2b,
                (byte) 0xe7,
                0x43,
                0x1d,
                (byte) 0x84,
                0x0e,
                0x21,
                0x51,
                0x4d,
                0x18,
                (byte) 0x83,
                (byte) 0xf0
              }));
  private static final List<String> PUBLIC_SIGNAL_LABELS =
      List.of(
          "sccp:groth16-bn254:signal:message-id:v1",
          "sccp:groth16-bn254:signal:payload-hash:v1",
          "sccp:groth16-bn254:signal:target-domain:v1",
          "sccp:groth16-bn254:signal:commitment-root:v1",
          "sccp:groth16-bn254:signal:finality-height:v1",
          "sccp:groth16-bn254:signal:finality-block-hash:v1",
          "sccp:groth16-bn254:signal:source-domain:v1",
          "sccp:groth16-bn254:signal:statement-hash:v1",
          "sccp:groth16-bn254:signal:destination-binding-hash:v1",
          "sccp:groth16-bn254:signal:route-configuration-hash:v1",
          "sccp:groth16-bn254:signal:sora-finality-anchor-hash:v1");
  private static final String PUBLIC_SIGNAL_SCHEMA_HASH = publicSignalSchemaHash();

  private SccpJsonParser() {}

  /** Validates one exact closed SCCP V1 route-governance action. */
  public static void validateRouteGovernanceAction(final Map<String, Object> value) {
    exactFields(value, governanceFields("action", "route"), "SCCP route governance action");
    final String action = requiredText(value, "action");
    final Map<String, Object> payload = requiredObject(value, "route");
    switch (action) {
      case "Register" -> {
        exactFields(
            payload, governanceFields("route", "native_trust_anchor"), "SCCP Register");
        final Map<String, Object> route = requiredObject(payload, "route");
        final SccpLaneIdV1 lane =
            parseInboundLane(requiredObject(route, "lane_id"), "SCCP Register.route.lane_id");
        final ParsedRoute parsed = parseGovernedRoute(route, lane, "SCCP Register.route");
        if (!"staged".equals(parsed.activation())) {
          throw new IllegalArgumentException("new SCCP routes must be staged");
        }
        if (payload.get("native_trust_anchor") != null) {
          final NativeTrustAnchor anchor =
              parseNativeTrustAnchor(
                  objectValue(
                      payload.get("native_trust_anchor"),
                      "SCCP Register.native_trust_anchor"),
                  lane,
                  "SCCP Register.native_trust_anchor");
          if (anchor.checkpointHeight().compareTo(MAX_JSON_SAFE_INTEGER) > 0) {
            throw new IllegalArgumentException(
                "SCCP Register.native_trust_anchor exceeds the exact JSON integer bound");
          }
        }
      }
      case "SetActivation" -> {
        exactFields(
            payload,
            governanceFields("key", "expected_current", "next", "inbound_finality_cutoff"),
            "SCCP SetActivation");
        parseGovernanceRouteKey(requiredObject(payload, "key"), "SCCP SetActivation.key");
        final String current =
            parseGovernanceActivation(
                requiredObject(payload, "expected_current"),
                "SCCP SetActivation.expected_current");
        final String next =
            parseGovernanceActivation(
                requiredObject(payload, "next"), "SCCP SetActivation.next");
        validateGovernanceCutoff(
            payload.get("inbound_finality_cutoff"),
            next,
            "SCCP SetActivation.inbound_finality_cutoff");
        if (!canTransitionGovernanceActivation(current, next)) {
          throw new IllegalArgumentException("SCCP activation transition is not legal");
        }
      }
      case "SwitchRevision" -> {
        exactFields(
            payload,
            governanceFields(
                "previous_key",
                "expected_previous",
                "previous_next",
                "previous_inbound_finality_cutoff",
                "successor_key",
                "successor_next"),
            "SCCP SwitchRevision");
        final GovernanceRouteKey previous =
            parseGovernanceRouteKey(
                requiredObject(payload, "previous_key"), "SCCP SwitchRevision.previous_key");
        final GovernanceRouteKey successor =
            parseGovernanceRouteKey(
                requiredObject(payload, "successor_key"), "SCCP SwitchRevision.successor_key");
        final String expected =
            parseGovernanceActivation(
                requiredObject(payload, "expected_previous"),
                "SCCP SwitchRevision.expected_previous");
        final String previousNext =
            parseGovernanceActivation(
                requiredObject(payload, "previous_next"),
                "SCCP SwitchRevision.previous_next");
        final String successorNext =
            parseGovernanceActivation(
                requiredObject(payload, "successor_next"),
                "SCCP SwitchRevision.successor_next");
        validateGovernanceCutoff(
            payload.get("previous_inbound_finality_cutoff"),
            previousNext,
            "SCCP SwitchRevision.previous_inbound_finality_cutoff");
        final boolean previousTransitionValid =
            "retired".equals(previousNext)
                ? governanceFields("bidirectional", "inbound_only", "paused").contains(expected)
                : canTransitionGovernanceActivation(expected, previousNext);
        if (!previous.lane().equals(successor.lane())
            || !previous.routeId().equals(successor.routeId())
            || !previous.assetKey().equals(successor.assetKey())
            || successor.revision() != previous.revision() + 1
            || !previousTransitionValid
            || !governanceFields("inbound_only", "paused", "retired").contains(previousNext)
            || !"bidirectional".equals(successorNext)) {
          throw new IllegalArgumentException(
              "SCCP revision switch is not a legal atomic cutover");
        }
      }
      case "InitializeTrustAnchor" -> {
        exactFields(
            payload,
            governanceFields("lane_id", "expected_current", "initial"),
            "SCCP InitializeTrustAnchor");
        final SccpLaneIdV1 lane =
            parseInboundLane(
                requiredObject(payload, "lane_id"),
                "SCCP InitializeTrustAnchor.lane_id");
        if (payload.get("expected_current") != null) {
          throw new IllegalArgumentException(
              "SCCP initial trust anchor must expect no current value");
        }
        final NativeTrustAnchor initial =
            parseNativeTrustAnchor(
                objectValue(payload.get("initial"), "SCCP InitializeTrustAnchor.initial"),
                lane,
                "SCCP InitializeTrustAnchor.initial");
        if (initial.checkpointHeight().compareTo(MAX_JSON_SAFE_INTEGER) > 0) {
          throw new IllegalArgumentException(
              "SCCP InitializeTrustAnchor.initial exceeds the exact JSON integer bound");
        }
      }
      case "AdvanceTrustAnchor" -> {
        exactFields(
            payload,
            governanceFields("lane_id", "expected_current", "next"),
            "SCCP AdvanceTrustAnchor");
        final SccpLaneIdV1 lane =
            parseInboundLane(
                requiredObject(payload, "lane_id"), "SCCP AdvanceTrustAnchor.lane_id");
        final NativeTrustAnchor current =
            parseNativeTrustAnchor(
                objectValue(
                    payload.get("expected_current"),
                    "SCCP AdvanceTrustAnchor.expected_current"),
                lane,
                "SCCP AdvanceTrustAnchor.expected_current");
        final NativeTrustAnchor next =
            parseNativeTrustAnchor(
                objectValue(payload.get("next"), "SCCP AdvanceTrustAnchor.next"),
                lane,
                "SCCP AdvanceTrustAnchor.next");
        if (!current.backend().equals(next.backend())
            || current.anchorHash().equals(next.anchorHash())
            || current.checkpointHeight().compareTo(MAX_JSON_SAFE_INTEGER) > 0
            || next.checkpointHeight().compareTo(MAX_JSON_SAFE_INTEGER) > 0
            || next.checkpointHeight().compareTo(current.checkpointHeight()) <= 0) {
          throw new IllegalArgumentException(
              "SCCP trust anchor must advance monotonically within one backend");
        }
      }
      case "Remove" -> parseGovernanceRouteKey(payload, "SCCP Remove");
      default ->
          throw new IllegalArgumentException("SCCP route governance action is unsupported");
    }
  }

  public static SccpModels.Capabilities parseCapabilities(final byte[] bytes) {
    final Map<String, Object> root = rootObject(bytes, "SCCP capabilities");
    exactFields(root, CAPABILITY_FIELDS, CAPABILITY_REQUIRED, "SCCP capabilities");
    final String proofSubmitPath =
        exactPath(root, "proof_submit_path", "/v1/bridge/proofs/submit", true);
    final String nativeMessageSubmitPath =
        exactPath(root, "native_message_submit_path", "/v1/bridge/messages", true);
    if ((proofSubmitPath == null) != (nativeMessageSubmitPath == null)) {
      throw new IllegalArgumentException(
          "SCCP write capability paths must be advertised together");
    }
    final SccpModels.Capabilities result =
        new SccpModels.Capabilities(
            requiredInt(root, "version", 1, 1),
            prefixedHash(root, "registry_revision"),
            exactPath(root, "registry_path", "/v1/sccp/registry", false),
            exactPath(
                root,
                "message_bundle_path",
                "/v1/sccp/proofs/message/{message_id}",
                false),
            exactPath(
                root,
                "proof_request_path",
                "/v1/sccp/proof-requests/{message_id}",
                false),
            exactPath(root, "recent_messages_path", "/v1/sccp/messages/recent", false),
            parseRegistryLimits(requiredObject(root, "registry_limits")),
            parseResourceLimits(requiredObject(root, "resource_limits")),
            proofSubmitPath,
            nativeMessageSubmitPath);
    requireDistinctHashes(List.of(result.registryRevision), "capability registry revision");
    return result;
  }

  private static long requiredU32(
      final Map<String, Object> value, final String field) {
    return requiredUnsignedInteger(value, field, MAX_U32, true).longValueExact();
  }

  private static SccpModels.RegistryLimits parseRegistryLimits(
      final Map<String, Object> value) {
    exactFields(value, REGISTRY_LIMIT_FIELDS, "SCCP registry limits");
    final SccpModels.RegistryLimits result =
        new SccpModels.RegistryLimits(
            requiredU32(value, "max_governed_lanes"),
            requiredU32(value, "max_live_governed_routes"),
            requiredU32(value, "max_live_routes_per_lane"),
            requiredU32(value, "max_retained_routes_per_lane"),
            requiredU32(value, "max_retained_native_trust_anchors_per_lane"));
    if (result.maxGovernedLanes != 16
        || result.maxLiveGovernedRoutes != 64
        || result.maxLiveRoutesPerLane != 8
        || result.maxRetainedRoutesPerLane != 64
        || result.maxRetainedNativeTrustAnchorsPerLane != 4_096) {
      throw new IllegalArgumentException(
          "SCCP registry limits must equal the fixed V1 capacities");
    }
    return result;
  }

  private static SccpModels.ResourceLimits parseResourceLimits(
      final Map<String, Object> value) {
    exactFields(value, RESOURCE_LIMIT_FIELDS, "SCCP resource limits");
    final SccpModels.ResourceLimits result =
        new SccpModels.ResourceLimits(
            requiredU32(value, "max_outbound_messages_per_block"),
            requiredUnsignedInteger(
                value,
                "max_outbound_message_payload_bytes",
                MAX_JSON_SAFE_INTEGER,
                true),
            requiredUnsignedInteger(
                value, "max_pending_outbound_messages", MAX_JSON_SAFE_INTEGER, true),
            requiredUnsignedInteger(
                value,
                "max_pending_outbound_payload_bytes",
                MAX_JSON_SAFE_INTEGER,
                true),
            requiredU32(value, "max_proofs_per_transaction"),
            requiredU32(value, "max_proofs_per_block"),
            requiredUnsignedInteger(
                value, "max_proof_bytes_per_proof", MAX_JSON_SAFE_INTEGER, true),
            requiredUnsignedInteger(
                value, "max_proof_bytes_per_transaction", MAX_JSON_SAFE_INTEGER, true),
            requiredUnsignedInteger(
                value, "max_proof_bytes_per_block", MAX_JSON_SAFE_INTEGER, true),
            requiredU32(value, "max_native_headers_per_transaction"),
            requiredU32(value, "max_native_headers_per_block"),
            requiredU32(value, "max_ethereum_light_client_updates_per_transaction"),
            requiredU32(value, "max_ethereum_light_client_updates_per_block"),
            requiredUnsignedInteger(
                value,
                "max_native_header_bytes_per_transaction",
                MAX_JSON_SAFE_INTEGER,
                true),
            requiredUnsignedInteger(
                value,
                "max_native_header_bytes_per_block",
                MAX_JSON_SAFE_INTEGER,
                true),
            requiredU32(value, "max_secp256k1_recoveries_per_transaction"),
            requiredU32(value, "max_secp256k1_recoveries_per_block"),
            requiredU32(value, "max_bls_aggregate_checks_per_transaction"),
            requiredU32(value, "max_bls_aggregate_checks_per_block"),
            requiredU32(value, "max_bls_signer_contributions_per_transaction"),
            requiredU32(value, "max_bls_signer_contributions_per_block"),
            requiredU32(value, "max_bn254_pairing_checks_per_transaction"),
            requiredU32(value, "max_bn254_pairing_checks_per_block"));
    if (result.maxOutboundMessagesPerBlock
            != SccpModels.SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1
        || !result.maxOutboundMessagePayloadBytes.equals(
            BigInteger.valueOf(SccpModels.SCCP_OUTBOUND_MESSAGE_MAX_PAYLOAD_BYTES_V1))) {
      throw new IllegalArgumentException(
          "SCCP outbound-message limits must equal the fixed V1 capacities");
    }
    if (result.maxProofBytesPerProof.compareTo(result.maxProofBytesPerTransaction) > 0) {
      throw new IllegalArgumentException(
          "SCCP per-proof byte limit exceeds its transaction limit");
    }
    final List<List<BigInteger>> ordered =
        List.of(
            List.of(
                BigInteger.valueOf(result.maxProofsPerTransaction),
                BigInteger.valueOf(result.maxProofsPerBlock)),
            List.of(result.maxProofBytesPerTransaction, result.maxProofBytesPerBlock),
            List.of(
                BigInteger.valueOf(result.maxNativeHeadersPerTransaction),
                BigInteger.valueOf(result.maxNativeHeadersPerBlock)),
            List.of(
                BigInteger.valueOf(result.maxEthereumLightClientUpdatesPerTransaction),
                BigInteger.valueOf(result.maxEthereumLightClientUpdatesPerBlock)),
            List.of(
                result.maxNativeHeaderBytesPerTransaction,
                result.maxNativeHeaderBytesPerBlock),
            List.of(
                BigInteger.valueOf(result.maxSecp256k1RecoveriesPerTransaction),
                BigInteger.valueOf(result.maxSecp256k1RecoveriesPerBlock)),
            List.of(
                BigInteger.valueOf(result.maxBlsAggregateChecksPerTransaction),
                BigInteger.valueOf(result.maxBlsAggregateChecksPerBlock)),
            List.of(
                BigInteger.valueOf(result.maxBlsSignerContributionsPerTransaction),
                BigInteger.valueOf(result.maxBlsSignerContributionsPerBlock)),
            List.of(
                BigInteger.valueOf(result.maxBn254PairingChecksPerTransaction),
                BigInteger.valueOf(result.maxBn254PairingChecksPerBlock)));
    for (final List<BigInteger> pair : ordered) {
      if (pair.get(0).compareTo(pair.get(1)) > 0) {
        throw new IllegalArgumentException(
            "SCCP transaction resource limits must not exceed block limits");
      }
    }
    return result;
  }

  public static SccpModels.RegistryV1 parseRegistry(final byte[] bytes) {
    final Map<String, Object> root = rootObject(bytes, "SCCP registry");
    exactFields(root, Set.of("version", "lanes"), "SCCP registry");
    final List<Object> rawLanes = requiredList(root, "lanes");
    if (rawLanes.size() > 16) {
      throw new IllegalArgumentException("SCCP registry contains more than 16 lanes");
    }
    final Set<String> laneKeys = new HashSet<>();
    final Set<String> routeKeys = new HashSet<>();
    final Set<String> destinationBindings = new HashSet<>();
    final Set<String> routeConfigurations = new HashSet<>();
    final List<Map<String, Object>> lanes = new ArrayList<>();
    int liveRouteCount = 0;
    for (int laneIndex = 0; laneIndex < rawLanes.size(); laneIndex++) {
      final String label = "SCCP registry.lanes[" + laneIndex + "]";
      final Map<String, Object> laneRecord = objectValue(rawLanes.get(laneIndex), label);
      exactFields(
          laneRecord,
          Set.of(
              "lane_id",
              "native_trust_anchors",
              "current_native_trust_anchor_hash",
              "routes"),
          label);
      final SccpLaneIdV1 lane = parseInboundLane(requiredObject(laneRecord, "lane_id"), label);
      if (!laneKeys.add(lane.toString())) {
        throw new IllegalArgumentException("SCCP registry contains a duplicate lane");
      }
      final List<Object> rawAnchors = requiredList(laneRecord, "native_trust_anchors");
      if (rawAnchors.size() > 4_096) {
        throw new IllegalArgumentException(
            label + " contains more than 4,096 retained native trust anchors");
      }
      final List<NativeTrustAnchor> anchors = new ArrayList<>();
      final Set<String> anchorHashes = new HashSet<>();
      NativeTrustAnchor previousAnchor = null;
      for (int anchorIndex = 0; anchorIndex < rawAnchors.size(); anchorIndex++) {
        final String anchorLabel =
            label + ".native_trust_anchors[" + anchorIndex + "]";
        final NativeTrustAnchor anchor =
            parseNativeTrustAnchor(
                objectValue(rawAnchors.get(anchorIndex), anchorLabel), lane, anchorLabel);
        if (!anchorHashes.add(anchor.anchorHash())) {
          throw new IllegalArgumentException(
              label + " contains a duplicate native trust-anchor hash");
        }
        if (previousAnchor != null
            && (!previousAnchor.backend().equals(anchor.backend())
                || anchor.checkpointHeight().compareTo(previousAnchor.checkpointHeight()) <= 0)) {
          throw new IllegalArgumentException(
              label
                  + ".native_trust_anchors must advance monotonically within one backend");
        }
        anchors.add(anchor);
        previousAnchor = anchor;
      }
      final String currentAnchorHash =
          laneRecord.get("current_native_trust_anchor_hash") == null
              ? null
              : upperBytes(laneRecord, "current_native_trust_anchor_hash", 32);
      final String expectedCurrentAnchorHash =
          previousAnchor == null ? null : previousAnchor.anchorHash();
      if (!java.util.Objects.equals(currentAnchorHash, expectedCurrentAnchorHash)) {
        throw new IllegalArgumentException(
            label
                + ".current_native_trust_anchor_hash must name the last retained anchor");
      }
      final List<Object> routes = requiredList(laneRecord, "routes");
      if (routes.isEmpty()) {
        throw new IllegalArgumentException(label + ".routes must contain at least one route");
      }
      if (routes.size() > 64) {
        throw new IllegalArgumentException(
            label + " contains more than 64 retained route revisions");
      }
      int laneLiveRouteCount = 0;
      final Map<String, List<ParsedRoute>> lineages = new LinkedHashMap<>();
      for (int routeIndex = 0; routeIndex < routes.size(); routeIndex++) {
        final String routeLabel = label + ".routes[" + routeIndex + "]";
        final ParsedRoute route =
            parseGovernedRoute(objectValue(routes.get(routeIndex), routeLabel), lane, routeLabel);
        if (!routeKeys.add(route.key())) {
          throw new IllegalArgumentException("SCCP registry contains a duplicate route");
        }
        if (!destinationBindings.add(route.destinationBindingHash())) {
          throw new IllegalArgumentException(
              "SCCP registry reuses a destination-binding hash");
        }
        if (!routeConfigurations.add(route.routeConfigurationHash())) {
          throw new IllegalArgumentException(
              "SCCP registry reuses a route-configuration hash");
        }
        if (!"retired".equals(route.activation())) {
          laneLiveRouteCount++;
          liveRouteCount++;
        }
        if (("bidirectional".equals(route.activation())
                || "inbound_only".equals(route.activation()))
            && previousAnchor == null) {
          throw new IllegalArgumentException(
              label + " cannot enable inbound settlement without a trust anchor");
        }
        if (route.inboundFinalityCutoff() != null) {
          int anchorIndex = -1;
          for (int index = 0; index < anchors.size(); index++) {
            if (anchors.get(index).anchorHash().equals(
                route.inboundFinalityCutoff().trustAnchorHash)) {
              anchorIndex = index;
              break;
            }
          }
          if (anchorIndex < 0
              || anchorIndex + 1 >= anchors.size()
              || !anchors
                  .get(anchorIndex + 1)
                  .checkpointHeight()
                  .equals(route.inboundFinalityCutoff().maxAnchorIntervalHeight)) {
            throw new IllegalArgumentException(
                routeLabel
                    + ".inbound_finality_cutoff must close one retained anchor interval");
          }
        }
        lineages.computeIfAbsent(route.lineage(), ignored -> new ArrayList<>()).add(route);
      }
      for (final List<ParsedRoute> revisions : lineages.values()) {
        revisions.sort((left, right) -> Long.compare(left.revision(), right.revision()));
        int enabled = 0;
        for (int index = 0; index < revisions.size(); index++) {
          if (revisions.get(index).revision() != index + 1L) {
            throw new IllegalArgumentException(
                "SCCP route revisions must start at one and contain no gaps");
          }
          if ("bidirectional".equals(revisions.get(index).activation())) enabled++;
        }
        if (enabled > 1) {
          throw new IllegalArgumentException(
              "SCCP registry enables multiple revisions of one route");
        }
      }
      if (laneLiveRouteCount > 8) {
        throw new IllegalArgumentException(label + " contains more than 8 live routes");
      }
      lanes.add(deepCopyObject(laneRecord));
    }
    if (liveRouteCount > 64) {
      throw new IllegalArgumentException("SCCP registry contains more than 64 live routes");
    }
    return new SccpModels.RegistryV1(requiredInt(root, "version", 1, 1), lanes);
  }

  public static SccpModels.MessageBundleV1 parseMessageBundle(final byte[] bytes) {
    final Map<String, Object> root = rootObject(bytes, "SCCP message bundle");
    exactFields(
        root,
        Set.of(
            "version",
            "commitment_root",
            "commitment",
            "merkle_proof",
            "payload",
            "finality_proof"),
        "SCCP message bundle");
    requiredInt(root, "version", 1, 1);
    final String commitmentRoot = prefixedHash(root, "commitment_root");
    final Map<String, Object> commitment = requiredObject(root, "commitment");
    exactFields(
        commitment,
        Set.of("version", "kind", "context", "message_id", "payload_hash"),
        "SCCP commitment");
    requiredInt(commitment, "version", 1, 1);
    if (!"Transfer".equals(requiredText(commitment, "kind"))) {
      throw new IllegalArgumentException("SCCP commitment kind must be Transfer");
    }
    final Map<String, Object> context = requiredObject(commitment, "context");
    exactFields(
        context,
        Set.of("lane", "destination_binding_hash", "route_configuration_hash"),
        "SCCP commitment context");
    final SccpLaneIdV1 lane =
        parseLane(requiredObject(context, "lane"), "SCCP commitment context.lane");
    if (!lane.isOutbound() || lane.source() != SccpNetworkV1.SORA_TAIRA) {
      throw new IllegalArgumentException(
          "SCCP message bundle must use an exact Taira-to-external lane");
    }
    final String binding = prefixedHash(context, "destination_binding_hash");
    final String configuration = prefixedHash(context, "route_configuration_hash");
    final String messageId = prefixedHash(commitment, "message_id");
    final String payloadHash = prefixedHash(commitment, "payload_hash");
    requireDistinctHashes(
        List.of(commitmentRoot, binding, configuration, messageId, payloadHash),
        "message bundle");
    validateTransferPayload(requiredObject(root, "payload"), lane);
    final Map<String, Object> merkle = requiredObject(root, "merkle_proof");
    exactFields(merkle, Set.of("steps"), "SCCP Merkle proof");
    final List<Object> steps = requiredList(merkle, "steps");
    if (steps.size() > 64) {
      throw new IllegalArgumentException("SCCP Merkle proof contains more than 64 steps");
    }
    for (int index = 0; index < steps.size(); index++) {
      final Map<String, Object> step = objectValue(steps.get(index), "SCCP Merkle step");
      exactFields(step, Set.of("sibling_hash", "sibling_is_left"), "SCCP Merkle step");
      prefixedHash(step, "sibling_hash");
      requiredBoolean(step, "sibling_is_left");
    }
    requiredHexBytes(root, "finality_proof", false);
    return new SccpModels.MessageBundleV1(
        1,
        messageId.substring(2),
        lane.source(),
        lane.target(),
        binding,
        configuration,
        deepCopyObject(root));
  }

  public static SccpModels.Groth16ProofRequestV1 parseProofRequest(final byte[] bytes) {
    final Map<String, Object> root = rootObject(bytes, "SCCP proof request");
    exactFields(root, PROOF_REQUEST_FIELDS, "SCCP proof request");
    requiredInt(root, "version", 1, 1);
    final Map<String, Object> backendObject = requiredObject(root, "backend");
    exactFields(backendObject, Set.of("backend", "family"), "SCCP proof backend");
    if (backendObject.get("family") != null) {
      throw new IllegalArgumentException("SCCP proof backend family must be null");
    }
    final String backend = requiredText(backendObject, "backend");
    if (!"evm_groth16_bn254_v1".equals(backend)
        && !"tron_groth16_bn254_v1".equals(backend)) {
      throw new IllegalArgumentException("SCCP proof backend is unsupported or retired");
    }
    final SccpNetworkV1 source =
        parseNetwork(requiredObject(root, "source_network"), "source_network");
    final SccpNetworkV1 target =
        parseNetwork(requiredObject(root, "target_network"), "target_network");
    if (source != SccpNetworkV1.SORA_TAIRA || !target.isExternal()) {
      throw new IllegalArgumentException(
          "SCCP proof request must use an exact Taira-to-external lane");
    }
    if (backend.startsWith("tron") != (target.domainId() == 5)) {
      throw new IllegalArgumentException("SCCP proof backend does not match target network");
    }
    final Map<String, Object> inputs = requiredObject(root, "public_inputs");
    exactFields(
        inputs,
        Set.of(
            "version",
            "message_id",
            "payload_hash",
            "target_domain",
            "commitment_root",
            "finality_height",
            "finality_block_hash"),
        "SCCP proof public inputs");
    requiredInt(inputs, "version", 1, 1);
    final String messageId = prefixedHash(inputs, "message_id");
    final String payloadHash = prefixedHash(inputs, "payload_hash");
    if (requiredInt(inputs, "target_domain", 1, 5) != target.domainId()) {
      throw new IllegalArgumentException(
          "SCCP proof target domain does not match target network");
    }
    final String commitmentRoot = prefixedHash(inputs, "commitment_root");
    if (new BigInteger(requiredDecimal(inputs, "finality_height", true)).bitLength() > 64) {
      throw new IllegalArgumentException("SCCP proof finality height must fit u64");
    }
    final String finalityBlockHash = prefixedHash(inputs, "finality_block_hash");
    final String verifierKeyHash = prefixedHash(root, "verifier_key_hash");
    validateVerifyingKey(
        requiredObject(root, "verifying_key"),
        verifierKeyHash.substring(2).toUpperCase(),
        "SCCP proof verifying key");
    final ParsedProofPolicy policyHashes =
        validateOutboundProofPolicyFields(root, "SCCP proof request");
    final String semanticHash = prefixedHash(root, "semantic_proof_profile_hash");
    if (!semanticHash.equals("0x" + policyHashes.profileHash().toLowerCase(Locale.ROOT))) {
      throw new IllegalArgumentException(
          "semantic_proof_profile_hash does not match its typed profile");
    }
    final String anchorHash = prefixedHash(root, "sora_finality_anchor_hash");
    if (!anchorHash.equals("0x" + policyHashes.anchorHash().toLowerCase(Locale.ROOT))) {
      throw new IllegalArgumentException(
          "sora_finality_anchor_hash does not match its typed anchor");
    }
    requiredHexBytes(root, "bundle_bytes", false);
    final List<String> roles =
        List.of(
            messageId,
            payloadHash,
            commitmentRoot,
            finalityBlockHash,
            verifierKeyHash,
            semanticHash,
            anchorHash,
            prefixedHash(root, "statement_hash"),
            prefixedHash(root, "destination_binding_hash"),
            prefixedHash(root, "route_configuration_hash"),
            prefixedHash(root, "request_hash"));
    requireDistinctHashes(roles, "proof request");
    return new SccpModels.Groth16ProofRequestV1(
        1,
        backend,
        source,
        target,
        messageId.substring(2),
        roles.get(roles.size() - 1),
        policyHashes.semanticProfile(),
        policyHashes.soraFinalityAnchor(),
        deepCopyObject(root));
  }

  public static SccpModels.RecentMessages parseRecentMessages(final byte[] bytes) {
    final Map<String, Object> root = rootObject(bytes, "SCCP recent messages");
    exactFields(root, Set.of("items", "next"), Set.of("items"), "SCCP recent messages");
    final List<SccpModels.RecentMessage> items = new ArrayList<>();
    final Set<String> ids = new HashSet<>();
    SccpModels.RecentMessage previous = null;
    final List<Object> values = requiredList(root, "items");
    if (values.size() > 50) {
      throw new IllegalArgumentException("SCCP recent response exceeds 50 items");
    }
    for (int index = 0; index < values.size(); index++) {
      final SccpModels.RecentMessage item =
          parseRecent(objectValue(values.get(index), "items[" + index + "]"), index);
      if (previous != null
          && !(previous.height.compareTo(item.height) > 0
              || (previous.height.equals(item.height)
                  && previous.commitmentIndex < item.commitmentIndex))) {
        throw new IllegalArgumentException(
            "SCCP recent messages must use strict height-descending/index-ascending order");
      }
      if (!ids.add(item.messageIdHex)) {
        throw new IllegalArgumentException("SCCP recent messages contain duplicate message ids");
      }
      previous = item;
      items.add(item);
    }
    final SccpModels.RecentCursor next;
    if (root.get("next") == null) {
      next = null;
    } else {
      final Map<String, Object> value = requiredObject(root, "next");
      exactFields(value, Set.of("from", "after_index"), "SCCP recent messages.next");
      next =
          new SccpModels.RecentCursor(
              requiredUnsignedInteger(value, "from", MAX_U64, true),
              requiredInt(
                  value,
                  "after_index",
                  0,
                  SccpModels.SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 - 1));
    }
    if (next != null) {
      if (items.isEmpty()) {
        throw new IllegalArgumentException(
            "SCCP recent messages.next requires a non-empty page");
      }
      final SccpModels.RecentMessage last = items.get(items.size() - 1);
      if (!next.from.equals(last.height) || next.afterIndex != last.commitmentIndex) {
        throw new IllegalArgumentException(
            "SCCP recent messages.next must identify the last returned item");
      }
    }
    return new SccpModels.RecentMessages(items, next);
  }

  private static ParsedRoute parseGovernedRoute(
      final Map<String, Object> value, final SccpLaneIdV1 lane, final String label) {
    exactFields(value, ROUTE_FIELDS, label);
    if (!parseInboundLane(requiredObject(value, "lane_id"), label).equals(lane)) {
      throw new IllegalArgumentException(label + ".lane_id does not match its registry lane");
    }
    final String routeId = canonicalRouteKey(value, "route_id");
    final String assetKey = canonicalRouteKey(value, "asset_key");
    final long revision = requiredLong(value, "revision", 1, 0xffff_ffffL);
    final ExternalNetworkParameters networkParameters = externalNetworkParameters(lane.source());
    if (!networkParameters.routeId().equals(routeId) || !"xor".equals(assetKey)) {
      throw new IllegalArgumentException(
          label + " does not identify the exact first-release XOR route");
    }
    final Map<String, Object> activationObject = requiredObject(value, "activation");
    exactFields(activationObject, Set.of("activation", "direction"), label + ".activation");
    if (activationObject.get("direction") != null) {
      throw new IllegalArgumentException(label + ".activation.direction must be null");
    }
    final String activation = requiredText(activationObject, "activation");
    if (!ACTIVATIONS.contains(activation)) {
      throw new IllegalArgumentException(label + ".activation is unsupported");
    }
    final SccpModels.InboundFinalityCutoffV1 inboundFinalityCutoff;
    if ("retired".equals(activation)) {
      final Map<String, Object> cutoff = requiredObject(value, "inbound_finality_cutoff");
      exactFields(
          cutoff,
          Set.of("trust_anchor_hash", "max_anchor_interval_height"),
          label + ".inbound_finality_cutoff");
      inboundFinalityCutoff =
          new SccpModels.InboundFinalityCutoffV1(
              upperBytes(cutoff, "trust_anchor_hash", 32),
              requiredUnsignedInteger(
                  cutoff, "max_anchor_interval_height", MAX_U64, true));
    } else {
      if (value.get("inbound_finality_cutoff") != null) {
        throw new IllegalArgumentException(
            label + ".inbound_finality_cutoff must be null unless the route is retired");
      }
      inboundFinalityCutoff = null;
    }
    final SourceRoles source =
        parseSourceIdentity(requiredObject(value, "source_identity"), lane, label);
    final DestinationRoles destination =
        parseDestination(requiredObject(value, "destination"), lane, revision, label);
    if (!source.family().equals(destination.family())
        || !source.address().equals(destination.routeAddress())
        || !source.runtimeHash().equals(destination.routeCodeHash())) {
      throw new IllegalArgumentException(
          label + " source emitter does not identify the destination route deployment");
    }
    if (!source.configurationHash().equals(destination.routeConfigurationHash())) {
      throw new IllegalArgumentException(
          label
              + " source emitter route_config_hash does not match the exact destination route configuration");
    }
    final Map<String, Object> settlement = requiredObject(value, "settlement");
    exactFields(
        settlement,
        Set.of("asset_definition_id", "custody_owner", "payload_amount_scale"),
        label + ".settlement");
    if (!TAIRA_XOR_ASSET_ID.equals(requiredText(settlement, "asset_definition_id"))) {
      throw new IllegalArgumentException(label + " settlement must use canonical Taira XOR");
    }
    requiredText(settlement, "custody_owner");
    requiredInt(settlement, "payload_amount_scale", 9, 9);
    final String lineage = routeId + '\0' + assetKey;
    return new ParsedRoute(
        lineage,
        lane.source().profileKey()
            + '\0'
            + lane.target().profileKey()
            + '\0'
            + lineage
            + '\0'
            + revision,
        revision,
        activation,
        inboundFinalityCutoff,
        destination.destinationBindingHash(),
        destination.routeConfigurationHash());
  }

  private static GovernanceRouteKey parseGovernanceRouteKey(
      final Map<String, Object> value, final String label) {
    exactFields(
        value, governanceFields("lane_id", "route_id", "asset_key", "revision"), label);
    return new GovernanceRouteKey(
        parseInboundLane(requiredObject(value, "lane_id"), label + ".lane_id"),
        canonicalRouteKey(value, "route_id"),
        canonicalRouteKey(value, "asset_key"),
        requiredLong(value, "revision", 1, 0xffff_ffffL));
  }

  private static String parseGovernanceActivation(
      final Map<String, Object> value, final String label) {
    exactFields(value, governanceFields("activation", "direction"), label);
    if (value.get("direction") != null) {
      throw new IllegalArgumentException(label + ".direction must be null");
    }
    final String activation = requiredText(value, "activation");
    if (!ACTIVATIONS.contains(activation)) {
      throw new IllegalArgumentException(label + ".activation is unsupported");
    }
    return activation;
  }

  private static void validateGovernanceCutoff(
      final Object value, final String activation, final String label) {
    if ("retired".equals(activation)) {
      final Map<String, Object> cutoff = objectValue(value, label);
      exactFields(
          cutoff,
          governanceFields("trust_anchor_hash", "max_anchor_interval_height"),
          label);
      upperBytes(cutoff, "trust_anchor_hash", 32);
      requiredUnsignedInteger(
          cutoff, "max_anchor_interval_height", MAX_JSON_SAFE_INTEGER, true);
    } else if (value != null) {
      throw new IllegalArgumentException(
          label + " must be null unless activation is retired");
    }
  }

  private static boolean canTransitionGovernanceActivation(
      final String current, final String next) {
    return switch (current) {
      case "staged" ->
          governanceFields("bidirectional", "inbound_only", "retired").contains(next);
      case "bidirectional" -> governanceFields("inbound_only", "paused").contains(next);
      case "inbound_only" -> governanceFields("paused", "retired").contains(next);
      case "paused" ->
          governanceFields("bidirectional", "inbound_only", "retired").contains(next);
      default -> false;
    };
  }

  private static Set<String> governanceFields(final String... names) {
    return new LinkedHashSet<>(Arrays.asList(names));
  }

  private static SourceRoles parseSourceIdentity(
      final Map<String, Object> value, final SccpLaneIdV1 lane, final String label) {
    exactFields(value, Set.of("lane", "emitter"), label + ".source_identity");
    if (!parseInboundLane(requiredObject(value, "lane"), label).equals(lane)) {
      throw new IllegalArgumentException(label + " source identity lane mismatch");
    }
    final Map<String, Object> emitter = requiredObject(value, "emitter");
    exactFields(emitter, Set.of("emitter", "identity"), label + ".emitter");
    final String family = requiredText(emitter, "emitter");
    if (!family.equals(familyFor(lane.source()))) {
      throw new IllegalArgumentException(label + " emitter family mismatch");
    }
    final Map<String, Object> identity = requiredObject(emitter, "identity");
    exactFields(
        identity,
        Set.of("address", "runtime_code_hash", "route_config_hash"),
        label + ".emitter.identity");
    final String address = upperBytes(identity, "address", 20);
    final String runtime = upperBytes(identity, "runtime_code_hash", 32);
    final String configuration = upperBytes(identity, "route_config_hash", 32);
    if (runtime.equals(configuration)) {
      throw new IllegalArgumentException(label + " emitter hash roles alias");
    }
    return new SourceRoles(family, address, runtime, configuration);
  }

  private static DestinationRoles parseDestination(
      final Map<String, Object> value,
      final SccpLaneIdV1 lane,
      final long routeRevision,
      final String label) {
    exactFields(value, Set.of("family", "deployment"), label + ".destination");
    final String family = requiredText(value, "family");
    if (!family.equals(familyFor(lane.source()))) {
      throw new IllegalArgumentException(label + " destination family mismatch");
    }
    final Map<String, Object> deployment = requiredObject(value, "deployment");
    exactFields(deployment, DESTINATION_FIELDS, label + ".deployment");
    final List<String> addresses =
        List.of(
            upperBytes(deployment, "token_address", 20),
            upperBytes(deployment, "verifier_address", 20),
            upperBytes(deployment, "route_address", 20));
    final List<String> hashes =
        List.of(
            upperBytes(deployment, "token_code_hash", 32),
            upperBytes(deployment, "verifier_code_hash", 32),
            upperBytes(deployment, "verifier_key_hash", 32),
            upperBytes(deployment, "route_code_hash", 32));
    if (new HashSet<>(addresses).size() != addresses.size()
        || new HashSet<>(hashes).size() != hashes.size()) {
      throw new IllegalArgumentException(
          label + " deployment reuses a role-separated address or hash");
    }
    validateVerifyingKey(
        requiredObject(deployment, "verifying_key"), hashes.get(2), label + ".verifying_key");
    final ParsedProofPolicy policyHashes =
        validateOutboundProofPolicy(
            requiredObject(deployment, "outbound_proof_policy"),
            label + ".outbound_proof_policy");
    final List<String> deploymentRoles = new ArrayList<>(hashes);
    deploymentRoles.add(policyHashes.profileHash());
    deploymentRoles.add(policyHashes.anchorHash());
    requireDistinctRawHashes(
        deploymentRoles, label + ".deployment proof-policy and deployment hashes");
    final long multiplier =
        requiredLong(
            deployment,
            "taira_to_token_multiplier",
            1_000_000_000L,
            1_000_000_000L);
    final DestinationHashes derived =
        deriveDestinationHashes(
            family,
            lane,
            addresses,
            hashes,
            policyHashes,
            routeRevision,
            multiplier);
    return new DestinationRoles(
        family,
        addresses.get(2),
        hashes.get(3),
        derived.destinationBindingHash(),
        derived.routeConfigurationHash());
  }

  private static DestinationHashes deriveDestinationHashes(
      final String family,
      final SccpLaneIdV1 lane,
      final List<String> addresses,
      final List<String> hashes,
      final ParsedProofPolicy policy,
      final long routeRevision,
      final long multiplier) {
    final boolean tron = "tron".equals(family);
    final ExternalNetworkParameters network = externalNetworkParameters(lane.source());
    final byte[] tokenAddress = hexBytes(addresses.get(0));
    final byte[] verifierAddress = hexBytes(addresses.get(1));
    final byte[] routeAddress = hexBytes(addresses.get(2));
    final byte[] tokenCodeHash = hexBytes(hashes.get(0));
    final byte[] verifierCodeHash = hexBytes(hashes.get(1));
    final byte[] verifierKeyHash = hexBytes(hashes.get(2));
    final byte[] semanticHash = hexBytes(policy.profileHash());
    final byte[] anchorHash = hexBytes(policy.anchorHash());

    final byte[] destinationBinding =
        keccak(
            concatenate(
                keccakText(
                    tron ? TRON_DESTINATION_BINDING_DOMAIN : EVM_DESTINATION_BINDING_DOMAIN),
                keccakText(tron ? TRON_GROTH16_BACKEND : EVM_GROTH16_BACKEND),
                abiWordUnsigned(network.chainOrNetworkId()),
                abiWordUnsigned(0),
                abiWordUnsigned(lane.source().domainId()),
                abiWordAddress(verifierAddress, tron),
                abiWordAddress(routeAddress, tron),
                verifierCodeHash,
                verifierKeyHash,
                semanticHash,
                anchorHash));

    final byte[] sourceLaneHash = SccpV1.laneHash(lane);
    final byte[] destinationLaneHash =
        SccpV1.laneHash(new SccpLaneIdV1(lane.target(), lane.source()));
    final List<String> routeHashRoles =
        new ArrayList<>(
            List.of(
                upperHex(sourceLaneHash),
                upperHex(destinationLaneHash),
                hashes.get(0),
                hashes.get(1),
                hashes.get(2),
                policy.profileHash(),
                policy.anchorHash()));
    if (tron) routeHashRoles.add(upperHex(destinationBinding));
    requireDistinctRawHashes(routeHashRoles, "SCCP route configuration");

    final List<byte[]> deploymentWords =
        new ArrayList<>(
            List.of(
                abiWordAddress(tokenAddress, false),
                tokenCodeHash,
                abiWordAddress(verifierAddress, false),
                verifierCodeHash,
                verifierKeyHash,
                semanticHash,
                anchorHash));
    if (tron) deploymentWords.add(destinationBinding);
    final byte[] deploymentConfigHash =
        keccak(concatenate(deploymentWords.toArray(new byte[0][])));
    final byte[] assetRouteConfigHash =
        keccak(
            concatenate(
                keccakText("xor"),
                keccakText(network.routeId()),
                abiWordUnsigned(routeRevision),
                abiWordUnsigned(multiplier)));
    final byte[] routeConfiguration =
        keccak(
            concatenate(
                keccakText(CONCRETE_ROUTE_CONFIG_DOMAIN),
                abiWordUnsigned(lane.source().domainId()),
                abiWordUnsigned(lane.source().tag()),
                abiWordUnsigned(network.chainOrNetworkId()),
                sourceLaneHash,
                destinationLaneHash,
                deploymentConfigHash,
                assetRouteConfigHash));
    return new DestinationHashes(
        upperHex(destinationBinding), upperHex(deploymentConfigHash), upperHex(routeConfiguration));
  }

  private static ExternalNetworkParameters externalNetworkParameters(
      final SccpNetworkV1 network) {
    return switch (network) {
      case ETHEREUM_MAINNET -> new ExternalNetworkParameters(1, "taira_eth_xor");
      case ETHEREUM_SEPOLIA ->
          new ExternalNetworkParameters(11_155_111L, "taira_eth_xor");
      case BSC_MAINNET -> new ExternalNetworkParameters(56, "taira_bsc_xor");
      case BSC_TESTNET -> new ExternalNetworkParameters(97, "taira_bsc_xor");
      case TRON_MAINNET -> new ExternalNetworkParameters(0x2b66_53dcL, "taira_tron_xor");
      case TRON_NILE -> new ExternalNetworkParameters(0xcd86_90dcL, "taira_tron_xor");
      case TRON_SHASTA -> new ExternalNetworkParameters(0x94a9_059eL, "taira_tron_xor");
      case SORA_TAIRA ->
          throw new IllegalArgumentException("SORA Taira is not an external SCCP network");
    };
  }

  private static void validateVerifyingKey(
      final Map<String, Object> value, final String expectedHash, final String label) {
    exactFields(value, VERIFYING_KEY_FIELDS, label);
    requiredInt(value, "version", 1, 1);
    final List<String> words = new ArrayList<>();
    words.addAll(parseG1(requiredObject(value, "alpha1"), label + ".alpha1"));
    for (final String field : List.of("beta2", "gamma2", "delta2")) {
      words.addAll(parseG2(requiredObject(value, field), label + '.' + field));
    }
    final Map<String, Object> ic = requiredObject(value, "ic");
    exactFields(ic, IC_FIELDS, label + ".ic");
    for (final String field : IC_FIELD_ORDER) {
      words.addAll(parseG1(requiredObject(ic, field), label + ".ic." + field));
    }
    if (words.size() != 38) {
      throw new IllegalArgumentException(label + " must contain exactly 38 ABI words");
    }
    final String actual = upperHex(keccak(hexBytes(String.join("", words))));
    if (!actual.equals(expectedHash)) {
      throw new IllegalArgumentException(label + " hash does not match verifier_key_hash");
    }
  }

  private static ParsedProofPolicy validateOutboundProofPolicyFields(
      final Map<String, Object> value, final String label) {
    final Map<String, Object> policy = new LinkedHashMap<>();
    policy.put("version", 1L);
    policy.put("semantic_profile", value.get("semantic_proof_profile"));
    policy.put("sora_finality_anchor", value.get("sora_finality_anchor"));
    return validateOutboundProofPolicy(policy, label);
  }

  private static ParsedProofPolicy validateOutboundProofPolicy(
      final Map<String, Object> value, final String label) {
    exactFields(
        value, Set.of("version", "semantic_profile", "sora_finality_anchor"), label);
    requiredInt(value, "version", 1, 1);
    final Map<String, Object> profile = requiredObject(value, "semantic_profile");
    exactFields(profile, Set.of("profile", "commitments"), label + ".semantic_profile");
    final String profileName = requiredText(profile, "profile");
    if (!SEMANTIC_PROFILE.equals(profileName)) {
      throw new IllegalArgumentException(label + " semantic profile is unsupported");
    }
    final Map<String, Object> commitments = requiredObject(profile, "commitments");
    exactFields(
        commitments,
        Set.of(
            "version",
            "circuit_commitment",
            "witness_generator_commitment",
            "public_signal_schema_hash"),
        label + ".commitments");
    final int commitmentVersion = requiredInt(commitments, "version", 1, 1);
    final List<String> semanticRoles =
        List.of(
            upperBytes(commitments, "circuit_commitment", 32),
            upperBytes(commitments, "witness_generator_commitment", 32),
            upperBytes(commitments, "public_signal_schema_hash", 32));
    if (!semanticRoles.get(2).equals(PUBLIC_SIGNAL_SCHEMA_HASH)) {
      throw new IllegalArgumentException(
          label + " public signal schema hash does not name the eleven-signal V1 schema");
    }
    requireDistinctRawHashes(semanticRoles, label + " semantic profile");
    final ByteArrayOutputStream canonicalProfile = new ByteArrayOutputStream();
    canonicalProfile.write(1);
    canonicalProfile.write(0);
    canonicalProfile.write(1);
    for (final String role : semanticRoles) {
      final byte[] bytes = hexBytes(role);
      canonicalProfile.write(bytes, 0, bytes.length);
    }
    final String profileHash =
        upperHex(
            prefixedKeccak(
                "sccp:semantic-proof-profile:v1", canonicalProfile.toByteArray()));
    final Map<String, Object> anchor = requiredObject(value, "sora_finality_anchor");
    exactFields(anchor, FINALITY_ANCHOR_FIELDS, label + ".sora_finality_anchor");
    final int anchorVersion = requiredInt(anchor, "version", 1, 1);
    final SccpNetworkV1 sourceNetwork =
        parseNetwork(requiredObject(anchor, "source_network"), label);
    if (sourceNetwork != SccpNetworkV1.SORA_TAIRA) {
      throw new IllegalArgumentException(label + " anchor must name SORA Taira");
    }
    final List<String> anchorRoles =
        List.of(
            upperBytes(anchor, "chain_id_hash", 32),
            upperBytes(anchor, "checkpoint_block_hash", 32),
            upperBytes(anchor, "checkpoint_context_id", 32),
            upperBytes(anchor, "checkpoint_finality_artifact_hash", 32));
    if (!TAIRA_CHAIN_ID_HASH.equals(anchorRoles.get(0))) {
      throw new IllegalArgumentException(label + " Taira chain id hash mismatch");
    }
    final int protocolVersion = requiredInt(anchor, "protocol_version", 4, 4);
    final BigInteger checkpointHeight =
        requiredUnsignedInteger(anchor, "checkpoint_height", MAX_U64, true);
    requireDistinctRawHashes(anchorRoles, label + " finality anchor");
    final ByteArrayOutputStream canonicalAnchor = new ByteArrayOutputStream();
    canonicalAnchor.write(1);
    canonicalAnchor.write(SccpNetworkV1.SORA_TAIRA.tag());
    writeU16(canonicalAnchor, protocolVersion);
    byte[] roleBytes = hexBytes(anchorRoles.get(0));
    canonicalAnchor.write(roleBytes, 0, roleBytes.length);
    writeU64(canonicalAnchor, checkpointHeight);
    roleBytes = hexBytes(anchorRoles.get(1));
    canonicalAnchor.write(roleBytes, 0, roleBytes.length);
    roleBytes = hexBytes(anchorRoles.get(2));
    canonicalAnchor.write(roleBytes, 0, roleBytes.length);
    roleBytes = hexBytes(anchorRoles.get(3));
    canonicalAnchor.write(roleBytes, 0, roleBytes.length);
    final String anchorHash =
        upperHex(
            prefixedKeccak("sccp:sora-finality-anchor:v1", canonicalAnchor.toByteArray()));
    final List<String> allRoles = new ArrayList<>(semanticRoles);
    allRoles.add(profileHash);
    allRoles.addAll(anchorRoles);
    allRoles.add(anchorHash);
    requireDistinctRawHashes(allRoles, label + " proof policy");
    return new ParsedProofPolicy(
        new SccpModels.SemanticProofProfileV1(
            profileName,
            new SccpModels.Groth16Bn254SemanticCircuitV1(
                commitmentVersion,
                semanticRoles.get(0),
                semanticRoles.get(1),
                semanticRoles.get(2)),
            "0x" + profileHash.toLowerCase(Locale.ROOT)),
        new SccpModels.SoraFinalityAnchorV1(
            anchorVersion,
            sourceNetwork,
            protocolVersion,
            anchorRoles.get(0),
            checkpointHeight,
            anchorRoles.get(1),
            anchorRoles.get(2),
            anchorRoles.get(3),
            "0x" + anchorHash.toLowerCase(Locale.ROOT)));
  }

  private static List<String> parseG1(
      final Map<String, Object> value, final String label) {
    exactFields(value, Set.of("x", "y"), label);
    final List<String> words =
        List.of(upperBytes(value, "x", 32, true), upperBytes(value, "y", 32, true));
    if (words.stream().allMatch(SccpJsonParser::isZeroHex)) {
      throw new IllegalArgumentException(label + " must not be the BN254 point at infinity");
    }
    for (int index = 0; index < words.size(); index++) {
      final String word = words.get(index);
      if (new BigInteger(word, 16).compareTo(BN254_MODULUS) >= 0) {
        throw new IllegalArgumentException(
            label + (index == 0 ? ".x" : ".y") + " is not a BN254 field element");
      }
    }
    return words;
  }

  private static List<String> parseG2(
      final Map<String, Object> value, final String label) {
    final List<String> fields = List.of("x_c0", "x_c1", "y_c0", "y_c1");
    exactFields(value, new LinkedHashSet<>(fields), label);
    final List<String> words = new ArrayList<>();
    for (final String field : fields) {
      final String word = upperBytes(value, field, 32, true);
      if (new BigInteger(word, 16).compareTo(BN254_MODULUS) >= 0) {
        throw new IllegalArgumentException(label + '.' + field + " is not a BN254 field element");
      }
      words.add(word);
    }
    if (words.stream().allMatch(SccpJsonParser::isZeroHex)) {
      throw new IllegalArgumentException(label + " must not be the BN254 point at infinity");
    }
    return words;
  }

  private static NativeTrustAnchor parseNativeTrustAnchor(
      final Map<String, Object> anchor, final SccpLaneIdV1 lane, final String label) {
    exactFields(
        anchor,
        Set.of("backend", "anchor_hash", "checkpoint_height"),
        label);
    final Map<String, Object> backend = requiredObject(anchor, "backend");
    exactFields(backend, Set.of("backend", "protocol"), label + ".backend");
    if (backend.get("protocol") != null) {
      throw new IllegalArgumentException(label + ".backend.protocol must be null");
    }
    final String expected =
        switch (lane.source().domainId()) {
          case 1 -> "ethereum_beacon_v1";
          case 2 -> "bsc_parlia_v1";
          case 5 -> "tron_dpos_v1";
          default -> throw new IllegalArgumentException("unsupported SCCP native lane");
        };
    if (!expected.equals(requiredText(backend, "backend"))) {
      throw new IllegalArgumentException(label + " backend does not match lane source");
    }
    return new NativeTrustAnchor(
        expected,
        upperBytes(anchor, "anchor_hash", 32),
        requiredUnsignedInteger(anchor, "checkpoint_height", MAX_U64, true));
  }

  private static void validateTransferPayload(
      final Map<String, Object> value, final SccpLaneIdV1 lane) {
    exactFields(value, Set.of("Transfer"), "SCCP payload");
    final Map<String, Object> transfer = requiredObject(value, "Transfer");
    exactFields(transfer, TRANSFER_FIELDS, "SCCP transfer");
    requiredInt(transfer, "version", 1, 1);
    if (requiredDomain(transfer, "source_domain") != lane.source().domainId()
        || requiredDomain(transfer, "dest_domain") != lane.target().domainId()) {
      throw new IllegalArgumentException("SCCP transfer domains do not match its lane");
    }
    if (new BigInteger(requiredDecimal(transfer, "nonce", false)).bitLength() > 64) {
      throw new IllegalArgumentException("SCCP transfer nonce must fit u64");
    }
    requiredLong(transfer, "route_revision", 1, 0xffff_ffffL);
    requiredDomain(transfer, "asset_home_domain");
    validateCodec(transfer, "asset_id_codec", "asset_id", null);
    if (new BigInteger(requiredDecimal(transfer, "amount", true)).bitLength() > 128) {
      throw new IllegalArgumentException("SCCP transfer amount must fit u128");
    }
    validateCodec(transfer, "sender_codec", "sender", lane.source().domainId());
    validateCodec(transfer, "recipient_codec", "recipient", lane.target().domainId());
    validateCodec(transfer, "route_id_codec", "route_id", null);
  }

  private static void validateCodec(
      final Map<String, Object> value,
      final String codecField,
      final String bytesField,
      final Integer domain) {
    final int codec = requiredInt(value, codecField, 1, 5);
    if (codec != 1 && codec != 2 && codec != 5) {
      throw new IllegalArgumentException(codecField + " is unsupported or retired");
    }
    if (domain != null) {
      final int expected =
          switch (domain) {
            case 0 -> 1;
            case 1, 2 -> 2;
            case 5 -> 5;
            default -> throw new IllegalArgumentException("unsupported SCCP domain");
          };
      if (codec != expected) {
        throw new IllegalArgumentException(codecField + " does not match its domain");
      }
    }
    final byte[] bytes = hexBytes(value, bytesField);
    boolean valid = false;
    if (codec == 1) {
      valid = bytes.length > 0 && bytes.length <= 256;
      for (final byte item : bytes) valid &= (item & 0xff) >= 0x21 && (item & 0xff) <= 0x7e;
    } else if (codec == 2) {
      valid = bytes.length == 20 && !allZero(bytes);
    } else if (codec == 5) {
      valid =
          bytes.length == 21
              && (bytes[0] & 0xff) == 0x41
              && !allZero(Arrays.copyOfRange(bytes, 1, bytes.length));
    }
    if (!valid) throw new IllegalArgumentException(bytesField + " does not match its codec");
  }

  private static SccpModels.RecentMessage parseRecent(
      final Map<String, Object> value, final int index) {
    final String label = "SCCP recent messages.items[" + index + "]";
    exactFields(value, RECENT_FIELDS, RECENT_REQUIRED, label);
    if (!"transfer".equals(requiredText(value, "kind"))) {
      throw new IllegalArgumentException(label + " kind is retired");
    }
    final SccpNetworkV1 source = exactProfile(value, "source_profile");
    final SccpNetworkV1 target = exactProfile(value, "target_profile");
    final SccpLaneIdV1 lane = new SccpLaneIdV1(source, target);
    if (!lane.isOutbound() || source != SccpNetworkV1.SORA_TAIRA) {
      throw new IllegalArgumentException(label + " must use a Taira-to-external lane");
    }
    if (requiredInt(value, "target_domain", 1, 5) != target.domainId()) {
      throw new IllegalArgumentException(label + " target profile/domain mismatch");
    }
    final String messageId = lowerHash(value, "message_id_hex");
    final String binding = prefixedHash(value, "destination_binding_hash");
    final String configuration = prefixedHash(value, "route_configuration_hash");
    requireDistinctHashes(List.of(messageId, binding, configuration), label);
    final String amount = requiredDecimal(value, "amount", true);
    if (new BigInteger(amount).bitLength() > 128) {
      throw new IllegalArgumentException(label + " amount must fit u128");
    }
    final Map<String, Object> links = requiredObject(value, "links");
    exactFields(links, Set.of("bundle_path", "proof_request_path"), label + ".links");
    final String bundlePath = requiredText(links, "bundle_path");
    final String requestPath = requiredText(links, "proof_request_path");
    if (!bundlePath.equals("/v1/sccp/proofs/message/" + messageId)
        || !requestPath.equals("/v1/sccp/proof-requests/" + messageId)) {
      throw new IllegalArgumentException(label + " readback link mismatch");
    }
    final String assetId = optionalText(value, "asset_id");
    final String routeId = optionalText(value, "route_id");
    final String recipient = optionalText(value, "recipient");
    final Map<String, Object> projection =
        validateRecentPayloadProjection(
            requiredObject(value, "payload_projection"),
            lane,
            amount,
            assetId,
            routeId,
            label + ".payload_projection");
    return new SccpModels.RecentMessage(
        requiredUnsignedInteger(value, "height", MAX_U64, true),
        requiredInt(
            value,
            "commitment_index",
            0,
            SccpModels.SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 - 1),
        messageId,
        source.profileKey(),
        target.profileKey(),
        binding,
        configuration,
        target.domainId(),
        assetId,
        routeId,
        recipient,
        amount,
        projection,
        new SccpModels.RecentMessageLinks(bundlePath, requestPath));
  }

  private static Map<String, Object> validateRecentPayloadProjection(
      final Map<String, Object> value,
      final SccpLaneIdV1 lane,
      final String summaryAmount,
      final String summaryAssetId,
      final String summaryRouteId,
      final String label) {
    exactFields(value, Set.of("Transfer"), label);
    final Map<String, Object> transfer = requiredObject(value, "Transfer");
    exactFields(transfer, PROJECTION_TRANSFER_FIELDS, label + ".Transfer");
    requiredInt(transfer, "version", 1, 1);
    requiredInt(transfer, "source_domain", 0, 0);
    if (requiredInt(transfer, "dest_domain", 1, 5) != lane.target().domainId()) {
      throw new IllegalArgumentException(
          label + ".Transfer.dest_domain does not match the target network");
    }
    requiredUnsignedInteger(transfer, "nonce", MAX_U64, false);
    requiredLong(transfer, "route_revision", 1, 0xffff_ffffL);
    requiredInt(transfer, "asset_home_domain", 0, 0);
    final String assetId =
        projectionCanonicalText(
            requiredObject(transfer, "asset_id"), label + ".Transfer.asset_id", "xor");
    if (summaryAssetId != null && !summaryAssetId.equals(assetId)) {
      throw new IllegalArgumentException(
          label + ".Transfer.asset_id does not match the recent-message summary");
    }
    final BigInteger amount =
        requiredUnsignedInteger(transfer, "amount", MAX_U128, true);
    if (!amount.toString().equals(summaryAmount)) {
      throw new IllegalArgumentException(
          label + ".Transfer.amount does not match the recent-message summary");
    }
    projectionCanonicalText(
        requiredObject(transfer, "sender"), label + ".Transfer.sender", null);
    validateProjectionRecipient(
        requiredObject(transfer, "recipient"), lane.target(), label + ".Transfer.recipient");
    final String expectedRouteId =
        switch (lane.target().domainId()) {
          case 1 -> "taira_eth_xor";
          case 2 -> "taira_bsc_xor";
          case 5 -> "taira_tron_xor";
          default -> throw new IllegalStateException("closed SCCP destination");
        };
    final String routeId =
        projectionCanonicalText(
            requiredObject(transfer, "route_id"),
            label + ".Transfer.route_id",
            expectedRouteId);
    if (summaryRouteId != null && !summaryRouteId.equals(routeId)) {
      throw new IllegalArgumentException(
          label + ".Transfer.route_id does not match the recent-message summary");
    }
    return deepCopyObject(value);
  }

  private static String projectionCanonicalText(
      final Map<String, Object> value, final String label, final String expected) {
    exactFields(value, Set.of("CanonicalText"), label);
    final Map<String, Object> inner = requiredObject(value, "CanonicalText");
    exactFields(inner, Set.of("value"), label + ".CanonicalText");
    final String text = requiredText(inner, "value");
    if (text.length() > 512 || (expected != null && !expected.equals(text))) {
      throw new IllegalArgumentException(label + " names an unsupported value");
    }
    return text;
  }

  private static void validateProjectionRecipient(
      final Map<String, Object> value,
      final SccpNetworkV1 target,
      final String label) {
    final String variant = target.domainId() == 5 ? "TronAddress21" : "EvmAddress20";
    exactFields(value, Set.of(variant), label);
    final Map<String, Object> inner = requiredObject(value, variant);
    exactFields(inner, Set.of("bytes"), label + '.' + variant);
    final String bytes = requiredText(inner, "bytes");
    final int expectedLength = "TronAddress21".equals(variant) ? 42 : 40;
    if (!bytes.matches("0x[0-9a-f]{" + expectedLength + "}")
        || isZeroHex(bytes.substring("TronAddress21".equals(variant) ? 4 : 2))
        || ("TronAddress21".equals(variant) && !bytes.startsWith("0x41"))) {
      throw new IllegalArgumentException(
          label + " does not contain a canonical nonzero " + variant);
    }
  }

  private static SccpLaneIdV1 parseInboundLane(
      final Map<String, Object> value, final String label) {
    final SccpLaneIdV1 lane = parseLane(value, label);
    if (!lane.isInbound() || lane.target() != SccpNetworkV1.SORA_TAIRA) {
      throw new IllegalArgumentException(
          label + " must be an exact supported external-to-Taira lane");
    }
    return lane;
  }

  private static SccpLaneIdV1 parseLane(
      final Map<String, Object> value, final String label) {
    exactFields(value, Set.of("source", "target"), label);
    return new SccpLaneIdV1(
        parseNetwork(requiredObject(value, "source"), label + ".source"),
        parseNetwork(requiredObject(value, "target"), label + ".target"));
  }

  private static SccpNetworkV1 parseNetwork(
      final Map<String, Object> value, final String label) {
    exactFields(value, Set.of("network", "profile"), label);
    if (value.get("profile") != null) {
      throw new IllegalArgumentException(label + ".profile must be null");
    }
    final SccpNetworkV1 network =
        SccpNetworkV1.fromProfileKey(requiredText(value, "network").replace('_', '-'));
    if (network == null) throw new IllegalArgumentException(label + " is unsupported or retired");
    return network;
  }

  private static SccpNetworkV1 exactProfile(
      final Map<String, Object> value, final String field) {
    final SccpNetworkV1 network = SccpNetworkV1.fromProfileKey(requiredText(value, field));
    if (network == null) throw new IllegalArgumentException(field + " is unsupported or retired");
    return network;
  }

  private static String familyFor(final SccpNetworkV1 network) {
    return network.domainId() == 5 ? "tron" : "evm";
  }

  private static String canonicalRouteKey(
      final Map<String, Object> value, final String field) {
    final String key = requiredText(value, field);
    if (!ROUTE_KEY.matcher(key).matches()) {
      throw new IllegalArgumentException(field + " must be canonical lowercase route text");
    }
    return key;
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> rootObject(final byte[] bytes, final String label) {
    final String text = new String(bytes, StandardCharsets.UTF_8);
    if (!Arrays.equals(bytes, text.getBytes(StandardCharsets.UTF_8))) {
      throw new IllegalArgumentException(label + " must be UTF-8 JSON");
    }
    return objectValue(JsonParser.parse(text), label);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> objectValue(final Object value, final String label) {
    if (!(value instanceof Map<?, ?>)) {
      throw new IllegalArgumentException(label + " must be an object");
    }
    for (final Object key : ((Map<?, ?>) value).keySet()) {
      if (!(key instanceof String)) {
        throw new IllegalArgumentException(label + " must contain string fields");
      }
    }
    return (Map<String, Object>) value;
  }

  private static Map<String, Object> requiredObject(
      final Map<String, Object> value, final String field) {
    return objectValue(value.get(field), field);
  }

  private static Map<String, Object> optionalObject(
      final Map<String, Object> value, final String field) {
    return value.get(field) == null ? null : objectValue(value.get(field), field);
  }

  @SuppressWarnings("unchecked")
  private static List<Object> requiredList(
      final Map<String, Object> value, final String field) {
    if (!(value.get(field) instanceof List<?>)) {
      throw new IllegalArgumentException(field + " must be an array");
    }
    return (List<Object>) value.get(field);
  }

  private static void exactFields(
      final Map<String, Object> value, final Set<String> allowed, final String label) {
    exactFields(value, allowed, allowed, label);
  }

  private static void exactFields(
      final Map<String, Object> value,
      final Set<String> allowed,
      final Set<String> required,
      final String label) {
    for (final String field : value.keySet()) {
      if (!allowed.contains(field)) {
        throw new IllegalArgumentException(label + " contains unknown or retired field " + field);
      }
    }
    for (final String field : required) {
      if (!value.containsKey(field)) {
        throw new IllegalArgumentException(label + " is missing required field " + field);
      }
    }
  }

  private static String requiredText(final Map<String, Object> value, final String field) {
    if (!(value.get(field) instanceof String)) {
      throw new IllegalArgumentException(field + " must be a string");
    }
    final String text = (String) value.get(field);
    if (text.isBlank() || !text.equals(text.trim())) {
      throw new IllegalArgumentException(field + " must be canonical text");
    }
    return text;
  }

  private static String optionalText(final Map<String, Object> value, final String field) {
    return value.get(field) == null ? null : requiredText(value, field);
  }

  private static boolean requiredBoolean(
      final Map<String, Object> value, final String field) {
    if (!(value.get(field) instanceof Boolean)) {
      throw new IllegalArgumentException(field + " must be boolean");
    }
    return (Boolean) value.get(field);
  }

  private static long requiredLong(
      final Map<String, Object> value,
      final String field,
      final long minimum,
      final long maximum) {
    if (!(value.get(field) instanceof Number)) {
      throw new IllegalArgumentException(field + " must be an integer");
    }
    if (value.get(field) instanceof java.math.BigDecimal) {
      throw new IllegalArgumentException(field + " must be a canonical unsigned integer");
    }
    final Number number = (Number) value.get(field);
    final long result;
    try {
      result = JsonNumbers.asLong(number, field);
    } catch (final IllegalStateException error) {
      throw new IllegalArgumentException(field + " must be an integer", error);
    }
    if (result < minimum || result > maximum) {
      throw new IllegalArgumentException(field + " is out of range");
    }
    return result;
  }

  private static int requiredInt(
      final Map<String, Object> value,
      final String field,
      final int minimum,
      final int maximum) {
    return (int) requiredLong(value, field, minimum, maximum);
  }

  private static BigInteger requiredUnsignedInteger(
      final Map<String, Object> value,
      final String field,
      final BigInteger maximum,
      final boolean positive) {
    if (!(value.get(field) instanceof Number)) {
      throw new IllegalArgumentException(field + " must be an integer");
    }
    if (value.get(field) instanceof java.math.BigDecimal) {
      throw new IllegalArgumentException(field + " must be a canonical unsigned integer");
    }
    final String text = value.get(field).toString();
    if (!text.matches(positive ? "[1-9][0-9]*" : "0|[1-9][0-9]*")) {
      throw new IllegalArgumentException(field + " must be a canonical unsigned integer");
    }
    final BigInteger result = new BigInteger(text);
    if (result.compareTo(maximum) > 0) {
      throw new IllegalArgumentException(field + " is out of range");
    }
    return result;
  }

  private static int requiredDomain(final Map<String, Object> value, final String field) {
    final int domain = requiredInt(value, field, 0, 5);
    if (domain != 0 && domain != 1 && domain != 2 && domain != 5) {
      throw new IllegalArgumentException(field + " is an unsupported or retired SCCP domain");
    }
    return domain;
  }

  private static String requiredDecimal(
      final Map<String, Object> value, final String field, final boolean positive) {
    if (!(value.get(field) instanceof String)) {
      throw new IllegalArgumentException(field + " must be a decimal string");
    }
    final String text = (String) value.get(field);
    if (!(positive ? POSITIVE_DECIMAL : UNSIGNED_DECIMAL).matcher(text).matches()) {
      throw new IllegalArgumentException(field + " must be a canonical unsigned decimal string");
    }
    return text;
  }

  private static String lowerHash(final Map<String, Object> value, final String field) {
    final String hash = requiredText(value, field);
    if (!LOWER_HASH.matcher(hash).matches() || hash.chars().allMatch(item -> item == '0')) {
      throw new IllegalArgumentException(field + " must be canonical lowercase nonzero hash");
    }
    return hash;
  }

  private static String prefixedHash(final Map<String, Object> value, final String field) {
    final String hash = requiredText(value, field);
    if (!PREFIXED_HASH.matcher(hash).matches()
        || hash.substring(2).chars().allMatch(item -> item == '0')) {
      throw new IllegalArgumentException(
          field + " must be canonical lowercase nonzero 0x-prefixed hash");
    }
    return hash;
  }

  private static String upperBytes(
      final Map<String, Object> value, final String field, final int bytes) {
    return upperBytes(value, field, bytes, false);
  }

  private static String upperBytes(
      final Map<String, Object> value,
      final String field,
      final int bytes,
      final boolean allowZero) {
    final String text = requiredText(value, field);
    if (!text.matches("[0-9A-F]{" + (bytes * 2) + "}")
        || (!allowZero && isZeroHex(text))) {
      throw new IllegalArgumentException(
          field + " must be canonical uppercase " + (allowZero ? "" : "nonzero ") + "hex");
    }
    return text;
  }

  private static boolean isZeroHex(final String value) {
    return value.chars().allMatch(item -> item == '0');
  }

  private static String requiredHexBytes(
      final Map<String, Object> value, final String field, final boolean allowEmpty) {
    final String text = requiredText(value, field);
    if (!text.startsWith("0x") || (text.length() & 1) != 0) {
      throw new IllegalArgumentException(field + " must be canonical lowercase 0x-prefixed bytes");
    }
    for (int index = 2; index < text.length(); index++) {
      final char item = text.charAt(index);
      if (!((item >= '0' && item <= '9') || (item >= 'a' && item <= 'f'))) {
        throw new IllegalArgumentException(
            field + " must be canonical lowercase 0x-prefixed bytes");
      }
    }
    if (!allowEmpty && text.length() == 2) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    if ((text.length() - 2) / 2 > 16 * 1024 * 1024) {
      throw new IllegalArgumentException(field + " exceeds its size bound");
    }
    return text;
  }

  private static byte[] hexBytes(final Map<String, Object> value, final String field) {
    return hexBytes(requiredHexBytes(value, field, false).substring(2));
  }

  private static byte[] hexBytes(final String value) {
    final byte[] result = new byte[value.length() / 2];
    for (int index = 0; index < result.length; index++) {
      result[index] = (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return result;
  }

  private static String exactPath(
      final Map<String, Object> value,
      final String field,
      final String expected,
      final boolean optional) {
    if (value.get(field) == null) {
      if (!optional) throw new IllegalArgumentException(field + " is required");
      return null;
    }
    final String actual = requiredText(value, field);
    if (!expected.equals(actual)) {
      throw new IllegalArgumentException(field + " must equal " + expected);
    }
    return actual;
  }

  private static void requireDistinctHashes(
      final List<String> values, final String label) {
    final Set<String> normalized = new HashSet<>();
    for (final String value : values) {
      if (!normalized.add(value.startsWith("0x") ? value.substring(2) : value)) {
        throw new IllegalArgumentException(label + " hash roles must be distinct");
      }
    }
  }

  private static void requireDistinctRawHashes(
      final List<String> values, final String label) {
    if (values.stream().anyMatch(SccpJsonParser::isZeroHex)
        || new HashSet<>(values).size() != values.size()) {
      throw new IllegalArgumentException(label + " hash roles must be nonzero and distinct");
    }
  }

  private static Map<String, Object> deepCopyObject(final Map<String, Object> value) {
    final Map<String, Object> result = new LinkedHashMap<>();
    for (final Map.Entry<String, Object> entry : value.entrySet()) {
      result.put(entry.getKey(), deepCopy(entry.getValue()));
    }
    return result;
  }

  private static Object deepCopy(final Object value) {
    if (value instanceof Map<?, ?>) return deepCopyObject(objectValue(value, "nested object"));
    if (value instanceof List<?>) {
      final List<Object> result = new ArrayList<>();
      for (final Object item : (List<?>) value) result.add(deepCopy(item));
      return result;
    }
    return value;
  }

  private static byte[] keccak(final byte[] bytes) {
    final KeccakDigest digest = new KeccakDigest(256);
    digest.update(bytes, 0, bytes.length);
    final byte[] result = new byte[32];
    digest.doFinal(result, 0);
    return result;
  }

  private static byte[] keccakText(final String value) {
    return keccak(value.getBytes(StandardCharsets.UTF_8));
  }

  private static byte[] concatenate(final byte[]... values) {
    int length = 0;
    for (final byte[] value : values) length = Math.addExact(length, value.length);
    final byte[] result = new byte[length];
    int offset = 0;
    for (final byte[] value : values) {
      System.arraycopy(value, 0, result, offset, value.length);
      offset += value.length;
    }
    return result;
  }

  private static byte[] abiWordUnsigned(final long value) {
    if (value < 0) throw new IllegalArgumentException("SCCP ABI integer must be unsigned");
    final byte[] word = new byte[32];
    for (int index = 0; index < 8; index++) {
      word[word.length - 1 - index] = (byte) ((value >>> (index * 8)) & 0xff);
    }
    return word;
  }

  private static byte[] abiWordAddress(final byte[] address, final boolean tron) {
    if (address.length != 20 || allZero(address)) {
      throw new IllegalArgumentException("SCCP ABI address must contain 20 nonzero bytes");
    }
    final byte[] word = new byte[32];
    if (tron) word[11] = 0x41;
    System.arraycopy(address, 0, word, 12, address.length);
    return word;
  }

  private static byte[] prefixedKeccak(final String prefix, final byte[] body) {
    final byte[] prefixBytes = prefix.getBytes(StandardCharsets.UTF_8);
    final byte[] preimage = Arrays.copyOf(prefixBytes, prefixBytes.length + body.length);
    System.arraycopy(body, 0, preimage, prefixBytes.length, body.length);
    return keccak(preimage);
  }

  private static String upperHex(final byte[] bytes) {
    final StringBuilder result = new StringBuilder(bytes.length * 2);
    for (final byte item : bytes) result.append(String.format("%02X", item & 0xff));
    return result.toString();
  }

  private static String publicSignalSchemaHash() {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32(out, PUBLIC_SIGNAL_LABELS.size());
    for (final String label : PUBLIC_SIGNAL_LABELS) {
      final byte[] bytes = label.getBytes(StandardCharsets.UTF_8);
      writeU32(out, bytes.length);
      out.write(bytes, 0, bytes.length);
    }
    final byte[] prefix =
        "sccp:groth16-bn254:public-signal-schema:v1".getBytes(StandardCharsets.UTF_8);
    final byte[] body = out.toByteArray();
    final byte[] preimage = Arrays.copyOf(prefix, prefix.length + body.length);
    System.arraycopy(body, 0, preimage, prefix.length, body.length);
    return upperHex(keccak(preimage));
  }

  private static void writeU32(final ByteArrayOutputStream out, final int value) {
    for (int shift = 0; shift < 4; shift++) out.write((value >>> (shift * 8)) & 0xff);
  }

  private static void writeU64(final ByteArrayOutputStream out, final BigInteger value) {
    for (int shift = 0; shift < 8; shift++) {
      out.write(value.shiftRight(shift * 8).and(BigInteger.valueOf(0xff)).intValue());
    }
  }

  private static void writeU16(final ByteArrayOutputStream out, final int value) {
    for (int shift = 0; shift < 2; shift++) out.write((value >>> (shift * 8)) & 0xff);
  }

  private static boolean allZero(final byte[] bytes) {
    for (final byte item : bytes) if (item != 0) return false;
    return true;
  }

  private record ParsedRoute(
      String lineage,
      String key,
      long revision,
      String activation,
      SccpModels.InboundFinalityCutoffV1 inboundFinalityCutoff,
      String destinationBindingHash,
      String routeConfigurationHash) {}

  private record GovernanceRouteKey(
      SccpLaneIdV1 lane, String routeId, String assetKey, long revision) {}

  private record SourceRoles(
      String family, String address, String runtimeHash, String configurationHash) {}

  private record DestinationRoles(
      String family,
      String routeAddress,
      String routeCodeHash,
      String destinationBindingHash,
      String routeConfigurationHash) {}

  private record DestinationHashes(
      String destinationBindingHash,
      String deploymentConfigHash,
      String routeConfigurationHash) {}

  private record ExternalNetworkParameters(long chainOrNetworkId, String routeId) {}

  private record NativeTrustAnchor(
      String backend, String anchorHash, BigInteger checkpointHeight) {}

  private record ParsedProofPolicy(
      SccpModels.SemanticProofProfileV1 semanticProfile,
      SccpModels.SoraFinalityAnchorV1 soraFinalityAnchor) {
    String profileHash() {
      return semanticProfile.profileHash.substring(2).toUpperCase(Locale.ROOT);
    }

    String anchorHash() {
      return soraFinalityAnchor.anchorHash.substring(2).toUpperCase(Locale.ROOT);
    }
  }
}
