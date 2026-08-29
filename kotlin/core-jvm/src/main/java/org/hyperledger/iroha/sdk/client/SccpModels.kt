package org.hyperledger.iroha.sdk.client

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.security.MessageDigest
import java.util.Collections
import org.bouncycastle.crypto.digests.KeccakDigest
import org.hyperledger.iroha.sdk.sccp.SccpLaneIdV1
import org.hyperledger.iroha.sdk.sccp.SccpNetworkV1
import org.hyperledger.iroha.sdk.sccp.SccpV1

/** Fixed maximum number of successful outbound SCCP messages in one V1 block. */
const val SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1: Int = 512

/** Fixed maximum retained canonical payload size for one V1 outbound SCCP message. */
const val SCCP_OUTBOUND_MESSAGE_MAX_PAYLOAD_BYTES_V1: Int = 4_096

private val SCCP_U64_MAX_VALUE: BigInteger =
    BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)

/** Validate one TON value-moving amount against its immutable first-release cap. */
fun requireSccpTonAmountWithinCapV1(
    amount: BigInteger,
    maxWrappedSupply: BigInteger,
): BigInteger {
    val maximumTonCoins = BigInteger.ONE.shiftLeft(120).subtract(BigInteger.ONE)
    require(maxWrappedSupply > BigInteger.ZERO && maxWrappedSupply <= maximumTonCoins) {
        "TON max_wrapped_supply must be in 1..2^120-1"
    }
    require(amount > BigInteger.ZERO && amount <= maxWrappedSupply) {
        "TON amount must be positive and no greater than max_wrapped_supply"
    }
    return amount
}

/** Fixed SCCP V1 route-registry capacity limits. */
data class SccpRegistryLimits(
    val maxGovernedLanes: Long,
    val maxLiveGovernedRoutes: Long,
    val maxLiveRoutesPerLane: Long,
    val maxRetainedRoutesPerLane: Long,
    val maxRetainedNativeTrustAnchorsPerLane: Long,
)

/** Consensus-critical SCCP proof and deterministic verifier-work limits. */
data class SccpResourceLimits(
    val maxOutboundMessagesPerBlock: Long,
    val maxOutboundMessagePayloadBytes: BigInteger,
    val maxPendingOutboundMessages: BigInteger,
    val maxPendingOutboundPayloadBytes: BigInteger,
    val maxProofsPerTransaction: Long,
    val maxProofsPerBlock: Long,
    val maxProofBytesPerProof: BigInteger,
    val maxProofBytesPerTransaction: BigInteger,
    val maxProofBytesPerBlock: BigInteger,
    val maxNativeHeadersPerTransaction: Long,
    val maxNativeHeadersPerBlock: Long,
    val maxEthereumLightClientUpdatesPerTransaction: Long,
    val maxEthereumLightClientUpdatesPerBlock: Long,
    val maxNativeHeaderBytesPerTransaction: BigInteger,
    val maxNativeHeaderBytesPerBlock: BigInteger,
    val maxSecp256k1RecoveriesPerTransaction: Long,
    val maxSecp256k1RecoveriesPerBlock: Long,
    val maxBlsAggregateChecksPerTransaction: Long,
    val maxBlsAggregateChecksPerBlock: Long,
    val maxBlsSignerContributionsPerTransaction: Long,
    val maxBlsSignerContributionsPerBlock: Long,
    val maxEd25519SignatureChecksPerTransaction: Long,
    val maxEd25519SignatureChecksPerBlock: Long,
    val maxEd25519ValidatorKeyChecksPerTransaction: Long,
    val maxEd25519ValidatorKeyChecksPerBlock: Long,
    val maxBn254PairingChecksPerTransaction: Long,
    val maxBn254PairingChecksPerBlock: Long,
    val maxBls12381PairingChecksPerTransaction: Long,
    val maxBls12381PairingChecksPerBlock: Long,
)

/** Closed first-release SCCP HTTP surface. */
data class SccpCapabilities(
    val version: Int,
    val registryRevision: String,
    val registryPath: String,
    val messageBundlePath: String,
    val proofRequestPath: String,
    val recentMessagesPath: String,
    val soraOutboundMaterialPath: String,
    val registryLimits: SccpRegistryLimits,
    val resourceLimits: SccpResourceLimits,
    val proofSubmitPath: String?,
    val nativeMessageSubmitPath: String?,
)

/** Authoritative typed route registry with deeply validated raw route records. */
data class SccpRegistryV1(val version: Int, val lanes: List<Map<String, Any?>>)

/** Immutable semantic-circuit commitments admitted by the SCCP V1 proof policy. */
data class SccpGroth16Bn254SemanticCircuitV1(
    val version: Int,
    val circuitCommitment: String,
    val witnessGeneratorCommitment: String,
    val publicSignalSchemaHash: String,
)

/** Immutable semantic proof profile and its validated canonical commitment hash. */
data class SccpSemanticProofProfileV1(
    val profile: String,
    val commitments: SccpGroth16Bn254SemanticCircuitV1,
    val profileHash: String,
)

/** Immutable Taira finality checkpoint committed by public signal 10. */
data class SccpSoraFinalityAnchorV1(
    val version: Int,
    val sourceNetwork: SccpNetworkV1,
    val protocolVersion: Int,
    val chainIdHash: String,
    val checkpointHeight: BigInteger,
    val checkpointBlockHash: String,
    val checkpointContextId: String,
    val checkpointFinalityArtifactHash: String,
    val anchorHash: String,
)

/** Inclusive authenticated-height cutoff retained for one retired route revision. */
data class SccpInboundFinalityCutoffV1(
    val trustAnchorHash: String,
    val maxAnchorIntervalHeight: BigInteger,
)

/** Canonical portable verification-key identity for SORA-side execution proofs. */
data class SccpPortableVerifyingKeyReferenceV1(
    val backend: String,
    val name: String,
    val version: Long,
    val commitment: String,
)

/** Mandatory proved burn-and-record execution policy for a governed SCCP route. */
data class SccpSoraOutboundExecutionPolicyV1(
    val version: Int,
    val semantics: String,
    val contractArtifactSha256: String,
    val verifyingKeyReference: SccpPortableVerifyingKeyReferenceV1,
    val gasLimit: Long,
)

/** Exact ordered five-key TON mint-breaker guardian set. */
data class SccpTonMintBreakerGuardianKeysV1(
    val guardian0: String,
    val guardian1: String,
    val guardian2: String,
    val guardian3: String,
    val guardian4: String,
) {
    init {
        val keys = ordered()
        require(keys.all { Regex("[0-9A-F]{64}").matches(it) && it.any { char -> char != '0' } }) {
            "TON mint-breaker guardian keys must be nonzero uppercase 32-byte hex"
        }
        require(keys.zipWithNext().all { (left, right) -> left < right }) {
            "TON mint-breaker guardian keys must be strictly increasing"
        }
    }

    /** Keys in canonical TON StateInit and SCCP hash-preimage order. */
    fun ordered(): List<String> = listOf(guardian0, guardian1, guardian2, guardian3, guardian4)
}

/** Strictly decoded finalized SCCP message bundle. */
data class SccpMessageBundleV1(
    val version: Int,
    val messageIdHex: String,
    val sourceNetwork: SccpNetworkV1,
    val targetNetwork: SccpNetworkV1,
    val destinationBindingHash: String,
    val routeConfigurationHash: String,
    val raw: Map<String, Any?>,
)

/** Strictly decoded state-derived Groth16 request. */
data class SccpGroth16ProofRequestV1(
    val version: Int,
    val backend: String,
    val sourceNetwork: SccpNetworkV1,
    val targetNetwork: SccpNetworkV1,
    val messageIdHex: String,
    val requestHash: String,
    val publicSignals: Map<String, String>?,
    val verifierCircuitHash: String?,
    val proofProfileCommitment: String?,
    val semanticProofProfile: SccpSemanticProofProfileV1,
    val soraFinalityAnchor: SccpSoraFinalityAnchorV1,
    val raw: Map<String, Any?>,
)

data class SccpRecentMessageLinks(val bundlePath: String, val proofRequestPath: String)

data class SccpRecentMessage(
    val height: BigInteger,
    val commitmentIndex: Int,
    val messageIdHex: String,
    val sourceProfile: String,
    val targetProfile: String,
    val destinationBindingHash: String,
    val routeConfigurationHash: String,
    val targetDomain: Int,
    val assetId: String?,
    val routeId: String?,
    val recipient: String?,
    val amount: String,
    val payloadProjection: Map<String, Any?>,
    val links: SccpRecentMessageLinks,
)

/** Exact continuation for the newest-first SCCP outbound-message index. */
data class SccpRecentCursor(val from: BigInteger, val afterIndex: Int) {
    init {
        require(from > BigInteger.ZERO && from <= SCCP_U64_MAX_VALUE) {
            "from must be a positive u64 height"
        }
        require(afterIndex in 0 until SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1) {
            "afterIndex must be between 0 and ${SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 - 1}"
        }
    }
}

data class SccpRecentMessages(
    val items: List<SccpRecentMessage>,
    val next: SccpRecentCursor?,
)

/** Strict decoders for the closed first-release SCCP JSON API. */
object SccpJsonParser {
    /** Validate one exact closed SCCP V1 route-governance action. */
    @JvmStatic
    fun validateRouteGovernanceAction(value: Map<String, Any?>) {
        exactFields(value, setOf("action", "route"), "SCCP route governance action")
        val action = requiredText(value, "action")
        val payload = requiredObject(value, "route")
        when (action) {
            "Register" -> {
                exactFields(payload, setOf("route", "native_trust_anchor"), "SCCP Register")
                val route = requiredObject(payload, "route")
                val lane = parseInboundLane(requiredObject(route, "lane_id"), "SCCP Register.route.lane_id")
                val parsed = parseGovernedRoute(route, lane, "SCCP Register.route")
                require(parsed.activation == "staged") { "new SCCP routes must be staged" }
                payload["native_trust_anchor"]?.let {
                    val anchor = parseNativeTrustAnchor(
                        it,
                        lane,
                        "SCCP Register.native_trust_anchor",
                    )
                    require(anchor.checkpointHeight <= MAX_JSON_SAFE_INTEGER) {
                        "SCCP Register.native_trust_anchor exceeds the exact JSON integer bound"
                    }
                }
            }
            "SetActivation" -> {
                exactFields(
                    payload,
                    setOf("key", "expected_current", "next", "inbound_finality_cutoff"),
                    "SCCP SetActivation",
                )
                parseGovernanceRouteKey(requiredObject(payload, "key"), "SCCP SetActivation.key")
                val current = parseGovernanceActivation(
                    requiredObject(payload, "expected_current"),
                    "SCCP SetActivation.expected_current",
                )
                val next = parseGovernanceActivation(
                    requiredObject(payload, "next"),
                    "SCCP SetActivation.next",
                )
                validateGovernanceCutoff(
                    payload["inbound_finality_cutoff"],
                    next,
                    "SCCP SetActivation.inbound_finality_cutoff",
                )
                require(canTransitionGovernanceActivation(current, next)) {
                    "SCCP activation transition is not legal"
                }
            }
            "SwitchRevision" -> {
                exactFields(
                    payload,
                    setOf(
                        "previous_key",
                        "expected_previous",
                        "previous_next",
                        "previous_inbound_finality_cutoff",
                        "successor_key",
                        "successor_next",
                    ),
                    "SCCP SwitchRevision",
                )
                val previous = parseGovernanceRouteKey(
                    requiredObject(payload, "previous_key"),
                    "SCCP SwitchRevision.previous_key",
                )
                val successor = parseGovernanceRouteKey(
                    requiredObject(payload, "successor_key"),
                    "SCCP SwitchRevision.successor_key",
                )
                val expected = parseGovernanceActivation(
                    requiredObject(payload, "expected_previous"),
                    "SCCP SwitchRevision.expected_previous",
                )
                val previousNext = parseGovernanceActivation(
                    requiredObject(payload, "previous_next"),
                    "SCCP SwitchRevision.previous_next",
                )
                val successorNext = parseGovernanceActivation(
                    requiredObject(payload, "successor_next"),
                    "SCCP SwitchRevision.successor_next",
                )
                validateGovernanceCutoff(
                    payload["previous_inbound_finality_cutoff"],
                    previousNext,
                    "SCCP SwitchRevision.previous_inbound_finality_cutoff",
                )
                val previousTransitionValid = if (previousNext == "retired") {
                    expected in setOf("bidirectional", "inbound_only", "paused")
                } else {
                    canTransitionGovernanceActivation(expected, previousNext)
                }
                require(
                    previous.lane == successor.lane &&
                        previous.routeId == successor.routeId &&
                        previous.assetKey == successor.assetKey &&
                        successor.revision == previous.revision + 1 &&
                        previousTransitionValid &&
                        previousNext in setOf("inbound_only", "paused", "retired") &&
                        successorNext == "bidirectional",
                ) { "SCCP revision switch is not a legal atomic cutover" }
            }
            "InitializeTrustAnchor" -> {
                exactFields(
                    payload,
                    setOf("lane_id", "expected_current", "initial"),
                    "SCCP InitializeTrustAnchor",
                )
                val lane = parseInboundLane(
                    requiredObject(payload, "lane_id"),
                    "SCCP InitializeTrustAnchor.lane_id",
                )
                require(payload["expected_current"] == null) {
                    "SCCP initial trust anchor must expect no current value"
                }
                val initial = parseNativeTrustAnchor(
                    payload["initial"],
                    lane,
                    "SCCP InitializeTrustAnchor.initial",
                )
                require(initial.checkpointHeight <= MAX_JSON_SAFE_INTEGER) {
                    "SCCP InitializeTrustAnchor.initial exceeds the exact JSON integer bound"
                }
            }
            "AdvanceTrustAnchor" -> {
                exactFields(
                    payload,
                    setOf("lane_id", "expected_current", "next"),
                    "SCCP AdvanceTrustAnchor",
                )
                val lane = parseInboundLane(
                    requiredObject(payload, "lane_id"),
                    "SCCP AdvanceTrustAnchor.lane_id",
                )
                val current = parseNativeTrustAnchor(
                    payload["expected_current"],
                    lane,
                    "SCCP AdvanceTrustAnchor.expected_current",
                )
                val next = parseNativeTrustAnchor(
                    payload["next"],
                    lane,
                    "SCCP AdvanceTrustAnchor.next",
                )
                require(
                    current.backend == next.backend &&
                        current.anchorHash != next.anchorHash &&
                        current.checkpointHeight <= MAX_JSON_SAFE_INTEGER &&
                        next.checkpointHeight <= MAX_JSON_SAFE_INTEGER &&
                        next.checkpointHeight > current.checkpointHeight,
                ) { "SCCP trust anchor must advance monotonically within one backend" }
            }
            "Remove" -> parseGovernanceRouteKey(payload, "SCCP Remove")
            else -> throw IllegalArgumentException("SCCP route governance action is unsupported")
        }
    }

