package org.hyperledger.iroha.android.client;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.sccp.SccpLaneIdV1;

/** Immutable exact-lane SCCP discovery and readback DTOs. */
public final class SccpModels {
  private SccpModels() {}

  public enum NativeBackendV1 {
    ETHEREUM_BEACON("ethereum_beacon_v1", "bridge/sccp/native/ethereum-beacon-v1"),
    BSC_PARLIA("bsc_parlia_v1", "bridge/sccp/native/bsc-parlia-v1"),
    SOLANA_TOWER("solana_tower_v1", "bridge/sccp/native/solana-tower-v1"),
    TON_MASTERCHAIN("ton_masterchain_v1", "bridge/sccp/native/ton-masterchain-v1"),
    TRON_DPOS("tron_dpos_v1", "bridge/sccp/native/tron-dpos-v1");

    public final String wireKey;
    public final String backendLabel;

    NativeBackendV1(final String wireKey, final String backendLabel) {
      this.wireKey = wireKey;
      this.backendLabel = backendLabel;
    }

    static NativeBackendV1 fromWireKey(final String value) {
      for (final NativeBackendV1 backend : values()) if (backend.wireKey.equals(value)) return backend;
      return null;
    }
  }

  public enum SourceEmitterFamilyV1 {
    EVM("evm"), SOLANA("solana"), TON("ton"), TRON("tron");

    public final String wireKey;
    SourceEmitterFamilyV1(final String wireKey) { this.wireKey = wireKey; }
    static SourceEmitterFamilyV1 fromWireKey(final String value) {
      for (final SourceEmitterFamilyV1 family : values()) if (family.wireKey.equals(value)) return family;
      return null;
    }
  }

  public enum DestinationVerifierPlanV1 {
    EVM_GROTH16_BN254_ADAPTER("EvmGroth16Bn254Adapter"),
    SOLANA_PROGRAM_NATIVE_RECURSIVE("SolanaProgramNativeRecursive"),
    TON_CONTRACT_NATIVE_RECURSIVE("TonContractNativeRecursive"),
    TRON_CONTRACT_GROTH16_BN254("TronContractGroth16Bn254");

    public final String wireKey;
    DestinationVerifierPlanV1(final String wireKey) { this.wireKey = wireKey; }
    static DestinationVerifierPlanV1 fromWireKey(final String value) {
      for (final DestinationVerifierPlanV1 plan : values()) if (plan.wireKey.equals(value)) return plan;
      return null;
    }
  }

  public enum PayloadKindV1 {
    ASSET_REGISTER("asset_register"),
    ROUTE_ACTIVATE("route_activate"),
    TRANSFER("transfer"),
    TOKEN_ADD("token_add"),
    TOKEN_PAUSE("token_pause"),
    TOKEN_RESUME("token_resume");

    public final String wireKey;
    PayloadKindV1(final String wireKey) { this.wireKey = wireKey; }
    static PayloadKindV1 fromWireKey(final String value) {
      for (final PayloadKindV1 kind : values()) if (kind.wireKey.equals(value)) return kind;
      return null;
    }
  }

  public enum CodecV1 {
    CANONICAL_TEXT(1, "canonical_text"),
    EVM_ADDRESS20(2, "evm_address20"),
    SOLANA_PUBKEY32(3, "solana_pubkey32"),
    TON_ACCOUNT36(4, "ton_account36"),
    TRON_ADDRESS21(5, "tron_address21"),
    SORA_ASSET_ID(6, "sora_asset_id");

    public final int id;
    public final String wireKey;
    CodecV1(final int id, final String wireKey) { this.id = id; this.wireKey = wireKey; }
    static CodecV1 fromId(final int id) {
      for (final CodecV1 codec : values()) if (codec.id == id) return codec;
      return null;
    }
  }

  public static final class CodecCapability {
    public final CodecV1 codec;
    public final String description;
    CodecCapability(final CodecV1 codec, final String description) {
      this.codec = codec; this.description = description;
    }
    public int id() { return codec.id; }
    public String key() { return codec.wireKey; }
  }

  public static final class NativeAdmissionCapability {
    public final NativeBackendV1 backend;
    public final String backendLabel;
    public final String trustAnchorHash;
    NativeAdmissionCapability(
        final NativeBackendV1 backend, final String backendLabel, final String trustAnchorHash) {
      this.backend = backend; this.backendLabel = backendLabel; this.trustAnchorHash = trustAnchorHash;
    }
  }

