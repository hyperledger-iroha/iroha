package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.sccp.SccpLaneIdV1
import org.hyperledger.iroha.sdk.sccp.SccpNetworkV1

/** Supported closed native verifier backends returned by SCCP discovery. */
enum class SccpNativeBackendV1(val wireKey: String, val backendLabel: String) {
    ETHEREUM_BEACON("ethereum_beacon_v1", "bridge/sccp/native/ethereum-beacon-v1"),
    BSC_PARLIA("bsc_parlia_v1", "bridge/sccp/native/bsc-parlia-v1"),
    SOLANA_TOWER("solana_tower_v1", "bridge/sccp/native/solana-tower-v1"),
    TON_MASTERCHAIN("ton_masterchain_v1", "bridge/sccp/native/ton-masterchain-v1"),
    TRON_DPOS("tron_dpos_v1", "bridge/sccp/native/tron-dpos-v1");

    fun supports(network: SccpNetworkV1): Boolean = when (this) {
        ETHEREUM_BEACON -> network == SccpNetworkV1.ETHEREUM_MAINNET || network == SccpNetworkV1.ETHEREUM_SEPOLIA
        BSC_PARLIA -> network == SccpNetworkV1.BSC_MAINNET || network == SccpNetworkV1.BSC_TESTNET
        SOLANA_TOWER -> network == SccpNetworkV1.SOLANA_MAINNET_BETA || network == SccpNetworkV1.SOLANA_TESTNET
        TON_MASTERCHAIN -> network == SccpNetworkV1.TON_MAINNET || network == SccpNetworkV1.TON_TESTNET
        TRON_DPOS -> network == SccpNetworkV1.TRON_MAINNET || network == SccpNetworkV1.TRON_NILE || network == SccpNetworkV1.TRON_SHASTA
    }

    companion object {
        fun fromWireKey(value: String): SccpNativeBackendV1? = values().firstOrNull { it.wireKey == value }
    }
}

/** Closed canonical payload codec inventory advertised by SCCP discovery. */
enum class SccpCodecV1(val id: Int, val wireKey: String) {
    CANONICAL_TEXT(1, "canonical_text"),
    EVM_ADDRESS20(2, "evm_address20"),
    SOLANA_PUBKEY32(3, "solana_pubkey32"),
    TON_ACCOUNT36(4, "ton_account36"),
    TRON_ADDRESS21(5, "tron_address21"),
    SORA_ASSET_ID(6, "sora_asset_id");

    companion object {
        fun fromId(id: Int): SccpCodecV1? = values().firstOrNull { it.id == id }
    }
}

data class SccpCodecCapability(val codec: SccpCodecV1, val description: String) {
    val id: Int get() = codec.id
    val key: String get() = codec.wireKey
}

data class SccpNativeAdmissionCapability(
    val backend: SccpNativeBackendV1,
    val backendLabel: String,
    val trustAnchorHash: String,
)

data class SccpRouteBrowserProverManifestRef(
    val moduleUrl: String,
    val moduleSpecifier: String?,
    val moduleHash: String,
    val manifestHash: String,
    val expectedExports: List<String>,
    val boundRouteHash: String,
    val boundProofHash: String,
)

enum class SccpSourceEmitterFamilyV1(val wireKey: String) {
    EVM("evm"), SOLANA("solana"), TON("ton"), TRON("tron");

    companion object {
        fun fromWireKey(value: String): SccpSourceEmitterFamilyV1? = values().firstOrNull { it.wireKey == value }
    }
}

/** Public typed source emitter identity. Identity fields are validated and immutable. */
data class SccpSourceEmitterV1(
    val family: SccpSourceEmitterFamilyV1,
    val identity: Map<String, Any>,
)

data class SccpSourceIdentityV1(val lane: SccpLaneIdV1, val emitter: SccpSourceEmitterV1)

