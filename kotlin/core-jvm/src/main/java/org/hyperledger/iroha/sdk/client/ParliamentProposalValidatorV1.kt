package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.text.Normalizer
import java.util.Base64
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.address.decodePublicKeyLiteral
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.model.requireCanonicalV1ContractAddress

/** Recursive fail-closed admission for the exact first-release proposal wire contract. */
internal object ParliamentProposalValidatorV1 {
    private val U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private val FIRST_RELEASE_MAX_EXACT_JSON_U64 = BigInteger("9007199254740991")
    private val ROUTE_TEXT = Regex("[a-z0-9](?:[a-z0-9_-]{0,62}[a-z0-9])?")
    private val KEBAB = Regex("[a-z0-9]+(?:-[a-z0-9]+)*")
    private val ALPHANUMERIC_PRERELEASE = Regex("(?=.*[A-Za-z-])[A-Za-z0-9-]+")

    @Suppress("UNCHECKED_CAST")
    fun parse(bytes: ByteArray): Map<String, Any?> {
        val text = String(bytes, StandardCharsets.UTF_8)
        require(text.toByteArray(StandardCharsets.UTF_8).contentEquals(bytes)) {
            "proposal must be UTF-8 JSON"
        }
        val proposal = objectValue(JsonParser.parse(text), "proposal")
        exact(proposal, setOf("kind", "payload"), "proposal")
        val kind = text(proposal["kind"], "proposal.kind")
        val payload = objectValue(proposal["payload"], "proposal.payload")
        when (kind) {
            "DeployContract" -> deployContract(payload)
            "RuntimeUpgrade" -> runtimeUpgrade(payload)
            "SccpRouteGovernance" -> sccpRoute(payload)
            "ValidationFeePolicy" -> validationFeePolicyProposal(payload)
            "ValidationFeePayoutLifecycle" -> validationFeePayoutLifecycle(payload)
            "MusubiRegistryGovernance" -> musubiAction(payload)
            "SorafsProviderGovernance" -> sorafsProvider(payload)
            else -> throw IllegalArgumentException("proposal.kind is unknown or retired")
        }
        return proposal
    }

    private fun deployContract(value: Map<String, Any?>) {
        exact(
            value,
            setOf("contract_address", "code_hash", "abi_hash", "abi_version", "manifest_provenance"),
            "DeployContract",
        )
        requireCanonicalV1ContractAddress(text(value["contract_address"], "contract_address"))
        lowerHex32(value["code_hash"], "code_hash")
        lowerHex32(value["abi_hash"], "abi_hash")
        require(uint(value["abi_version"], "abi_version") == BigInteger.ONE) {
            "abi_version must equal 1"
        }
        value["manifest_provenance"]?.let {
            manifestProvenance(objectValue(it, "manifest_provenance"), "manifest_provenance")
        }
    }

    private fun manifestProvenance(value: Map<String, Any?>, label: String) {
        exact(value, setOf("signer", "signature"), label)
        canonicalPublicKey(value["signer"], "$label.signer")
        canonicalSignature(value["signature"], "$label.signature")
    }