  public static final class BrowserProverManifestRef {
    public final String moduleUrl;
    public final String moduleSpecifier;
    public final String moduleHash;
    public final String manifestHash;
    public final List<String> expectedExports;
    public final String boundRouteHash;
    public final String boundProofHash;
    BrowserProverManifestRef(
        final String moduleUrl,
        final String moduleSpecifier,
        final String moduleHash,
        final String manifestHash,
        final List<String> expectedExports,
        final String boundRouteHash,
        final String boundProofHash) {
      this.moduleUrl = moduleUrl;
      this.moduleSpecifier = moduleSpecifier;
      this.moduleHash = moduleHash;
      this.manifestHash = manifestHash;
      this.expectedExports = immutableList(expectedExports);
      this.boundRouteHash = boundRouteHash;
      this.boundProofHash = boundProofHash;
    }
  }

  public static final class SourceEmitterV1 {
    public final SourceEmitterFamilyV1 family;
    public final Map<String, Object> identity;
    SourceEmitterV1(final SourceEmitterFamilyV1 family, final Map<String, Object> identity) {
      this.family = family;
      this.identity = Collections.unmodifiableMap(new LinkedHashMap<>(identity));
    }
  }

  public static final class SourceIdentityV1 {
    public final SccpLaneIdV1 lane;
    public final SourceEmitterV1 emitter;
    SourceIdentityV1(final SccpLaneIdV1 lane, final SourceEmitterV1 emitter) {
      this.lane = lane; this.emitter = emitter;
    }
  }

  public static final class ExactInboundLaneCapability {
    public final String sourceProfile;
    public final String targetProfile;
    public final int sourceDomain;
    public final int targetDomain;
    public final String sourceIdentityHash;
    public final SourceIdentityV1 sourceIdentity;
    public final boolean admissionEnabled;
    public final NativeAdmissionCapability nativeAdmission;
    public final BrowserProverManifestRef nativeProofBuilder;
    ExactInboundLaneCapability(
        final String sourceProfile,
        final String targetProfile,
        final int sourceDomain,
        final int targetDomain,
        final String sourceIdentityHash,
        final SourceIdentityV1 sourceIdentity,
        final boolean admissionEnabled,
        final NativeAdmissionCapability nativeAdmission,
        final BrowserProverManifestRef nativeProofBuilder) {
      this.sourceProfile = sourceProfile; this.targetProfile = targetProfile;
      this.sourceDomain = sourceDomain; this.targetDomain = targetDomain;
      this.sourceIdentityHash = sourceIdentityHash; this.sourceIdentity = sourceIdentity;
      this.admissionEnabled = admissionEnabled; this.nativeAdmission = nativeAdmission;
      this.nativeProofBuilder = nativeProofBuilder;
    }
  }

  public static final class OutboundProofCapability {
    public final String messageBundlePath;
    public final String proofArtifactPath;
    public final String proofJobPath;
    public final String recentMessagesPath;
    public final String manifestPath;
    OutboundProofCapability(
        final String messageBundlePath,
        final String proofArtifactPath,
        final String proofJobPath,
        final String recentMessagesPath,
        final String manifestPath) {
      this.messageBundlePath = messageBundlePath; this.proofArtifactPath = proofArtifactPath;
      this.proofJobPath = proofJobPath; this.recentMessagesPath = recentMessagesPath;
      this.manifestPath = manifestPath;
    }
  }

  public static final class Capabilities {
    public final int version;
    public final String registryRevision;
    public final String nativeMessageSubmitPath;
    public final OutboundProofCapability outbound;
    public final List<PayloadKindV1> messagePayloadKinds;
    public final List<CodecCapability> codecs;
    public final List<ExactInboundLaneCapability> inboundLanes;
    Capabilities(
        final int version,
        final String registryRevision,
        final String nativeMessageSubmitPath,
        final OutboundProofCapability outbound,
        final List<PayloadKindV1> messagePayloadKinds,
        final List<CodecCapability> codecs,
        final List<ExactInboundLaneCapability> inboundLanes) {
      this.version = version; this.registryRevision = registryRevision;
      this.nativeMessageSubmitPath = nativeMessageSubmitPath; this.outbound = outbound;
      this.messagePayloadKinds = immutableList(messagePayloadKinds);
      this.codecs = immutableList(codecs); this.inboundLanes = immutableList(inboundLanes);
    }
  }