data class SccpExactInboundLaneCapability(
    val sourceProfile: String,
    val targetProfile: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val sourceIdentityHash: String,
    val sourceIdentity: SccpSourceIdentityV1,
    val admissionEnabled: Boolean,
    val nativeAdmission: SccpNativeAdmissionCapability?,
    val nativeProofBuilder: SccpRouteBrowserProverManifestRef?,
)

data class SccpOutboundProofCapability(
    val messageBundlePath: String,
    val proofArtifactPath: String,
    val proofJobPath: String,
    val recentMessagesPath: String,
    val manifestPath: String,
)

data class SccpCapabilities(
    val version: Int,
    val registryRevision: String,
    val nativeMessageSubmitPath: String?,
    val outbound: SccpOutboundProofCapability,
    val messagePayloadKinds: List<SccpPayloadKindV1>,
    val codecs: List<SccpCodecCapability>,
    val inboundLanes: List<SccpExactInboundLaneCapability>,
)

enum class SccpDestinationVerifierPlanV1(val wireKey: String) {
    EVM_GROTH16_BN254_ADAPTER("EvmGroth16Bn254Adapter"),
    SOLANA_PROGRAM_NATIVE_RECURSIVE("SolanaProgramNativeRecursive"),
    TON_CONTRACT_NATIVE_RECURSIVE("TonContractNativeRecursive"),
    TRON_CONTRACT_GROTH16_BN254("TronContractGroth16Bn254");

    companion object {
        fun fromWireKey(value: String): SccpDestinationVerifierPlanV1? = values().firstOrNull { it.wireKey == value }
    }
}

data class SccpOutboundDestinationRoute(
    val sourceProfile: String,
    val targetProfile: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val routeId: String,
    val assetKey: String,
    val verifierPlan: SccpDestinationVerifierPlanV1,
    val verifierIdentity: String,
    val verifierCodeHash: String,
    val verifierKeyHash: String?,
    val proofArtifactHash: String?,
    val provingKeyHash: String?,
    val destinationBindingKey: String,
    val destinationBindingHash: String,
    val browserProver: SccpRouteBrowserProverManifestRef?,
)

data class SccpProofManifestSet(
    val version: Int,
    val registryRevision: String,
    val inboundNativeLanes: List<SccpExactInboundLaneCapability>,
    val outboundDestinationRoutes: List<SccpOutboundDestinationRoute>,
)

data class SccpRecentMessageLinks(val bundlePath: String, val artifactPath: String, val jobPath: String)

data class SccpRecentMessage(
    val height: Long,
    val messageIdHex: String,
    val kind: SccpPayloadKindV1,
    val sourceProfile: String,
    val targetProfile: String,
    val destinationBindingHash: String,
    val targetDomain: Int,
    val counterpartyDomain: Int,
    val assetId: String?,
    val routeId: String?,
    val recipient: String?,
    val amount: String?,
    val payloadProjection: Map<String, Any?>?,
    val links: SccpRecentMessageLinks,
)

data class SccpRecentMessages(val items: List<SccpRecentMessage>)

/** Strict parser for SCCP public discovery and newest-first readback DTOs. */
object SccpJsonParser {
    @JvmStatic fun parseCapabilities(bytes: ByteArray): SccpCapabilities {
        val root = rootObject(bytes, "SCCP capabilities")
        exactFields(root, CAPABILITY_FIELDS, "SCCP capabilities")
        val version = requiredInt(root, "version", 1, 1)
        val revision = requiredHash(root, "registry_revision")
        val outbound = parseOutboundCapability(requiredObject(root, "outbound"))
        val payloadKinds = requiredStringList(root, "message_payload_kinds", false).map { value ->
            SccpPayloadKindV1.fromWireKey(value)
                ?: throw IllegalArgumentException("message_payload_kinds contains an unknown or retired kind")
        }
        require(payloadKinds.distinct().size == payloadKinds.size) { "message_payload_kinds contains duplicates" }
        val codecs = requiredList(root, "codecs").mapIndexed { index, value -> parseCodec(objectValue(value, "codecs[$index]")) }
        require(codecs.map { it.id }.distinct().size == codecs.size) { "SCCP codec ids must be unique" }
        val inbound = requiredList(root, "inbound_lanes").mapIndexed { index, value -> parseInbound(objectValue(value, "inbound_lanes[$index]")) }
        return SccpCapabilities(
            version, revision,
            optionalPath(root, "native_message_submit_path"),
            outbound, payloadKinds, codecs, inbound,
        )
    }

