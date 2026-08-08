package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.alias.AliasSetupPlanRequestV1;
import org.hyperledger.iroha.android.alias.AliasAutoRenewPlanRequestV1;
import org.hyperledger.iroha.android.alias.AliasLeaseRenewPlanRequestV1;
import org.hyperledger.iroha.android.alias.AliasLifecycleInstructionFrameCodec;
import org.hyperledger.iroha.android.alias.AliasLifecyclePlanApply;
import org.hyperledger.iroha.android.alias.AliasLifecyclePlanBodyNoritoEncoder;
import org.hyperledger.iroha.android.alias.AliasLifecyclePlanRequestV1;
import org.hyperledger.iroha.android.alias.AliasLifecycleTransactionPlanV1;
import org.hyperledger.iroha.android.alias.AliasEnsureInstructionFrameCodec;
import org.hyperledger.iroha.android.alias.AliasPlanApply;
import org.hyperledger.iroha.android.alias.AliasPlanBodyNoritoEncoder;
import org.hyperledger.iroha.android.alias.AliasTransactionPlanV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingPlanReceiptV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingPlanRequestV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingResponseV1;
import org.hyperledger.iroha.android.alias.AliasSetupModels;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.android.crypto.Signer;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.tx.TransactionBuilder;
import org.hyperledger.iroha.android.tx.SignedTransaction;

/** High-level client for interacting with Iroha nodes from Android applications. */
public interface IrohaClient {

  /**
   * Submits a signed transaction to the node.
   *
   * <p>The signed bytes are dispatched at most once. A transport or ambiguous HTTP failure
   * completes the future with {@link AmbiguousTransactionSubmissionException}; reconcile its
   * transaction hash before constructing and signing any replacement.
   */
  CompletableFuture<ClientResponse> submitTransaction(SignedTransaction transaction);

  /**
   * Submits a version-tagged SignedTransaction encoded as canonical Norito JSON.
   *
   * <p>This helper is for callers that already have the direct Torii JSON ingress envelope.
   */
  default CompletableFuture<ClientResponse> submitTransactionJson(
      final byte[] encodedVersionedTransactionJson) {
    final CompletableFuture<ClientResponse> future = new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "submitTransactionJson requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Submits one canonical SCCP destination-proof artifact. */
  default CompletableFuture<ClientResponse> submitSccpDestinationProof(
      final SccpDestinationProofSubmitRequest request) {
    final CompletableFuture<ClientResponse> future = new CompletableFuture<>();
    future.completeExceptionally(
        new UnsupportedOperationException("submitSccpDestinationProof not supported"));
    return future;
  }

  /** Submits one canonical protocol-native external-to-SORA SCCP proof. */
  default CompletableFuture<ClientResponse> submitSccpNativeMessage(
      final SccpNativeMessageSubmitRequest request) {
    final CompletableFuture<ClientResponse> future = new CompletableFuture<>();
    future.completeExceptionally(
        new UnsupportedOperationException("submitSccpNativeMessage not supported"));
    return future;
  }

  /**
   * Submits an already versioned Norito transaction entrypoint to the node.
   *
   * <p>This is intended for sealed commitment/reveal entrypoints and other non-legacy transaction
   * envelopes that are not represented as a plain {@link SignedTransaction}.
   */
  default CompletableFuture<ClientResponse> submitTransactionEntrypoint(
      final byte[] encodedVersionedEntrypoint) {
    final CompletableFuture<ClientResponse> future = new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "submitTransactionEntrypoint requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Submits a version-tagged TransactionEntrypoint encoded as canonical Norito JSON. */
  default CompletableFuture<ClientResponse> submitTransactionEntrypointJson(
      final byte[] encodedVersionedEntrypointJson) {
    final CompletableFuture<ClientResponse> future = new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "submitTransactionEntrypointJson requires a concrete IrohaClient implementation"));
    return future;
  }