    @JvmStatic fun parseCapabilities(bytes: ByteArray): SccpCapabilities {
        val root = rootObject(bytes, "SCCP capabilities")
        exactFields(root, CAPABILITY_FIELDS, "SCCP capabilities", CAPABILITY_REQUIRED)
        val proofSubmitPath = optionalExactPath(
            root,
            "proof_submit_path",
            "/v1/bridge/proofs/submit",
            true,
        )
        val nativeMessageSubmitPath = optionalExactPath(
            root,
            "native_message_submit_path",
            "/v1/bridge/messages",
            true,
        )
        require((proofSubmitPath == null) == (nativeMessageSubmitPath == null)) {
            "SCCP write capability paths must be advertised together"
        }
        val result = SccpCapabilities(
            requiredInt(root, "version", 1, 1),
            prefixedHash(root, "registry_revision"),
            optionalExactPath(root, "registry_path", "/v1/sccp/registry", false)!!,
            optionalExactPath(
                root,
                "message_bundle_path",
                "/v1/sccp/proofs/message/{message_id}",
                false,
            )!!,
            optionalExactPath(
                root,
                "proof_request_path",
                "/v1/sccp/proof-requests/{message_id}",
                false,
            )!!,
            optionalExactPath(root, "recent_messages_path", "/v1/sccp/messages/recent", false)!!,
            optionalExactPath(
                root,
                "sora_outbound_material_path",
                "/v1/sccp/routes/{source_profile}/{route_id}/{asset_key}/{revision}/sora-outbound-material",
                false,
            )!!,
            parseRegistryLimits(requiredObject(root, "registry_limits")),
            parseResourceLimits(requiredObject(root, "resource_limits")),
            proofSubmitPath,
            nativeMessageSubmitPath,
        )
        requireDistinctHashes(listOf(result.registryRevision), "capability registry revision")
        return result
    }

    private fun parseRegistryLimits(value: Map<String, Any?>): SccpRegistryLimits {
        exactFields(value, REGISTRY_LIMIT_FIELDS, "SCCP registry limits")
        fun u32(field: String): Long =
            requiredUnsignedInteger(value, field, MAX_U32, true).longValueExact()
        return SccpRegistryLimits(
            u32("max_governed_lanes"),
            u32("max_live_governed_routes"),
            u32("max_live_routes_per_lane"),
            u32("max_retained_routes_per_lane"),
            u32("max_retained_native_trust_anchors_per_lane"),
        ).also {
            require(
                it == SccpRegistryLimits(16, 64, 8, 64, 4_096),
            ) { "SCCP registry limits must equal the fixed V1 capacities" }
        }
    }

    private fun parseResourceLimits(value: Map<String, Any?>): SccpResourceLimits {
        exactFields(value, RESOURCE_LIMIT_FIELDS, "SCCP resource limits")
        fun u32(field: String): Long =
            requiredUnsignedInteger(value, field, MAX_U32, true).longValueExact()
        fun u64(field: String): BigInteger =
            requiredUnsignedInteger(value, field, MAX_JSON_SAFE_INTEGER, true)
        val result = SccpResourceLimits(
            u32("max_outbound_messages_per_block"),
            u64("max_outbound_message_payload_bytes"),
            u64("max_pending_outbound_messages"),
            u64("max_pending_outbound_payload_bytes"),
            u32("max_proofs_per_transaction"),
            u32("max_proofs_per_block"),
            u64("max_proof_bytes_per_proof"),
            u64("max_proof_bytes_per_transaction"),
            u64("max_proof_bytes_per_block"),
            u32("max_native_headers_per_transaction"),
            u32("max_native_headers_per_block"),
            u32("max_ethereum_light_client_updates_per_transaction"),
            u32("max_ethereum_light_client_updates_per_block"),
            u64("max_native_header_bytes_per_transaction"),
            u64("max_native_header_bytes_per_block"),
            u32("max_secp256k1_recoveries_per_transaction"),
            u32("max_secp256k1_recoveries_per_block"),
            u32("max_bls_aggregate_checks_per_transaction"),
            u32("max_bls_aggregate_checks_per_block"),
            u32("max_bls_signer_contributions_per_transaction"),
            u32("max_bls_signer_contributions_per_block"),
            u32("max_ed25519_signature_checks_per_transaction"),
            u32("max_ed25519_signature_checks_per_block"),
            u32("max_ed25519_validator_key_checks_per_transaction"),
            u32("max_ed25519_validator_key_checks_per_block"),
            u32("max_bn254_pairing_checks_per_transaction"),
            u32("max_bn254_pairing_checks_per_block"),
            u32("max_bls12_381_pairing_checks_per_transaction"),
            u32("max_bls12_381_pairing_checks_per_block"),
        )
        require(
            result.maxOutboundMessagesPerBlock ==
                SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1.toLong() &&
                result.maxOutboundMessagePayloadBytes ==
                BigInteger.valueOf(SCCP_OUTBOUND_MESSAGE_MAX_PAYLOAD_BYTES_V1.toLong()),
        ) { "SCCP outbound-message limits must equal the fixed V1 capacities" }
        require(result.maxProofBytesPerProof <= result.maxProofBytesPerTransaction) {
            "SCCP per-proof byte limit exceeds its transaction limit"
        }
        val ordered = listOf(
            BigInteger.valueOf(result.maxProofsPerTransaction) to
                BigInteger.valueOf(result.maxProofsPerBlock),
            result.maxProofBytesPerTransaction to result.maxProofBytesPerBlock,
            BigInteger.valueOf(result.maxNativeHeadersPerTransaction) to
                BigInteger.valueOf(result.maxNativeHeadersPerBlock),
            BigInteger.valueOf(result.maxEthereumLightClientUpdatesPerTransaction) to
                BigInteger.valueOf(result.maxEthereumLightClientUpdatesPerBlock),
            result.maxNativeHeaderBytesPerTransaction to result.maxNativeHeaderBytesPerBlock,
            BigInteger.valueOf(result.maxSecp256k1RecoveriesPerTransaction) to
                BigInteger.valueOf(result.maxSecp256k1RecoveriesPerBlock),
            BigInteger.valueOf(result.maxBlsAggregateChecksPerTransaction) to
                BigInteger.valueOf(result.maxBlsAggregateChecksPerBlock),
            BigInteger.valueOf(result.maxBlsSignerContributionsPerTransaction) to
                BigInteger.valueOf(result.maxBlsSignerContributionsPerBlock),
            BigInteger.valueOf(result.maxEd25519SignatureChecksPerTransaction) to
                BigInteger.valueOf(result.maxEd25519SignatureChecksPerBlock),
            BigInteger.valueOf(result.maxEd25519ValidatorKeyChecksPerTransaction) to
                BigInteger.valueOf(result.maxEd25519ValidatorKeyChecksPerBlock),
            BigInteger.valueOf(result.maxBn254PairingChecksPerTransaction) to
                BigInteger.valueOf(result.maxBn254PairingChecksPerBlock),
            BigInteger.valueOf(result.maxBls12381PairingChecksPerTransaction) to
                BigInteger.valueOf(result.maxBls12381PairingChecksPerBlock),
        )
        require(ordered.all { (transaction, block) -> transaction <= block }) {
            "SCCP transaction resource limits must not exceed block limits"
        }
        return result
    }

    @JvmStatic fun parseRegistry(bytes: ByteArray): SccpRegistryV1 {
        val root = rootObject(bytes, "SCCP registry")
        exactFields(root, setOf("version", "lanes"), "SCCP registry")
        val lanes = requiredList(root, "lanes")
        require(lanes.size <= 16) { "SCCP registry contains more than 16 lanes" }
        val laneKeys = mutableSetOf<String>()
        val routeKeys = mutableSetOf<String>()
        val destinationBindings = mutableSetOf<String>()
        val routeConfigurations = mutableSetOf<String>()
        var routeCount = 0
        val normalized = lanes.mapIndexed { laneIndex, raw ->
            val label = "SCCP registry.lanes[$laneIndex]"
            val laneRecord = objectValue(raw, label)
            exactFields(
                laneRecord,
                setOf(
                    "lane_id",
                    "native_trust_anchors",
                    "current_native_trust_anchor_hash",
                    "routes",
                ),
                label,
            )
            val lane = parseInboundLane(requiredObject(laneRecord, "lane_id"), "$label.lane_id")
            require(laneKeys.add(lane.toString())) { "SCCP registry contains a duplicate lane" }
            val anchorValues = requiredList(laneRecord, "native_trust_anchors")
            require(anchorValues.size <= 4_096) {
                "$label contains more than 4,096 retained native trust anchors"
            }
            val anchors = anchorValues.mapIndexed { anchorIndex, raw ->
                val anchorLabel = "$label.native_trust_anchors[$anchorIndex]"
                require(raw != null) { "$anchorLabel must not be null" }
                parseNativeTrustAnchor(raw, lane, anchorLabel)
            }
            val anchorHashes = mutableSetOf<String>()
            anchors.forEachIndexed { anchorIndex, anchor ->
                require(anchorHashes.add(anchor.anchorHash)) {
                    "$label contains a duplicate native trust-anchor hash"
                }
                if (anchorIndex != 0) {
                    val previous = anchors[anchorIndex - 1]
                    require(
                        anchor.backend == previous.backend &&
                            anchor.checkpointHeight > previous.checkpointHeight,
                    ) {
                        "$label.native_trust_anchors must advance monotonically within one backend"
                    }
                }
            }
            val currentAnchorHash = if (laneRecord["current_native_trust_anchor_hash"] == null) {
                null
            } else {
                upperBytes(laneRecord, "current_native_trust_anchor_hash", 32)
            }
            require(currentAnchorHash == anchors.lastOrNull()?.anchorHash) {
                "$label.current_native_trust_anchor_hash must name the last retained anchor"
            }
            val routes = requiredList(laneRecord, "routes")
            require(routes.isNotEmpty()) { "$label.routes must contain at least one route" }
            require(routes.size <= 64) { "$label contains more than 64 retained route revisions" }
            var liveRouteCount = 0
            val lineages = linkedMapOf<String, MutableList<Pair<Long, String>>>()
            routes.forEachIndexed { routeIndex, routeRaw ->
                val routeLabel = "$label.routes[$routeIndex]"
                val parsed = parseGovernedRoute(objectValue(routeRaw, routeLabel), lane, routeLabel)
                require(routeKeys.add(parsed.key)) { "SCCP registry contains a duplicate route" }
                require(destinationBindings.add(parsed.destinationBindingHash)) {
                    "SCCP registry reuses a destination-binding hash"
                }
                require(routeConfigurations.add(parsed.routeConfigurationHash)) {
                    "SCCP registry reuses a route-configuration hash"
                }
                if (parsed.activation == "bidirectional" || parsed.activation == "inbound_only") {
                    require(anchors.isNotEmpty()) {
                        "$routeLabel enables inbound settlement without a native trust anchor"
                    }
                }
                parsed.inboundFinalityCutoff?.let { cutoff ->
                    val anchorIndex = anchors.indexOfFirst { it.anchorHash == cutoff.trustAnchorHash }
                    require(
                        anchorIndex >= 0 &&
                            anchors.getOrNull(anchorIndex + 1)?.checkpointHeight ==
                            cutoff.maxAnchorIntervalHeight,
                    ) {
                        "$routeLabel.inbound_finality_cutoff must close one retained anchor interval"
                    }
                }
                if (parsed.activation != "retired") liveRouteCount += 1
                lineages.getOrPut(parsed.lineage, ::mutableListOf)
                    .add(parsed.revision to parsed.activation)
            }
            require(liveRouteCount <= 8) { "$label contains more than 8 live routes" }
            routeCount += liveRouteCount
            for (revisions in lineages.values) {
                revisions.sortBy { it.first }
                revisions.forEachIndexed { index, revision ->
                    require(revision.first == index.toLong() + 1) {
                        "SCCP route revisions must start at one and contain no gaps"
                    }
                }
                require(revisions.count { it.second == "bidirectional" } <= 1) {
                    "SCCP registry enables multiple revisions of one route"
                }
            }
            deepCopyObject(laneRecord)
        }
        require(routeCount <= 64) { "SCCP registry contains more than 64 live routes" }
        return SccpRegistryV1(requiredInt(root, "version", 1, 1), normalized)
    }