    @JvmStatic fun parseProofManifests(bytes: ByteArray): SccpProofManifestSet {
        val root = rootObject(bytes, "SCCP proof manifests")
        exactFields(root, MANIFEST_FIELDS, "SCCP proof manifests")
        val inbound = requiredList(root, "inbound_native_lanes").mapIndexed { index, value -> parseInbound(objectValue(value, "inbound_native_lanes[$index]")) }
        val outbound = requiredList(root, "outbound_destination_routes").mapIndexed { index, value -> parseRoute(objectValue(value, "outbound_destination_routes[$index]")) }
        return SccpProofManifestSet(
            requiredInt(root, "version", 1, 1),
            requiredHash(root, "registry_revision"),
            inbound,
            outbound,
        )
    }

    @JvmStatic fun parseRecentMessages(bytes: ByteArray): SccpRecentMessages {
        val root = rootObject(bytes, "SCCP recent messages")
        exactFields(root, setOf("items"), "SCCP recent messages")
        val items = requiredList(root, "items").mapIndexed { index, value -> parseRecent(objectValue(value, "items[$index]")) }
        require(items.zipWithNext().all { (left, right) -> left.height >= right.height }) {
            "SCCP recent messages must be newest-first"
        }
        return SccpRecentMessages(items)
    }

    private fun parseCodec(value: Map<String, Any?>): SccpCodecCapability {
        exactFields(value, setOf("id", "key", "description"), "SCCP codec")
        val codec = SccpCodecV1.fromId(requiredInt(value, "id", 1, 6))
            ?: throw IllegalArgumentException("unsupported SCCP codec id")
        require(requiredNonBlank(value, "key") == codec.wireKey) {
            "SCCP codec key does not match its canonical tag"
        }
        return SccpCodecCapability(codec, requiredNonBlank(value, "description"))
    }

    private fun parseOutboundCapability(value: Map<String, Any?>): SccpOutboundProofCapability {
        exactFields(value, OUTBOUND_FIELDS, "SCCP outbound capability")
        return SccpOutboundProofCapability(
            requiredPath(value, "message_bundle_path"),
            requiredPath(value, "proof_artifact_path"),
            requiredPath(value, "proof_job_path"),
            requiredPath(value, "recent_messages_path"),
            requiredPath(value, "manifest_path"),
        )
    }

    private fun parseInbound(value: Map<String, Any?>): SccpExactInboundLaneCapability {
        exactFields(value, INBOUND_FIELDS, "SCCP inbound lane")
        val source = requiredProfile(value, "source_profile")
        val target = requiredProfile(value, "target_profile")
        val lane = SccpLaneIdV1(source, target)
        require(lane.isInbound) { "inbound SCCP capability must be external-to-SORA" }
        val sourceDomain = requiredInt(value, "source_domain", 0, 5)
        val targetDomain = requiredInt(value, "target_domain", 0, 5)
        require(sourceDomain == source.domainId && targetDomain == target.domainId) {
            "inbound SCCP profile/domain mismatch"
        }
        val identity = parseSourceIdentity(requiredObject(value, "source_identity"))
        require(identity.lane == lane) { "source_identity lane must match capability lane" }
        val admission = optionalObject(value, "native_admission")?.let { parseNativeAdmission(it, source) }
        val enabled = requiredBoolean(value, "admission_enabled")
        require(!enabled || admission != null) { "enabled native admission requires verifier metadata" }
        return SccpExactInboundLaneCapability(
            source.profileKey, target.profileKey, sourceDomain, targetDomain,
            requiredHash(value, "source_identity_hash"), identity, enabled, admission,
            optionalObject(value, "native_proof_builder")?.let(::parseBrowserProver),
        )
    }

