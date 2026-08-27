package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.util.Optional
import java.util.concurrent.CompletableFuture
import org.hyperledger.iroha.sdk.alias.AliasSetupPlanRequestV1
import org.hyperledger.iroha.sdk.alias.AliasEnsureInstructionFrameCodec
import org.hyperledger.iroha.sdk.alias.AliasAutoRenewPlanRequestV1
import org.hyperledger.iroha.sdk.alias.AliasLeaseRenewPlanRequestV1
import org.hyperledger.iroha.sdk.alias.AliasLifecycleInstructionFrameCodec
import org.hyperledger.iroha.sdk.alias.AliasLifecyclePlanApply
import org.hyperledger.iroha.sdk.alias.AliasLifecyclePlanBodyNoritoEncoder
import org.hyperledger.iroha.sdk.alias.AliasLifecyclePlanRequestV1
import org.hyperledger.iroha.sdk.alias.AliasLifecycleTransactionPlanV1
import org.hyperledger.iroha.sdk.alias.AliasPlanApply
import org.hyperledger.iroha.sdk.alias.AliasPlanBodyNoritoEncoder
import org.hyperledger.iroha.sdk.alias.AliasTransactionPlanV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPlanReceiptV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPlanRequestV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPrepareResponseV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPreparedTransactionV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingProofRequiredPrepareResponseV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingCurrentStateV1
import org.hyperledger.iroha.sdk.alias.AccountFaucetClaimV1
import org.hyperledger.iroha.sdk.alias.AccountFaucetPolicyV1
import org.hyperledger.iroha.sdk.alias.AccountFaucetPreparedTransactionV1
import org.hyperledger.iroha.sdk.alias.PreparedTransactionSubmitResponseV1
import org.hyperledger.iroha.sdk.alias.TairaPublicResetMutationBindingV1
import org.hyperledger.iroha.sdk.alias.AliasSetupReportV1
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.consensus.SumeragiDiagnosticsStatus
import org.hyperledger.iroha.sdk.consensus.SumeragiV2Status
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.tx.TransactionBuilder
import org.hyperledger.iroha.sdk.tx.SignedTransaction

/** High-level client for interacting with Iroha nodes. */
interface IrohaClient {

    /**
     * Submits a signed transaction to the node.
     *
     * The signed bytes are dispatched at most once. A transport or ambiguous HTTP failure completes
     * the future with [AmbiguousTransactionSubmissionException]; reconcile its transaction hash
     * before constructing and signing any replacement. A non-canonical admission response completes
     * the future with [TransactionSubmissionHttpException]; HTTP 202 is the sole success status.
     */
    fun submitTransaction(transaction: SignedTransaction): CompletableFuture<ClientResponse>

    /**
     * Submits a version-tagged SignedTransaction encoded as canonical Norito JSON.
     *
     * This helper is for callers that already have the direct Torii JSON ingress envelope.
     */
    fun submitTransactionJson(encodedVersionedTransactionJson: ByteArray): CompletableFuture<ClientResponse> {
        val future = CompletableFuture<ClientResponse>()
        future.completeExceptionally(
            IllegalStateException("submitTransactionJson requires a concrete IrohaClient implementation")
        )
        return future
    }

    /** Submits one canonical SCCP destination-proof artifact. */
    fun submitSccpDestinationProof(
        request: SccpDestinationProofSubmitRequest,
    ): CompletableFuture<ClientResponse> {
        val future = CompletableFuture<ClientResponse>()
        future.completeExceptionally(
            UnsupportedOperationException("submitSccpDestinationProof not supported")
        )
        return future
    }

    /** Submits one canonical protocol-native external-to-SORA SCCP proof. */
    fun submitSccpNativeMessage(
        request: SccpNativeMessageSubmitRequest,
    ): CompletableFuture<ClientResponse> {
        val future = CompletableFuture<ClientResponse>()
        future.completeExceptionally(
            UnsupportedOperationException("submitSccpNativeMessage not supported")
        )
        return future
    }

    /**
     * Submits an already versioned Norito transaction entrypoint to the node.
     *
     * This is intended for sealed commitment/reveal entrypoints and other non-legacy transaction
     * envelopes that are not represented as a plain [SignedTransaction].
     */
    fun submitTransactionEntrypoint(encodedVersionedEntrypoint: ByteArray): CompletableFuture<ClientResponse> {
        val future = CompletableFuture<ClientResponse>()
        future.completeExceptionally(
            IllegalStateException("submitTransactionEntrypoint requires a concrete IrohaClient implementation")
        )
        return future
    }