    @JvmStatic fun parseMessageBundle(bytes: ByteArray): SccpMessageBundleV1 {
        val root = rootObject(bytes, "SCCP message bundle")
        exactFields(
            root,
            setOf(
                "version",
                "commitment_root",
                "commitment",
                "merkle_proof",
                "payload",
                "finality_proof",
            ),
            "SCCP message bundle",
        )
        requiredInt(root, "version", 1, 1)
        val commitmentRoot = prefixedHash(root, "commitment_root")
        val commitment = requiredObject(root, "commitment")
        exactFields(
            commitment,
            setOf("version", "kind", "context", "message_id", "payload_hash"),
            "SCCP commitment",
        )
        requiredInt(commitment, "version", 1, 1)
        require(requiredText(commitment, "kind") == "Transfer") {
            "SCCP commitment kind must be Transfer"
        }
        val context = requiredObject(commitment, "context")
        exactFields(
            context,
            setOf("lane", "destination_binding_hash", "route_configuration_hash"),
            "SCCP commitment context",
        )
        val lane = parseLane(requiredObject(context, "lane"), "SCCP commitment context.lane")
        require(lane.isOutbound && lane.source == SccpNetworkV1.SORA_TAIRA) {
            "SCCP message bundle must use an exact Taira-to-external lane"
        }
        val binding = prefixedHash(context, "destination_binding_hash")
        val configuration = prefixedHash(context, "route_configuration_hash")
        val messageId = prefixedHash(commitment, "message_id")
        val payloadHash = prefixedHash(commitment, "payload_hash")
        requireDistinctHashes(
            listOf(commitmentRoot, binding, configuration, messageId, payloadHash),
            "message bundle",
        )
        validateTransferPayload(requiredObject(root, "payload"), lane)
        val merkle = requiredObject(root, "merkle_proof")
        exactFields(merkle, setOf("steps"), "SCCP Merkle proof")
        val steps = requiredList(merkle, "steps")
        require(steps.size <= 64) { "SCCP Merkle proof contains more than 64 steps" }
        steps.forEachIndexed { index, step ->
            val item = objectValue(step, "SCCP Merkle proof.steps[$index]")
            exactFields(item, setOf("sibling_hash", "sibling_is_left"), "SCCP Merkle step")
            prefixedHash(item, "sibling_hash")
            requiredBoolean(item, "sibling_is_left")
        }
        requireHexBytes(root, "finality_proof", false)
        return SccpMessageBundleV1(
            1,
            messageId.removePrefix("0x"),
            lane.source,
            lane.target,
            binding,
            configuration,
            deepCopyObject(root),
        )
    }

    @JvmStatic fun parseProofRequest(bytes: ByteArray): SccpGroth16ProofRequestV1 {
        val root = rootObject(bytes, "SCCP proof request")
        val backendObject = requiredObject(root, "backend")
        exactFields(backendObject, setOf("backend", "family"), "SCCP proof backend")
        require(backendObject["family"] == null) { "SCCP proof backend family must be null" }
        val backend = requiredText(backendObject, "backend")
        require(
            backend == "evm_groth16_bn254_v1" || backend == "tron_groth16_bn254_v1" ||
                backend == "ton_groth16_bls12381_v1",
        ) {
            "SCCP proof backend is unsupported or retired"
        }
        val proofFields = if (backend == "ton_groth16_bls12381_v1") {
            PROOF_REQUEST_FIELDS + TON_PROOF_REQUEST_FIELDS
        } else {
            PROOF_REQUEST_FIELDS
        }
        exactFields(root, proofFields, "SCCP proof request")
        requiredInt(root, "version", 1, 1)
        val source = parseNetwork(requiredObject(root, "source_network"), "source_network")
        val target = parseNetwork(requiredObject(root, "target_network"), "target_network")
        require(source == SccpNetworkV1.SORA_TAIRA && target.isExternal) {
            "SCCP proof request must use an exact Taira-to-external lane"
        }
        val backendMatchesTarget = when (backend) {
            "evm_groth16_bn254_v1" -> target.domainId == 1 || target.domainId == 2
            "tron_groth16_bn254_v1" -> target.domainId == 3
            else -> target.domainId == 4
        }
        require(backendMatchesTarget) {
            "SCCP proof backend does not match target network"
        }
        val publicInputs = requiredObject(root, "public_inputs")
        exactFields(
            publicInputs,
            setOf(
                "version",
                "message_id",
                "payload_hash",
                "target_domain",
                "commitment_root",
                "finality_height",
                "finality_block_hash",
            ),
            "SCCP proof public inputs",
        )
        requiredInt(publicInputs, "version", 1, 1)
        val messageId = prefixedHash(publicInputs, "message_id")
        val payloadHash = prefixedHash(publicInputs, "payload_hash")
        require(requiredInt(publicInputs, "target_domain", 1, 4) == target.domainId) {
            "SCCP proof target domain does not match target network"
        }
        val commitmentRoot = prefixedHash(publicInputs, "commitment_root")
        val finalityHeight = BigInteger(requiredDecimal(publicInputs, "finality_height", true))
        require(finalityHeight.bitLength() <= 64) {
            "SCCP proof finality height must fit u64"
        }
        val finalityBlockHash = prefixedHash(publicInputs, "finality_block_hash")
        val verifierKeyHash = prefixedHash(root, "verifier_key_hash")
        if (backend == "ton_groth16_bls12381_v1") {
            validateBls12381VerifyingKey(
                requiredObject(root, "verifying_key"),
                verifierKeyHash.removePrefix("0x").uppercase(),
                "SCCP proof verifying key",
            )
        } else {
            validateVerifyingKey(
                requiredObject(root, "verifying_key"),
                verifierKeyHash.removePrefix("0x").uppercase(),
                "SCCP proof verifying key",
            )
        }
        val expectedProfile = if (backend == "ton_groth16_bls12381_v1") {
            TON_SEMANTIC_PROFILE
        } else {
            SEMANTIC_PROFILE
        }
        val policyHashes = validateOutboundProofPolicyFields(
            root,
            "SCCP proof request",
            expectedProfile,
        )
        val semanticHash = prefixedHash(root, "semantic_proof_profile_hash")
        require(semanticHash == "0x${policyHashes.profileHash.lowercase()}") {
            "semantic_proof_profile_hash does not match its typed profile"
        }
        val anchorHash = prefixedHash(root, "sora_finality_anchor_hash")
        require(anchorHash == "0x${policyHashes.anchorHash.lowercase()}") {
            "sora_finality_anchor_hash does not match its typed anchor"
        }
        requireHexBytes(root, "bundle_bytes", false)
        val statementHash = prefixedHash(root, "statement_hash")
        val destinationBindingHash = prefixedHash(root, "destination_binding_hash")
        val routeConfigurationHash = prefixedHash(root, "route_configuration_hash")
        val requestHash = prefixedHash(root, "request_hash")
        var verifierCircuitHash: String? = null
        var proofProfileCommitment: String? = null
        var publicSignals: Map<String, String>? = null
        if (backend == "ton_groth16_bls12381_v1") {
            verifierCircuitHash = prefixedHash(root, "verifier_circuit_hash")
            proofProfileCommitment = prefixedHash(root, "proof_profile_commitment")
            require(
                verifierCircuitHash ==
                    "0x${policyHashes.semanticProfile.commitments.circuitCommitment.lowercase()}",
            ) { "SCCP TON verifier circuit does not match its semantic profile" }
            require(
                proofProfileCommitment == "0x${tonProofProfileCommitment().toUpperHex().lowercase()}",
            ) { "SCCP TON proof profile commitment is not canonical" }
            publicSignals = validateTonPublicSignals(
                requiredObject(root, "public_signals"),
                listOf(
                    messageId.removePrefix("0x").hexToBytes(),
                    payloadHash.removePrefix("0x").hexToBytes(),
                    abiWord(BigInteger.valueOf(target.domainId.toLong())),
                    commitmentRoot.removePrefix("0x").hexToBytes(),
                    abiWord(finalityHeight),
                    finalityBlockHash.removePrefix("0x").hexToBytes(),
                    abiWord(BigInteger.ZERO),
                    statementHash.removePrefix("0x").hexToBytes(),
                    destinationBindingHash.removePrefix("0x").hexToBytes(),
                    routeConfigurationHash.removePrefix("0x").hexToBytes(),
                    anchorHash.removePrefix("0x").hexToBytes(),
                ),
            )
        }
        val roles = listOf(
            messageId,
            payloadHash,
            commitmentRoot,
            finalityBlockHash,
            verifierKeyHash,
            semanticHash,
            anchorHash,
            statementHash,
            destinationBindingHash,
            routeConfigurationHash,
            requestHash,
        ) + listOfNotNull(verifierCircuitHash, proofProfileCommitment)
        requireDistinctHashes(roles, "proof request")
        return SccpGroth16ProofRequestV1(
            1,
            backend,
            source,
            target,
            messageId.removePrefix("0x"),
            requestHash,
            publicSignals,
            verifierCircuitHash,
            proofProfileCommitment,
            policyHashes.semanticProfile,
            policyHashes.soraFinalityAnchor,
            deepCopyObject(root),
        )
    }

    @JvmStatic fun parseRecentMessages(bytes: ByteArray): SccpRecentMessages {
        val root = rootObject(bytes, "SCCP recent messages")
        exactFields(
            root,
            setOf("items", "next"),
            "SCCP recent messages",
            setOf("items"),
        )
        val values = requiredList(root, "items")
        require(values.size <= 50) { "SCCP recent response exceeds 50 items" }
        val items = values.mapIndexed { index, raw ->
            parseRecent(objectValue(raw, "items[$index]"), index)
        }
        require(items.zipWithNext().all { (left, right) ->
            left.height > right.height ||
                (left.height == right.height && left.commitmentIndex < right.commitmentIndex)
        }) {
            "SCCP recent messages must use strict height-descending/index-ascending order"
        }
        require(items.map(SccpRecentMessage::messageIdHex).distinct().size == items.size) {
            "SCCP recent messages contain duplicate message ids"
        }
        val next = if (root["next"] == null) {
            null
        } else {
            val value = requiredObject(root, "next")
            exactFields(value, setOf("from", "after_index"), "SCCP recent messages.next")
            SccpRecentCursor(
                requiredUnsignedInteger(value, "from", MAX_U64, true),
                requiredInt(
                    value,
                    "after_index",
                    0,
                    SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 - 1,
                ),
            )
        }
        if (next != null) {
            val last = items.lastOrNull()
                ?: throw IllegalArgumentException("SCCP recent messages.next requires a non-empty page")
            require(next.from == last.height && next.afterIndex == last.commitmentIndex) {
                "SCCP recent messages.next must identify the last returned item"
            }
        }
        return SccpRecentMessages(items, next)
    }

    private data class ParsedRoute(
        val lineage: String,
        val key: String,
        val revision: Long,
        val activation: String,
        val inboundFinalityCutoff: SccpInboundFinalityCutoffV1?,
        val destinationBindingHash: String,
        val routeConfigurationHash: String,
        val soraOutboundExecutionPolicy: SccpSoraOutboundExecutionPolicyV1,
        val maxOutstandingLiability: BigInteger,
    )

    private data class SourceRoles(
        val family: String,
        val address: String,
        val runtimeHash: String,
        val routeConfigurationHash: String,
    )

    private data class DestinationRoles(
        val family: String,
        val tokenCodeHash: String,
        val verifierCodeHash: String,
        val verifierKeyHash: String,
        val semanticProfileHash: String,
        val finalityAnchorHash: String,
        val routeAddress: String,
        val routeCodeHash: String,
        val multiplier: Long,
        val maxWrappedSupply: BigInteger,
        val destinationBindingHash: String,
        val deploymentConfigurationHash: String,
        val governedAddressRoles: List<String>,
        val governedHashRoles: List<String>,
    )

    private data class TonAddress(val workchain: Int, val account: String) {
        val identity: String get() = "$workchain:$account"
    }

    private data class NativeTrustAnchor(
        val backend: String,
        val anchorHash: String,
        val checkpointHeight: BigInteger,
    )

    private data class ParsedProofPolicy(
        val semanticProfile: SccpSemanticProofProfileV1,
        val soraFinalityAnchor: SccpSoraFinalityAnchorV1,
    ) {
        val profileHash: String get() = semanticProfile.profileHash.removePrefix("0x").uppercase()
        val anchorHash: String get() = soraFinalityAnchor.anchorHash.removePrefix("0x").uppercase()
    }

    private data class GovernanceRouteKey(
        val lane: SccpLaneIdV1,
        val routeId: String,
        val assetKey: String,
        val revision: Long,
    )

    private fun parseGovernanceRouteKey(
        value: Map<String, Any?>,
        label: String,
    ): GovernanceRouteKey {
        exactFields(value, setOf("lane_id", "route_id", "asset_key", "revision"), label)
        return GovernanceRouteKey(
            parseInboundLane(requiredObject(value, "lane_id"), "$label.lane_id"),
            canonicalRouteKey(value, "route_id"),
            canonicalRouteKey(value, "asset_key"),
            requiredLong(value, "revision", 1, 0xffff_ffffL),
        )
    }

    private fun parseGovernanceActivation(value: Map<String, Any?>, label: String): String {
        exactFields(value, setOf("activation", "direction"), label)
        require(value["direction"] == null) { "$label.direction must be null" }
        val activation = requiredText(value, "activation")
        require(activation in ACTIVATIONS) { "$label.activation is unsupported" }
        return activation
    }