    private fun runtimeUpgrade(value: Map<String, Any?>) {
        exact(value, setOf("manifest"), "RuntimeUpgrade")
        val manifest = objectValue(value["manifest"], "RuntimeUpgrade.manifest")
        exact(
            manifest,
            setOf(
                "name", "description", "abi_version", "abi_hash", "added_syscalls",
                "added_pointer_types", "start_height", "end_height", "sbom_digests",
                "slsa_attestation", "provenance",
            ),
            "RuntimeUpgrade.manifest",
        )
        text(manifest["name"], "manifest.name")
        string(manifest["description"], "manifest.description")
        require(uint(manifest["abi_version"], "manifest.abi_version") == BigInteger.ONE) {
            "manifest.abi_version must equal 1"
        }
        bytes(manifest["abi_hash"], 32, "manifest.abi_hash", false)
        val syscalls = list(manifest["added_syscalls"], "manifest.added_syscalls")
        val pointers = list(manifest["added_pointer_types"], "manifest.added_pointer_types")
        syscalls.forEachIndexed { index, item ->
            require(uint(item, "manifest.added_syscalls[$index]") <= BigInteger.valueOf(0xffff))
        }
        pointers.forEachIndexed { index, item ->
            require(uint(item, "manifest.added_pointer_types[$index]") <= BigInteger.valueOf(0xffff))
        }
        require(syscalls.isEmpty() && pointers.isEmpty()) { "V1 ABI delta lists must be empty" }
        val start = uint(manifest["start_height"], "manifest.start_height")
        val end = uint(manifest["end_height"], "manifest.end_height")
        require(end > start) { "manifest.end_height must exceed start_height" }
        list(manifest["sbom_digests"], "manifest.sbom_digests").forEachIndexed { index, item ->
            val digest = objectValue(item, "manifest.sbom_digests[$index]")
            exact(digest, setOf("algorithm", "digest"), "manifest.sbom_digests[$index]")
            text(digest["algorithm"], "manifest.sbom_digests[$index].algorithm")
            canonicalBase64(digest["digest"], "manifest.sbom_digests[$index].digest")
        }
        canonicalBase64(manifest["slsa_attestation"], "manifest.slsa_attestation")
        list(manifest["provenance"], "manifest.provenance").forEachIndexed { index, item ->
            manifestProvenance(objectValue(item, "manifest.provenance[$index]"), "manifest.provenance[$index]")
        }
    }

    private fun sccpRoute(value: Map<String, Any?>) {
        exact(value, setOf("anchor"), "SccpRouteGovernance")
        val anchor = objectValue(value["anchor"], "SccpRouteGovernance.anchor")
        exact(anchor, setOf("network_id", "action"), "SccpRouteGovernance.anchor")
        NetworkId.parse(text(anchor["network_id"], "anchor.network_id"))
        SccpJsonParser.validateRouteGovernanceAction(
            objectValue(anchor["action"], "anchor.action"),
        )
    }

    private fun validationFeePolicyProposal(value: Map<String, Any?>) {
        exact(
            value,
            setOf("proposal_operator", "policy", "payout_lifecycle_proposal_id"),
            "ValidationFeePolicy",
        )
        account(value["proposal_operator"], "proposal_operator")
        val policy = objectValue(value["policy"], "policy")
        validationFeePolicy(policy)
        val lifecycle = value["payout_lifecycle_proposal_id"]?.let {
            bytes(it, 32, "payout_lifecycle_proposal_id", true)
        }
        require((policy["treasury_payout_binding"] == null) == (lifecycle == null)) {
            "payout lifecycle id must be present exactly when the policy has a payout binding"
        }
    }

    private fun validationFeePolicy(value: Map<String, Any?>) {
        exact(
            value,
            setOf(
                "schema_version", "network_id", "policy_version", "previous_policy_hash",
                "ds_asset_id", "ds_scale", "fee", "treasury_account_id", "charging_mode",
                "effective_from_height", "expires_after_height", "exemption_classes",
                "treasury_payout_binding",
            ),
            "validation fee policy",
        )
        require(uint(value["schema_version"], "policy.schema_version") == BigInteger.ONE)
        NetworkId.parse(text(value["network_id"], "policy.network_id"))
        val version = u64String(value["policy_version"], "policy.policy_version", true)
        val previousHash = value["previous_policy_hash"]?.let {
            bytes(it, 32, "policy.previous_policy_hash", false)
        }
        require((version == BigInteger.ONE) == (previousHash == null)) {
            "policy.previous_policy_hash does not match policy_version"
        }
        asset(value["ds_asset_id"], "policy.ds_asset_id")
        require(uint(value["ds_scale"], "policy.ds_scale") == BigInteger.valueOf(2))
        val fee = quantity(value["fee"], "policy.fee")
        account(value["treasury_account_id"], "policy.treasury_account_id")
        val mode = chargingMode(objectValue(value["charging_mode"], "policy.charging_mode"))
        val effective = u64String(value["effective_from_height"], "policy.effective_from_height", false)
        value["expires_after_height"]?.let {
            require(u64String(it, "policy.expires_after_height", false) > effective) {
                "policy.expires_after_height must exceed effective_from_height"
            }
        }
        val exemptions = list(value["exemption_classes"], "policy.exemption_classes").mapIndexed { index, item ->
            text(item, "policy.exemption_classes[$index]")
        }
        require(exemptions.distinct().size == exemptions.size && exemptions.all { it == "TREASURY_PAYOUT" }) {
            "policy.exemption_classes contains an unsupported or duplicate class"
        }
        val binding = value["treasury_payout_binding"]?.let {
            payoutBinding(objectValue(it, "policy.treasury_payout_binding"))
        }
        require((binding == null) == ("TREASURY_PAYOUT" !in exemptions)) {
            "policy payout binding does not match exemption classes"
        }
        if (mode == "DISABLED") {
            require(fee == "0" && exemptions.isEmpty() && binding == null) {
                "disabled validation fees require zero fee and no payout exemption"
            }
        } else {
            require(fee == "0.1") { "enabled V1 validation fee must equal 0.1" }
        }
    }