    /**
     * Submits a version-tagged TransactionEntrypoint encoded as canonical Norito JSON.
     */
    fun submitTransactionEntrypointJson(encodedVersionedEntrypointJson: ByteArray): CompletableFuture<ClientResponse> {
        val future = CompletableFuture<ClientResponse>()
        future.completeExceptionally(
            IllegalStateException("submitTransactionEntrypointJson requires a concrete IrohaClient implementation")
        )
        return future
    }

    /**
     * Polls the pipeline status endpoint until the transaction reaches a terminal state.
     *
     * The default implementation reports that the operation is unsupported.
     */
    fun waitForTransactionStatus(
        hashHex: String,
        options: PipelineStatusOptions?,
    ): CompletableFuture<Map<String, Any>> {
        val future = CompletableFuture<Map<String, Any>>()
        future.completeExceptionally(
            IllegalStateException("waitForTransactionStatus requires a concrete IrohaClient implementation")
        )
        return future
    }

    /**
     * Proposes a generic multisig instruction batch through Torii's `/v1/multisig/propose`.
     *
     * Implementations should encode request instructions as base64 native Norito `InstructionBox`
     * frames in the JSON body.
     */
    fun proposeMultisig(request: MultisigProposeRequest): CompletableFuture<MultisigResponse> {
        val future = CompletableFuture<MultisigResponse>()
        future.completeExceptionally(
            IllegalStateException("proposeMultisig requires a concrete IrohaClient implementation")
        )
        return future
    }

    /**
     * Resolves an account alias to its underlying Iroha account id via Torii's
     * `/v1/aliases/resolve` endpoint.
     *
     * The default implementation reports that the operation is unsupported.
     */
    fun resolveAccountAlias(alias: String): CompletableFuture<Optional<AccountAliasResolution>> {
        val future = CompletableFuture<Optional<AccountAliasResolution>>()
        future.completeExceptionally(
            IllegalStateException("resolveAccountAlias requires a concrete IrohaClient implementation")
        )
        return future
    }