    private fun validateGovernanceCutoff(value: Any?, activation: String, label: String) {
        if (activation == "retired") {
            val cutoff = objectValue(value, label)
            exactFields(cutoff, setOf("trust_anchor_hash", "max_anchor_interval_height"), label)
            upperBytes(cutoff, "trust_anchor_hash", 32)
            requiredUnsignedInteger(
                cutoff,
                "max_anchor_interval_height",
                MAX_JSON_SAFE_INTEGER,
                true,
            )
        } else {
            require(value == null) { "$label must be null unless activation is retired" }
        }
    }

    private fun canTransitionGovernanceActivation(current: String, next: String): Boolean =
        when (current) {
            "staged" -> next in setOf("bidirectional", "inbound_only", "retired")
            "bidirectional" -> next in setOf("inbound_only", "paused")
            "inbound_only" -> next in setOf("paused", "retired")
            "paused" -> next in setOf("bidirectional", "inbound_only", "retired")
            else -> false
        }

    private fun parseGovernedRoute(
        value: Map<String, Any?>,
        lane: SccpLaneIdV1,
        label: String,
    ): ParsedRoute {
        exactFields(value, ROUTE_FIELDS, label)
        require(parseInboundLane(requiredObject(value, "lane_id"), "$label.lane_id") == lane) {
            "$label.lane_id does not match its registry lane"
        }
        val routeId = canonicalRouteKey(value, "route_id")
        val assetKey = canonicalRouteKey(value, "asset_key")
        val revision = requiredLong(value, "revision", 1, 0xffff_ffffL)
        val activationObject = requiredObject(value, "activation")
        exactFields(activationObject, setOf("activation", "direction"), "$label.activation")
        require(activationObject["direction"] == null) { "$label.activation.direction must be null" }
        val activation = requiredText(activationObject, "activation")
        require(activation in ACTIVATIONS) { "$label.activation is unsupported" }
        val inboundFinalityCutoff = if (activation == "retired") {
            val cutoff = requiredObject(value, "inbound_finality_cutoff")
            exactFields(
                cutoff,
                setOf("trust_anchor_hash", "max_anchor_interval_height"),
                "$label.inbound_finality_cutoff",
            )
            SccpInboundFinalityCutoffV1(
                upperBytes(cutoff, "trust_anchor_hash", 32),
                requiredUnsignedInteger(cutoff, "max_anchor_interval_height", MAX_U64, true),
            )
        } else {
            require(value["inbound_finality_cutoff"] == null) {
                "$label.inbound_finality_cutoff must be null unless the route is retired"
            }
            null
        }
        val source = parseSourceIdentity(requiredObject(value, "source_identity"), lane, "$label.source_identity")
        val destination = parseDestination(requiredObject(value, "destination"), lane, "$label.destination")
        val executionPolicy = parseSoraOutboundExecutionPolicy(
            requiredObject(value, "sora_outbound_execution_policy"),
            "$label.sora_outbound_execution_policy",
        )
        val sourceMatchesDestination = if (source.family == "ton" && destination.family == "ton") {
            source.address == destination.routeAddress &&
                source.runtimeHash == destination.routeCodeHash
        } else {
            source.family == destination.family &&
                source.address == destination.routeAddress &&
                source.runtimeHash == destination.routeCodeHash
        }
        require(sourceMatchesDestination) {
            "$label source emitter does not identify the destination route deployment"
        }
        val settlement = requiredObject(value, "settlement")
        exactFields(
            settlement,
            setOf("asset_definition_id", "payload_amount_scale", "max_outstanding_liability"),
            "$label.settlement",
        )
        require(requiredText(settlement, "asset_definition_id") == TAIRA_XOR_ASSET_ID) {
            "$label settlement must use canonical Taira XOR"
        }
        val payloadAmountScale = requiredInt(settlement, "payload_amount_scale", 9, 9)
        val maxOutstandingLiability = requiredUnsignedInteger(
            settlement,
            "max_outstanding_liability",
            MAX_U128,
            true,
        )
        require(maxOutstandingLiability.multiply(BigInteger.valueOf(destination.multiplier)) == destination.maxWrappedSupply) {
            "$label wrapped-supply cap does not match its SORA liability cap"
        }
        val routeConfigurationHash = routeConfigurationHash(
            lane,
            routeId,
            assetKey,
            revision,
            payloadAmountScale,
            destination,
        )
        require(source.routeConfigurationHash == routeConfigurationHash) {
            "$label source route_config_hash does not match the immutable deployment"
        }
        requireDistinctRawHashes(
            listOf(
                executionPolicy.contractArtifactSha256,
                executionPolicy.verifyingKeyReference.commitment,
                routeConfigurationHash,
                destination.destinationBindingHash,
                destination.verifierKeyHash,
                destination.semanticProfileHash,
                destination.finalityAnchorHash,
            ) + if (destination.family == "ton") {
                destination.governedHashRoles.take(5).filterIndexed { index, _ -> index == 1 || index == 4 }
            } else {
                emptyList()
            },
            "$label governed execution and deployment",
        )
        val lineage = "$routeId\u0000$assetKey"
        return ParsedRoute(
            lineage,
            "${lane.source.profileKey}\u0000${lane.target.profileKey}\u0000$lineage\u0000$revision",
            revision,
            activation,
            inboundFinalityCutoff,
            destination.destinationBindingHash,
            routeConfigurationHash,
            executionPolicy,
            maxOutstandingLiability,
        )
    }

    private fun parseSoraOutboundExecutionPolicy(
        value: Map<String, Any?>,
        label: String,
    ): SccpSoraOutboundExecutionPolicyV1 {
        exactFields(
            value,
            setOf("version", "semantics", "contract_artifact_sha256", "vk_ref", "gas_limit"),
            label,
        )
        val version = requiredInt(value, "version", 1, 1)
        val semantics = requiredText(value, "semantics")
        require(semantics == SORA_OUTBOUND_EXECUTION_SEMANTICS) {
            "$label.semantics is unsupported"
        }
        val artifact = upperBytes(value, "contract_artifact_sha256", 32)
        val referenceValue = requiredObject(value, "vk_ref")
        exactFields(referenceValue, setOf("backend", "name", "version", "commitment"), "$label.vk_ref")
        val backend = requiredText(referenceValue, "backend")
        val name = requiredText(referenceValue, "name")
        require(portableVerifyingKeyField(backend) && portableVerifyingKeyField(name)) {
            "$label.vk_ref is not a portable verifying-key identity"
        }
        val reference = SccpPortableVerifyingKeyReferenceV1(
            backend,
            name,
            requiredLong(referenceValue, "version", 1, 0xffff_ffffL),
            upperBytes(referenceValue, "commitment", 32),
        )
        require(artifact != reference.commitment) {
            "$label reuses its artifact and verification-key hash roles"
        }
        return SccpSoraOutboundExecutionPolicyV1(
            version,
            semantics,
            artifact,
            reference,
            requiredLong(value, "gas_limit", 1, 1_000_000_000),
        )
    }

    private fun portableVerifyingKeyField(value: String): Boolean {
        if (value.toByteArray(Charsets.UTF_8).size !in 1..256 ||
            !value.first().isAsciiLowercaseOrDigit() ||
            !value.last().isAsciiLowercaseOrDigit()
        ) return false
        if (listOf("..", "//", ":::", "/:", ":/", "/.", "./", ":.", ".:").any(value::contains)) {
            return false
        }
        return value.all { it.isAsciiLowercaseOrDigit() || it in "-_/:." }
    }

    private fun Char.isAsciiLowercaseOrDigit(): Boolean = this in 'a'..'z' || this in '0'..'9'

    private fun parseSourceIdentity(
        value: Map<String, Any?>,
        lane: SccpLaneIdV1,
        label: String,
    ): SourceRoles {
        exactFields(value, setOf("lane", "emitter"), label)
        require(parseInboundLane(requiredObject(value, "lane"), "$label.lane") == lane) {
            "$label lane mismatch"
        }
        val emitter = requiredObject(value, "emitter")
        exactFields(emitter, setOf("emitter", "identity"), "$label.emitter")
        val family = requiredText(emitter, "emitter")
        require(family == familyFor(lane.source)) { "$label emitter family mismatch" }
        val identity = requiredObject(emitter, "identity")
        val address: String
        val runtime: String
        if (family == "ton") {
            exactFields(
                identity,
                setOf("address", "code_hash", "route_config_hash"),
                "$label.emitter.identity",
            )
            address = tonAddress(requiredObject(identity, "address"), "$label.emitter.identity.address").identity
            runtime = upperBytes(identity, "code_hash", 32)
        } else {
            exactFields(
                identity,
                setOf("address", "runtime_code_hash", "route_config_hash"),
                "$label.emitter.identity",
            )
            address = upperBytes(identity, "address", 20)
            runtime = upperBytes(identity, "runtime_code_hash", 32)
        }
        val configuration = upperBytes(identity, "route_config_hash", 32)
        require(runtime != configuration) { "$label emitter hash roles alias" }
        return SourceRoles(family, address, runtime, configuration)
    }

    private fun parseDestination(
        value: Map<String, Any?>,
        lane: SccpLaneIdV1,
        label: String,
    ): DestinationRoles {
        exactFields(value, setOf("family", "deployment"), label)
        val family = requiredText(value, "family")
        require(family == familyFor(lane.source)) { "$label family mismatch" }
        val deployment = requiredObject(value, "deployment")
        if (family == "ton") return parseTonDestination(deployment, lane, "$label.deployment")
        exactFields(deployment, DESTINATION_FIELDS, "$label.deployment")
        val addresses = listOf(
            "token_address",
            "verifier_address",
            "route_address",
            "replay_verifier_address",
            "mint_breaker_address",
        ).map {
            upperBytes(deployment, it, 20)
        }
        val hashes = listOf(
            "token_code_hash",
            "verifier_code_hash",
            "verifier_key_hash",
            "route_code_hash",
            "replay_verifier_code_hash",
            "mint_breaker_code_hash",
        ).map { upperBytes(deployment, it, 32) }
        require(addresses.distinct().size == addresses.size && hashes.distinct().size == hashes.size) {
            "$label deployment reuses a role-separated address or hash"
        }
        listOf(hashes[0], hashes[1], hashes[3], hashes[4], hashes[5]).forEach {
            require(it != KECCAK256_EMPTY_BYTES) {
                "$label deployment runtime code hash must not identify empty bytecode"
            }
        }
        validateVerifyingKey(
            requiredObject(deployment, "verifying_key"),
            hashes[2],
            "$label.deployment.verifying_key",
        )
        val policyHashes = validateOutboundProofPolicy(
            requiredObject(deployment, "outbound_proof_policy"),
            "$label.deployment.outbound_proof_policy",
            SEMANTIC_PROFILE,
        )
        requireDistinctRawHashes(
            hashes + listOf(policyHashes.profileHash, policyHashes.anchorHash),
            "$label.deployment proof-policy and deployment hashes",
        )
        val multiplier = requiredLong(
            deployment,
            "taira_to_token_multiplier",
            1_000_000_000,
            1_000_000_000,
        )
        val maxWrappedSupply = requiredUnsignedInteger(
            deployment,
            "max_wrapped_supply",
            MAX_U128,
            true,
        )
        val destinationBindingHash = destinationBindingHash(
            lane.source,
            family,
            addresses[1],
            addresses[2],
            hashes[1],
            hashes[2],
            policyHashes,
            addresses[3],
            hashes[4],
            addresses[4],
            hashes[5],
        )
        val deploymentConfiguration = mutableListOf(
            abiAddress(addresses[0]),
            hashes[0].hexToBytes(),
            abiAddress(addresses[1]),
            hashes[1].hexToBytes(),
            hashes[2].hexToBytes(),
            policyHashes.profileHash.hexToBytes(),
            policyHashes.anchorHash.hexToBytes(),
        )
        if (family == "tron") {
            deploymentConfiguration += destinationBindingHash.hexToBytes()
        }
        deploymentConfiguration += abiAddress(addresses[3])
        deploymentConfiguration += hashes[4].hexToBytes()
        deploymentConfiguration += abiAddress(addresses[4])
        deploymentConfiguration += hashes[5].hexToBytes()
        return DestinationRoles(
            family,
            hashes[0],
            hashes[1],
            hashes[2],
            policyHashes.profileHash,
            policyHashes.anchorHash,
            addresses[2],
            hashes[3],
            multiplier,
            maxWrappedSupply,
            destinationBindingHash,
            keccak(concatenate(deploymentConfiguration)).toUpperHex(),
            addresses,
            hashes + listOf(policyHashes.profileHash, policyHashes.anchorHash),
        )
    }