    private fun chargingMode(value: Map<String, Any?>): String {
        exact(value, setOf("charging_mode", "value"), "charging_mode")
        val mode = text(value["charging_mode"], "charging_mode.charging_mode")
        require(mode in setOf("DISABLED", "PER_QUALIFYING_TRANSFER_INSTRUCTION")) {
            "charging_mode is unsupported"
        }
        require(value["value"] == null) { "charging_mode.value must be null" }
        return mode
    }

    private fun validationFeePayoutLifecycle(value: Map<String, Any?>) {
        exact(value, setOf("proposal_operator", "payout_binding"), "ValidationFeePayoutLifecycle")
        account(value["proposal_operator"], "proposal_operator")
        payoutBinding(objectValue(value["payout_binding"], "payout_binding"))
    }

    private fun payoutBinding(value: Map<String, Any?>): Unit {
        exact(
            value,
            setOf(
                "contract_address", "code_hash", "entrypoint", "treasury_account_id",
                "ds_asset_id", "xor_asset_id", "pool_vault_account_id", "batch_ds",
                "min_xor_out", "max_xor_out", "recipients",
            ),
            "payout_binding",
        )
        requireCanonicalV1ContractAddress(text(value["contract_address"], "payout_binding.contract_address"))
        bytes(value["code_hash"], 32, "payout_binding.code_hash", true)
        require(text(value["entrypoint"], "payout_binding.entrypoint") == "autonomous_validation_fee_tick")
        val treasury = account(value["treasury_account_id"], "payout_binding.treasury_account_id")
        val vault = account(value["pool_vault_account_id"], "payout_binding.pool_vault_account_id")
        require(treasury != vault) { "treasury and pool vault accounts must differ" }
        val ds = asset(value["ds_asset_id"], "payout_binding.ds_asset_id")
        val xor = asset(value["xor_asset_id"], "payout_binding.xor_asset_id")
        require(ds != xor) { "DS and XOR assets must differ" }
        require(quantity(value["batch_ds"], "payout_binding.batch_ds") == "10")
        require(quantity(value["min_xor_out"], "payout_binding.min_xor_out") == "4")
        require(quantity(value["max_xor_out"], "payout_binding.max_xor_out") == "100")
        val recipients = list(value["recipients"], "payout_binding.recipients").mapIndexed { index, item ->
            val recipient = objectValue(item, "payout_binding.recipients[$index]")
            exact(recipient, setOf("account_id", "share"), "payout_binding.recipients[$index]")
            require(quantity(recipient["share"], "payout_binding.recipients[$index].share") == "0.25")
            account(recipient["account_id"], "payout_binding.recipients[$index].account_id")
        }
        require(
            recipients.size == 4 && recipients.distinct().size == 4 &&
                treasury !in recipients && vault !in recipients,
        ) { "payout recipients must contain four unique non-pool accounts" }
    }