    /** Resolves a restricted account alias with canonical Iroha request authentication. */
    fun resolveAccountAlias(
        alias: String,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<Optional<AccountAliasResolution>> {
        val future = CompletableFuture<Optional<AccountAliasResolution>>()
        future.completeExceptionally(
            IllegalStateException("authenticated resolveAccountAlias requires a concrete IrohaClient implementation")
        )
        return future
    }

    /** Plans one indivisible alias setup request using canonical request authentication. */
    fun planAliasSetup(
        request: AliasSetupPlanRequestV1,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AliasTransactionPlanV1> {
        val future = CompletableFuture<AliasTransactionPlanV1>()
        future.completeExceptionally(
            IllegalStateException("planAliasSetup requires a concrete IrohaClient implementation")
        )
        return future
    }

    /** Plans one expiry-CAS lease renewal using canonical request authentication. */
    fun planAliasLeaseRenewal(
        request: AliasLeaseRenewPlanRequestV1,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AliasLifecycleTransactionPlanV1> {
        val future = CompletableFuture<AliasLifecycleTransactionPlanV1>()
        future.completeExceptionally(
            IllegalStateException("planAliasLeaseRenewal requires a concrete IrohaClient implementation")
        )
        return future
    }

    /** Plans one revision-CAS native auto-renew configuration. */
    fun planAliasAutoRenew(
        request: AliasAutoRenewPlanRequestV1,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AliasLifecycleTransactionPlanV1> {
        val future = CompletableFuture<AliasLifecycleTransactionPlanV1>()
        future.completeExceptionally(
            IllegalStateException("planAliasAutoRenew requires a concrete IrohaClient implementation")
        )
        return future
    }

    /** Verifies, locally signs, and submits one plan through the ordinary transaction endpoint. */
    fun submitAliasSetupPlan(
        request: AliasSetupPlanRequestV1,
        plan: AliasTransactionPlanV1,
        networkId: NetworkId,
        bodyEncoder: AliasPlanBodyNoritoEncoder,
        frameCodec: AliasEnsureInstructionFrameCodec,
        chainDiscriminant: Int,
        transactionBuilder: TransactionBuilder,
        signer: Signer,
        feePayment: FeePaymentIntent,
        creationTimeMs: Long = System.currentTimeMillis(),
        nonce: Long? = null,
    ): CompletableFuture<ClientResponse> = AliasPlanApply.signAndSubmit(
        this,
        request,
        plan,
        networkId,
        bodyEncoder,
        frameCodec,
        chainDiscriminant,
        transactionBuilder,
        signer,
        feePayment,
        creationTimeMs,
        nonce,
    )

    /** Verifies with the canonical alias codecs, locally signs, and submits one setup plan. */
    fun submitAliasSetupPlan(
        request: AliasSetupPlanRequestV1,
        plan: AliasTransactionPlanV1,
        networkId: NetworkId,
        chainDiscriminant: Int,
        transactionBuilder: TransactionBuilder,
        signer: Signer,
        feePayment: FeePaymentIntent,
        creationTimeMs: Long = System.currentTimeMillis(),
        nonce: Long? = null,
    ): CompletableFuture<ClientResponse> = AliasPlanApply.signAndSubmit(
        this,
        request,
        plan,
        networkId,
        chainDiscriminant,
        transactionBuilder,
        signer,
        feePayment,
        creationTimeMs,
        nonce,
    )

    /** Verifies, locally signs, and submits one lease or auto-renew lifecycle plan. */
    fun submitAliasLifecyclePlan(
        request: AliasLifecyclePlanRequestV1,
        plan: AliasLifecycleTransactionPlanV1,
        networkId: NetworkId,
        bodyEncoder: AliasLifecyclePlanBodyNoritoEncoder,
        frameCodec: AliasLifecycleInstructionFrameCodec,
        chainDiscriminant: Int,
        transactionBuilder: TransactionBuilder,
        signer: Signer,
        feePayment: FeePaymentIntent,
        creationTimeMs: Long = System.currentTimeMillis(),
        nonce: Long? = null,
    ): CompletableFuture<ClientResponse> = AliasLifecyclePlanApply.signAndSubmit(
        this,
        request,
        plan,
        networkId,
        bodyEncoder,
        frameCodec,
        chainDiscriminant,
        transactionBuilder,
        signer,
        feePayment,
        creationTimeMs,
        nonce,
    )

    /** Verifies with canonical alias codecs, locally signs, and submits one lifecycle plan. */
    fun submitAliasLifecyclePlan(
        request: AliasLifecyclePlanRequestV1,
        plan: AliasLifecycleTransactionPlanV1,
        networkId: NetworkId,
        chainDiscriminant: Int,
        transactionBuilder: TransactionBuilder,
        signer: Signer,
        feePayment: FeePaymentIntent,
        creationTimeMs: Long = System.currentTimeMillis(),
        nonce: Long? = null,
    ): CompletableFuture<ClientResponse> = AliasLifecyclePlanApply.signAndSubmit(
        this,
        request,
        plan,
        networkId,
        chainDiscriminant,
        transactionBuilder,
        signer,
        feePayment,
        creationTimeMs,
        nonce,
    )

    /** Resolves a public numeric alias index. */
    fun resolveAccountAliasIndex(index: BigInteger): CompletableFuture<Optional<AccountAliasIndexResolution>> {
        val future = CompletableFuture<Optional<AccountAliasIndexResolution>>()
        future.completeExceptionally(
            IllegalStateException("resolveAccountAliasIndex requires a concrete IrohaClient implementation")
        )
        return future
    }

    /** Resolves a restricted numeric alias index with canonical request authentication. */
    fun resolveAccountAliasIndex(
        index: BigInteger,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<Optional<AccountAliasIndexResolution>> {
        val future = CompletableFuture<Optional<AccountAliasIndexResolution>>()
        future.completeExceptionally(
            IllegalStateException("authenticated resolveAccountAliasIndex requires a concrete IrohaClient implementation")
        )
        return future
    }

    /** Lists visible aliases bound to an account. */
    fun listAccountAliases(
        request: AccountAliasesByAccountRequest,
    ): CompletableFuture<Optional<AccountAliasesByAccount>> {
        val future = CompletableFuture<Optional<AccountAliasesByAccount>>()
        future.completeExceptionally(
            IllegalStateException("listAccountAliases requires a concrete IrohaClient implementation")
        )
        return future
    }

    /** Requests a receipt and pins its signature to the configured onboarding authority. */
    fun planSponsoredAccountOnboarding(
        request: AccountOnboardingPlanRequestV1,
        onboardingToken: String,
        expectedAuthority: String,
        expectedNetworkId: NetworkId,
    ): CompletableFuture<AccountOnboardingPlanReceiptV1>

    /** Revalidates a receipt and required fee intent, then returns an exact transaction or live-proof requirement. */
    fun prepareSponsoredAccountOnboarding(
        request: AccountOnboardingPlanRequestV1,
        receipt: AccountOnboardingPlanReceiptV1,
        binding: TairaPublicResetMutationBindingV1,
        feePayment: FeePaymentIntent,
        onboardingToken: String,
        expectedAuthority: String,
        expectedNetworkId: NetworkId,
    ): CompletableFuture<AccountOnboardingPrepareResponseV1>

    /** Reauthenticates ProofRequired and obtains one atomic committed account-and-alias state. */
    fun verifyAccountOnboardingCurrentState(
        proofRequired: AccountOnboardingProofRequiredPrepareResponseV1,
        request: AccountOnboardingPlanRequestV1,
        receipt: AccountOnboardingPlanReceiptV1,
        binding: TairaPublicResetMutationBindingV1,
        expectedAuthority: String,
        expectedNetworkId: NetworkId,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AccountOnboardingCurrentStateV1>

    /** Submits an exact prepared onboarding envelope only if it preserves an independent expected fee intent. */
    fun submitPreparedAccountOnboarding(
        request: AccountOnboardingPlanRequestV1,
        prepared: AccountOnboardingPreparedTransactionV1,
        expectedFeePayment: FeePaymentIntent,
        onboardingToken: String,
        expectedAuthority: String,
        expectedNetworkId: NetworkId,
    ): CompletableFuture<PreparedTransactionSubmitResponseV1>

    /** Prepares and authenticates one exact faucet transaction against independent local policy. */
    fun prepareAccountFaucetTransaction(
        claim: AccountFaucetClaimV1,
        binding: TairaPublicResetMutationBindingV1,
        feePayment: FeePaymentIntent,
        policy: AccountFaucetPolicyV1,
        expectedNetworkId: NetworkId,
    ): CompletableFuture<AccountFaucetPreparedTransactionV1>

    /** Submits only a faucet envelope that still matches independent fee and faucet policy. */
    fun submitPreparedAccountFaucetTransaction(
        prepared: AccountFaucetPreparedTransactionV1,
        expectedFeePayment: FeePaymentIntent,
        policy: AccountFaucetPolicyV1,
        expectedNetworkId: NetworkId,
    ): CompletableFuture<PreparedTransactionSubmitResponseV1>

    /** Fetches authenticated, secret-free onboarding readiness diagnostics. */
    fun getAccountOnboardingReadiness(
        onboardingToken: String,
    ): CompletableFuture<AliasSetupReportV1> {
        val future = CompletableFuture<AliasSetupReportV1>()
        future.completeExceptionally(
            IllegalStateException("getAccountOnboardingReadiness requires a concrete IrohaClient implementation")
        )
        return future
    }

    /** Fetches the complete non-authoritative `/v1/sumeragi/diagnostics` payload. */
    fun getSumeragiDiagnostics(): CompletableFuture<SumeragiDiagnosticsStatus> {
        val future = CompletableFuture<SumeragiDiagnosticsStatus>()
        future.completeExceptionally(
            IllegalStateException("getSumeragiDiagnostics requires a concrete IrohaClient implementation")
        )
        return future
    }

    /** Fetches the authoritative, fail-closed `/v1/sumeragi/status` snapshot. */
    fun getSumeragiStatus(): CompletableFuture<SumeragiV2Status> {
        val future = CompletableFuture<SumeragiV2Status>()
        future.completeExceptionally(
            IllegalStateException("getSumeragiStatus requires a concrete IrohaClient implementation")
        )
        return future
    }

    /** Lists visible aliases with canonical request authentication. */
    fun listAccountAliases(
        request: AccountAliasesByAccountRequest,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<Optional<AccountAliasesByAccount>> {
        val future = CompletableFuture<Optional<AccountAliasesByAccount>>()
        future.completeExceptionally(
            IllegalStateException("authenticated listAccountAliases requires a concrete IrohaClient implementation")
        )
        return future
    }

    /** Fetches the complete Kotodama manifest registered for one code hash. */
    fun getContractManifest(codeHash: String): CompletableFuture<ContractManifestRecord> {
        val future = CompletableFuture<ContractManifestRecord>()
        future.completeExceptionally(
            IllegalStateException("getContractManifest requires a concrete IrohaClient implementation")
        )
        return future
    }
}
