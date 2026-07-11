package org.hyperledger.iroha.sdk.client

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.util.Collections
import org.bouncycastle.crypto.digests.KeccakDigest
import org.hyperledger.iroha.sdk.sccp.SccpLaneIdV1
import org.hyperledger.iroha.sdk.sccp.SccpNetworkV1
import org.hyperledger.iroha.sdk.sccp.SccpV1

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
    val maxBn254PairingChecksPerTransaction: Long,
    val maxBn254PairingChecksPerBlock: Long,
)

/** Closed first-release SCCP HTTP surface. */
data class SccpCapabilities(
    val version: Int,
    val registryRevision: String,
    val registryPath: String,
    val messageBundlePath: String,
    val proofRequestPath: String,
    val recentMessagesPath: String,
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
    val chainIdHash: String,
    val checkpointHeight: BigInteger,
    val checkpointBlockHash: String,
    val validatorSetEpoch: BigInteger,
    val validatorSetHash: String,
    val validatorSetHashVersion: Int,
    val anchorHash: String,
)

/** Inclusive authenticated-height cutoff retained for one retired route revision. */
data class SccpInboundFinalityCutoffV1(
    val trustAnchorHash: String,
    val maxAnchorIntervalHeight: BigInteger,
)

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
    val semanticProofProfile: SccpSemanticProofProfileV1,
    val soraFinalityAnchor: SccpSoraFinalityAnchorV1,
    val raw: Map<String, Any?>,
)

data class SccpRecentMessageLinks(val bundlePath: String, val proofRequestPath: String)