    private fun parseSourceIdentity(value: Map<String, Any?>): SccpSourceIdentityV1 {
        exactFields(value, setOf("lane", "emitter"), "SCCP source identity")
        val laneObject = requiredObject(value, "lane")
        exactFields(laneObject, setOf("source", "target"), "SCCP source identity lane")
        val lane = SccpLaneIdV1(parseNetwork(requiredObject(laneObject, "source")), parseNetwork(requiredObject(laneObject, "target")))
        require(lane.isInbound) { "source identity must use an inbound exact lane" }
        return SccpSourceIdentityV1(lane, parseEmitter(requiredObject(value, "emitter"), lane.source))
    }

    private fun parseNetwork(value: Map<String, Any?>): SccpNetworkV1 {
        exactFields(value, setOf("network", "profile"), "SCCP network")
        require(value["profile"] == null) { "unit SCCP network profile content must be null" }
        val wire = requiredNonBlank(value, "network")
        val profile = wire.replace('_', '-')
        return SccpNetworkV1.fromProfileKey(profile)
            ?: throw IllegalArgumentException("unsupported SCCP network profile: $wire")
    }

    private fun parseEmitter(value: Map<String, Any?>, source: SccpNetworkV1): SccpSourceEmitterV1 {
        exactFields(value, setOf("emitter", "identity"), "SCCP source emitter")
        val family = SccpSourceEmitterFamilyV1.fromWireKey(requiredNonBlank(value, "emitter"))
            ?: throw IllegalArgumentException("unsupported SCCP source emitter")
        val expected = when (source) {
            SccpNetworkV1.ETHEREUM_MAINNET, SccpNetworkV1.ETHEREUM_SEPOLIA,
            SccpNetworkV1.BSC_MAINNET, SccpNetworkV1.BSC_TESTNET -> SccpSourceEmitterFamilyV1.EVM
            SccpNetworkV1.SOLANA_MAINNET_BETA, SccpNetworkV1.SOLANA_TESTNET -> SccpSourceEmitterFamilyV1.SOLANA
            SccpNetworkV1.TON_MAINNET, SccpNetworkV1.TON_TESTNET -> SccpSourceEmitterFamilyV1.TON
            SccpNetworkV1.TRON_MAINNET, SccpNetworkV1.TRON_NILE, SccpNetworkV1.TRON_SHASTA -> SccpSourceEmitterFamilyV1.TRON
            else -> throw IllegalArgumentException("SORA cannot be an SCCP source emitter")
        }
        require(family == expected) { "source emitter family does not match exact profile" }
        val identity = requiredObject(value, "identity")
        val normalized = when (family) {
            SccpSourceEmitterFamilyV1.EVM, SccpSourceEmitterFamilyV1.TRON -> {
                exactFields(identity, setOf("address", "runtime_code_hash", "route_config_hash"), "SCCP EVM/TRON emitter")
                val address = requiredFixedUpperHex(identity, "address", 20)
                val runtime = requiredFixedUpperHex(identity, "runtime_code_hash", 32)
                val routeConfig = requiredFixedUpperHex(identity, "route_config_hash", 32)
                require(runtime != routeConfig) { "source emitter runtime and route-config hashes must differ" }
                linkedMapOf("address" to address, "runtime_code_hash" to runtime, "route_config_hash" to routeConfig)
            }
            SccpSourceEmitterFamilyV1.SOLANA -> {
                exactFields(identity, setOf("program_id", "executable_hash", "authorized_emitter"), "SCCP Solana emitter")
                val roles = listOf(
                    requiredFixedUpperHex(identity, "program_id", 32),
                    requiredFixedUpperHex(identity, "executable_hash", 32),
                    requiredFixedUpperHex(identity, "authorized_emitter", 32),
                )
                require(roles.distinct().size == roles.size) { "Solana emitter roles must be distinct" }
                linkedMapOf("program_id" to roles[0], "executable_hash" to roles[1], "authorized_emitter" to roles[2])
            }
            SccpSourceEmitterFamilyV1.TON -> {
                exactFields(identity, setOf("workchain", "account_id", "code_hash", "immutable_config_hash"), "SCCP TON emitter")
                val workchain = requiredInt(identity, "workchain", 0, 0)
                val roles = listOf(
                    requiredFixedUpperHex(identity, "account_id", 32),
                    requiredFixedUpperHex(identity, "code_hash", 32),
                    requiredFixedUpperHex(identity, "immutable_config_hash", 32),
                )
                require(roles.distinct().size == roles.size) { "TON emitter roles must be distinct" }
                linkedMapOf("workchain" to workchain, "account_id" to roles[0], "code_hash" to roles[1], "immutable_config_hash" to roles[2])
            }
        }
        return SccpSourceEmitterV1(family, normalized)
    }