    private fun parseTonDestination(
        deployment: Map<String, Any?>,
        lane: SccpLaneIdV1,
        label: String,
    ): DestinationRoles {
        exactFields(deployment, TON_DESTINATION_FIELDS, label)
        val master = tonAddress(requiredObject(deployment, "jetton_master_address"), "$label.jetton_master_address")
        val route = tonAddress(requiredObject(deployment, "route_address"), "$label.route_address")
        require(master != route) { "$label reuses a TON contract address" }
        val masterCode = upperBytes(deployment, "jetton_master_code_hash", 32)
        val masterInitialData = upperBytes(deployment, "jetton_master_initial_data_hash", 32)
        val walletCode = upperBytes(deployment, "jetton_wallet_code_hash", 32)
        val routeCode = upperBytes(deployment, "route_code_hash", 32)
        val routeInitialData = upperBytes(deployment, "route_initial_data_hash", 32)
        val embeddedCode = upperBytes(deployment, "embedded_verifier_code_hash", 32)
        val circuit = upperBytes(deployment, "verifier_circuit_hash", 32)
        val keyHash = upperBytes(deployment, "verifier_key_hash", 32)
        val proofProfile = upperBytes(deployment, "proof_profile_commitment", 32)
        val guardianKeys = tonGuardianKeys(
            requiredObject(deployment, "mint_breaker_guardian_keys"),
            "$label.mint_breaker_guardian_keys",
        )
        validateBls12381VerifyingKey(
            requiredObject(deployment, "verifying_key"),
            keyHash,
            "$label.verifying_key",
        )
        val policy = validateOutboundProofPolicy(
            requiredObject(deployment, "outbound_proof_policy"),
            "$label.outbound_proof_policy",
            TON_SEMANTIC_PROFILE,
        )
        require(circuit == policy.semanticProfile.commitments.circuitCommitment) {
            "$label verifier_circuit_hash does not match its semantic circuit"
        }
        require(proofProfile == tonProofProfileCommitment().toUpperHex()) {
            "$label proof_profile_commitment is not the exact TON V1 profile"
        }
        val governedHashes = listOf(
            masterCode,
            masterInitialData,
            walletCode,
            routeCode,
            routeInitialData,
            embeddedCode,
            circuit,
            keyHash,
            proofProfile,
            policy.profileHash,
            policy.anchorHash,
        )
        requireDistinctRawHashes(governedHashes, "$label TON deployment")
        val multiplier = requiredLong(deployment, "taira_to_token_multiplier", 1, 1)
        val maxWrappedSupply = requiredUnsignedInteger(
            deployment,
            "max_wrapped_supply",
            MAX_TON_COINS,
            true,
        )
        val binding = tonDestinationBindingHash(
            lane.source,
            masterCode,
            walletCode,
            routeCode,
            embeddedCode,
            circuit,
            keyHash,
            proofProfile,
            guardianKeys,
            policy,
        )
        val configuration = concatenate(
            listOf(
                masterCode.hexToBytes(),
                walletCode.hexToBytes(),
                routeCode.hexToBytes(),
                embeddedCode.hexToBytes(),
                circuit.hexToBytes(),
                keyHash.hexToBytes(),
                proofProfile.hexToBytes(),
            ) + guardianKeys.ordered().map { it.hexToBytes() } + listOf(
                policy.profileHash.hexToBytes(),
                policy.anchorHash.hexToBytes(),
                binding.hexToBytes(),
            ),
        )
        return DestinationRoles(
            "ton",
            masterCode,
            embeddedCode,
            keyHash,
            policy.profileHash,
            policy.anchorHash,
            route.identity,
            routeCode,
            multiplier,
            maxWrappedSupply,
            binding,
            sha256(configuration).toUpperHex(),
            listOf(master.identity, route.identity),
            governedHashes,
        )
    }

    private fun destinationBindingHash(
        network: SccpNetworkV1,
        family: String,
        verifierAddress: String,
        routeAddress: String,
        verifierCodeHash: String,
        verifierKeyHash: String,
        policyHashes: ParsedProofPolicy,
        replayVerifierAddress: String,
        replayVerifierCodeHash: String,
        mintBreakerAddress: String,
        mintBreakerCodeHash: String,
    ): String {
        val networkValue = when (network) {
            SccpNetworkV1.ETHEREUM_MAINNET -> 1L
            SccpNetworkV1.BSC_MAINNET -> 56L
            SccpNetworkV1.TRON_MAINNET -> 0x2b66_53dcL
            SccpNetworkV1.TON_MAINNET,
            SccpNetworkV1.SORA_TAIRA -> error("closed destination lane")
        }
        val isTron = family == "tron"
        val payload = listOf(
            keccak(
                (if (isTron) TRON_DESTINATION_BINDING_DOMAIN else EVM_DESTINATION_BINDING_DOMAIN)
                    .toByteArray(Charsets.UTF_8),
            ),
            keccak(
                (if (isTron) TRON_GROTH16_BACKEND else EVM_GROTH16_BACKEND)
                    .toByteArray(Charsets.UTF_8),
            ),
            abiWord(networkValue),
            abiWord(0),
            abiWord(network.domainId.toLong()),
            if (isTron) abiTronAddress(verifierAddress) else abiAddress(verifierAddress),
            if (isTron) abiTronAddress(routeAddress) else abiAddress(routeAddress),
            verifierCodeHash.hexToBytes(),
            verifierKeyHash.hexToBytes(),
            policyHashes.profileHash.hexToBytes(),
            policyHashes.anchorHash.hexToBytes(),
            if (isTron) abiTronAddress(replayVerifierAddress) else abiAddress(replayVerifierAddress),
            replayVerifierCodeHash.hexToBytes(),
            if (isTron) abiTronAddress(mintBreakerAddress) else abiAddress(mintBreakerAddress),
            mintBreakerCodeHash.hexToBytes(),
        )
        return keccak(concatenate(payload)).toUpperHex()
    }

    private fun tonDestinationBindingHash(
        network: SccpNetworkV1,
        jettonMasterCodeHash: String,
        jettonWalletCodeHash: String,
        routeCodeHash: String,
        embeddedVerifierCodeHash: String,
        verifierCircuitHash: String,
        verifierKeyHash: String,
        proofProfileCommitment: String,
        guardianKeys: SccpTonMintBreakerGuardianKeysV1,
        policy: ParsedProofPolicy,
    ): String {
        val globalId = when (network) {
            SccpNetworkV1.TON_MAINNET -> -239
            else -> throw IllegalArgumentException("TON destination binding requires a TON network")
        }
        val payload = ByteArrayOutputStream().also { output ->
            output.write(TON_DESTINATION_BINDING_DOMAIN.toByteArray(Charsets.UTF_8))
            output.write(1)
            writeBytes(output, TON_GROTH16_BACKEND.toByteArray(Charsets.US_ASCII))
            writeBytes(output, SccpV1.canonicalNetworkBytes(network))
            writeI32(output, globalId)
            writeU32(output, 0)
            writeU32(output, 4)
            output.write(jettonMasterCodeHash.hexToBytes())
            output.write(jettonWalletCodeHash.hexToBytes())
            output.write(routeCodeHash.hexToBytes())
            output.write(embeddedVerifierCodeHash.hexToBytes())
            output.write(verifierCircuitHash.hexToBytes())
            output.write(verifierKeyHash.hexToBytes())
            output.write(proofProfileCommitment.hexToBytes())
            guardianKeys.ordered().forEach { output.write(it.hexToBytes()) }
            output.write(policy.profileHash.hexToBytes())
            output.write(policy.anchorHash.hexToBytes())
        }.toByteArray()
        return sha256(payload).toUpperHex()
    }

    private fun routeConfigurationHash(
        lane: SccpLaneIdV1,
        routeId: String,
        assetKey: String,
        revision: Long,
        payloadAmountScale: Int,
        destination: DestinationRoles,
    ): String {
        if (destination.family == "ton") {
            require(
                routeId == "taira_ton_xor" && assetKey == "xor" && payloadAmountScale == 9,
            ) { "SCCP TON route identity does not match its exact deployment" }
            val globalId = when (lane.source) {
                SccpNetworkV1.TON_MAINNET -> -239
                else -> throw IllegalArgumentException("SCCP TON route requires a TON lane")
            }
            val sourceLaneHash = SccpV1.laneHash(lane).toUpperHex()
            val destinationLaneHash = SccpV1.laneHash(
                SccpLaneIdV1(lane.target, lane.source),
            ).toUpperHex()
            requireDistinctRawHashes(
                listOf(sourceLaneHash, destinationLaneHash) +
                    destination.governedHashRoles + destination.destinationBindingHash,
                "SCCP TON route",
            )
            val assetRoute = ByteArrayOutputStream().also { output ->
                writeBytes(output, "xor".toByteArray(Charsets.US_ASCII))
                writeBytes(output, "taira_ton_xor".toByteArray(Charsets.US_ASCII))
                writeU32(output, revision.toInt())
                writeU64(output, BigInteger.valueOf(destination.multiplier))
                writeU128(output, destination.maxWrappedSupply)
            }.toByteArray()
            val payload = ByteArrayOutputStream().also { output ->
                output.write(CONCRETE_ROUTE_CONFIGURATION_DOMAIN.toByteArray(Charsets.UTF_8))
                output.write(1)
                writeU32(output, 4)
                writeBytes(output, SccpV1.canonicalNetworkBytes(lane.source))
                writeI32(output, globalId)
                output.write(sourceLaneHash.hexToBytes())
                output.write(destinationLaneHash.hexToBytes())
                output.write(destination.deploymentConfigurationHash.hexToBytes())
                output.write(sha256(assetRoute))
            }.toByteArray()
            return sha256(payload).toUpperHex()
        }
        val expectedRouteId: String
        val networkValue: Long
        when (lane.source) {
            SccpNetworkV1.ETHEREUM_MAINNET -> {
                expectedRouteId = "taira_eth_xor"
                networkValue = 1
            }
            SccpNetworkV1.BSC_MAINNET -> {
                expectedRouteId = "taira_bsc_xor"
                networkValue = 56
            }
            SccpNetworkV1.TRON_MAINNET -> {
                expectedRouteId = "taira_tron_xor"
                networkValue = 0x2b66_53dcL
            }
            SccpNetworkV1.TON_MAINNET -> error("TON route handled above")
            SccpNetworkV1.SORA_TAIRA -> error("closed source lane")
        }
        require(
            routeId == expectedRouteId && assetKey == "xor" && payloadAmountScale == 9,
        ) { "SCCP route identity does not match its exact deployment" }
        val sourceLaneHash = SccpV1.laneHash(lane).toUpperHex()
        val destinationLaneHash = SccpV1.laneHash(
            SccpLaneIdV1(lane.target, lane.source),
        ).toUpperHex()
        val hashRoles = mutableListOf(sourceLaneHash, destinationLaneHash).apply {
            addAll(destination.governedHashRoles)
        }
        if (destination.family == "tron") {
            hashRoles += destination.destinationBindingHash
        }
        requireDistinctRawHashes(hashRoles, "SCCP route")
        val assetRouteConfigurationHash = keccak(
            concatenate(
                listOf(
                    keccak("xor".toByteArray(Charsets.US_ASCII)),
                    keccak(routeId.toByteArray(Charsets.US_ASCII)),
                    abiWord(revision),
                    abiWord(destination.multiplier),
                    abiWord(destination.maxWrappedSupply),
                ),
            ),
        )
        return keccak(
            concatenate(
                listOf(
                    keccak(CONCRETE_ROUTE_CONFIGURATION_DOMAIN.toByteArray(Charsets.UTF_8)),
                    abiWord(lane.source.domainId.toLong()),
                    abiWord(lane.source.tag.toLong()),
                    abiWord(networkValue),
                    sourceLaneHash.hexToBytes(),
                    destinationLaneHash.hexToBytes(),
                    destination.deploymentConfigurationHash.hexToBytes(),
                    assetRouteConfigurationHash,
                ),
            ),
        ).toUpperHex()
    }

    private fun validateVerifyingKey(
        value: Map<String, Any?>,
        expectedHash: String,
        label: String,
    ) {
        exactFields(value, VERIFYING_KEY_FIELDS, label)
        requiredInt(value, "version", 1, 1)
        val words = mutableListOf<String>()
        words += parseG1(requiredObject(value, "alpha1"), "$label.alpha1")
        for (field in listOf("beta2", "gamma2", "delta2")) {
            words += parseG2(requiredObject(value, field), "$label.$field")
        }
        val ic = requiredObject(value, "ic")
        exactFields(ic, IC_FIELDS, "$label.ic")
        for (field in IC_FIELDS) words += parseG1(requiredObject(ic, field), "$label.ic.$field")
        require(words.size == 38) { "$label must contain exactly 38 ABI words" }
        val bytes = words.joinToString("").hexToBytes()
        val actual = keccak(bytes).toUpperHex()
        require(actual == expectedHash) { "$label hash does not match verifier_key_hash" }
    }

    private fun validateBls12381VerifyingKey(
        value: Map<String, Any?>,
        expectedHash: String,
        label: String,
    ) {
        exactFields(value, VERIFYING_KEY_FIELDS, label)
        requiredInt(value, "version", 1, 1)
        val points = mutableListOf<String>()
        points += upperBytesAllowZero(value, "alpha1", 48).also {
            require(isCanonicalBls12381G1(it)) { "$label.alpha1 is not canonical compressed G1" }
        }
        for (field in listOf("beta2", "gamma2", "delta2")) {
            points += upperBytesAllowZero(value, field, 96).also {
                require(isCanonicalBls12381G2(it)) { "$label.$field is not canonical compressed G2" }
            }
        }
        val ic = requiredObject(value, "ic")
        exactFields(ic, IC_FIELDS, "$label.ic")
        for (field in IC_FIELDS) {
            points += upperBytesAllowZero(ic, field, 48).also {
                require(isCanonicalBls12381G1(it)) { "$label.ic.$field is not canonical compressed G1" }
            }
        }
        val canonical = byteArrayOf(1) + points.joinToString("").hexToBytes()
        require(sha256(canonical).toUpperHex() == expectedHash) {
            "$label hash does not match verifier_key_hash"
        }
    }

    private fun isCanonicalBls12381G1(value: String): Boolean {
        val bytes = value.hexToBytes()
        if (bytes.size != 48 || bytes[0].toInt() and 0x80 == 0 || bytes[0].toInt() and 0x40 != 0) {
            return false
        }
        bytes[0] = (bytes[0].toInt() and 0x1f).toByte()
        return BigInteger(1, bytes) < BLS12381_BASE_MODULUS
    }

    private fun isCanonicalBls12381G2(value: String): Boolean {
        val bytes = value.hexToBytes()
        return bytes.size == 96 &&
            isCanonicalBls12381G1(bytes.copyOfRange(0, 48).toUpperHex()) &&
            BigInteger(1, bytes.copyOfRange(48, 96)) < BLS12381_BASE_MODULUS
    }

