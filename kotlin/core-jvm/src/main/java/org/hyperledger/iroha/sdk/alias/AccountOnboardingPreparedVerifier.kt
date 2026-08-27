package org.hyperledger.iroha.sdk.alias

import java.io.ByteArrayOutputStream
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher
import org.hyperledger.iroha.sdk.tx.norito.SignedTransactionEncoder
import org.hyperledger.iroha.sdk.tx.norito.TransactionPayloadAdapter

/** Stable cross-SDK signature transcript for Taira prepared transactions. */
object PreparedTransactionSignatureV1 {
    const val TRANSCRIPT_SCHEMA: String = "iroha.taira.prepared-signature-transcript.v1"
    private val DOMAIN = "iroha:taira:prepared-transaction:v1\u0000".toByteArray(StandardCharsets.UTF_8)

    /** Exact transcript authenticated by an onboarding prepared envelope. */
    @JvmStatic
    fun onboardingPrepared(envelope: AccountOnboardingPreparedTransactionV1): ByteArray {
        val transcript = base(envelope.schema, envelope.operation, envelope.binding)
        field(transcript, "semantic_hash_hex", envelope.semanticHashHex)
        field(transcript, "account_id", envelope.accountId)
        field(transcript, "alias", envelope.alias)
        field(transcript, "disposition", envelope.disposition.wireValue)
        field(transcript, "transaction_hash_hex", envelope.transactionHashHex)
        field(transcript, "signed_transaction_wire_sha256", envelope.signedTransactionWireSha256)
        field(transcript, "signed_transaction_wire", decodeLowerHex(envelope.signedTransactionWireHex))
        return transcript.toByteArray()
    }

    /** Exact transcript authenticated by a nonterminal onboarding proof requirement. */
    @JvmStatic
    fun onboardingProofRequired(result: AccountOnboardingProofRequiredPrepareResponseV1): ByteArray {
        val transcript = base(result.schema, result.operation, result.binding)
        field(transcript, "outcome", result.outcome)
        field(transcript, "proof_kind", result.proofKind)
        field(transcript, "semantic_hash_hex", result.semanticHashHex)
        field(transcript, "account_id", result.accountId)
        field(transcript, "alias", result.alias)
        field(transcript, "disposition", result.disposition.wireValue)
        return transcript.toByteArray()
    }

    /** Exact transcript authenticated by a faucet prepared envelope. */
    @JvmStatic
    fun faucetPrepared(envelope: AccountFaucetPreparedTransactionV1): ByteArray {
        val transcript = base(envelope.schema, envelope.operation, envelope.binding)
        field(transcript, "claim.account_id", envelope.claim.accountId)
        field(transcript, "claim.pow_anchor_height", envelope.claim.powAnchorHeight.toString())
        field(transcript, "claim.pow_nonce_hex", envelope.claim.powNonceHex)
        field(transcript, "semantic_hash_hex", envelope.semanticHashHex)
        field(transcript, "account_id", envelope.accountId)
        field(transcript, "asset_definition_id", envelope.assetDefinitionId)
        field(transcript, "asset_id", envelope.assetId)
        field(transcript, "amount", envelope.amount.toString())
        field(transcript, "transaction_hash_hex", envelope.transactionHashHex)
        field(transcript, "signed_transaction_wire_sha256", envelope.signedTransactionWireSha256)
        field(transcript, "signed_transaction_wire", decodeLowerHex(envelope.signedTransactionWireHex))
        return transcript.toByteArray()
    }

    /** Iroha BLAKE2b-256 digest signed by the prepared-result authority. */
    @JvmStatic
    fun digest(transcript: ByteArray): ByteArray = IrohaHash.prehash(transcript)

    private fun base(
        envelopeSchema: String,
        operation: String,
        binding: TairaPublicResetMutationBindingV1,
    ): ByteArrayOutputStream = ByteArrayOutputStream().also { transcript ->
        frame(transcript, DOMAIN)
        field(transcript, "transcript_schema", TRANSCRIPT_SCHEMA)
        field(transcript, "envelope_schema", envelopeSchema)
        field(transcript, "operation", operation)
        field(transcript, "binding.schema", binding.schema)
        field(transcript, "binding.authorization_sha256", binding.authorizationSha256)
        field(transcript, "binding.authorization_nonce", binding.authorizationNonce)
        field(transcript, "binding.kind", binding.kind)
        field(transcript, "binding.phase", binding.phase)
        field(transcript, "binding.idempotency_key", binding.idempotencyKey)
        field(
            transcript,
            "binding.execution_expires_at_unix_ms",
            binding.executionExpiresAtUnixMs.toString(),
        )
    }

    private fun field(output: ByteArrayOutputStream, label: String, value: String) {
        field(output, label, value.toByteArray(StandardCharsets.UTF_8))
    }

    private fun field(output: ByteArrayOutputStream, label: String, value: ByteArray) {
        frame(output, label.toByteArray(StandardCharsets.UTF_8))
        frame(output, value)
    }