    private fun parseNativeAdmission(value: Map<String, Any?>, source: SccpNetworkV1): SccpNativeAdmissionCapability {
        exactFields(value, setOf("backend", "backend_label", "trust_anchor_hash"), "SCCP native admission")
        val backendObject = requiredObject(value, "backend")
        exactFields(backendObject, setOf("backend", "protocol"), "SCCP native backend")
        require(backendObject["protocol"] == null) { "unit native backend content must be null" }
        val backend = SccpNativeBackendV1.fromWireKey(requiredNonBlank(backendObject, "backend"))
            ?: throw IllegalArgumentException("unsupported SCCP native backend")
        require(backend.supports(source)) { "native backend does not support exact source profile" }
        val label = requiredNonBlank(value, "backend_label")
        require(label == backend.backendLabel) { "native backend label mismatch" }
        return SccpNativeAdmissionCapability(backend, label, requiredHash(value, "trust_anchor_hash"))
    }

    private fun parseBrowserProver(value: Map<String, Any?>): SccpRouteBrowserProverManifestRef {
        exactFields(value, BROWSER_FIELDS, "SCCP browser prover")
        val exports = requiredStringList(value, "expected_exports", true)
        require(exports.distinct().size == exports.size) { "browser prover exports must be unique" }
        return SccpRouteBrowserProverManifestRef(
            requiredAbsoluteUrl(value, "module_url"), optionalNonBlank(value, "module_specifier"),
            requiredHash(value, "module_hash"), requiredHash(value, "manifest_hash"), exports,
            requiredHash(value, "bound_route_hash"), requiredHash(value, "bound_proof_hash"),
        )
    }

    private fun parseRoute(value: Map<String, Any?>): SccpOutboundDestinationRoute {
        exactFields(value, ROUTE_FIELDS, "SCCP outbound destination route")
        val source = requiredProfile(value, "source_profile")
        val target = requiredProfile(value, "target_profile")
        val lane = SccpLaneIdV1(source, target)
        require(lane.isOutbound) { "outbound destination route must be SORA-to-external" }
        val sourceDomain = requiredInt(value, "source_domain", 0, 5)
        val targetDomain = requiredInt(value, "target_domain", 0, 5)
        require(sourceDomain == source.domainId && targetDomain == target.domainId) {
            "outbound route profile/domain mismatch"
        }
        val plan = SccpDestinationVerifierPlanV1.fromWireKey(requiredNonBlank(value, "verifier_plan"))
            ?: throw IllegalArgumentException("unknown or retired SCCP verifier plan")
        return SccpOutboundDestinationRoute(
            source.profileKey, target.profileKey, sourceDomain, targetDomain,
            requiredNonBlank(value, "route_id"), requiredNonBlank(value, "asset_key"), plan,
            requiredNonBlank(value, "verifier_identity"), requiredHash(value, "verifier_code_hash"),
            optionalHash(value, "verifier_key_hash"), optionalHash(value, "proof_artifact_hash"),
            optionalHash(value, "proving_key_hash"), requiredNonBlank(value, "destination_binding_key"),
            requiredHash(value, "destination_binding_hash"), optionalObject(value, "browser_prover")?.let(::parseBrowserProver),
        )
    }

