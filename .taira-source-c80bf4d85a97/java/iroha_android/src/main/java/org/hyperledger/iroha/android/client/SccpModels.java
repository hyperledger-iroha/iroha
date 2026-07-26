package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.sccp.SccpNetworkV1;

/** Immutable DTOs for the closed exact first-release SCCP API. */
public final class SccpModels {
  /** Fixed maximum number of successful outbound SCCP messages in one V1 block. */
  public static final int SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 = 512;

  /** Fixed maximum retained canonical payload size for one V1 outbound SCCP message. */
  public static final int SCCP_OUTBOUND_MESSAGE_MAX_PAYLOAD_BYTES_V1 = 4_096;

  private static final BigInteger U64_MAX =
      BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);

  private SccpModels() {}

  /** The sole payload admitted by SCCP V1. */
  public enum PayloadKindV1 {
    TRANSFER("transfer");

    public final String wireKey;

    PayloadKindV1(final String wireKey) {
      this.wireKey = wireKey;
    }

    static PayloadKindV1 fromWireKey(final String value) {
      return TRANSFER.wireKey.equals(value) ? TRANSFER : null;
    }
  }

  /** Fixed SCCP V1 route-registry capacity limits. */
  public static final class RegistryLimits {
    public final long maxGovernedLanes;
    public final long maxLiveGovernedRoutes;
    public final long maxLiveRoutesPerLane;
    public final long maxRetainedRoutesPerLane;
    public final long maxRetainedNativeTrustAnchorsPerLane;

    RegistryLimits(
        final long maxGovernedLanes,
        final long maxLiveGovernedRoutes,
        final long maxLiveRoutesPerLane,
        final long maxRetainedRoutesPerLane,
        final long maxRetainedNativeTrustAnchorsPerLane) {
      this.maxGovernedLanes = maxGovernedLanes;
      this.maxLiveGovernedRoutes = maxLiveGovernedRoutes;
      this.maxLiveRoutesPerLane = maxLiveRoutesPerLane;
      this.maxRetainedRoutesPerLane = maxRetainedRoutesPerLane;
      this.maxRetainedNativeTrustAnchorsPerLane = maxRetainedNativeTrustAnchorsPerLane;
    }
  }

  /** Consensus-critical SCCP proof and deterministic verifier-work limits. */
  public static final class ResourceLimits {
    public final long maxOutboundMessagesPerBlock;
    public final BigInteger maxOutboundMessagePayloadBytes;
    public final BigInteger maxPendingOutboundMessages;
    public final BigInteger maxPendingOutboundPayloadBytes;
    public final long maxProofsPerTransaction;
    public final long maxProofsPerBlock;
    public final BigInteger maxProofBytesPerProof;
    public final BigInteger maxProofBytesPerTransaction;
    public final BigInteger maxProofBytesPerBlock;
    public final long maxNativeHeadersPerTransaction;
    public final long maxNativeHeadersPerBlock;
    public final long maxEthereumLightClientUpdatesPerTransaction;
    public final long maxEthereumLightClientUpdatesPerBlock;
    public final BigInteger maxNativeHeaderBytesPerTransaction;
    public final BigInteger maxNativeHeaderBytesPerBlock;
    public final long maxSecp256k1RecoveriesPerTransaction;
    public final long maxSecp256k1RecoveriesPerBlock;
    public final long maxBlsAggregateChecksPerTransaction;
    public final long maxBlsAggregateChecksPerBlock;
    public final long maxBlsSignerContributionsPerTransaction;
    public final long maxBlsSignerContributionsPerBlock;
    public final long maxBn254PairingChecksPerTransaction;
    public final long maxBn254PairingChecksPerBlock;

    ResourceLimits(
        final long maxOutboundMessagesPerBlock,
        final BigInteger maxOutboundMessagePayloadBytes,
        final BigInteger maxPendingOutboundMessages,
        final BigInteger maxPendingOutboundPayloadBytes,
        final long maxProofsPerTransaction,
        final long maxProofsPerBlock,
        final BigInteger maxProofBytesPerProof,
        final BigInteger maxProofBytesPerTransaction,
        final BigInteger maxProofBytesPerBlock,
        final long maxNativeHeadersPerTransaction,
        final long maxNativeHeadersPerBlock,
        final long maxEthereumLightClientUpdatesPerTransaction,
        final long maxEthereumLightClientUpdatesPerBlock,
        final BigInteger maxNativeHeaderBytesPerTransaction,
        final BigInteger maxNativeHeaderBytesPerBlock,
        final long maxSecp256k1RecoveriesPerTransaction,
        final long maxSecp256k1RecoveriesPerBlock,
        final long maxBlsAggregateChecksPerTransaction,
        final long maxBlsAggregateChecksPerBlock,
        final long maxBlsSignerContributionsPerTransaction,
        final long maxBlsSignerContributionsPerBlock,
        final long maxBn254PairingChecksPerTransaction,
        final long maxBn254PairingChecksPerBlock) {
      this.maxOutboundMessagesPerBlock = maxOutboundMessagesPerBlock;
      this.maxOutboundMessagePayloadBytes = maxOutboundMessagePayloadBytes;
      this.maxPendingOutboundMessages = maxPendingOutboundMessages;
      this.maxPendingOutboundPayloadBytes = maxPendingOutboundPayloadBytes;
      this.maxProofsPerTransaction = maxProofsPerTransaction;
      this.maxProofsPerBlock = maxProofsPerBlock;
      this.maxProofBytesPerProof = maxProofBytesPerProof;
      this.maxProofBytesPerTransaction = maxProofBytesPerTransaction;
      this.maxProofBytesPerBlock = maxProofBytesPerBlock;
      this.maxNativeHeadersPerTransaction = maxNativeHeadersPerTransaction;
      this.maxNativeHeadersPerBlock = maxNativeHeadersPerBlock;
      this.maxEthereumLightClientUpdatesPerTransaction =
          maxEthereumLightClientUpdatesPerTransaction;
      this.maxEthereumLightClientUpdatesPerBlock = maxEthereumLightClientUpdatesPerBlock;
      this.maxNativeHeaderBytesPerTransaction = maxNativeHeaderBytesPerTransaction;
      this.maxNativeHeaderBytesPerBlock = maxNativeHeaderBytesPerBlock;
      this.maxSecp256k1RecoveriesPerTransaction = maxSecp256k1RecoveriesPerTransaction;
      this.maxSecp256k1RecoveriesPerBlock = maxSecp256k1RecoveriesPerBlock;
      this.maxBlsAggregateChecksPerTransaction = maxBlsAggregateChecksPerTransaction;
      this.maxBlsAggregateChecksPerBlock = maxBlsAggregateChecksPerBlock;
      this.maxBlsSignerContributionsPerTransaction = maxBlsSignerContributionsPerTransaction;
      this.maxBlsSignerContributionsPerBlock = maxBlsSignerContributionsPerBlock;
      this.maxBn254PairingChecksPerTransaction = maxBn254PairingChecksPerTransaction;
      this.maxBn254PairingChecksPerBlock = maxBn254PairingChecksPerBlock;
    }
  }

  /** Exact paths and registry revision advertised by Torii. */
  public static final class Capabilities {
    public final int version;
    public final String registryRevision;
    public final String registryPath;
    public final String messageBundlePath;
    public final String proofRequestPath;
    public final String recentMessagesPath;
    public final RegistryLimits registryLimits;
    public final ResourceLimits resourceLimits;
    public final String proofSubmitPath;
    public final String nativeMessageSubmitPath;

    Capabilities(
        final int version,
        final String registryRevision,
        final String registryPath,
        final String messageBundlePath,
        final String proofRequestPath,
        final String recentMessagesPath,
        final RegistryLimits registryLimits,
        final ResourceLimits resourceLimits,
        final String proofSubmitPath,
        final String nativeMessageSubmitPath) {
      this.version = version;
      this.registryRevision = registryRevision;
      this.registryPath = registryPath;
      this.messageBundlePath = messageBundlePath;
      this.proofRequestPath = proofRequestPath;
      this.recentMessagesPath = recentMessagesPath;
      this.registryLimits = registryLimits;
      this.resourceLimits = resourceLimits;
      this.proofSubmitPath = proofSubmitPath;
      this.nativeMessageSubmitPath = nativeMessageSubmitPath;
    }
  }

  /** Authoritative typed route registry after deep validation. */
  public static final class RegistryV1 {
    public final int version;
    public final List<Map<String, Object>> lanes;

    RegistryV1(final int version, final List<Map<String, Object>> lanes) {
      this.version = version;
      this.lanes = immutableMaps(lanes);
    }
  }

  /** Immutable semantic-circuit commitments admitted by the SCCP V1 proof policy. */
  public static final class Groth16Bn254SemanticCircuitV1 {
    public final int version;
    public final String circuitCommitment;
    public final String witnessGeneratorCommitment;
    public final String publicSignalSchemaHash;

    Groth16Bn254SemanticCircuitV1(
        final int version,
        final String circuitCommitment,
        final String witnessGeneratorCommitment,
        final String publicSignalSchemaHash) {
      this.version = version;
      this.circuitCommitment = circuitCommitment;
      this.witnessGeneratorCommitment = witnessGeneratorCommitment;
      this.publicSignalSchemaHash = publicSignalSchemaHash;
    }
  }

  /** Immutable semantic proof profile and its validated canonical commitment hash. */
  public static final class SemanticProofProfileV1 {
    public final String profile;
    public final Groth16Bn254SemanticCircuitV1 commitments;
    public final String profileHash;

    SemanticProofProfileV1(
        final String profile,
        final Groth16Bn254SemanticCircuitV1 commitments,
        final String profileHash) {
      this.profile = profile;
      this.commitments = commitments;
      this.profileHash = profileHash;
    }
  }

  /** Immutable Taira finality checkpoint committed by public signal 10. */
  public static final class SoraFinalityAnchorV1 {
    public final int version;
    public final SccpNetworkV1 sourceNetwork;
    public final int protocolVersion;
    public final String chainIdHash;
    public final BigInteger checkpointHeight;
    public final String checkpointBlockHash;
    public final String checkpointContextId;
    public final String checkpointFinalityArtifactHash;
    public final String anchorHash;

    SoraFinalityAnchorV1(
        final int version,
        final SccpNetworkV1 sourceNetwork,
        final int protocolVersion,
        final String chainIdHash,
        final BigInteger checkpointHeight,
        final String checkpointBlockHash,
        final String checkpointContextId,
        final String checkpointFinalityArtifactHash,
        final String anchorHash) {
      this.version = version;
      this.sourceNetwork = sourceNetwork;
      this.protocolVersion = protocolVersion;
      this.chainIdHash = chainIdHash;
      this.checkpointHeight = checkpointHeight;
      this.checkpointBlockHash = checkpointBlockHash;
      this.checkpointContextId = checkpointContextId;
      this.checkpointFinalityArtifactHash = checkpointFinalityArtifactHash;
      this.anchorHash = anchorHash;
    }
  }

  /** Inclusive authenticated-height cutoff retained for one retired route revision. */
  public static final class InboundFinalityCutoffV1 {
    public final String trustAnchorHash;
    public final BigInteger maxAnchorIntervalHeight;

    InboundFinalityCutoffV1(
        final String trustAnchorHash, final BigInteger maxAnchorIntervalHeight) {
      this.trustAnchorHash = trustAnchorHash;
      this.maxAnchorIntervalHeight = maxAnchorIntervalHeight;
    }
  }

  /** Strictly decoded finalized SCCP message bundle. */
  public static final class MessageBundleV1 {
    public final int version;
    public final String messageIdHex;
    public final SccpNetworkV1 sourceNetwork;
    public final SccpNetworkV1 targetNetwork;
    public final String destinationBindingHash;
    public final String routeConfigurationHash;
    public final Map<String, Object> raw;

    MessageBundleV1(
        final int version,
        final String messageIdHex,
        final SccpNetworkV1 sourceNetwork,
        final SccpNetworkV1 targetNetwork,
        final String destinationBindingHash,
        final String routeConfigurationHash,
        final Map<String, Object> raw) {
      this.version = version;
      this.messageIdHex = messageIdHex;
      this.sourceNetwork = sourceNetwork;
      this.targetNetwork = targetNetwork;
      this.destinationBindingHash = destinationBindingHash;
      this.routeConfigurationHash = routeConfigurationHash;
      this.raw = immutableMap(raw);
    }
  }

  /** Strictly decoded query-free state-derived Groth16 request. */
  public static final class Groth16ProofRequestV1 {
    public final int version;
    public final String backend;
    public final SccpNetworkV1 sourceNetwork;
    public final SccpNetworkV1 targetNetwork;
    public final String messageIdHex;
    public final String requestHash;
    public final SemanticProofProfileV1 semanticProofProfile;
    public final SoraFinalityAnchorV1 soraFinalityAnchor;
    public final Map<String, Object> raw;

    Groth16ProofRequestV1(
        final int version,
        final String backend,
        final SccpNetworkV1 sourceNetwork,
        final SccpNetworkV1 targetNetwork,
        final String messageIdHex,
        final String requestHash,
        final SemanticProofProfileV1 semanticProofProfile,
        final SoraFinalityAnchorV1 soraFinalityAnchor,
        final Map<String, Object> raw) {
      this.version = version;
      this.backend = backend;
      this.sourceNetwork = sourceNetwork;
      this.targetNetwork = targetNetwork;
      this.messageIdHex = messageIdHex;
      this.requestHash = requestHash;
      this.semanticProofProfile = semanticProofProfile;
      this.soraFinalityAnchor = soraFinalityAnchor;
      this.raw = immutableMap(raw);
    }
  }

  public static final class RecentMessageLinks {
    public final String bundlePath;
    public final String proofRequestPath;

    RecentMessageLinks(final String bundlePath, final String proofRequestPath) {
      this.bundlePath = bundlePath;
      this.proofRequestPath = proofRequestPath;
    }
  }

  public static final class RecentMessage {
    public final BigInteger height;
    public final int commitmentIndex;
    public final String messageIdHex;
    public final String sourceProfile;
    public final String targetProfile;
    public final String destinationBindingHash;
    public final String routeConfigurationHash;
    public final int targetDomain;
    public final String assetId;
    public final String routeId;
    public final String recipient;
    public final String amount;
    public final Map<String, Object> payloadProjection;
    public final RecentMessageLinks links;

    RecentMessage(
        final BigInteger height,
        final int commitmentIndex,
        final String messageIdHex,
        final String sourceProfile,
        final String targetProfile,
        final String destinationBindingHash,
        final String routeConfigurationHash,
        final int targetDomain,
        final String assetId,
        final String routeId,
        final String recipient,
        final String amount,
        final Map<String, Object> payloadProjection,
        final RecentMessageLinks links) {
      this.height = height;
      this.commitmentIndex = commitmentIndex;
      this.messageIdHex = messageIdHex;
      this.sourceProfile = sourceProfile;
      this.targetProfile = targetProfile;
      this.destinationBindingHash = destinationBindingHash;
      this.routeConfigurationHash = routeConfigurationHash;
      this.targetDomain = targetDomain;
      this.assetId = assetId;
      this.routeId = routeId;
      this.recipient = recipient;
      this.amount = amount;
      this.payloadProjection = immutableMap(payloadProjection);
      this.links = links;
    }
  }

  /** Exact continuation for the newest-first SCCP outbound-message index. */
  public static final class RecentCursor {
    public final BigInteger from;
    public final int afterIndex;

    public RecentCursor(final BigInteger from, final int afterIndex) {
      if (from == null || from.signum() <= 0 || from.compareTo(U64_MAX) > 0) {
        throw new IllegalArgumentException("from must be a positive u64 height");
      }
      if (afterIndex < 0 || afterIndex >= SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1) {
        throw new IllegalArgumentException(
            "afterIndex must be between 0 and "
                + (SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 - 1));
      }
      this.from = from;
      this.afterIndex = afterIndex;
    }
  }

  public static final class RecentMessages {
    public final List<RecentMessage> items;
    public final RecentCursor next;

    RecentMessages(final List<RecentMessage> items, final RecentCursor next) {
      this.items = Collections.unmodifiableList(new ArrayList<>(items));
      this.next = next;
    }
  }

  private static List<Map<String, Object>> immutableMaps(
      final List<Map<String, Object>> values) {
    final List<Map<String, Object>> result = new ArrayList<>();
    for (final Map<String, Object> value : values) result.add(immutableMap(value));
    return Collections.unmodifiableList(result);
  }

  static Map<String, Object> immutableMap(final Map<String, Object> value) {
    final Map<String, Object> result = new LinkedHashMap<>();
    for (final Map.Entry<String, Object> entry : value.entrySet()) {
      result.put(entry.getKey(), immutableValue(entry.getValue()));
    }
    return Collections.unmodifiableMap(result);
  }

  private static Object immutableValue(final Object value) {
    if (value instanceof Map<?, ?> map) {
      final Map<String, Object> typed = new LinkedHashMap<>();
      for (final Map.Entry<?, ?> entry : map.entrySet()) {
        typed.put((String) entry.getKey(), immutableValue(entry.getValue()));
      }
      return Collections.unmodifiableMap(typed);
    }
    if (value instanceof List<?> list) {
      final List<Object> typed = new ArrayList<>();
      for (final Object entry : list) typed.add(immutableValue(entry));
      return Collections.unmodifiableList(typed);
    }
    return value;
  }
}