    private fun musubiAction(value: Map<String, Any?>) {
        exact(value, setOf("kind", "value"), "MusubiRegistryGovernance")
        val kind = text(value["kind"], "MusubiRegistryGovernance.kind")
        val action = objectValue(value["value"], "MusubiRegistryGovernance.value")
        when (kind) {
            "RecoverPackageOwners" -> {
                exact(action, setOf("package", "owners", "expected_revision"), kind)
                musubiPackage(objectValue(action["package"], "$kind.package"), "$kind.package")
                val owners = list(action["owners"], "$kind.owners").mapIndexed { index, item ->
                    account(item, "$kind.owners[$index]")
                }
                require(owners.size in 1..64 && owners.distinct().size == owners.size)
                require(uint(action["expected_revision"], "$kind.expected_revision") > BigInteger.ZERO)
            }
            "RetargetAlias" -> {
                exact(action, setOf("alias", "target", "expected_revision"), kind)
                kebab(stringTuple(action["alias"], "$kind.alias"), "$kind.alias", 32)
                musubiPackage(objectValue(action["target"], "$kind.target"), "$kind.target")
                require(uint(action["expected_revision"], "$kind.expected_revision") > BigInteger.ZERO)
            }
            "TakedownArtifact" -> {
                exact(action, setOf("release", "reason", "expected_artifact_governance_revision"), kind)
                musubiRelease(objectValue(action["release"], "$kind.release"), "$kind.release")
                reason(stringTuple(action["reason"], "$kind.reason"), "$kind.reason")
                require(
                    uint(
                        action["expected_artifact_governance_revision"],
                        "$kind.expected_artifact_governance_revision",
                    ) > BigInteger.ZERO,
                )
            }
            "SetRegistryPolicy" -> {
                exact(action, setOf("policy", "expected_revision"), kind)
                val expected = uint(action["expected_revision"], "$kind.expected_revision")
                require(expected > BigInteger.ZERO)
                val revision = musubiRegistryPolicy(objectValue(action["policy"], "$kind.policy"), "$kind.policy")
                require(revision == expected + BigInteger.ONE) { "policy revision must follow expected_revision" }
            }
            else -> throw IllegalArgumentException("Musubi governance action is unsupported")
        }
    }

    private fun musubiPackage(value: Map<String, Any?>, label: String) {
        exact(value, setOf("home_dataspace", "scope", "name"), label)
        uint(value["home_dataspace"], "$label.home_dataspace")
        val scope = objectValue(value["scope"], "$label.scope")
        exact(scope, setOf("kind", "value"), "$label.scope")
        when (text(scope["kind"], "$label.scope.kind")) {
            "DataspaceRoot" -> require(scope["value"] == null) { "$label.scope.value must be null" }
            "Domain" -> canonicalName(scope["value"], "$label.scope.value")
            else -> throw IllegalArgumentException("$label.scope.kind is unsupported")
        }
        kebab(stringTuple(value["name"], "$label.name"), "$label.name", 64)
    }

    private fun musubiRelease(value: Map<String, Any?>, label: String) {
        exact(value, setOf("package", "version"), label)
        musubiPackage(objectValue(value["package"], "$label.package"), "$label.package")
        val version = objectValue(value["version"], "$label.version")
        exact(version, setOf("major", "minor", "patch", "prerelease"), "$label.version")
        uint(version["major"], "$label.version.major")
        uint(version["minor"], "$label.version.minor")
        uint(version["patch"], "$label.version.patch")
        val prerelease = list(version["prerelease"], "$label.version.prerelease")
        require(prerelease.size <= 16)
        prerelease.forEachIndexed { index, item ->
            val identifier = objectValue(item, "$label.version.prerelease[$index]")
            exact(identifier, setOf("kind", "value"), "$label.version.prerelease[$index]")
            when (text(identifier["kind"], "$label.version.prerelease[$index].kind")) {
                "Numeric" -> uint(identifier["value"], "$label.version.prerelease[$index].value")
                "AlphaNumeric" -> {
                    val literal = text(identifier["value"], "$label.version.prerelease[$index].value")
                    require(literal.toByteArray(StandardCharsets.UTF_8).size <= 64 && ALPHANUMERIC_PRERELEASE.matches(literal))
                }
                else -> throw IllegalArgumentException("unsupported prerelease identifier")
            }
        }
    }