    private fun parseRecent(value: Map<String, Any?>): SccpRecentMessage {
        exactFields(value, RECENT_FIELDS, "SCCP recent message")
        val source = requiredProfile(value, "source_profile")
        val target = requiredProfile(value, "target_profile")
        val lane = SccpLaneIdV1(source, target)
        require(lane.isOutbound) { "recent SCCP message must use an outbound exact lane" }
        val targetDomain = requiredInt(value, "target_domain", 1, 5)
        val counterparty = requiredInt(value, "counterparty_domain", 1, 5)
        require(targetDomain == target.domainId && counterparty == target.domainId) {
            "recent SCCP message profile/domain mismatch"
        }
        val amount = optionalNonBlank(value, "amount")
        if (amount != null) require(Regex("[1-9][0-9]*").matches(amount)) { "SCCP amount must be canonical positive decimal" }
        val linksObject = requiredObject(value, "links")
        exactFields(linksObject, setOf("bundle_path", "artifact_path", "job_path"), "SCCP recent message links")
        val projection = optionalObject(value, "payload_projection")?.toMap()
        return SccpRecentMessage(
            requiredLong(value, "height", 1), requiredHash(value, "message_id_hex"),
            SccpPayloadKindV1.fromWireKey(requiredNonBlank(value, "kind"))
                ?: throw IllegalArgumentException("recent SCCP message kind is unknown or retired"),
            source.profileKey, target.profileKey,
            requiredHash(value, "destination_binding_hash"), targetDomain, counterparty,
            optionalNonBlank(value, "asset_id"), optionalNonBlank(value, "route_id"),
            optionalNonBlank(value, "recipient"), amount, projection,
            SccpRecentMessageLinks(
                requiredPath(linksObject, "bundle_path"), requiredPath(linksObject, "artifact_path"), requiredPath(linksObject, "job_path"),
            ),
        )
    }

    private fun rootObject(bytes: ByteArray, label: String): Map<String, Any?> {
        val text = String(bytes, StandardCharsets.UTF_8)
        require(text.toByteArray(StandardCharsets.UTF_8).contentEquals(bytes)) { "$label must be UTF-8 JSON" }
        return objectValue(JsonParser.parse(text), label)
    }

    @Suppress("UNCHECKED_CAST")
    private fun objectValue(value: Any?, label: String): Map<String, Any?> {
        require(value is Map<*, *> && value.keys.all { it is String }) { "$label must be a JSON object" }
        return value as Map<String, Any?>
    }

    private fun exactFields(value: Map<String, Any?>, allowed: Set<String>, label: String) {
        val unknown = value.keys.firstOrNull { it !in allowed }
        require(unknown == null) { "$label contains unknown field `$unknown`" }
    }

    private fun requiredObject(value: Map<String, Any?>, field: String): Map<String, Any?> = objectValue(value[field], field)
    private fun optionalObject(value: Map<String, Any?>, field: String): Map<String, Any?>? = value[field]?.let { objectValue(it, field) }
    private fun requiredList(value: Map<String, Any?>, field: String): List<Any?> = value[field] as? List<*> ?: throw IllegalArgumentException("$field must be an array")
    private fun requiredBoolean(value: Map<String, Any?>, field: String): Boolean = value[field] as? Boolean ?: throw IllegalArgumentException("$field must be a boolean")

    private fun requiredNonBlank(value: Map<String, Any?>, field: String): String {
        val item = value[field] as? String ?: throw IllegalArgumentException("$field must be a string")
        require(item.isNotBlank() && item == item.trim()) { "$field must be canonical nonblank text" }
        return item
    }

    private fun optionalNonBlank(value: Map<String, Any?>, field: String): String? {
        if (value[field] == null) return null
        return requiredNonBlank(value, field)
    }

