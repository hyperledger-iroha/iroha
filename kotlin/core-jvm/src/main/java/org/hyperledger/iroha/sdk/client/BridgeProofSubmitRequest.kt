package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.sccp.EvmSccpSubmission
import org.hyperledger.iroha.sdk.sccp.SccpSourceProofs
import org.hyperledger.iroha.sdk.sccp.TronSccpSubmission

/** Request payload for `POST /v1/bridge/proofs/submit`. */
class BridgeProofSubmitRequest(
    authority: String,
    val privateKey: Any? = null,
    publicKeyHex: String? = null,
    signatureB64: String? = null,
    burnBundle: Map<String, Any?>? = null,
    messageBundle: Map<String, Any?>? = null,
    networkIdHex: String? = null,
    verifierAddressHex: String? = null,
    bridgeAddressHex: String? = null,
    verifierCodeHashHex: String? = null,
    verifierKeyHashHex: String? = null,
    expectedDestinationBindingHashHex: String? = null,
    tronVerifierAddress: String? = null,
    proofBytesHex: String? = null,
    val creationTimeMs: Any? = null,
) {
    val authority: String = requireNonBlank(authority, "authority")
    val publicKeyHex: String? = normalizeOptional(publicKeyHex)
    val signatureB64: String? = normalizeOptional(signatureB64)
    val burnBundle: Map<String, Any?>? = burnBundle?.toMap()
    val messageBundle: Map<String, Any?>? = messageBundle?.toMap()
    val networkIdHex: String? = normalizeOptional(networkIdHex)
    val verifierAddressHex: String? = normalizeOptional(verifierAddressHex)
    val bridgeAddressHex: String? = normalizeOptional(bridgeAddressHex)
    val verifierCodeHashHex: String? = normalizeOptional(verifierCodeHashHex)
    val verifierKeyHashHex: String? = normalizeOptional(verifierKeyHashHex)
    val expectedDestinationBindingHashHex: String? = normalizeOptional(expectedDestinationBindingHashHex)
    val tronVerifierAddress: String? = normalizeOptional(tronVerifierAddress)
    val proofBytesHex: String? = normalizeOptional(proofBytesHex)

    init {
        val bundleCount = listOfNotNull(this.burnBundle, this.messageBundle).size
        require(bundleCount == 1) {
            "bridge proof submit must provide exactly one of burnBundle or messageBundle"
        }
        if (this.burnBundle != null && hasSccpDestinationMaterial()) {
            throw IllegalArgumentException(
                "SCCP destination fields and proofBytesHex are only valid for messageBundle submissions"
            )
        }
    }

    fun toJsonMap(): Map<String, Any?> = buildMap {
        put("authority", authority)
        privateKey?.let { put("private_key", it) }
        publicKeyHex?.let { put("public_key_hex", it) }
        signatureB64?.let { put("signature_b64", it) }
        burnBundle?.let { put("burn_bundle", it) }
        messageBundle?.let { put("message_bundle", it) }
        networkIdHex?.let { put("network_id_hex", it) }
        verifierAddressHex?.let { put("verifier_address_hex", it) }
        bridgeAddressHex?.let { put("bridge_address_hex", it) }
        verifierCodeHashHex?.let { put("verifier_code_hash_hex", it) }
        verifierKeyHashHex?.let { put("verifier_key_hash_hex", it) }
        expectedDestinationBindingHashHex?.let { put("expected_destination_binding_hash_hex", it) }
        tronVerifierAddress?.let { put("tron_verifier_address", it) }
        proofBytesHex?.let { put("proof_bytes_hex", it) }
        creationTimeMs?.let { put("creation_time_ms", it) }
    }

    fun toJsonBytes(): ByteArray =
        JsonEncoder.encode(toJsonMap()).toByteArray(Charsets.UTF_8)

    private fun hasSccpDestinationMaterial(): Boolean =
        networkIdHex != null ||
            verifierAddressHex != null ||
            bridgeAddressHex != null ||
            verifierCodeHashHex != null ||
            verifierKeyHashHex != null ||
            expectedDestinationBindingHashHex != null ||
            tronVerifierAddress != null ||
            proofBytesHex != null

    companion object {
        /** Build an on-chain bridge-proof submit request from an EVM-family SCCP proof submission. */
        @JvmStatic
        @JvmOverloads
        fun fromEvmSccpSubmission(
            authority: String,
            messageBundle: Map<String, Any?>,
            submission: EvmSccpSubmission,
            destinationBinding: SccpSourceProofs.EvmDestinationBinding,
            privateKey: Any? = null,
            publicKeyHex: String? = null,
            signatureB64: String? = null,
            creationTimeMs: Any? = null,
        ): BridgeProofSubmitRequest {
            requireEvmSubmissionMatchesDestination(submission, destinationBinding)
            requireSccpProofMatchesMessageBundle(submission.proofBytes, messageBundle)
            return BridgeProofSubmitRequest(
                authority = authority,
                privateKey = privateKey,
                publicKeyHex = publicKeyHex,
                signatureB64 = signatureB64,
                messageBundle = messageBundle,
                networkIdHex = destinationBinding.networkId,
                verifierAddressHex = destinationBinding.verifierAddress,
                bridgeAddressHex = destinationBinding.bridgeAddress,
                verifierCodeHashHex = destinationBinding.verifierCodeHash,
                verifierKeyHashHex = destinationBinding.verifierKeyHash,
                expectedDestinationBindingHashHex = destinationBinding.hash,
                proofBytesHex = "0x" + hexLower(submission.proofBytes),
                creationTimeMs = creationTimeMs,
            )
        }

        /** Build an on-chain bridge-proof submit request from a TRON SCCP proof submission. */
        @JvmStatic
        @JvmOverloads
        fun fromTronSccpSubmission(
            authority: String,
            messageBundle: Map<String, Any?>,
            submission: TronSccpSubmission,
            destinationBinding: SccpSourceProofs.TronDestinationBinding,
            privateKey: Any? = null,
            publicKeyHex: String? = null,
            signatureB64: String? = null,
            creationTimeMs: Any? = null,
        ): BridgeProofSubmitRequest {
            requireTronSubmissionMatchesDestination(submission, destinationBinding)
            requireSccpProofMatchesMessageBundle(submission.proofBytes, messageBundle)
            return BridgeProofSubmitRequest(
                authority = authority,
                privateKey = privateKey,
                publicKeyHex = publicKeyHex,
                signatureB64 = signatureB64,
                messageBundle = messageBundle,
                networkIdHex = destinationBinding.networkId,
                verifierCodeHashHex = destinationBinding.verifierCodeHash,
                verifierKeyHashHex = destinationBinding.verifierKeyHash,
                expectedDestinationBindingHashHex = destinationBinding.hash,
                tronVerifierAddress = destinationBinding.verifierAddress,
                proofBytesHex = "0x" + hexLower(submission.proofBytes),
                creationTimeMs = creationTimeMs,
            )
        }

        private fun requireEvmSubmissionMatchesDestination(
            submission: EvmSccpSubmission,
            destinationBinding: SccpSourceProofs.EvmDestinationBinding,
        ) {
            require(submission.version == 1) { "EVM SCCP submission version must be 1" }
            require(submission.submissionKind == "contract_call") {
                "EVM SCCP submission must be a contract_call"
            }
            require(submission.sourceDomain == destinationBinding.sourceDomain) {
                "EVM SCCP submission sourceDomain must match destination binding"
            }
            require(submission.targetDomain == destinationBinding.targetDomain) {
                "EVM SCCP submission targetDomain must match destination binding"
            }
            require(submission.verifierBackend == destinationBinding.verifierBackend) {
                "EVM SCCP submission verifierBackend must match destination binding"
            }
            require(submission.proofFamily == destinationBinding.proofFamily) {
                "EVM SCCP submission proofFamily must match destination binding"
            }
            require(submission.destinationBindingHash == destinationBinding.hash) {
                "EVM SCCP submission destinationBindingHash must match destination binding"
            }
        }

        private fun requireTronSubmissionMatchesDestination(
            submission: TronSccpSubmission,
            destinationBinding: SccpSourceProofs.TronDestinationBinding,
        ) {
            require(submission.version == 1) { "TRON SCCP submission version must be 1" }
            require(submission.submissionKind == "contract_call") {
                "TRON SCCP submission must be a contract_call"
            }
            require(submission.sourceDomain == destinationBinding.sourceDomain) {
                "TRON SCCP submission sourceDomain must match destination binding"
            }
            require(submission.targetDomain == destinationBinding.targetDomain) {
                "TRON SCCP submission targetDomain must match destination binding"
            }
            require(submission.verifierBackend == destinationBinding.verifierBackend) {
                "TRON SCCP submission verifierBackend must match destination binding"
            }
            require(submission.proofFamily == destinationBinding.proofFamily) {
                "TRON SCCP submission proofFamily must match destination binding"
            }
            require(submission.destinationBindingHash == destinationBinding.hash) {
                "TRON SCCP submission destinationBindingHash must match destination binding"
            }
        }

        private fun requireSccpProofMatchesMessageBundle(
            proofBytes: ByteArray,
            messageBundle: Map<String, Any?>,
        ) {
            require(proofBytes.size == SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1) {
                "SCCP proof bytes must be a 384-byte Groth16 ABI tuple"
            }
            require(proofBytes.any { it.toInt() != 0 }) { "SCCP proof bytes must not be all zero" }
            HttpClientTransport.validateSccpGroth16ProofHex(hexLower(proofBytes))
            val commitment = messageBundle["commitment"] as? Map<*, *>
                ?: throw IllegalArgumentException("message_bundle.commitment.message_id is required")
            val messageIdValue = commitment["message_id"] ?: commitment["messageId"]
                ?: throw IllegalArgumentException(
                    "message_bundle.commitment.message_id and message_bundle.commitment_root are required"
                )
            val commitmentRootValue = messageBundle["commitment_root"] ?: messageBundle["commitmentRoot"]
                ?: throw IllegalArgumentException(
                    "message_bundle.commitment.message_id and message_bundle.commitment_root are required"
                )
            require(messageIdValue is String) {
                "message_bundle.commitment.message_id must contain 64 hex characters"
            }
            require(commitmentRootValue is String) {
                "message_bundle.commitment_root must contain 64 hex characters"
            }
            val messageId = normalizeHex32(messageIdValue, "message_bundle.commitment.message_id")
            val commitmentRoot = normalizeHex32(commitmentRootValue, "message_bundle.commitment_root")
            require(proofWordHex(proofBytes, 0) == abiWordU32Hex(1)) {
                "proof_bytes_hex.version must be 1"
            }
            require(proofWordHex(proofBytes, 1) == messageId) {
                "proof_bytes_hex.message_id must match message_bundle.commitment.message_id"
            }
            require(proofWordHex(proofBytes, 2) == abiWordU32Hex(SccpSourceProofs.DOMAIN_SORA)) {
                "proof_bytes_hex.source_domain must be SORA"
            }
            require(proofWordHex(proofBytes, 3) == commitmentRoot) {
                "proof_bytes_hex.commitment_root must match message_bundle.commitment_root"
            }
        }

        private fun normalizeHex32(value: String, field: String): String {
            require(value.trim() == value) { "$field must be a canonical hex string" }
            var normalized = value
            if (normalized.startsWith("0x") || normalized.startsWith("0X")) {
                normalized = normalized.substring(2)
            }
            require(normalized.length == 64) { "$field must contain 64 hex characters" }
            require(normalized.all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }) {
                "$field must contain 64 hex characters"
            }
            return normalized.lowercase()
        }

        private fun abiWordU32Hex(value: Int): String = value.toString(16).padStart(64, '0')

        private fun proofWordHex(proofBytes: ByteArray, index: Int): String {
            val offset = index * 32
            return hexLower(proofBytes.copyOfRange(offset, offset + 32))
        }

        private const val SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1 = 384
    }
}

private fun requireNonBlank(value: String, field: String): String {
    val trimmed = value.trim()
    require(trimmed.isNotEmpty()) { "$field is required" }
    return trimmed
}

private fun normalizeOptional(value: String?): String? {
    if (value == null) return null
    val trimmed = value.trim()
    return trimmed.ifEmpty { null }
}

private fun hexLower(bytes: ByteArray): String {
    val out = StringBuilder(bytes.size * 2)
    for (byte in bytes) {
        val value = byte.toInt() and 0xff
        if (value < 16) out.append('0')
        out.append(value.toString(16))
    }
    return out.toString()
}