    private fun tonAddress(value: Map<String, Any?>, label: String): TonAddress {
        exactFields(value, setOf("workchain", "account"), label)
        val workchain = requiredInt(value, "workchain", Int.MIN_VALUE, Int.MAX_VALUE)
        val account = upperBytes(value, "account", 32)
        require(workchain == 0) { "$label must use TON basechain workchain 0" }
        return TonAddress(workchain, account)
    }

    private fun tonGuardianKeys(
        value: Map<String, Any?>,
        label: String,
    ): SccpTonMintBreakerGuardianKeysV1 {
        exactFields(
            value,
            setOf("guardian_0", "guardian_1", "guardian_2", "guardian_3", "guardian_4"),
            label,
        )
        return SccpTonMintBreakerGuardianKeysV1(
            upperBytes(value, "guardian_0", 32),
            upperBytes(value, "guardian_1", 32),
            upperBytes(value, "guardian_2", 32),
            upperBytes(value, "guardian_3", 32),
            upperBytes(value, "guardian_4", 32),
        )
    }

    private fun validateOutboundProofPolicyFields(
        value: Map<String, Any?>,
        label: String,
        expectedProfile: String,
    ): ParsedProofPolicy {
        val policy = linkedMapOf<String, Any?>(
            "version" to 1,
            "semantic_profile" to value["semantic_proof_profile"],
            "sora_finality_anchor" to value["sora_finality_anchor"],
        )
        return validateOutboundProofPolicy(policy, "$label outbound proof policy", expectedProfile)
    }

    private fun validateOutboundProofPolicy(
        value: Map<String, Any?>,
        label: String,
        expectedProfile: String? = null,
    ): ParsedProofPolicy {
        exactFields(value, setOf("version", "semantic_profile", "sora_finality_anchor"), label)
        requiredInt(value, "version", 1, 1)
        val profile = requiredObject(value, "semantic_profile")
        exactFields(profile, setOf("profile", "commitments"), "$label.semantic_profile")
        val profileName = requiredText(profile, "profile")
        require(profileName == SEMANTIC_PROFILE || profileName == TON_SEMANTIC_PROFILE) {
            "$label semantic profile is unsupported"
        }
        require(expectedProfile == null || profileName == expectedProfile) {
            "$label semantic profile does not match its destination backend"
        }
        val commitments = requiredObject(profile, "commitments")
        exactFields(
            commitments,
            setOf(
                "version",
                "circuit_commitment",
                "witness_generator_commitment",
                "public_signal_schema_hash",
            ),
            "$label.semantic_profile.commitments",
        )
        val commitmentVersion = requiredInt(commitments, "version", 1, 1)
        val circuitCommitment = upperBytes(commitments, "circuit_commitment", 32)
        val witnessGeneratorCommitment = upperBytes(commitments, "witness_generator_commitment", 32)
        val publicSignalSchemaHash = upperBytes(commitments, "public_signal_schema_hash", 32)
        val semanticRoles = listOf(
            circuitCommitment,
            witnessGeneratorCommitment,
            publicSignalSchemaHash,
        )
        val expectedSchema = if (profileName == TON_SEMANTIC_PROFILE) {
            BLS12381_PUBLIC_SIGNAL_SCHEMA_HASH
        } else {
            PUBLIC_SIGNAL_SCHEMA_HASH
        }
        require(semanticRoles[2] == expectedSchema) {
            "$label public signal schema hash does not name the eleven-signal V1 schema"
        }
        requireDistinctRawHashes(semanticRoles, "$label semantic profile")
        val profileHash = keccak(
            "sccp:semantic-proof-profile:v1".toByteArray(Charsets.UTF_8) +
                byteArrayOf(1, (if (profileName == TON_SEMANTIC_PROFILE) 1 else 0).toByte(), 1) +
                semanticRoles.joinToString("").hexToBytes(),
        ).toUpperHex()
        val anchor = requiredObject(value, "sora_finality_anchor")
        exactFields(anchor, FINALITY_ANCHOR_FIELDS, "$label.sora_finality_anchor")
        val anchorVersion = requiredInt(anchor, "version", 1, 1)
        val sourceNetwork = parseNetwork(requiredObject(anchor, "source_network"), "$label.source_network")
        require(sourceNetwork == SccpNetworkV1.SORA_TAIRA) {
            "$label anchor must name SORA Taira"
        }
        val anchorRoles = listOf(
            upperBytes(anchor, "chain_id_hash", 32),
            upperBytes(anchor, "checkpoint_block_hash", 32),
            upperBytes(anchor, "checkpoint_context_id", 32),
            upperBytes(anchor, "checkpoint_finality_artifact_hash", 32),
        )
        require(anchorRoles[0] == TAIRA_CHAIN_ID_HASH) { "$label Taira chain id hash mismatch" }
        val protocolVersion = requiredInt(anchor, "protocol_version", 4, 4)
        val checkpointHeight = requiredUnsignedInteger(anchor, "checkpoint_height", MAX_U64, true)
        requireDistinctRawHashes(anchorRoles, "$label finality anchor")
        val canonicalAnchor = ByteArrayOutputStream().also { output ->
            output.write(1)
            output.write(SccpNetworkV1.SORA_TAIRA.tag)
            writeU16(output, protocolVersion)
            output.write(anchorRoles[0].hexToBytes())
            writeU64(output, checkpointHeight)
            output.write(anchorRoles[1].hexToBytes())
            output.write(anchorRoles[2].hexToBytes())
            output.write(anchorRoles[3].hexToBytes())
        }.toByteArray()
        val anchorHash = keccak(
            "sccp:sora-finality-anchor:v1".toByteArray(Charsets.UTF_8) + canonicalAnchor,
        ).toUpperHex()
        requireDistinctRawHashes(
            semanticRoles + listOf(profileHash) + anchorRoles + listOf(anchorHash),
            "$label proof policy",
        )
        return ParsedProofPolicy(
            SccpSemanticProofProfileV1(
                profileName,
                SccpGroth16Bn254SemanticCircuitV1(
                    commitmentVersion,
                    circuitCommitment,
                    witnessGeneratorCommitment,
                    publicSignalSchemaHash,
                ),
                "0x${profileHash.lowercase()}",
            ),
            SccpSoraFinalityAnchorV1(
                anchorVersion,
                sourceNetwork,
                protocolVersion,
                anchorRoles[0],
                checkpointHeight,
                anchorRoles[1],
                anchorRoles[2],
                anchorRoles[3],
                "0x${anchorHash.lowercase()}",
            ),
        )
    }

    private fun parseG1(value: Map<String, Any?>, label: String): List<String> {
        exactFields(value, setOf("x", "y"), label)
        val words = listOf(upperBytesAllowZero(value, "x", 32), upperBytesAllowZero(value, "y", 32))
        require(words.any { it.any { char -> char != '0' } }) { "$label must not be infinity" }
        words.forEach { require(BigInteger(it, 16) < BN254_MODULUS) { "$label is outside BN254" } }
        return words
    }

    private fun parseG2(value: Map<String, Any?>, label: String): List<String> {
        val fields = listOf("x_c0", "x_c1", "y_c0", "y_c1")
        exactFields(value, fields.toSet(), label)
        val words = fields.map { upperBytesAllowZero(value, it, 32) }
        require(words.any { it.any { char -> char != '0' } }) { "$label must not be infinity" }
        words.forEach { require(BigInteger(it, 16) < BN254_MODULUS) { "$label is outside BN254" } }
        return words
    }

    private fun parseNativeTrustAnchor(
        value: Any?,
        lane: SccpLaneIdV1,
        label: String,
    ): NativeTrustAnchor {
        val anchor = objectValue(value, label)
        exactFields(anchor, setOf("backend", "anchor_hash", "checkpoint_height"), label)
        val backend = requiredObject(anchor, "backend")
        exactFields(backend, setOf("backend", "protocol"), "$label.backend")
        require(backend["protocol"] == null) { "$label.backend.protocol must be null" }
        val key = requiredText(backend, "backend")
        val allowed = when (lane.source.domainId) {
            1 -> "ethereum_beacon_v1"
            2 -> "bsc_parlia_v1"
            4 -> "ton_masterchain_v1"
            3 -> "tron_dpos_v1"
            else -> error("closed lane")
        }
        require(key == allowed) { "$label backend does not match lane source" }
        return NativeTrustAnchor(
            key,
            upperBytes(anchor, "anchor_hash", 32),
            requiredUnsignedInteger(anchor, "checkpoint_height", MAX_U64, true),
        )
    }

    private fun validateTransferPayload(value: Map<String, Any?>, lane: SccpLaneIdV1) {
        exactFields(value, setOf("Transfer"), "SCCP payload")
        val transfer = requiredObject(value, "Transfer")
        exactFields(transfer, TRANSFER_FIELDS, "SCCP transfer")
        requiredInt(transfer, "version", 1, 1)
        require(requiredDomain(transfer, "source_domain") == lane.source.domainId)
        require(requiredDomain(transfer, "dest_domain") == lane.target.domainId)
        require(BigInteger(requiredDecimal(transfer, "nonce", false)).bitLength() <= 64) {
            "SCCP transfer nonce must fit u64"
        }
        requiredLong(transfer, "route_revision", 1, 0xffff_ffffL)
        requiredDomain(transfer, "asset_home_domain")
        validateCodec(transfer, "asset_id_codec", "asset_id", null)
        require(BigInteger(requiredDecimal(transfer, "amount", true)).bitLength() <= 128) {
            "SCCP transfer amount must fit u128"
        }
        validateCodec(transfer, "sender_codec", "sender", lane.source.domainId)
        validateCodec(transfer, "recipient_codec", "recipient", lane.target.domainId)
        validateCodec(transfer, "route_id_codec", "route_id", null)
    }

    private fun validateCodec(
        value: Map<String, Any?>,
        codecField: String,
        bytesField: String,
        domain: Int?,
    ) {
        val codec = requiredInt(value, codecField, 0, 3)
        require(codec in 0..3) {
            "$codecField is unsupported or retired"
        }
        if (domain != null) {
            val expected = when (domain) {
                0 -> 0
                1, 2 -> 1
                3 -> 2
                4 -> 3
                else -> throw IllegalArgumentException("unsupported SCCP domain")
            }
            require(codec == expected) { "$codecField does not match its domain" }
        }
        val bytes = hexBytes(value, bytesField)
        when (codec) {
            0 -> require(bytes.isNotEmpty() && bytes.size <= 256 && bytes.all { (it.toInt() and 0xff) in 0x21..0x7e })
            1 -> require(bytes.size == 20 && bytes.any { it.toInt() != 0 })
            2 -> require(bytes.size == 21 && bytes[0] == 0x41.toByte() && bytes.drop(1).any { it.toInt() != 0 })
            3 -> require(
                bytes.size == 36 && bytes.take(4).all { it == 0.toByte() } &&
                    bytes.drop(4).any { it != 0.toByte() },
            )
        }
    }

    private fun parseRecent(value: Map<String, Any?>, index: Int): SccpRecentMessage {
        val label = "SCCP recent messages.items[$index]"
        exactFields(value, RECENT_FIELDS, label, RECENT_REQUIRED)
        require(requiredText(value, "kind") == "transfer") { "$label kind is retired" }
        val source = exactProfile(value, "source_profile")
        val target = exactProfile(value, "target_profile")
        val lane = SccpLaneIdV1(source, target)
        require(lane.isOutbound && source == SccpNetworkV1.SORA_TAIRA) {
            "$label must use a Taira-to-external lane"
        }
        require(requiredInt(value, "target_domain", 1, 4) == target.domainId) {
            "$label target profile/domain mismatch"
        }
        val messageId = lowerHash(value, "message_id_hex")
        val binding = prefixedHash(value, "destination_binding_hash")
        val configuration = prefixedHash(value, "route_configuration_hash")
        requireDistinctHashes(listOf(messageId, binding, configuration), label)
        val amount = requiredDecimal(value, "amount", true)
        require(BigInteger(amount).bitLength() <= 128) { "$label amount must fit u128" }
        val links = requiredObject(value, "links")
        exactFields(links, setOf("bundle_path", "proof_request_path"), "$label.links")
        val bundlePath = requiredText(links, "bundle_path")
        val requestPath = requiredText(links, "proof_request_path")
        require(bundlePath == "/v1/sccp/proofs/message/$messageId") { "$label bundle link mismatch" }
        require(requestPath == "/v1/sccp/proof-requests/$messageId") { "$label proof-request link mismatch" }
        val assetId = optionalText(value, "asset_id")
        val routeId = optionalText(value, "route_id")
        val recipient = optionalText(value, "recipient")
        val projection = validateRecentPayloadProjection(
            requiredObject(value, "payload_projection"),
            lane,
            amount,
            assetId,
            routeId,
            "$label.payload_projection",
        )
        return SccpRecentMessage(
            requiredUnsignedInteger(value, "height", MAX_U64, true),
            requiredInt(
                value,
                "commitment_index",
                0,
                SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 - 1,
            ),
            messageId,
            source.profileKey,
            target.profileKey,
            binding,
            configuration,
            target.domainId,
            assetId,
            routeId,
            recipient,
            amount,
            projection,
            SccpRecentMessageLinks(bundlePath, requestPath),
        )
    }