data class SccpRecentMessage(
    val height: Long,
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

data class SccpRecentMessages(val items: List<SccpRecentMessage>)

/** Strict decoders for the closed first-release SCCP JSON API. */
object SccpJsonParser {
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
            u32("max_bn254_pairing_checks_per_transaction"),
            u32("max_bn254_pairing_checks_per_block"),
        )
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
            BigInteger.valueOf(result.maxBn254PairingChecksPerTransaction) to
                BigInteger.valueOf(result.maxBn254PairingChecksPerBlock),
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
        exactFields(root, PROOF_REQUEST_FIELDS, "SCCP proof request")
        requiredInt(root, "version", 1, 1)
        val backendObject = requiredObject(root, "backend")
        exactFields(backendObject, setOf("backend", "family"), "SCCP proof backend")
        require(backendObject["family"] == null) { "SCCP proof backend family must be null" }
        val backend = requiredText(backendObject, "backend")
        require(backend == "evm_groth16_bn254_v1" || backend == "tron_groth16_bn254_v1") {
            "SCCP proof backend is unsupported or retired"
        }
        val source = parseNetwork(requiredObject(root, "source_network"), "source_network")
        val target = parseNetwork(requiredObject(root, "target_network"), "target_network")
        require(source == SccpNetworkV1.SORA_TAIRA && target.isExternal) {
            "SCCP proof request must use an exact Taira-to-external lane"
        }
        require((backend.startsWith("tron")) == (target.domainId == 5)) {
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
        require(requiredInt(publicInputs, "target_domain", 1, 5) == target.domainId) {
            "SCCP proof target domain does not match target network"
        }
        val commitmentRoot = prefixedHash(publicInputs, "commitment_root")
        require(BigInteger(requiredDecimal(publicInputs, "finality_height", true)).bitLength() <= 64) {
            "SCCP proof finality height must fit u64"
        }
        val finalityBlockHash = prefixedHash(publicInputs, "finality_block_hash")
        val verifierKeyHash = prefixedHash(root, "verifier_key_hash")
        validateVerifyingKey(
            requiredObject(root, "verifying_key"),
            verifierKeyHash.removePrefix("0x").uppercase(),
            "SCCP proof verifying key",
        )
        val policyHashes = validateOutboundProofPolicyFields(root, "SCCP proof request")
        val semanticHash = prefixedHash(root, "semantic_proof_profile_hash")
        require(semanticHash == "0x${policyHashes.profileHash.lowercase()}") {
            "semantic_proof_profile_hash does not match its typed profile"
        }
        val anchorHash = prefixedHash(root, "sora_finality_anchor_hash")
        require(anchorHash == "0x${policyHashes.anchorHash.lowercase()}") {
            "sora_finality_anchor_hash does not match its typed anchor"
        }
        requireHexBytes(root, "bundle_bytes", false)
        val roles = listOf(
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
            prefixedHash(root, "request_hash"),
        )
        requireDistinctHashes(roles, "proof request")
        return SccpGroth16ProofRequestV1(
            1,
            backend,
            source,
            target,
            messageId.removePrefix("0x"),
            roles.last(),
            policyHashes.semanticProfile,
            policyHashes.soraFinalityAnchor,
            deepCopyObject(root),
        )
    }

    @JvmStatic fun parseRecentMessages(bytes: ByteArray): SccpRecentMessages {
        val root = rootObject(bytes, "SCCP recent messages")
        exactFields(root, setOf("items"), "SCCP recent messages")
        val values = requiredList(root, "items")
        require(values.size <= 50) { "SCCP recent response exceeds 50 items" }
        val items = values.mapIndexed { index, raw ->
            parseRecent(objectValue(raw, "items[$index]"), index)
        }
        require(items.zipWithNext().all { (left, right) -> left.height >= right.height }) {
            "SCCP recent messages must be newest-first"
        }
        require(items.map(SccpRecentMessage::messageIdHex).distinct().size == items.size) {
            "SCCP recent messages contain duplicate message ids"
        }
        return SccpRecentMessages(items)
    }

    private data class ParsedRoute(
        val lineage: String,
        val key: String,
        val revision: Long,
        val activation: String,
        val inboundFinalityCutoff: SccpInboundFinalityCutoffV1?,
        val destinationBindingHash: String,
        val routeConfigurationHash: String,
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
        val destinationBindingHash: String,
        val deploymentConfigurationHash: String,
    )

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
        require(
            source.family == destination.family &&
                source.address == destination.routeAddress &&
                source.runtimeHash == destination.routeCodeHash,
        ) {
            "$label source emitter does not identify the destination route deployment"
        }
        val settlement = requiredObject(value, "settlement")
        exactFields(
            settlement,
            setOf("asset_definition_id", "custody_account_id", "payload_amount_scale"),
            "$label.settlement",
        )
        require(requiredText(settlement, "asset_definition_id") == TAIRA_XOR_ASSET_ID) {
            "$label settlement must use canonical Taira XOR"
        }
        requiredText(settlement, "custody_account_id")
        val payloadAmountScale = requiredInt(settlement, "payload_amount_scale", 9, 9)
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
        val lineage = "$routeId\u0000$assetKey"
        return ParsedRoute(
            lineage,
            "${lane.source.profileKey}\u0000${lane.target.profileKey}\u0000$lineage\u0000$revision",
            revision,
            activation,
            inboundFinalityCutoff,
            destination.destinationBindingHash,
            routeConfigurationHash,
        )
    }

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
        exactFields(
            identity,
            setOf("address", "runtime_code_hash", "route_config_hash"),
            "$label.emitter.identity",
        )
        val address = upperBytes(identity, "address", 20)
        val runtime = upperBytes(identity, "runtime_code_hash", 32)
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
        exactFields(deployment, DESTINATION_FIELDS, "$label.deployment")
        val addresses = listOf("token_address", "verifier_address", "route_address").map {
            upperBytes(deployment, it, 20)
        }
        val hashes = listOf(
            "token_code_hash",
            "verifier_code_hash",
            "verifier_key_hash",
            "route_code_hash",
        ).map { upperBytes(deployment, it, 32) }
        require(addresses.distinct().size == addresses.size && hashes.distinct().size == hashes.size) {
            "$label deployment reuses a role-separated address or hash"
        }
        validateVerifyingKey(
            requiredObject(deployment, "verifying_key"),
            hashes[2],
            "$label.deployment.verifying_key",
        )
        val policyHashes = validateOutboundProofPolicy(
            requiredObject(deployment, "outbound_proof_policy"),
            "$label.deployment.outbound_proof_policy",
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
        val destinationBindingHash = destinationBindingHash(
            lane.source,
            family,
            addresses[1],
            addresses[2],
            hashes[1],
            hashes[2],
            policyHashes,
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
            destinationBindingHash,
            keccak(concatenate(deploymentConfiguration)).toUpperHex(),
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
    ): String {
        val networkValue = when (network) {
            SccpNetworkV1.ETHEREUM_MAINNET -> 1L
            SccpNetworkV1.ETHEREUM_SEPOLIA -> 11_155_111L
            SccpNetworkV1.BSC_MAINNET -> 56L
            SccpNetworkV1.BSC_TESTNET -> 97L
            SccpNetworkV1.TRON_MAINNET -> 0x2b66_53dcL
            SccpNetworkV1.TRON_NILE -> 0xcd86_90dcL
            SccpNetworkV1.TRON_SHASTA -> 0x94a9_059eL
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
        )
        return keccak(concatenate(payload)).toUpperHex()
    }

    private fun routeConfigurationHash(
        lane: SccpLaneIdV1,
        routeId: String,
        assetKey: String,
        revision: Long,
        payloadAmountScale: Int,
        destination: DestinationRoles,
    ): String {
        val expectedRouteId: String
        val networkValue: Long
        when (lane.source) {
            SccpNetworkV1.ETHEREUM_MAINNET -> {
                expectedRouteId = "taira_eth_xor"
                networkValue = 1
            }
            SccpNetworkV1.ETHEREUM_SEPOLIA -> {
                expectedRouteId = "taira_eth_xor"
                networkValue = 11_155_111
            }
            SccpNetworkV1.BSC_MAINNET -> {
                expectedRouteId = "taira_bsc_xor"
                networkValue = 56
            }
            SccpNetworkV1.BSC_TESTNET -> {
                expectedRouteId = "taira_bsc_xor"
                networkValue = 97
            }
            SccpNetworkV1.TRON_MAINNET -> {
                expectedRouteId = "taira_tron_xor"
                networkValue = 0x2b66_53dcL
            }
            SccpNetworkV1.TRON_NILE -> {
                expectedRouteId = "taira_tron_xor"
                networkValue = 0xcd86_90dcL
            }
            SccpNetworkV1.TRON_SHASTA -> {
                expectedRouteId = "taira_tron_xor"
                networkValue = 0x94a9_059eL
            }
            SccpNetworkV1.SORA_TAIRA -> error("closed source lane")
        }
        require(
            routeId == expectedRouteId && assetKey == "xor" && payloadAmountScale == 9,
        ) { "SCCP route identity does not match its exact deployment" }
        val sourceLaneHash = SccpV1.laneHash(lane).toUpperHex()
        val destinationLaneHash = SccpV1.laneHash(
            SccpLaneIdV1(lane.target, lane.source),
        ).toUpperHex()
        val hashRoles = mutableListOf(
            sourceLaneHash,
            destinationLaneHash,
            destination.tokenCodeHash,
            destination.verifierCodeHash,
            destination.verifierKeyHash,
            destination.semanticProfileHash,
            destination.finalityAnchorHash,
        )
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

    private fun validateOutboundProofPolicyFields(
        value: Map<String, Any?>,
        label: String,
    ): ParsedProofPolicy {
        val policy = linkedMapOf<String, Any?>(
            "version" to 1,
            "semantic_profile" to value["semantic_proof_profile"],
            "sora_finality_anchor" to value["sora_finality_anchor"],
        )
        return validateOutboundProofPolicy(policy, "$label outbound proof policy")
    }

    private fun validateOutboundProofPolicy(
        value: Map<String, Any?>,
        label: String,
    ): ParsedProofPolicy {
        exactFields(value, setOf("version", "semantic_profile", "sora_finality_anchor"), label)
        requiredInt(value, "version", 1, 1)
        val profile = requiredObject(value, "semantic_profile")
        exactFields(profile, setOf("profile", "commitments"), "$label.semantic_profile")
        val profileName = requiredText(profile, "profile")
        require(profileName == SEMANTIC_PROFILE) {
            "$label semantic profile is unsupported"
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
        require(semanticRoles[2] == PUBLIC_SIGNAL_SCHEMA_HASH) {
            "$label public signal schema hash does not name the eleven-signal V1 schema"
        }
        requireDistinctRawHashes(semanticRoles, "$label semantic profile")
        val profileHash = keccak(
            "sccp:semantic-proof-profile:v1".toByteArray(Charsets.UTF_8) +
                byteArrayOf(1, 0, 1) +
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
            upperBytes(anchor, "validator_set_hash", 32),
        )
        require(anchorRoles[0] == TAIRA_CHAIN_ID_HASH) { "$label Taira chain id hash mismatch" }
        val checkpointHeight = requiredUnsignedInteger(anchor, "checkpoint_height", MAX_U64, true)
        val validatorSetEpoch = requiredUnsignedInteger(anchor, "validator_set_epoch", MAX_U64, false)
        val hashVersion = requiredInt(anchor, "validator_set_hash_version", 1, 1)
        requireDistinctRawHashes(anchorRoles, "$label finality anchor")
        val canonicalAnchor = ByteArrayOutputStream().also { output ->
            output.write(1)
            output.write(SccpNetworkV1.SORA_TAIRA.tag)
            output.write(anchorRoles[0].hexToBytes())
            writeU64(output, checkpointHeight)
            output.write(anchorRoles[1].hexToBytes())
            writeU64(output, validatorSetEpoch)
            output.write(anchorRoles[2].hexToBytes())
            writeU16(output, hashVersion)
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
                anchorRoles[0],
                checkpointHeight,
                anchorRoles[1],
                validatorSetEpoch,
                anchorRoles[2],
                hashVersion,
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
            5 -> "tron_dpos_v1"
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
        val codec = requiredInt(value, codecField, 1, 5)
        require(codec == 1 || codec == 2 || codec == 5) { "$codecField is unsupported or retired" }
        if (domain != null) {
            val expected = when (domain) {
                0 -> 1
                1, 2 -> 2
                5 -> 5
                else -> throw IllegalArgumentException("unsupported SCCP domain")
            }
            require(codec == expected) { "$codecField does not match its domain" }
        }
        val bytes = hexBytes(value, bytesField)
        when (codec) {
            1 -> require(bytes.isNotEmpty() && bytes.size <= 256 && bytes.all { (it.toInt() and 0xff) in 0x21..0x7e })
            2 -> require(bytes.size == 20 && bytes.any { it.toInt() != 0 })
            5 -> require(bytes.size == 21 && bytes[0] == 0x41.toByte() && bytes.drop(1).any { it.toInt() != 0 })
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
        require(requiredInt(value, "target_domain", 1, 5) == target.domainId) {
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
            requiredLong(value, "height", 1, Long.MAX_VALUE),
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
        require(requiredInt(transfer, "dest_domain", 1, 5) == lane.target.domainId) {
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
            5 -> "taira_tron_xor"
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
        val variant = if (target.domainId == 5) "TronAddress21" else "EvmAddress20"
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
        val profile = requiredText(value, "network").replace('_', '-')
        return SccpNetworkV1.fromProfileKey(profile)
            ?: throw IllegalArgumentException("$label is unsupported or retired")
    }

    private fun exactProfile(value: Map<String, Any?>, field: String): SccpNetworkV1 {
        val profile = requiredText(value, field)
        return SccpNetworkV1.fromProfileKey(profile)
            ?: throw IllegalArgumentException("$field is unsupported or retired")
    }

    private fun familyFor(network: SccpNetworkV1): String =
        if (network.domainId == 5) "tron" else "evm"

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
            require(it == 0 || it == 1 || it == 2 || it == 5) {
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

    private fun prefixedHash(value: Map<String, Any?>, field: String): String =
        requiredText(value, field).also {
            require(Regex("0x[0-9a-f]{64}").matches(it) && it.drop(2).any { char -> char != '0' }) {
                "$field must be canonical lowercase nonzero 0x-prefixed 32-byte hex"
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
        require(values.distinct().size == values.size) { "$label hash roles must be distinct" }
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

    private fun concatenate(values: List<ByteArray>): ByteArray =
        ByteArrayOutputStream(values.sumOf { it.size }).also { output ->
            values.forEach { output.write(it) }
        }.toByteArray()

    private fun abiWord(value: Long): ByteArray {
        require(value >= 0) { "ABI word value must be unsigned" }
        val encoded = BigInteger.valueOf(value).toByteArray().let {
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

    private fun writeU32(out: ByteArrayOutputStream, value: Int) {
        repeat(4) { shift -> out.write((value ushr (shift * 8)) and 0xff) }
    }

    private fun writeU64(out: ByteArrayOutputStream, value: BigInteger) {
        repeat(8) { shift ->
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
        "max_bn254_pairing_checks_per_block",
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
        "taira_to_token_multiplier",
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
        "chain_id_hash",
        "checkpoint_height",
        "checkpoint_block_hash",
        "validator_set_epoch",
        "validator_set_hash",
        "validator_set_hash_version",
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
    private const val EVM_DESTINATION_BINDING_DOMAIN =
        "iroha:sccp:evm-destination-binding:v1"
    private const val TRON_DESTINATION_BINDING_DOMAIN =
        "iroha:sccp:tron-destination-binding:v1"
    private const val EVM_GROTH16_BACKEND = "evm-groth16-bn254-v1"
    private const val TRON_GROTH16_BACKEND = "tron-groth16-bn254-v1"
    private const val CONCRETE_ROUTE_CONFIGURATION_DOMAIN = "sccp:concrete-route-config:v1"
    private const val TAIRA_XOR_ASSET_ID = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
    private val BN254_MODULUS = BigInteger(
        "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47",
        16,
    )
    private val MAX_U32 = BigInteger.ONE.shiftLeft(32).subtract(BigInteger.ONE)
    private val MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private val MAX_JSON_SAFE_INTEGER = BigInteger("9007199254740991")
    private val MAX_U128 = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE)
    private val TAIRA_CHAIN_ID_HASH = keccak(
        byteArrayOf(
            0x80.toByte(), 0x95.toByte(), 0x74, 0xf5.toByte(), 0xfe.toByte(), 0xe7.toByte(), 0x5e,
            0x69, 0xbf.toByte(), 0xcf.toByte(), 0x52, 0x45, 0x1e, 0x42, 0xd5.toByte(), 0x0f,
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
    private const val MAX_VARIABLE_BYTES = 16 * 1024 * 1024
}