    private fun musubiRegistryPolicy(value: Map<String, Any?>, label: String): BigInteger {
        exact(value, setOf("version", "revision", "mode", "allowlisted_dataspaces", "alias_pricing"), label)
        require(uint(value["version"], "$label.version") == BigInteger.ONE)
        val revision = uint(value["revision"], "$label.revision")
        require(revision > BigInteger.ZERO)
        val mode = objectValue(value["mode"], "$label.mode")
        exact(mode, setOf("kind", "value"), "$label.mode")
        val modeKind = text(mode["kind"], "$label.mode.kind")
        require(modeKind in setOf("Closed", "Allowlisted", "Open") && mode["value"] == null)
        val allowed = list(value["allowlisted_dataspaces"], "$label.allowlisted_dataspaces").mapIndexed { index, item ->
            uint(item, "$label.allowlisted_dataspaces[$index]")
        }
        require(allowed.zipWithNext().all { (left, right) -> left < right })
        require(modeKind == "Allowlisted" || allowed.isEmpty())
        val pricing = objectValue(value["alias_pricing"], "$label.alias_pricing")
        val pricingFields = setOf(
            "revision", "length_1_xor", "length_2_xor", "length_3_xor",
            "length_4_xor", "length_5_to_32_xor",
        )
        exact(pricing, pricingFields, "$label.alias_pricing")
        pricingFields.forEach { field ->
            require(uint(pricing[field], "$label.alias_pricing.$field") > BigInteger.ZERO)
        }
        return revision
    }

    private fun sorafsProvider(value: Map<String, Any?>) {
        exact(value, setOf("action"), "SorafsProviderGovernance")
        val action = objectValue(value["action"], "SorafsProviderGovernance.action")
        exact(action, setOf("action", "value"), "SorafsProviderGovernance.action")
        val kind = text(action["action"], "SorafsProviderGovernance.action.action")
        val payload = objectValue(action["value"], "SorafsProviderGovernance.action.value")
        when (kind) {
            "establish" -> {
                exact(payload, setOf("provider_id", "owner"), "provider establish")
                providerId(payload["provider_id"], "provider_id")
                account(payload["owner"], "owner")
            }
            "rebind" -> {
                exact(payload, setOf("provider_id", "expected_owner", "next_owner"), "provider rebind")
                providerId(payload["provider_id"], "provider_id")
                val current = account(payload["expected_owner"], "expected_owner")
                val next = account(payload["next_owner"], "next_owner")
                require(current != next) { "next_owner must differ from expected_owner" }
            }
            "remove" -> {
                exact(payload, setOf("provider_id", "expected_owner"), "provider remove")
                providerId(payload["provider_id"], "provider_id")
                account(payload["expected_owner"], "expected_owner")
            }
            else -> throw IllegalArgumentException("Sorafs provider action is unsupported")
        }
    }

    private fun providerId(value: Any?, label: String) {
        val tuple = list(value, label)
        require(tuple.size == 1) { "$label must use the exact ProviderId tuple" }
        bytes(tuple[0], 32, "$label[0]", true)
    }

    private fun account(value: Any?, label: String): String =
        requireCanonicalI105Address(text(value, label), label)

    private fun asset(value: Any?, label: String): String {
        val literal = text(value, label)
        require(AssetDefinitionIdEncoder.isCanonicalAddress(literal)) { "$label must be a canonical AssetDefinitionId" }
        return literal
    }

    private fun quantity(value: Any?, label: String): String {
        val literal = string(value, label)
        require(Regex("(?:0|[1-9][0-9]*)(?:\\.[0-9]*[1-9])?").matches(literal)) {
            "$label must be a canonical non-negative quantity"
        }
        return literal
    }

    private fun canonicalBase64(value: Any?, label: String) {
        val literal = string(value, label)
        val decoded = try {
            Base64.getDecoder().decode(literal)
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("$label must be canonical padded base64", ex)
        }
        require(Base64.getEncoder().encodeToString(decoded) == literal) {
            "$label must be canonical padded base64"
        }
    }

    private fun canonicalPublicKey(value: Any?, label: String) {
        val literal = text(value, label)
        val parsed = requireNotNull(decodePublicKeyLiteral(literal)) {
            "$label must be a canonical public-key multihash"
        }
        require(encodePublicKeyMultihash(parsed.curveId, parsed.keyBytes) == literal) {
            "$label must use the canonical bare public-key spelling"
        }
    }