    private fun validateRecentPayloadProjection(
        value: Map<String, Any?>,
        lane: SccpLaneIdV1,
        summaryAmount: String,
        summaryAssetId: String?,
        summaryRouteId: String?,
        label: String,
    ): Map<String, Any?> {
        exactFields(value, setOf("Transfer"), label)
        val transfer = requiredObject(value, "Transfer")
        exactFields(transfer, PROJECTION_TRANSFER_FIELDS, "$label.Transfer")
        requiredInt(transfer, "version", 1, 1)
        requiredInt(transfer, "source_domain", 0, 0)
        require(requiredInt(transfer, "dest_domain", 1, 4) == lane.target.domainId) {
            "$label.Transfer.dest_domain does not match the target network"
        }
        requiredUnsignedInteger(transfer, "nonce", MAX_U64, false)
        requiredLong(transfer, "route_revision", 1, 0xffff_ffffL)
        requiredInt(transfer, "asset_home_domain", 0, 0)
        val assetId = projectionCanonicalText(
            requiredObject(transfer, "asset_id"),
            "$label.Transfer.asset_id",
            "xor",
        )
        require(summaryAssetId == null || summaryAssetId == assetId) {
            "$label.Transfer.asset_id does not match the recent-message summary"
        }
        val amount = requiredUnsignedInteger(transfer, "amount", MAX_U128, true)
        require(amount.toString() == summaryAmount) {
            "$label.Transfer.amount does not match the recent-message summary"
        }
        projectionCanonicalText(
            requiredObject(transfer, "sender"),
            "$label.Transfer.sender",
            null,
        )
        validateProjectionRecipient(
            requiredObject(transfer, "recipient"),
            lane.target,
            "$label.Transfer.recipient",
        )
        val expectedRouteId = when (lane.target.domainId) {
            1 -> "taira_eth_xor"
            2 -> "taira_bsc_xor"
            4 -> "taira_ton_xor"
            3 -> "taira_tron_xor"
            else -> error("closed SCCP destination")
        }
        val routeId = projectionCanonicalText(
            requiredObject(transfer, "route_id"),
            "$label.Transfer.route_id",
            expectedRouteId,
        )
        require(summaryRouteId == null || summaryRouteId == routeId) {
            "$label.Transfer.route_id does not match the recent-message summary"
        }
        return deepCopyObject(value)
    }

    private fun projectionCanonicalText(
        value: Map<String, Any?>,
        label: String,
        expected: String?,
    ): String {
        exactFields(value, setOf("CanonicalText"), label)
        val inner = requiredObject(value, "CanonicalText")
        exactFields(inner, setOf("value"), "$label.CanonicalText")
        val text = requiredText(inner, "value")
        require(text.length <= 512) { "$label.CanonicalText.value exceeds 512 characters" }
        require(expected == null || text == expected) { "$label names an unsupported value" }
        return text
    }

    private fun validateProjectionRecipient(
        value: Map<String, Any?>,
        target: SccpNetworkV1,
        label: String,
    ) {
        if (target.domainId == 4) {
            exactFields(value, setOf("TonAccount36"), label)
            val inner = requiredObject(value, "TonAccount36")
            exactFields(inner, setOf("workchain", "account"), "$label.TonAccount36")
            require(requiredInt(inner, "workchain", Int.MIN_VALUE, Int.MAX_VALUE) == 0) {
                "$label TON recipient must use basechain workchain 0"
            }
            val account = requiredText(inner, "account")
            require(Regex("0x[0-9a-f]{64}").matches(account) && account.drop(2).any { it != '0' }) {
                "$label does not contain a canonical nonzero TonAccount36"
            }
            return
        }
        val variant = if (target.domainId == 3) "TronAddress21" else "EvmAddress20"
        exactFields(value, setOf(variant), label)
        val inner = requiredObject(value, variant)
        exactFields(inner, setOf("bytes"), "$label.$variant")
        val bytes = requiredText(inner, "bytes")
        val expectedLength = if (variant == "TronAddress21") 42 else 40
        val addressPayload = if (variant == "TronAddress21") bytes.drop(4) else bytes.drop(2)
        require(
            Regex("0x[0-9a-f]{$expectedLength}").matches(bytes) &&
                addressPayload.any { it != '0' } &&
                (variant != "TronAddress21" || bytes.startsWith("0x41")),
        ) { "$label does not contain a canonical nonzero $variant" }
    }

    private fun parseInboundLane(value: Map<String, Any?>, label: String): SccpLaneIdV1 =
        parseLane(value, label).also {
            require(it.isInbound && it.target == SccpNetworkV1.SORA_TAIRA) {
                "$label must be an exact supported external-to-Taira lane"
            }
        }

    private fun parseLane(value: Map<String, Any?>, label: String): SccpLaneIdV1 {
        exactFields(value, setOf("source", "target"), label)
        return SccpLaneIdV1(
            parseNetwork(requiredObject(value, "source"), "$label.source"),
            parseNetwork(requiredObject(value, "target"), "$label.target"),
        )
    }

    private fun parseNetwork(value: Map<String, Any?>, label: String): SccpNetworkV1 {
        exactFields(value, setOf("network", "profile"), label)
        require(value["profile"] == null) { "$label.profile must be null" }
        val wireName = requiredText(value, "network")
        return SccpNetworkV1.values().singleOrNull {
            it.profileKey.replace('-', '_') == wireName
        }
            ?: throw IllegalArgumentException("$label is unsupported or retired")
    }

    private fun exactProfile(value: Map<String, Any?>, field: String): SccpNetworkV1 {
        val profile = requiredText(value, field)
        return SccpNetworkV1.fromProfileKey(profile)
            ?: throw IllegalArgumentException("$field is unsupported or retired")
    }

    private fun familyFor(network: SccpNetworkV1): String =
        when (network.domainId) {
            4 -> "ton"
            3 -> "tron"
            else -> "evm"
        }

    private fun canonicalRouteKey(value: Map<String, Any?>, field: String): String =
        requiredText(value, field).also {
            require(Regex("[a-z0-9](?:[a-z0-9_-]{0,62}[a-z0-9])?").matches(it)) {
                "$field must be canonical lowercase route text"
            }
        }

    @Suppress("UNCHECKED_CAST")
    private fun rootObject(bytes: ByteArray, label: String): Map<String, Any?> {
        val text = String(bytes, Charsets.UTF_8)
        require(text.toByteArray(Charsets.UTF_8).contentEquals(bytes)) { "$label must be UTF-8 JSON" }
        val root = JsonParser.parse(text)
        require(root is Map<*, *> && root.keys.all { it is String }) { "$label must be an object" }
        return root as Map<String, Any?>
    }

    @Suppress("UNCHECKED_CAST")
    private fun objectValue(value: Any?, label: String): Map<String, Any?> {
        require(value is Map<*, *> && value.keys.all { it is String }) { "$label must be an object" }
        return value as Map<String, Any?>
    }

    private fun requiredObject(value: Map<String, Any?>, field: String): Map<String, Any?> =
        objectValue(value[field], field)

    private fun optionalObject(value: Map<String, Any?>, field: String): Map<String, Any?>? =
        value[field]?.let { objectValue(it, field) }

    private fun requiredList(value: Map<String, Any?>, field: String): List<Any?> =
        value[field] as? List<Any?> ?: throw IllegalArgumentException("$field must be an array")

    private fun exactFields(
        value: Map<String, Any?>,
        allowed: Set<String>,
        label: String,
        required: Set<String> = allowed,
    ) {
        val unknown = value.keys.firstOrNull { it !in allowed }
        require(unknown == null) { "$label contains unknown or retired field $unknown" }
        val missing = required.firstOrNull { it !in value }
        require(missing == null) { "$label is missing required field $missing" }
    }

    private fun requiredText(value: Map<String, Any?>, field: String): String {
        val text = value[field] as? String ?: throw IllegalArgumentException("$field must be a string")
        require(text.isNotBlank() && text == text.trim()) { "$field must be canonical text" }
        return text
    }

    private fun optionalText(value: Map<String, Any?>, field: String): String? =
        if (value[field] == null) null else requiredText(value, field)

    private fun requiredBoolean(value: Map<String, Any?>, field: String): Boolean =
        value[field] as? Boolean ?: throw IllegalArgumentException("$field must be boolean")

    private fun requiredLong(
        value: Map<String, Any?>,
        field: String,
        minimum: Long,
        maximum: Long,
    ): Long {
        val number = value[field] as? Number ?: throw IllegalArgumentException("$field must be an integer")
        val result = number.toLong()
        require(number.toString() == result.toString() && result in minimum..maximum) {
            "$field is out of range"
        }
        return result
    }

    private fun requiredInt(
        value: Map<String, Any?>,
        field: String,
        minimum: Int,
        maximum: Int,
    ): Int = requiredLong(value, field, minimum.toLong(), maximum.toLong()).toInt()

    private fun requiredUnsignedInteger(
        value: Map<String, Any?>,
        field: String,
        maximum: BigInteger,
        positive: Boolean,
    ): BigInteger {
        val number = value[field] as? Number ?: throw IllegalArgumentException("$field must be an integer")
        require(number !is java.math.BigDecimal) {
            "$field must be a canonical unsigned integer"
        }
        val text = number.toString()
        require(Regex(if (positive) "[1-9][0-9]*" else "0|[1-9][0-9]*").matches(text)) {
            "$field must be a canonical unsigned integer"
        }
        return BigInteger(text).also {
            require(it <= maximum) { "$field is out of range" }
        }
    }

    private fun requiredDomain(value: Map<String, Any?>, field: String): Int =
        requiredInt(value, field, 0, 5).also {
            require(it == 0 || it == 1 || it == 2 || it == 4 || it == 5) {
                "$field is an unsupported or retired SCCP domain"
            }
        }

    private fun requiredDecimal(value: Map<String, Any?>, field: String, positive: Boolean): String {
        val text = value[field] as? String ?: throw IllegalArgumentException("$field must be a decimal string")
        require(Regex(if (positive) "[1-9][0-9]*" else "0|[1-9][0-9]*").matches(text)) {
            "$field must be a canonical unsigned decimal string"
        }
        return text
    }

    private fun lowerHash(value: Map<String, Any?>, field: String): String =
        requiredText(value, field).also {
            require(Regex("[0-9a-f]{64}").matches(it) && it.any { char -> char != '0' }) {
                "$field must be canonical lowercase nonzero 32-byte hex"
            }
        }

    private fun prefixedHash(
        value: Map<String, Any?>,
        field: String,
        allowZero: Boolean = false,
    ): String =
        requiredText(value, field).also {
            require(
                Regex("0x[0-9a-f]{64}").matches(it) &&
                    (allowZero || it.drop(2).any { char -> char != '0' }),
            ) {
                "$field must be canonical lowercase 0x-prefixed 32-byte hex"
            }
        }

    private fun upperBytes(value: Map<String, Any?>, field: String, bytes: Int): String =
        requiredText(value, field).also {
            require(Regex("[0-9A-F]{${bytes * 2}}").matches(it) && it.any { char -> char != '0' }) {
                "$field must be canonical uppercase nonzero $bytes-byte hex"
            }
        }

    private fun upperBytesAllowZero(
        value: Map<String, Any?>,
        field: String,
        bytes: Int,
    ): String = requiredText(value, field).also {
        require(Regex("[0-9A-F]{${bytes * 2}}").matches(it)) {
            "$field must be canonical uppercase $bytes-byte hex"
        }
    }

    private fun requireHexBytes(value: Map<String, Any?>, field: String, allowEmpty: Boolean): String {
        val text = requiredText(value, field)
        require(text.startsWith("0x") && text.length % 2 == 0 && text.drop(2).all { it in '0'..'9' || it in 'a'..'f' }) {
            "$field must be canonical lowercase 0x-prefixed bytes"
        }
        require(allowEmpty || text.length > 2) { "$field must not be empty" }
        require((text.length - 2) / 2 <= MAX_VARIABLE_BYTES) { "$field exceeds its size bound" }
        return text
    }

    private fun hexBytes(value: Map<String, Any?>, field: String): ByteArray =
        requireHexBytes(value, field, false).removePrefix("0x").hexToBytes()

    private fun optionalExactPath(
        value: Map<String, Any?>,
        field: String,
        expected: String,
        optional: Boolean,
    ): String? {
        if (value[field] == null) {
            require(optional) { "$field is required" }
            return null
        }
        return requiredText(value, field).also {
            require(it == expected) { "$field must equal $expected" }
        }
    }

    private fun requireDistinctHashes(values: List<String>, label: String) {
        val normalized = values.map { it.removePrefix("0x") }
        require(normalized.distinct().size == normalized.size) { "$label hash roles must be distinct" }
    }

    private fun requireDistinctRawHashes(values: List<String>, label: String) {
        require(
            values.all { value -> value.any { it != '0' } } &&
                values.distinct().size == values.size,
        ) { "$label hash roles must be nonzero and distinct" }
    }

    private fun deepCopyObject(value: Map<String, Any?>): Map<String, Any?> =
        Collections.unmodifiableMap(
            value.entries.associateTo(linkedMapOf()) { (key, entry) -> key to deepCopy(entry) },
        )

    private fun deepCopy(value: Any?): Any? = when (value) {
        is Map<*, *> -> value.entries.associate { (key, entry) -> key as String to deepCopy(entry) }
            .let(Collections::unmodifiableMap)
        is List<*> -> Collections.unmodifiableList(value.map(::deepCopy))
        else -> value
    }

    private fun String.hexToBytes(): ByteArray = ByteArray(length / 2) { index ->
        substring(index * 2, index * 2 + 2).toInt(16).toByte()
    }

    private fun ByteArray.toUpperHex(): String = joinToString("") {
        "%02X".format(it.toInt() and 0xff)
    }

    private fun keccak(bytes: ByteArray): ByteArray {
        val digest = KeccakDigest(256)
        digest.update(bytes, 0, bytes.size)
        return ByteArray(32).also { digest.doFinal(it, 0) }
    }