  public static final class OutboundDestinationRoute {
    public final String sourceProfile;
    public final String targetProfile;
    public final int sourceDomain;
    public final int targetDomain;
    public final String routeId;
    public final String assetKey;
    public final DestinationVerifierPlanV1 verifierPlan;
    public final String verifierIdentity;
    public final String verifierCodeHash;
    public final String verifierKeyHash;
    public final String proofArtifactHash;
    public final String provingKeyHash;
    public final String destinationBindingKey;
    public final String destinationBindingHash;
    public final BrowserProverManifestRef browserProver;
    OutboundDestinationRoute(
        final String sourceProfile, final String targetProfile,
        final int sourceDomain, final int targetDomain,
        final String routeId, final String assetKey,
        final DestinationVerifierPlanV1 verifierPlan, final String verifierIdentity,
        final String verifierCodeHash, final String verifierKeyHash,
        final String proofArtifactHash, final String provingKeyHash,
        final String destinationBindingKey, final String destinationBindingHash,
        final BrowserProverManifestRef browserProver) {
      this.sourceProfile = sourceProfile; this.targetProfile = targetProfile;
      this.sourceDomain = sourceDomain; this.targetDomain = targetDomain;
      this.routeId = routeId; this.assetKey = assetKey; this.verifierPlan = verifierPlan;
      this.verifierIdentity = verifierIdentity; this.verifierCodeHash = verifierCodeHash;
      this.verifierKeyHash = verifierKeyHash; this.proofArtifactHash = proofArtifactHash;
      this.provingKeyHash = provingKeyHash; this.destinationBindingKey = destinationBindingKey;
      this.destinationBindingHash = destinationBindingHash; this.browserProver = browserProver;
    }
  }

  public static final class ProofManifestSet {
    public final int version;
    public final String registryRevision;
    public final List<ExactInboundLaneCapability> inboundNativeLanes;
    public final List<OutboundDestinationRoute> outboundDestinationRoutes;
    ProofManifestSet(
        final int version, final String registryRevision,
        final List<ExactInboundLaneCapability> inboundNativeLanes,
        final List<OutboundDestinationRoute> outboundDestinationRoutes) {
      this.version = version; this.registryRevision = registryRevision;
      this.inboundNativeLanes = immutableList(inboundNativeLanes);
      this.outboundDestinationRoutes = immutableList(outboundDestinationRoutes);
    }
  }

  public static final class RecentMessageLinks {
    public final String bundlePath;
    public final String artifactPath;
    public final String jobPath;
    RecentMessageLinks(final String bundlePath, final String artifactPath, final String jobPath) {
      this.bundlePath = bundlePath; this.artifactPath = artifactPath; this.jobPath = jobPath;
    }
  }

  public static final class RecentMessage {
    public final long height;
    public final String messageIdHex;
    public final PayloadKindV1 kind;
    public final String sourceProfile;
    public final String targetProfile;
    public final String destinationBindingHash;
    public final int targetDomain;
    public final int counterpartyDomain;
    public final String assetId;
    public final String routeId;
    public final String recipient;
    public final String amount;
    public final Map<String, Object> payloadProjection;
    public final RecentMessageLinks links;
    RecentMessage(
        final long height, final String messageIdHex, final PayloadKindV1 kind,
        final String sourceProfile, final String targetProfile,
        final String destinationBindingHash, final int targetDomain,
        final int counterpartyDomain, final String assetId, final String routeId,
        final String recipient, final String amount, final Map<String, Object> payloadProjection,
        final RecentMessageLinks links) {
      this.height = height; this.messageIdHex = messageIdHex; this.kind = kind;
      this.sourceProfile = sourceProfile; this.targetProfile = targetProfile;
      this.destinationBindingHash = destinationBindingHash; this.targetDomain = targetDomain;
      this.counterpartyDomain = counterpartyDomain; this.assetId = assetId; this.routeId = routeId;
      this.recipient = recipient; this.amount = amount;
      this.payloadProjection = payloadProjection == null ? null : Collections.unmodifiableMap(new LinkedHashMap<>(payloadProjection));
      this.links = links;
    }
  }

  public static final class RecentMessages {
    public final List<RecentMessage> items;
    RecentMessages(final List<RecentMessage> items) { this.items = immutableList(items); }
  }

  private static <T> List<T> immutableList(final List<T> values) {
    return Collections.unmodifiableList(new ArrayList<>(values));
  }
}