    private fun requiredPath(value: Map<String, Any?>, field: String): String = requiredNonBlank(value, field).also {
        require(it.startsWith('/') && !it.contains("//")) { "$field must be an absolute Torii path" }
    }
    private fun optionalPath(value: Map<String, Any?>, field: String): String? = optionalNonBlank(value, field)?.also {
        require(it.startsWith('/') && !it.contains("//")) { "$field must be an absolute Torii path" }
    }
    private fun requiredAbsoluteUrl(value: Map<String, Any?>, field: String): String = requiredNonBlank(value, field).also {
        require(it.startsWith("https://")) { "$field must be an HTTPS URL" }
    }

    private fun requiredInt(value: Map<String, Any?>, field: String, minimum: Int, maximum: Int): Int {
        val number = value[field] as? Number ?: throw IllegalArgumentException("$field must be an integer")
        val long = number.toLong()
        require(number.toString() == long.toString() && long in minimum.toLong()..maximum.toLong()) { "$field is out of range" }
        return long.toInt()
    }

    private fun requiredLong(value: Map<String, Any?>, field: String, minimum: Long): Long {
        val number = value[field] as? Number ?: throw IllegalArgumentException("$field must be an integer")
        val long = number.toLong()
        require(number.toString() == long.toString() && long >= minimum) { "$field is out of range" }
        return long
    }

    private fun requiredStringList(value: Map<String, Any?>, field: String, nonempty: Boolean): List<String> {
        val result = requiredList(value, field).mapIndexed { index, item ->
            require(item is String && item.isNotBlank() && item == item.trim()) { "$field[$index] must be canonical text" }
            item
        }
        if (nonempty) require(result.isNotEmpty()) { "$field must not be empty" }
        return result
    }

    private fun requiredHash(value: Map<String, Any?>, field: String): String {
        val item = requiredNonBlank(value, field)
        require(Regex("0x[0-9a-f]{64}").matches(item) && item.substring(2).any { it != '0' }) {
            "$field must be canonical lowercase nonzero 32-byte hex"
        }
        return item
    }
    private fun optionalHash(value: Map<String, Any?>, field: String): String? = if (value[field] == null) null else requiredHash(value, field)

    private fun requiredFixedUpperHex(value: Map<String, Any?>, field: String, bytes: Int): String {
        val item = requiredNonBlank(value, field)
        require(Regex("[0-9A-F]{${bytes * 2}}").matches(item) && item.any { it != '0' }) {
            "$field must be canonical uppercase nonzero fixed hex"
        }
        return item
    }

    private fun requiredProfile(value: Map<String, Any?>, field: String): SccpNetworkV1 {
        val profile = requiredNonBlank(value, field)
        return SccpNetworkV1.fromProfileKey(profile)
            ?: throw IllegalArgumentException("$field is not an exact SCCP profile")
    }

    private val CAPABILITY_FIELDS = setOf("version", "registry_revision", "native_message_submit_path", "outbound", "message_payload_kinds", "codecs", "inbound_lanes")
    private val OUTBOUND_FIELDS = setOf("message_bundle_path", "proof_artifact_path", "proof_job_path", "recent_messages_path", "manifest_path")
    private val INBOUND_FIELDS = setOf("source_profile", "target_profile", "source_domain", "target_domain", "source_identity_hash", "source_identity", "admission_enabled", "native_admission", "native_proof_builder")
    private val BROWSER_FIELDS = setOf("module_url", "module_specifier", "module_hash", "manifest_hash", "expected_exports", "bound_route_hash", "bound_proof_hash")
    private val MANIFEST_FIELDS = setOf("version", "registry_revision", "inbound_native_lanes", "outbound_destination_routes")
    private val ROUTE_FIELDS = setOf("source_profile", "target_profile", "source_domain", "target_domain", "route_id", "asset_key", "verifier_plan", "verifier_identity", "verifier_code_hash", "verifier_key_hash", "proof_artifact_hash", "proving_key_hash", "destination_binding_key", "destination_binding_hash", "browser_prover")
    private val RECENT_FIELDS = setOf("height", "message_id_hex", "kind", "source_profile", "target_profile", "destination_binding_hash", "target_domain", "counterparty_domain", "asset_id", "route_id", "recipient", "amount", "payload_projection", "links")
}