    private fun frame(output: ByteArrayOutputStream, value: ByteArray) {
        var length = value.size.toLong()
        for (shift in 56 downTo 0 step 8) {
            output.write(((length ushr shift) and 0xffL).toInt())
        }
        output.write(value)
    }
}

/** Fail-closed verification for onboarding prepare and exact-submit responses. */
object AccountOnboardingPreparedVerifier {
    /** Authenticates an exact prepared transaction for the original receipt, binding, and expected fee intent. */
    @JvmStatic
    fun requireValidPrepared(
        prepared: AccountOnboardingPreparedTransactionV1,
        request: AccountOnboardingPlanRequestV1,
        receipt: AccountOnboardingPlanReceiptV1,
        binding: TairaPublicResetMutationBindingV1,
        expectedFeePayment: FeePaymentIntent,
        expectedNetworkId: NetworkId,
        expectedAuthority: String,
    ): SignedTransaction {
        AccountOnboardingReceiptVerifier.requireValidForRequest(
            request,
            receipt,
            expectedNetworkId,
            expectedAuthority,
        )
        require(sameBinding(prepared.binding, binding) && sameReceipt(prepared.receipt, receipt)) {
            "prepared onboarding envelope differs from the exact receipt or binding"
        }
        require(expectedFeePayment.hasSamePayerAndGasBound(prepared.feePayment)) {
            "prepared onboarding fee intent changed payer, sponsor revision, or gas bound"
        }
        val receiptHash = requireNotNull(AliasHashText.decode(receipt.planHash)) {
            "receipt plan hash is invalid"
        }
        require(prepared.semanticHashHex == hexLower(receiptHash)) {
            "prepared onboarding semantic hash differs from the receipt"
        }
        require(
            prepared.accountId == receipt.body.request.accountId &&
                prepared.alias == receipt.body.request.alias &&
                dispositionTransitionAllowed(receipt.body.resource.disposition, prepared.disposition),
        ) {
            "prepared onboarding identity or disposition differs from the receipt"
        }
        val transcript = PreparedTransactionSignatureV1.onboardingPrepared(prepared)
        require(
            AccountOnboardingReceiptVerifier.verifyAuthoritySignature(
                receipt.body.authority,
                PreparedTransactionSignatureV1.digest(transcript),
                prepared.serverSignature,
            ),
        ) { "prepared onboarding server signature is invalid" }

        val wire = decodeLowerHex(prepared.signedTransactionWireHex)
        require(hexLower(MessageDigest.getInstance("SHA-256").digest(wire)) == prepared.signedTransactionWireSha256) {
            "prepared onboarding wire SHA-256 differs from the envelope"
        }
        val transaction = SignedTransactionEncoder.decodeVersioned(wire)
        require(SignedTransactionEncoder.encodeVersioned(transaction).contentEquals(wire)) {
            "prepared onboarding wire is not canonical fixed-V1 SignedTransaction"
        }
        require(SignedTransactionHasher.hashHex(transaction) == prepared.transactionHashHex) {
            "prepared onboarding transaction hash differs from the envelope"
        }
        val payload = TransactionPayloadAdapter.validateCanonicalPayloadBytes(transaction.encodedPayload())
        require(
            AccountOnboardingReceiptVerifier.verifyAuthoritySignature(
                payload.authority,
                IrohaHash.prehash(transaction.encodedPayload()),
                transaction.signature(),
            ),
        ) { "prepared onboarding transaction signature is invalid" }
        require(
            payload.networkId == expectedNetworkId &&
                AccountOnboardingReceiptVerifier.sameAccountIdentity(
                    payload.authority,
                    receipt.body.authority,
                ),
        ) {
            "prepared onboarding transaction network or authority was substituted"
        }
        require(payload.feePayment == prepared.feePayment) {
            "prepared onboarding fee intent differs from the signed transaction"
        }
        val expectedMetadata = linkedMapOf(
            "taira_public_reset_binding" to JsonValue.parse(JsonEncoder.encode(binding.toJsonMap())),
            "taira_prepared_operation" to JsonValue.string(AccountOnboardingPreparedTransactionV1.OPERATION),
            "taira_prepared_semantic_hash" to JsonValue.string(prepared.semanticHashHex),
        )
        require(payload.metadata == expectedMetadata) {
            "prepared onboarding transaction metadata differs from the envelope"
        }
        val executable = payload.executable as? Executable.Instructions
            ?: throw IllegalArgumentException("prepared onboarding transaction must contain instructions")
        val planned = receipt.body.instructions.map {
            InstructionBox.fromWirePayload(it.wireId, it.framedPayload)
        }
        require(executable.instructions.isNotEmpty() && orderedSubset(executable.instructions, planned)) {
            "prepared onboarding instructions are not an ordered subset of the signed receipt"
        }
        return transaction
    }

