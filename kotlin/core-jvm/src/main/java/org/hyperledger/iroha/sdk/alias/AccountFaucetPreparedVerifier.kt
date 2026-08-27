package org.hyperledger.iroha.sdk.alias

import java.security.MessageDigest
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.model.instructions.RegisterAccountWirePayloadEncoder
import org.hyperledger.iroha.sdk.core.model.instructions.TransferWirePayloadEncoder
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher
import org.hyperledger.iroha.sdk.tx.norito.SignedTransactionEncoder
import org.hyperledger.iroha.sdk.tx.norito.TransactionPayloadAdapter

/** Fail-closed verification for first-release account-faucet prepare and submit results. */
object AccountFaucetPreparedVerifier {

    /**
     * Authenticates one exact prepared faucet transaction against independent fee and faucet
     * policy expectations.
     */
    @JvmStatic
    fun requireValidPrepared(
        prepared: AccountFaucetPreparedTransactionV1,
        claim: AccountFaucetClaimV1,
        binding: TairaPublicResetMutationBindingV1,
        expectedFeePayment: FeePaymentIntent,
        policy: AccountFaucetPolicyV1,
        expectedNetworkId: NetworkId,
    ): SignedTransaction {
        require(prepared.binding.toJsonMap() == binding.toJsonMap() && prepared.claim.toJsonMap() == claim.toJsonMap()) {
            "prepared faucet envelope differs from the exact claim or binding"
        }
        require(prepared.semanticHashHex == claim.semanticHashHex() && prepared.accountId == claim.accountId) {
            "prepared faucet semantic identity differs from the exact claim"
        }
        require(
            prepared.assetDefinitionId == policy.assetDefinitionId &&
                prepared.assetId == "${policy.assetDefinitionId}#${claim.accountId}" &&
                prepared.amount == policy.amount,
        ) {
            "prepared faucet asset definition, destination, or amount differs from trusted policy"
        }
        require(expectedFeePayment.hasSamePayerAndGasBound(prepared.feePayment)) {
            "prepared faucet fee intent changed payer, sponsor revision, or gas bound"
        }
        require(
            AccountOnboardingReceiptVerifier.verifyAuthoritySignature(
                policy.faucetAuthority,
                PreparedTransactionSignatureV1.digest(PreparedTransactionSignatureV1.faucetPrepared(prepared)),
                prepared.serverSignature,
            ),
        ) { "prepared faucet server signature is invalid for the trusted authority" }

        val wire = decodeLowerHex(prepared.signedTransactionWireHex)
        require(hexLower(MessageDigest.getInstance("SHA-256").digest(wire)) == prepared.signedTransactionWireSha256) {
            "prepared faucet wire SHA-256 differs from the envelope"
        }
        val transaction = try {
            SignedTransactionEncoder.decodeVersioned(wire).also { decoded ->
                require(SignedTransactionEncoder.encodeVersioned(decoded).contentEquals(wire)) {
                    "prepared faucet wire is not canonical fixed-V1 SignedTransaction"
                }
            }
        } catch (error: RuntimeException) {
            throw IllegalArgumentException("prepared faucet wire is invalid", error)
        }
        require(SignedTransactionHasher.hashHex(transaction) == prepared.transactionHashHex) {
            "prepared faucet transaction hash differs from the envelope"
        }
        val payload = TransactionPayloadAdapter.validateCanonicalPayloadBytes(transaction.encodedPayload())
        require(
            AccountOnboardingReceiptVerifier.verifyAuthoritySignature(
                payload.authority,
                IrohaHash.prehash(transaction.encodedPayload()),
                transaction.signature(),
            ),
        ) { "prepared faucet transaction signature is invalid" }
        require(
            payload.networkId == expectedNetworkId &&
                AccountOnboardingReceiptVerifier.sameAccountIdentity(payload.authority, policy.faucetAuthority),
        ) { "prepared faucet transaction network or authority was substituted" }
        require(payload.feePayment == prepared.feePayment) {
            "prepared faucet fee intent differs from the signed transaction"
        }
        val expectedMetadata = mapOf(
            "taira_public_reset_binding" to JsonValue.parse(JsonEncoder.encode(binding.toJsonMap())),
            "taira_prepared_operation" to JsonValue.string(AccountFaucetPreparedTransactionV1.OPERATION),
            "taira_prepared_semantic_hash" to JsonValue.string(prepared.semanticHashHex),
        )
        require(payload.metadata == expectedMetadata) {
            "prepared faucet transaction metadata differs from the envelope"
        }
        requireExactInstructions(payload.executable, claim, policy)
        return transaction
    }

    /** Reconciles only a policy-authenticated prepared faucet envelope and its exact submit hash. */
    @JvmStatic
    fun requireValidSubmitResponse(
        response: PreparedTransactionSubmitResponseV1,
        prepared: AccountFaucetPreparedTransactionV1,
        expectedFeePayment: FeePaymentIntent,
        policy: AccountFaucetPolicyV1,
        expectedNetworkId: NetworkId,
        httpStatus: Int,
    ): PreparedTransactionSubmitResponseV1 {
        requireValidPrepared(
            prepared,
            prepared.claim,
            prepared.binding,
            expectedFeePayment,
            policy,
            expectedNetworkId,
        )
        require(httpStatus == 200 || httpStatus == 202) {
            "prepared faucet submit requires HTTP 200 or 202"
        }
        require(
            response.binding.toJsonMap() == prepared.binding.toJsonMap() &&
                response.operation == prepared.operation &&
                response.transactionHashHex == prepared.transactionHashHex,
        ) { "prepared faucet submit response is not bound to the exact envelope" }
        require(httpStatus != 202 || response.outcome == PreparedTransactionOutcomeV1.PENDING) {
            "HTTP 202 prepared faucet submit must remain Pending"
        }
        return response
    }

    private fun requireExactInstructions(
        executable: Executable,
        claim: AccountFaucetClaimV1,
        policy: AccountFaucetPolicyV1,
    ) {
        require(executable is Executable.Instructions) {
            "prepared faucet transaction must contain a direct instruction sequence"
        }
        val transfer = TransferWirePayloadEncoder.encodeAssetTransfer(
            "${policy.assetDefinitionId}#${policy.faucetAuthority}",
            policy.amount,
            claim.accountId,
        )
        val withoutRegistration = listOf(transfer)
        val withRegistration = listOf(
            RegisterAccountWirePayloadEncoder.encodeRegisterAccount(claim.accountId),
            transfer,
        )
        require(executable.instructions == withoutRegistration || executable.instructions == withRegistration) {
            "prepared faucet transaction instructions differ from the exact claim and trusted policy"
        }
    }

    private fun hexLower(bytes: ByteArray): String {
        val digits = "0123456789abcdef"
        return buildString(bytes.size * 2) {
            bytes.forEach { byte ->
                val value = byte.toInt() and 0xff
                append(digits[value ushr 4])
                append(digits[value and 0x0f])
            }
        }
    }
}