    private fun canonicalSignature(value: Any?, label: String) {
        val literal = string(value, label)
        require(literal.length % 2 == 0 && Regex("[0-9A-F]+").matches(literal)) {
            "$label must be nonempty canonical uppercase hexadecimal"
        }
        require(literal.chunked(2).any { it != "00" }) { "$label must be nonzero" }
    }

    private fun stringTuple(value: Any?, label: String): String {
        val tuple = list(value, label)
        require(tuple.size == 1 && tuple[0] is String) { "$label must use an exact one-string tuple" }
        return tuple[0] as String
    }

    private fun kebab(value: String, label: String, maximumBytes: Int) {
        require(value.toByteArray(StandardCharsets.UTF_8).size <= maximumBytes && KEBAB.matches(value)) {
            "$label must be canonical lowercase ASCII kebab text"
        }
    }

    private fun canonicalName(value: Any?, label: String) {
        val literal = text(value, label)
        require(
            literal.toByteArray(StandardCharsets.UTF_8).size <= 255 &&
                Normalizer.normalize(literal, Normalizer.Form.NFC) == literal &&
                literal.none { it.isWhitespace() || it in setOf('@', '#', '$') || Character.isISOControl(it) } &&
                literal.none { it.code == 0x061c || it.code in 0x200e..0x200f || it.code in 0x202a..0x202e || it.code in 0x2066..0x2069 },
        ) { "$label must be a canonical Iroha Name" }
    }

    private fun reason(value: String, label: String) {
        require(
            value.isNotEmpty() && value == value.trim() &&
                value.toByteArray(StandardCharsets.UTF_8).size <= 1024 &&
                value.none { it.code in 0..0x1f || it.code == 0x7f },
        ) { "$label must be bounded canonical public text" }
    }

    private fun lowerHex32(value: Any?, label: String) {
        require(value is String && Regex("[0-9a-f]{64}").matches(value)) {
            "$label must contain 32 lowercase hexadecimal bytes"
        }
    }

    private fun bytes(value: Any?, size: Int, label: String, nonzero: Boolean): ByteArray {
        val items = list(value, label)
        require(items.size == size) { "$label must contain exactly $size bytes" }
        val result = ByteArray(size)
        items.forEachIndexed { index, item ->
            val parsed = uint(item, "$label[$index]")
            require(parsed <= BigInteger.valueOf(255)) { "$label[$index] must be a byte" }
            result[index] = parsed.toByte()
        }
        require(!nonzero || result.any { it.toInt() != 0 }) { "$label must be nonzero" }
        return result
    }

    private fun u64String(value: Any?, label: String, positive: Boolean): BigInteger {
        val literal = string(value, label)
        require(Regex("0|[1-9][0-9]*").matches(literal)) { "$label must be a canonical u64 decimal string" }
        val parsed = BigInteger(literal)
        require(parsed <= U64_MAX && (!positive || parsed > BigInteger.ZERO)) { "$label is outside u64" }
        return parsed
    }

    private fun uint(value: Any?, label: String): BigInteger {
        require(value is Number) { "$label must be an unsigned JSON integer" }
        val literal = value.toString()
        require(Regex("0|[1-9][0-9]*").matches(literal)) { "$label must be an unsigned JSON integer" }
        val parsed = BigInteger(literal)
        require(parsed <= FIRST_RELEASE_MAX_EXACT_JSON_U64) {
            "$label exceeds the first-release exact JSON integer bound"
        }
        return parsed
    }

    private fun text(value: Any?, label: String): String {
        val literal = string(value, label)
        require(literal.isNotEmpty() && literal == literal.trim()) { "$label must be canonical nonempty text" }
        return literal
    }

    private fun string(value: Any?, label: String): String =
        value as? String ?: throw IllegalArgumentException("$label must be a string")

    private fun list(value: Any?, label: String): List<*> =
        value as? List<*> ?: throw IllegalArgumentException("$label must be an array")

    @Suppress("UNCHECKED_CAST")
    private fun objectValue(value: Any?, label: String): Map<String, Any?> {
        require(value is Map<*, *> && value.keys.all { it is String }) { "$label must be an object" }
        return value as Map<String, Any?>
    }

    private fun exact(value: Map<String, Any?>, fields: Set<String>, label: String) {
        require(value.keys == fields) { "$label contains unknown, aliased, or missing fields" }
    }
}