    /** Authenticates a nonterminal result that still requires one fresh atomic observation. */
    @JvmStatic
    fun requireValidProofRequired(
        proofRequired: AccountOnboardingProofRequiredPrepareResponseV1,
        request: AccountOnboardingPlanRequestV1,
        receipt: AccountOnboardingPlanReceiptV1,
        binding: TairaPublicResetMutationBindingV1,
        expectedNetworkId: NetworkId,
        expectedAuthority: String,
    ): AccountOnboardingProofRequiredPrepareResponseV1 {
        AccountOnboardingReceiptVerifier.requireValidForRequest(
            request,
            receipt,
            expectedNetworkId,
            expectedAuthority,
        )
        val receiptHash = requireNotNull(AliasHashText.decode(receipt.planHash)) {
            "receipt plan hash is invalid"
        }
        require(
            sameBinding(proofRequired.binding, binding) &&
                proofRequired.outcome == AccountOnboardingProofRequiredPrepareResponseV1.OUTCOME &&
                proofRequired.proofKind == AccountOnboardingProofRequiredPrepareResponseV1.PROOF_KIND &&
                proofRequired.semanticHashHex == hexLower(receiptHash) &&
                proofRequired.accountId == receipt.body.request.accountId &&
                proofRequired.alias == receipt.body.request.alias &&
                proofRequired.disposition == AliasPlanDispositionV1.NO_OP,
        ) { "proof-required onboarding result differs from the exact receipt or binding" }
        require(
            AccountOnboardingReceiptVerifier.verifyAuthoritySignature(
                receipt.body.authority,
                PreparedTransactionSignatureV1.digest(
                    PreparedTransactionSignatureV1.onboardingProofRequired(proofRequired),
                ),
                proofRequired.serverSignature,
            ),
        ) { "proof-required onboarding server signature is invalid" }
        return proofRequired
    }

    /** Reconciles only an independently fee-checked prepared envelope and its exact submit result. */
    @JvmStatic
    fun requireValidSubmitResponse(
        response: PreparedTransactionSubmitResponseV1,
        prepared: AccountOnboardingPreparedTransactionV1,
        expectedFeePayment: FeePaymentIntent,
        httpStatus: Int,
    ): PreparedTransactionSubmitResponseV1 {
        require(expectedFeePayment.hasSamePayerAndGasBound(prepared.feePayment)) {
            "prepared onboarding fee intent changed payer, sponsor revision, or gas bound"
        }
        require(httpStatus == 200 || httpStatus == 202) {
            "prepared onboarding submit requires HTTP 200 or 202"
        }
        require(
            sameBinding(response.binding, prepared.binding) &&
                response.operation == prepared.operation &&
                response.transactionHashHex == prepared.transactionHashHex,
        ) { "prepared onboarding submit response is not bound to the exact envelope" }
        require(httpStatus != 202 || response.outcome == PreparedTransactionOutcomeV1.PENDING) {
            "HTTP 202 prepared onboarding submit must remain Pending"
        }
        return response
    }

    private fun dispositionTransitionAllowed(
        planned: AliasPlanDispositionV1,
        live: AliasPlanDispositionV1,
    ): Boolean = when (planned) {
        AliasPlanDispositionV1.CREATE -> live == AliasPlanDispositionV1.CREATE ||
            live == AliasPlanDispositionV1.REPAIR || live == AliasPlanDispositionV1.NO_OP
        AliasPlanDispositionV1.REPAIR -> live == AliasPlanDispositionV1.REPAIR ||
            live == AliasPlanDispositionV1.NO_OP
        AliasPlanDispositionV1.NO_OP -> live == AliasPlanDispositionV1.NO_OP
        AliasPlanDispositionV1.CONFLICT -> false
    }

    private fun orderedSubset(actual: List<InstructionBox>, planned: List<InstructionBox>): Boolean {
        var plannedIndex = 0
        for (instruction in actual) {
            while (plannedIndex < planned.size && planned[plannedIndex] != instruction) plannedIndex++
            if (plannedIndex == planned.size) return false
            plannedIndex++
        }
        return true
    }

    private fun sameBinding(
        left: TairaPublicResetMutationBindingV1,
        right: TairaPublicResetMutationBindingV1,
    ): Boolean = left.toJsonMap() == right.toJsonMap()

    private fun sameReceipt(
        left: AccountOnboardingPlanReceiptV1,
        right: AccountOnboardingPlanReceiptV1,
    ): Boolean = left.toJsonMap() == right.toJsonMap()
}

internal fun decodeLowerHex(value: String): ByteArray {
    require(value.isNotEmpty() && value.length % 2 == 0 && value.all { it in '0'..'9' || it in 'a'..'f' }) {
        "value must be canonical lowercase hexadecimal"
    }
    return ByteArray(value.length / 2) { index ->
        ((Character.digit(value[index * 2], 16) shl 4) or Character.digit(value[index * 2 + 1], 16)).toByte()
    }
}

internal fun hexLower(bytes: ByteArray): String {
    val digits = "0123456789abcdef"
    return buildString(bytes.size * 2) {
        bytes.forEach { byte ->
            val value = byte.toInt() and 0xff
            append(digits[value ushr 4])
            append(digits[value and 0x0f])
        }
    }
}