    private fun sha256(bytes: ByteArray): ByteArray =
        MessageDigest.getInstance("SHA-256").digest(bytes)

    private fun concatenate(values: List<ByteArray>): ByteArray =
        ByteArrayOutputStream(values.sumOf { it.size }).also { output ->
            values.forEach { output.write(it) }
        }.toByteArray()

    private fun abiWord(value: Long): ByteArray {
        require(value >= 0) { "ABI word value must be unsigned" }
        return abiWord(BigInteger.valueOf(value))
    }

    private fun abiWord(value: BigInteger): ByteArray {
        require(value.signum() >= 0) { "ABI word value must be unsigned" }
        val encoded = value.toByteArray().let {
            if (it.size > 1 && it[0] == 0.toByte()) it.copyOfRange(1, it.size) else it
        }
        require(encoded.size <= 32) { "ABI word value exceeds 256 bits" }
        return ByteArray(32).also { encoded.copyInto(it, 32 - encoded.size) }
    }

    private fun abiAddress(value: String): ByteArray = ByteArray(12) + value.hexToBytes()

    private fun abiTronAddress(value: String): ByteArray =
        ByteArray(11) + byteArrayOf(0x41) + value.hexToBytes()

    private fun publicSignalSchemaHash(): String {
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32(out, PUBLIC_SIGNAL_LABELS.size)
        PUBLIC_SIGNAL_LABELS.forEach { label ->
            val bytes = label.toByteArray(Charsets.UTF_8)
            writeU32(out, bytes.size)
            out.write(bytes)
        }
        return keccak(
            "sccp:groth16-bn254:public-signal-schema:v1".toByteArray(Charsets.UTF_8) +
                out.toByteArray(),
        ).toUpperHex()
    }

    private fun bls12381PublicSignalSchemaHash(): String {
        val out = ByteArrayOutputStream()
        out.write(1)
        writeU32(out, BLS12381_PUBLIC_SIGNAL_LABELS.size)
        BLS12381_PUBLIC_SIGNAL_LABELS.forEach { label ->
            writeBytes(out, label.toByteArray(Charsets.UTF_8))
        }
        return sha256(
            "sccp:groth16-bls12381:public-signal-schema:v1".toByteArray(Charsets.UTF_8) +
                out.toByteArray(),
        ).toUpperHex()
    }

    private fun tonProofProfileCommitment(): ByteArray = sha256(
        "sccp:ton:groth16-bls12381:proof-profile:v1".toByteArray(Charsets.UTF_8) +
            byteArrayOf(1) +
            "ietf-bls12381-compressed-g1-48-g2-96".toByteArray(Charsets.US_ASCII) +
            "groth16-a-g1-b-g2-c-g1".toByteArray(Charsets.US_ASCII) +
            "sha256-sha256-label-value-mod-r".toByteArray(Charsets.US_ASCII) +
            BLS12381_SCALAR_MODULUS.toFixedUnsigned(32) +
            BLS12381_PUBLIC_SIGNAL_SCHEMA_HASH.hexToBytes(),
    )

    private fun validateTonPublicSignals(
        value: Map<String, Any?>,
        inputWords: List<ByteArray>,
    ): Map<String, String> {
        require(inputWords.size == BLS12381_PUBLIC_SIGNAL_FIELDS.size)
        exactFields(value, BLS12381_PUBLIC_SIGNAL_FIELDS.toSet(), "SCCP TON public signals")
        val parsed = BLS12381_PUBLIC_SIGNAL_FIELDS.associateWith {
            prefixedHash(value, it, allowZero = true)
        }
        val expected = BLS12381_PUBLIC_SIGNAL_LABELS.zip(inputWords).map { (label, input) ->
            val labelHash = sha256(label.toByteArray(Charsets.UTF_8))
            val scalar = BigInteger(1, sha256(labelHash + input)).mod(BLS12381_SCALAR_MODULUS)
            "0x${scalar.toFixedUnsigned(32).toUpperHex().lowercase()}"
        }
        require(BLS12381_PUBLIC_SIGNAL_FIELDS.map { parsed.getValue(it) } == expected) {
            "SCCP TON public signals do not match their exact request roles"
        }
        return Collections.unmodifiableMap(parsed)
    }

    private fun BigInteger.toFixedUnsigned(size: Int): ByteArray {
        val source = toByteArray().let {
            if (it.size > 1 && it[0] == 0.toByte()) it.copyOfRange(1, it.size) else it
        }
        require(source.size <= size)
        return ByteArray(size).also { source.copyInto(it, size - source.size) }
    }

    private fun writeU32(out: ByteArrayOutputStream, value: Int) {
        repeat(4) { shift -> out.write((value ushr (shift * 8)) and 0xff) }
    }

    private fun writeI32(out: ByteArrayOutputStream, value: Int) = writeU32(out, value)

    private fun writeBytes(out: ByteArrayOutputStream, value: ByteArray) {
        writeU32(out, value.size)
        out.write(value)
    }

    private fun writeU64(out: ByteArrayOutputStream, value: BigInteger) {
        repeat(8) { shift ->
            out.write(value.shiftRight(shift * 8).and(BigInteger.valueOf(0xff)).toInt())
        }
    }

    private fun writeU128(out: ByteArrayOutputStream, value: BigInteger) {
        repeat(16) { shift ->
            out.write(value.shiftRight(shift * 8).and(BigInteger.valueOf(0xff)).toInt())
        }
    }

    private fun writeU16(out: ByteArrayOutputStream, value: Int) {
        repeat(2) { shift -> out.write((value ushr (shift * 8)) and 0xff) }
    }

    private val CAPABILITY_FIELDS = setOf(
        "version",
        "registry_revision",
        "registry_path",
        "message_bundle_path",
        "proof_request_path",
        "recent_messages_path",
        "sora_outbound_material_path",
        "registry_limits",
        "resource_limits",
        "proof_submit_path",
        "native_message_submit_path",
    )
    private val CAPABILITY_REQUIRED = setOf(
        "version",
        "registry_revision",
        "registry_path",
        "message_bundle_path",
        "proof_request_path",
        "recent_messages_path",
        "sora_outbound_material_path",
        "registry_limits",
        "resource_limits",
    )
    private val REGISTRY_LIMIT_FIELDS = setOf(
        "max_governed_lanes",
        "max_live_governed_routes",
        "max_live_routes_per_lane",
        "max_retained_routes_per_lane",
        "max_retained_native_trust_anchors_per_lane",
    )
    private val RESOURCE_LIMIT_FIELDS = setOf(
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
        "max_ed25519_signature_checks_per_transaction",
        "max_ed25519_signature_checks_per_block",
        "max_ed25519_validator_key_checks_per_transaction",
        "max_ed25519_validator_key_checks_per_block",
        "max_bn254_pairing_checks_per_transaction",
        "max_bn254_pairing_checks_per_block",
        "max_bls12_381_pairing_checks_per_transaction",
        "max_bls12_381_pairing_checks_per_block",
    )
    private val ROUTE_FIELDS = setOf(
        "lane_id",
        "route_id",
        "asset_key",
        "revision",
        "activation",
        "inbound_finality_cutoff",
        "source_identity",
        "destination",
        "sora_outbound_execution_policy",
        "settlement",
    )
    private val DESTINATION_FIELDS = setOf(
        "token_address",
        "token_code_hash",
        "verifier_address",
        "verifier_code_hash",
        "verifying_key",
        "verifier_key_hash",
        "outbound_proof_policy",
        "route_address",
        "route_code_hash",
        "replay_verifier_address",
        "replay_verifier_code_hash",
        "mint_breaker_address",
        "mint_breaker_code_hash",
        "taira_to_token_multiplier",
        "max_wrapped_supply",
    )
    private val TON_DESTINATION_FIELDS = setOf(
        "jetton_master_address",
        "jetton_master_code_hash",
        "jetton_master_initial_data_hash",
        "jetton_wallet_code_hash",
        "route_address",
        "route_code_hash",
        "route_initial_data_hash",
        "embedded_verifier_code_hash",
        "verifier_circuit_hash",
        "verifying_key",
        "verifier_key_hash",
        "proof_profile_commitment",
        "mint_breaker_guardian_keys",
        "outbound_proof_policy",
        "taira_to_token_multiplier",
        "max_wrapped_supply",
    )
    private val VERIFYING_KEY_FIELDS = setOf("version", "alpha1", "beta2", "gamma2", "delta2", "ic")
    private val IC_FIELDS = linkedSetOf(
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
        "signal_10",
    )
    private val FINALITY_ANCHOR_FIELDS = setOf(
        "version",
        "source_network",
        "protocol_version",
        "chain_id_hash",
        "checkpoint_height",
        "checkpoint_block_hash",
        "checkpoint_context_id",
        "checkpoint_finality_artifact_hash",
    )
    private val PROOF_REQUEST_FIELDS = setOf(
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
        "request_hash",
    )
    private val TON_PROOF_REQUEST_FIELDS = setOf(
        "public_signals",
        "verifier_circuit_hash",
        "proof_profile_commitment",
    )
    private val TRANSFER_FIELDS = setOf(
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
        "route_id",
    )
    private val PROJECTION_TRANSFER_FIELDS = setOf(
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
        "route_id",
    )
    private val RECENT_FIELDS = setOf(
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
        "links",
    )
    private val RECENT_REQUIRED = setOf(
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
        "links",
    )
    private val ACTIVATIONS = setOf("staged", "bidirectional", "inbound_only", "paused", "retired")
    private const val SEMANTIC_PROFILE = "sora_taira_finality_inclusion_groth16_bn254"
    private const val TON_SEMANTIC_PROFILE =
        "sora_taira_finality_inclusion_groth16_bls12381"
    private const val EVM_DESTINATION_BINDING_DOMAIN =
        "iroha:sccp:evm-destination-binding:v1"
    private const val TRON_DESTINATION_BINDING_DOMAIN =
        "iroha:sccp:tron-destination-binding:v1"
    private const val TON_DESTINATION_BINDING_DOMAIN =
        "iroha:sccp:ton-destination-binding:v1"
    private const val EVM_GROTH16_BACKEND = "evm-groth16-bn254-v1"
    private const val TRON_GROTH16_BACKEND = "tron-groth16-bn254-v1"
    private const val TON_GROTH16_BACKEND = "ton-groth16-bls12381-v1"
    private const val CONCRETE_ROUTE_CONFIGURATION_DOMAIN = "sccp:concrete-route-config:v1"
    private const val TAIRA_XOR_ASSET_ID = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
    private const val SORA_OUTBOUND_EXECUTION_SEMANTICS =
        "ivm_proved_record_sccp_message_v1"
    private val BN254_MODULUS = BigInteger(
        "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47",
        16,
    )
    private val BLS12381_BASE_MODULUS = BigInteger(
        "1a0111ea397fe69a4b1ba7b6434bacd764774b84f38512bf6730d2a0f6b0f6241eabfffeb153ffffb9feffffffffaaab",
        16,
    )
    private val BLS12381_SCALAR_MODULUS = BigInteger(
        "73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001",
        16,
    )
    private val MAX_U32 = BigInteger.ONE.shiftLeft(32).subtract(BigInteger.ONE)
    private val MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private val MAX_JSON_SAFE_INTEGER = BigInteger("9007199254740991")
    private val MAX_U128 = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE)
    private val MAX_TON_COINS = BigInteger.ONE.shiftLeft(120).subtract(BigInteger.ONE)
    private const val KECCAK256_EMPTY_BYTES =
        "C5D2460186F7233C927E7DB2DCC703C0E500B653CA82273B7BFAD8045D85A470"
    private val TAIRA_CHAIN_ID_HASH = keccak(
        byteArrayOf(
            0xfc.toByte(), 0x56, 0x98.toByte(), 0x4b, 0x2b, 0xe7.toByte(), 0x43, 0x1d,
            0x84.toByte(), 0x0e, 0x21, 0x51, 0x4d, 0x18, 0x83.toByte(), 0xf0.toByte(),
        ),
    ).toUpperHex()
    private val PUBLIC_SIGNAL_LABELS = listOf(
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
        "sccp:groth16-bn254:signal:sora-finality-anchor-hash:v1",
    )
    private val PUBLIC_SIGNAL_SCHEMA_HASH = publicSignalSchemaHash()
    private val BLS12381_PUBLIC_SIGNAL_LABELS = listOf(
        "sccp:groth16-bls12381:signal:message-id:v1",
        "sccp:groth16-bls12381:signal:payload-hash:v1",
        "sccp:groth16-bls12381:signal:target-domain:v1",
        "sccp:groth16-bls12381:signal:commitment-root:v1",
        "sccp:groth16-bls12381:signal:finality-height:v1",
        "sccp:groth16-bls12381:signal:finality-block-hash:v1",
        "sccp:groth16-bls12381:signal:source-domain:v1",
        "sccp:groth16-bls12381:signal:statement-hash:v1",
        "sccp:groth16-bls12381:signal:destination-binding-hash:v1",
        "sccp:groth16-bls12381:signal:route-config-hash:v1",
        "sccp:groth16-bls12381:signal:sora-finality-anchor-hash:v1",
    )
    private val BLS12381_PUBLIC_SIGNAL_FIELDS = listOf(
        "message_id",
        "payload_hash",
        "target_domain",
        "commitment_root",
        "finality_height",
        "finality_block_hash",
        "source_domain",
        "statement_hash",
        "destination_binding_hash",
        "route_configuration_hash",
        "sora_finality_anchor_hash",
    )
    private val BLS12381_PUBLIC_SIGNAL_SCHEMA_HASH = bls12381PublicSignalSchemaHash()
    private const val MAX_VARIABLE_BYTES = 16 * 1024 * 1024
}