  /**
   * Polls the pipeline status endpoint until the transaction reaches a terminal state.
   *
   * <p>The default implementation reports that the operation is unsupported.
   */
  default CompletableFuture<Map<String, Object>> waitForTransactionStatus(
      final String hashHex, final PipelineStatusOptions options) {
    final CompletableFuture<Map<String, Object>> future = new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "waitForTransactionStatus requires a concrete IrohaClient implementation"));
    return future;
  }

  /**
   * Proposes a generic multisig instruction batch via `POST /v1/multisig/propose`.
   *
   * <p>Request instructions are encoded as base64 native Norito {@code InstructionBox} frames in
   * the JSON body.
   */
  default CompletableFuture<MultisigResponse> proposeMultisig(
      final MultisigProposeRequest request) {
    final CompletableFuture<MultisigResponse> future = new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "proposeMultisig requires a concrete IrohaClient implementation"));
    return future;
  }

  /**
   * Resolves an account alias literal against the node's alias registry via
   * `POST /v1/aliases/resolve`.
   *
   * <p>The returned future resolves to {@link Optional#empty()} when the node responds with
   * HTTP 404. Implementations that cannot reach a node should fail the future exceptionally.
   */
  default CompletableFuture<Optional<AccountAliasResolution>> resolveAccountAlias(
      final String alias) {
    final CompletableFuture<Optional<AccountAliasResolution>> future = new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "resolveAccountAlias requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Plans one indivisible alias setup request using canonical request authentication. */
  default CompletableFuture<AliasTransactionPlanV1> planAliasSetup(
      final AliasSetupPlanRequestV1 request,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    final CompletableFuture<AliasTransactionPlanV1> future = new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "planAliasSetup requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Plans one expiry-CAS lease renewal using canonical request authentication. */
  default CompletableFuture<AliasLifecycleTransactionPlanV1> planAliasLeaseRenewal(
      final AliasLeaseRenewPlanRequestV1 request,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    final CompletableFuture<AliasLifecycleTransactionPlanV1> future = new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "planAliasLeaseRenewal requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Plans one revision-CAS native auto-renew configuration. */
  default CompletableFuture<AliasLifecycleTransactionPlanV1> planAliasAutoRenew(
      final AliasAutoRenewPlanRequestV1 request,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    final CompletableFuture<AliasLifecycleTransactionPlanV1> future = new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "planAliasAutoRenew requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Verifies, locally signs, and submits one plan through the ordinary transaction endpoint. */
  default CompletableFuture<ClientResponse> submitAliasSetupPlan(
      final AliasSetupPlanRequestV1 request,
      final AliasTransactionPlanV1 plan,
      final NetworkId networkId,
      final AliasPlanBodyNoritoEncoder bodyEncoder,
      final AliasEnsureInstructionFrameCodec frameCodec,
      final int chainDiscriminant,
      final TransactionBuilder transactionBuilder,
      final Signer signer,
      final FeePaymentIntent feePayment,
      final long creationTimeMs,
      final Long nonce)
      throws NoritoException, SigningException {
    return AliasPlanApply.signAndSubmit(
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
        java.util.Collections.emptyMap());
  }

  /** Verifies with the canonical alias codecs, locally signs, and submits one setup plan. */
  default CompletableFuture<ClientResponse> submitAliasSetupPlan(
      final AliasSetupPlanRequestV1 request,
      final AliasTransactionPlanV1 plan,
      final NetworkId networkId,
      final int chainDiscriminant,
      final TransactionBuilder transactionBuilder,
      final Signer signer,
      final FeePaymentIntent feePayment,
      final long creationTimeMs,
      final Long nonce)
      throws NoritoException, SigningException {
    return AliasPlanApply.signAndSubmit(
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
        java.util.Collections.emptyMap());
  }

  /** Verifies, locally signs, and submits one lease or auto-renew lifecycle plan. */
  default CompletableFuture<ClientResponse> submitAliasLifecyclePlan(
      final AliasLifecyclePlanRequestV1 request,
      final AliasLifecycleTransactionPlanV1 plan,
      final NetworkId networkId,
      final AliasLifecyclePlanBodyNoritoEncoder bodyEncoder,
      final AliasLifecycleInstructionFrameCodec frameCodec,
      final int chainDiscriminant,
      final TransactionBuilder transactionBuilder,
      final Signer signer,
      final FeePaymentIntent feePayment,
      final long creationTimeMs,
      final Long nonce)
      throws NoritoException, SigningException {
    return AliasLifecyclePlanApply.signAndSubmit(
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
        java.util.Collections.emptyMap());
  }

  /** Verifies with the canonical codecs, locally signs, and submits one lifecycle plan. */
  default CompletableFuture<ClientResponse> submitAliasLifecyclePlan(
      final AliasLifecyclePlanRequestV1 request,
      final AliasLifecycleTransactionPlanV1 plan,
      final NetworkId networkId,
      final int chainDiscriminant,
      final TransactionBuilder transactionBuilder,
      final Signer signer,
      final FeePaymentIntent feePayment,
      final long creationTimeMs,
      final Long nonce)
      throws NoritoException, SigningException {
    return AliasLifecyclePlanApply.signAndSubmit(
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
        java.util.Collections.emptyMap());
  }

  /** Resolves a restricted alias with canonical account/signature/timestamp/nonce headers. */
  default CompletableFuture<Optional<AccountAliasResolution>> resolveAccountAlias(
      final String alias, final ToriiCanonicalRequestAuth canonicalAuth) {
    final CompletableFuture<Optional<AccountAliasResolution>> future = new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "authenticated resolveAccountAlias requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Resolves a public numeric alias index. */
  default CompletableFuture<Optional<AccountAliasIndexResolution>> resolveAccountAliasIndex(
      final BigInteger index) {
    final CompletableFuture<Optional<AccountAliasIndexResolution>> future =
        new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "resolveAccountAliasIndex requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Resolves a restricted numeric alias index with canonical request authentication. */
  default CompletableFuture<Optional<AccountAliasIndexResolution>> resolveAccountAliasIndex(
      final BigInteger index, final ToriiCanonicalRequestAuth canonicalAuth) {
    final CompletableFuture<Optional<AccountAliasIndexResolution>> future =
        new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "authenticated resolveAccountAliasIndex requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Lists visible aliases bound to an account. */
  default CompletableFuture<Optional<AccountAliasesByAccount>> listAccountAliases(
      final AccountAliasesByAccountRequest request) {
    final CompletableFuture<Optional<AccountAliasesByAccount>> future =
        new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "listAccountAliases requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Requests a stateless sponsored-onboarding receipt using a dedicated header token. */
  default CompletableFuture<AccountOnboardingPlanReceiptV1> planSponsoredAccountOnboarding(
      final AccountOnboardingPlanRequestV1 request, final String onboardingToken) {
    final CompletableFuture<AccountOnboardingPlanReceiptV1> future = new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "planSponsoredAccountOnboarding requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Requests a receipt and pins its signature to the configured onboarding authority. */
  default CompletableFuture<AccountOnboardingPlanReceiptV1> planSponsoredAccountOnboarding(
      final AccountOnboardingPlanRequestV1 request,
      final String onboardingToken,
      final String expectedAuthority) {
    final CompletableFuture<AccountOnboardingPlanReceiptV1> future = new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "pinned planSponsoredAccountOnboarding requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Revalidates and applies a stateless sponsored-onboarding receipt. */
  default CompletableFuture<AccountOnboardingResponseV1> applySponsoredAccountOnboarding(
      final AccountOnboardingPlanReceiptV1 receipt, final String onboardingToken) {
    final CompletableFuture<AccountOnboardingResponseV1> future = new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "applySponsoredAccountOnboarding requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Applies a receipt only when its signature matches the configured onboarding authority. */
  default CompletableFuture<AccountOnboardingResponseV1> applySponsoredAccountOnboarding(
      final AccountOnboardingPlanReceiptV1 receipt,
      final String onboardingToken,
      final String expectedAuthority) {
    final CompletableFuture<AccountOnboardingResponseV1> future = new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "pinned applySponsoredAccountOnboarding requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Fetches authenticated, secret-free onboarding readiness diagnostics. */
  default CompletableFuture<AliasSetupModels.AliasSetupReportV1> getAccountOnboardingReadiness(
      final String onboardingToken) {
    final CompletableFuture<AliasSetupModels.AliasSetupReportV1> future =
        new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "getAccountOnboardingReadiness requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Lists visible aliases with canonical request authentication. */
  default CompletableFuture<Optional<AccountAliasesByAccount>> listAccountAliases(
      final AccountAliasesByAccountRequest request,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    final CompletableFuture<Optional<AccountAliasesByAccount>> future =
        new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "authenticated listAccountAliases requires a concrete IrohaClient implementation"));
    return future;
  }

  /** Fetches the complete Kotodama manifest registered for one code hash. */
  default CompletableFuture<ContractManifestRecord> getContractManifest(final String codeHash) {
    final CompletableFuture<ContractManifestRecord> future = new CompletableFuture<>();
    future.completeExceptionally(
        new IllegalStateException(
            "getContractManifest requires a concrete IrohaClient implementation"));
    return future;
  }

}
