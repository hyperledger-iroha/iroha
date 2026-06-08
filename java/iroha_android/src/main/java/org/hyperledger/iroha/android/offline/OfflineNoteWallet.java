package org.hyperledger.iroha.android.offline;

import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.LongSupplier;
import org.hyperledger.iroha.android.client.ClientResponse;

/** One-call Offline Note wallet facade for load, receive, pay, accept, redeem, and sync. */
public final class OfflineNoteWallet {
  private static final AtomicInteger LOAD_THREAD_COUNTER = new AtomicInteger();
  private static final ExecutorService LOAD_EXECUTOR =
      Executors.newCachedThreadPool(
          task -> {
            final Thread thread =
                new Thread(
                    task,
                    "iroha-offline-note-wallet-" + LOAD_THREAD_COUNTER.incrementAndGet());
            thread.setDaemon(true);
            return thread;
          });

  private final String chainId;
  private final String accountId;
  private final OfflineNoteAttestationProvider attestationProvider;
  private final OfflineNoteStore store;
  private final OfflineNoteIssuerClient issuerClient;
  private final OfflineNoteTransactionSubmitter transactionSubmitter;
  private final OfflineNoteSyncResolver syncResolver;
  private final OfflineNoteProofProvider proofProvider;
  private final OfflineNoteProofVerifier proofVerifier;
  private final OfflineNoteCertificateVerifier certificateVerifier;
  private final OfflineNoteOwnerCertificateSigner ownerCertificateSigner;
  private final OfflineNoteRandomSource randomSource;
  private final OfflineNoteIdGenerator idGenerator;
  private final LongSupplier clock;
  private final OfflineBearerCashPolicyV1 bearerCashPolicy;

  public OfflineNoteWallet(
      final String chainId,
      final String accountId,
      final OfflineNoteAttestationProvider attestationProvider) {
    this(
        chainId,
        accountId,
        attestationProvider,
        new InMemoryOfflineNoteStore(),
        null,
        null,
        null,
        new NativeOfflineNoteProofProvider(),
        new Halo2OfflineNoteProofVerifier(),
        new RejectingOfflineNoteCertificateVerifier(),
        new SecureOfflineNoteRandomSource(),
        new UuidOfflineNoteIdGenerator(),
        System::currentTimeMillis);
  }

  public OfflineNoteWallet(
      final String chainId,
      final String accountId,
      final OfflineNoteAttestationProvider attestationProvider,
      final OfflineNoteStore store,
      final OfflineNoteIssuerClient issuerClient,
      final OfflineNoteTransactionSubmitter transactionSubmitter,
      final OfflineNoteProofProvider proofProvider,
      final OfflineNoteRandomSource randomSource,
      final OfflineNoteIdGenerator idGenerator,
      final LongSupplier clock) {
    this(
        chainId,
        accountId,
        attestationProvider,
        store,
        issuerClient,
        transactionSubmitter,
        null,
        proofProvider,
        new Halo2OfflineNoteProofVerifier(),
        new RejectingOfflineNoteCertificateVerifier(),
        randomSource,
        idGenerator,
        clock);
  }

  public OfflineNoteWallet(
      final String chainId,
      final String accountId,
      final OfflineNoteAttestationProvider attestationProvider,
      final OfflineNoteStore store,
      final OfflineNoteIssuerClient issuerClient,
      final OfflineNoteTransactionSubmitter transactionSubmitter,
      final OfflineNoteProofProvider proofProvider,
      final OfflineNoteProofVerifier proofVerifier,
      final OfflineNoteCertificateVerifier certificateVerifier,
      final OfflineNoteRandomSource randomSource,
      final OfflineNoteIdGenerator idGenerator,
      final LongSupplier clock,
      final OfflineNoteOwnerCertificateSigner ownerCertificateSigner) {
    this(
        chainId,
        accountId,
        attestationProvider,
        store,
        issuerClient,
        transactionSubmitter,
        null,
        proofProvider,
        proofVerifier,
        certificateVerifier,
        randomSource,
        idGenerator,
        clock,
        ownerCertificateSigner);
  }

  public OfflineNoteWallet(
      final String chainId,
      final String accountId,
      final OfflineNoteAttestationProvider attestationProvider,
      final OfflineNoteStore store,
      final OfflineNoteIssuerClient issuerClient,
      final OfflineNoteTransactionSubmitter transactionSubmitter,
      final OfflineNoteProofProvider proofProvider,
      final OfflineNoteProofVerifier proofVerifier,
      final OfflineNoteCertificateVerifier certificateVerifier,
      final OfflineNoteRandomSource randomSource,
      final OfflineNoteIdGenerator idGenerator,
      final LongSupplier clock) {
    this(
        chainId,
        accountId,
        attestationProvider,
        store,
        issuerClient,
        transactionSubmitter,
        null,
        proofProvider,
        proofVerifier,
        certificateVerifier,
        randomSource,
        idGenerator,
        clock);
  }

  public OfflineNoteWallet(
      final String chainId,
      final String accountId,
      final OfflineNoteAttestationProvider attestationProvider,
      final OfflineNoteStore store,
      final OfflineNoteIssuerClient issuerClient,
      final OfflineNoteTransactionSubmitter transactionSubmitter,
      final OfflineNoteProofProvider proofProvider,
      final OfflineNoteProofVerifier proofVerifier,
      final OfflineNoteRandomSource randomSource,
      final OfflineNoteIdGenerator idGenerator,
      final LongSupplier clock) {
    this(
        chainId,
        accountId,
        attestationProvider,
        store,
        issuerClient,
        transactionSubmitter,
        null,
        proofProvider,
        proofVerifier,
        new RejectingOfflineNoteCertificateVerifier(),
        randomSource,
        idGenerator,
        clock);
  }

  public OfflineNoteWallet(
      final String chainId,
      final String accountId,
      final OfflineNoteAttestationProvider attestationProvider,
      final OfflineNoteStore store,
      final OfflineNoteIssuerClient issuerClient,
      final OfflineNoteTransactionSubmitter transactionSubmitter,
      final OfflineNoteSyncResolver syncResolver,
      final OfflineNoteProofProvider proofProvider,
      final OfflineNoteRandomSource randomSource,
      final OfflineNoteIdGenerator idGenerator,
      final LongSupplier clock) {
    this(
        chainId,
        accountId,
        attestationProvider,
        store,
        issuerClient,
        transactionSubmitter,
        syncResolver,
        proofProvider,
        new Halo2OfflineNoteProofVerifier(),
        new RejectingOfflineNoteCertificateVerifier(),
        randomSource,
        idGenerator,
        clock);
  }

  public OfflineNoteWallet(
      final String chainId,
      final String accountId,
      final OfflineNoteAttestationProvider attestationProvider,
      final OfflineNoteStore store,
      final OfflineNoteIssuerClient issuerClient,
      final OfflineNoteTransactionSubmitter transactionSubmitter,
      final OfflineNoteSyncResolver syncResolver,
      final OfflineNoteProofProvider proofProvider,
      final OfflineNoteProofVerifier proofVerifier,
      final OfflineNoteRandomSource randomSource,
      final OfflineNoteIdGenerator idGenerator,
      final LongSupplier clock) {
    this(
        chainId,
        accountId,
        attestationProvider,
        store,
        issuerClient,
        transactionSubmitter,
        syncResolver,
        proofProvider,
        proofVerifier,
        new RejectingOfflineNoteCertificateVerifier(),
        randomSource,
        idGenerator,
        clock);
  }

  public OfflineNoteWallet(
      final String chainId,
      final String accountId,
      final OfflineNoteAttestationProvider attestationProvider,
      final OfflineNoteStore store,
      final OfflineNoteIssuerClient issuerClient,
      final OfflineNoteTransactionSubmitter transactionSubmitter,
      final OfflineNoteSyncResolver syncResolver,
      final OfflineNoteProofProvider proofProvider,
      final OfflineNoteProofVerifier proofVerifier,
      final OfflineNoteCertificateVerifier certificateVerifier,
      final OfflineNoteRandomSource randomSource,
      final OfflineNoteIdGenerator idGenerator,
      final LongSupplier clock) {
    this(
        chainId,
        accountId,
        attestationProvider,
        store,
        issuerClient,
        transactionSubmitter,
        syncResolver,
        proofProvider,
        proofVerifier,
        certificateVerifier,
        randomSource,
        idGenerator,
        clock,
        OfflineBearerCashPolicyV1.DEFAULT);
  }

  public OfflineNoteWallet(
      final String chainId,
      final String accountId,
      final OfflineNoteAttestationProvider attestationProvider,
      final OfflineNoteStore store,
      final OfflineNoteIssuerClient issuerClient,
      final OfflineNoteTransactionSubmitter transactionSubmitter,
      final OfflineNoteSyncResolver syncResolver,
      final OfflineNoteProofProvider proofProvider,
      final OfflineNoteProofVerifier proofVerifier,
      final OfflineNoteCertificateVerifier certificateVerifier,
      final OfflineNoteRandomSource randomSource,
      final OfflineNoteIdGenerator idGenerator,
      final LongSupplier clock,
      final OfflineNoteOwnerCertificateSigner ownerCertificateSigner) {
    this(
        chainId,
        accountId,
        attestationProvider,
        store,
        issuerClient,
        transactionSubmitter,
        syncResolver,
        proofProvider,
        proofVerifier,
        certificateVerifier,
        randomSource,
        idGenerator,
        clock,
        OfflineBearerCashPolicyV1.DEFAULT,
        ownerCertificateSigner);
  }

  public OfflineNoteWallet(
      final String chainId,
      final String accountId,
      final OfflineNoteAttestationProvider attestationProvider,
      final OfflineNoteStore store,
      final OfflineNoteIssuerClient issuerClient,
      final OfflineNoteTransactionSubmitter transactionSubmitter,
      final OfflineNoteSyncResolver syncResolver,
      final OfflineNoteProofProvider proofProvider,
      final OfflineNoteProofVerifier proofVerifier,
      final OfflineNoteCertificateVerifier certificateVerifier,
      final OfflineNoteRandomSource randomSource,
      final OfflineNoteIdGenerator idGenerator,
      final LongSupplier clock,
      final OfflineBearerCashPolicyV1 bearerCashPolicy) {
    this(
        chainId,
        accountId,
        attestationProvider,
        store,
        issuerClient,
        transactionSubmitter,
        syncResolver,
        proofProvider,
        proofVerifier,
        certificateVerifier,
        randomSource,
        idGenerator,
        clock,
        bearerCashPolicy,
        null);
  }

  public OfflineNoteWallet(
      final String chainId,
      final String accountId,
      final OfflineNoteAttestationProvider attestationProvider,
      final OfflineNoteStore store,
      final OfflineNoteIssuerClient issuerClient,
      final OfflineNoteTransactionSubmitter transactionSubmitter,
      final OfflineNoteSyncResolver syncResolver,
      final OfflineNoteProofProvider proofProvider,
      final OfflineNoteProofVerifier proofVerifier,
      final OfflineNoteCertificateVerifier certificateVerifier,
      final OfflineNoteRandomSource randomSource,
      final OfflineNoteIdGenerator idGenerator,
      final LongSupplier clock,
      final OfflineBearerCashPolicyV1 bearerCashPolicy,
      final OfflineNoteOwnerCertificateSigner ownerCertificateSigner) {
    this.chainId = requireNonBlank(chainId, "chainId");
    this.accountId = requireNonBlank(accountId, "accountId");
    this.attestationProvider = Objects.requireNonNull(attestationProvider, "attestationProvider");
    this.store = Objects.requireNonNull(store, "store");
    this.issuerClient = issuerClient;
    this.transactionSubmitter = transactionSubmitter;
    this.syncResolver = syncResolver;
    this.proofProvider = Objects.requireNonNull(proofProvider, "proofProvider");
    this.proofVerifier = Objects.requireNonNull(proofVerifier, "proofVerifier");
    this.certificateVerifier = Objects.requireNonNull(certificateVerifier, "certificateVerifier");
    this.ownerCertificateSigner = ownerCertificateSigner;
    this.randomSource = Objects.requireNonNull(randomSource, "randomSource");
    this.idGenerator = Objects.requireNonNull(idGenerator, "idGenerator");
    this.clock = Objects.requireNonNull(clock, "clock");
    this.bearerCashPolicy = Objects.requireNonNull(bearerCashPolicy, "bearerCashPolicy");
  }

  public List<OfflineNoteWalletNote> listNotes() {
    return store.listNotes();
  }

  public CompletableFuture<OfflineNoteWalletNote> load(
      final String assetDefinitionId, final String amount) {
    if (issuerClient == null) {
      return failedFuture(new IllegalStateException("Offline Note issuer client is required for load"));
    }
    final String assetId = walletAssetId(assetDefinitionId, accountId);
    final CompletableFuture<OfflineNoteWalletNote> result = new CompletableFuture<>();
    issuerClient.prepareLoad(chainId, accountId, assetDefinition(assetId), amount)
        .whenComplete(
            (context, prepareError) ->
                LOAD_EXECUTOR.execute(
                    () -> {
                      if (prepareError != null) {
                        result.completeExceptionally(unwrapCompletion(prepareError));
                        return;
                      }
                      if (context == null) {
                        result.completeExceptionally(
                            new IllegalStateException(
                                "Offline Note issuer returned no load context"));
                        return;
                      }
                      final byte[] noteSecret;
                      final OfflineNote.CommitmentOrigin.IssuerLoad origin;
                      final byte[] noteCommitment;
                      final OfflineNoteIssueRequest request;
                      try {
                        requireTrustedIssuerCertificate(context.keyCertificate(), accountId);
                        noteSecret = random32();
                        origin =
                            new OfflineNote.CommitmentOrigin.IssuerLoad(
                                context.operationId(),
                                context.lineageId(),
                                context.localRevision());
                        noteCommitment =
                            deriveNoteCommitment(
                                context.keyCertificate(), assetId, amount, noteSecret, origin);
                        request =
                            new OfflineNoteIssueRequest(
                                chainId,
                                accountId,
                                assetDefinition(assetId),
                                assetId,
                                amount,
                                context,
                                noteCommitment);
                      } catch (final Throwable error) {
                        result.completeExceptionally(error);
                        return;
                      }
                      final CompletableFuture<OfflineNoteIssueResponse> issueFuture;
                      try {
                        issueFuture = issuerClient.issueNote(request);
                      } catch (final Throwable error) {
                        result.completeExceptionally(error);
                        return;
                      }
                      issueFuture
                          .whenComplete(
                              (response, issueError) ->
                                  LOAD_EXECUTOR.execute(
                                      () -> {
                                        if (issueError != null) {
                                          result.completeExceptionally(
                                              unwrapCompletion(issueError));
                                          return;
                                        }
                                        if (response == null) {
                                          result.completeExceptionally(
                                              new IllegalStateException(
                                                  "Offline Note issuer returned no issue response"));
                                          return;
                                        }
                                        try {
                                          if (!Arrays.equals(
                                              response.noteCommitment(), noteCommitment)) {
                                            throw new IllegalStateException(
                                                "issuer returned a different Offline Note commitment");
                                          }
                                          final OfflineNote.KeyCertificate certificate =
                                              response.keyCertificate() == null
                                                  ? context.keyCertificate()
                                                  : response.keyCertificate();
                                          requireTrustedIssuerCertificate(certificate, accountId);
                                          final long now = clock.getAsLong();
                                          final OfflineNoteWalletNote note =
                                              new OfflineNoteWalletNote(
                                                  chainId,
                                                  accountId,
                                                  assetId,
                                                  amount,
                                                  certificate,
                                                  noteCommitment,
                                                  noteSecret,
                                                  origin,
                                                  OfflineNoteWalletNoteState.SPENDABLE,
                                                  now,
                                                  now);
                                          store.upsert(note);
                                          result.complete(note);
                                        } catch (final Throwable error) {
                                          result.completeExceptionally(error);
                                        }
                                      }));
                    }));
    return result;
  }

  public OfflineNoteReceiveRequest prepareReceive(
      final String assetDefinitionId, final String amount) {
    final String paymentRequestId = idGenerator.nextId("payment-request");
    final OfflineNote.KeyCertificate keyCertificate =
        requireOwnerCertificateSigner().freshOwnerCertificate(accountId);
    requireTrustedOwnerCertificate(keyCertificate, accountId);
    final String assetId = walletAssetId(assetDefinitionId, accountId);
    final byte[] noteSecret = random32();
    final OfflineNote.CommitmentOrigin.P2pOutput origin =
        new OfflineNote.CommitmentOrigin.P2pOutput(paymentRequestId, 0);
    final byte[] outputCommitment =
        deriveNoteCommitment(keyCertificate, assetId, amount, noteSecret, origin);
    final long now = clock.getAsLong();
    final OfflineNoteWalletNote pending =
        new OfflineNoteWalletNote(
            chainId,
            accountId,
            assetId,
            amount,
            keyCertificate,
            outputCommitment,
            noteSecret,
            origin,
            OfflineNoteWalletNoteState.RECEIVE_PENDING,
            now,
            now);
    store.upsert(pending);
    return new OfflineNoteReceiveRequest(
        chainId,
        paymentRequestId,
        accountId,
        assetDefinition(assetId),
        assetId,
        pending.canonicalAmount(),
        keyCertificate,
        outputCommitment);
  }

  public OfflineNotePaymentToken pay(final OfflineNoteReceiveRequest receiveRequest) {
    Objects.requireNonNull(receiveRequest, "receiveRequest");
    if (!chainId.equals(receiveRequest.chainId())) {
      throw new IllegalArgumentException("receive request chainId does not match wallet chainId");
    }
    requireTrustedOwnerCertificate(receiveRequest.keyCertificate(), receiveRequest.accountId());
    rejectReusedReceiveRequest(receiveRequest.paymentRequestId());
    final long createdAtMs = clock.getAsLong();
    final BigDecimal requestedAmount = decimal(receiveRequest.canonicalAmount());
    final List<OfflineNoteWalletNote> selected =
        selectSpendableNotes(receiveRequest.assetDefinitionId(), requestedAmount);
    BigDecimal inputAmount = BigDecimal.ZERO;
    for (final OfflineNoteWalletNote note : selected) {
      inputAmount = inputAmount.add(decimal(note.canonicalAmount()));
    }
    final BigDecimal changeAmount = inputAmount.subtract(requestedAmount);
    if (changeAmount.signum() < 0) {
      throw new IllegalArgumentException("selected input amount is below requested amount");
    }

    final OfflineNoteWalletNote senderNote = selected.get(0);
    final OfflineNote.KeyCertificate senderCertificate = senderNote.keyCertificate();
    requireTrustedCertificateForOrigin(senderCertificate, senderNote.origin(), accountId);
    final byte[] senderCertificateHash = senderCertificate.payloadHash();
    for (final OfflineNoteWalletNote note : selected) {
      bearerAuditTrail(note);
      requireTrustedCertificateForOrigin(note.keyCertificate(), note.origin(), accountId);
      if (!Arrays.equals(note.keyCertificate().payloadHash(), senderCertificateHash)) {
        throw new IllegalArgumentException("selected input notes must use the same key certificate");
      }
    }

    final List<byte[]> inputNullifiers = new ArrayList<>();
    final List<OfflineNote.IssuedClaim> inputClaims = new ArrayList<>();
    for (final OfflineNoteWalletNote note : selected) {
      inputNullifiers.add(deriveInputNullifier(note));
      inputClaims.add(note.issuedClaim());
    }
    final List<OfflineNote.AuditOutputClaim> outputClaims = new ArrayList<>();
    outputClaims.add(
        new OfflineNote.AuditOutputClaim(
            receiveRequest.outputCommitment(),
            receiveRequest.keyCertificate(),
            receiveRequest.assetId(),
            receiveRequest.canonicalAmount()));
    final byte[] tokenNonce = random32();
    OfflineNoteWalletNote changeNote = null;
    if (changeAmount.signum() > 0) {
      final byte[] changeSecret = random32();
      final String changeAmountString = canonicalDecimal(changeAmount);
      final String changeAssetId = walletAssetId(receiveRequest.assetDefinitionId(), accountId);
      final OfflineNote.KeyCertificate changeCertificate =
          requireOwnerCertificateSigner().freshOwnerCertificate(accountId);
      requireTrustedOwnerCertificate(changeCertificate, accountId);
      final OfflineNote.CommitmentOrigin.P2pOutput changeOrigin =
          new OfflineNote.CommitmentOrigin.P2pOutput(receiveRequest.paymentRequestId(), 1);
      final byte[] changeCommitment =
          deriveNoteCommitment(
              changeCertificate, changeAssetId, changeAmountString, changeSecret, changeOrigin);
      changeNote =
          new OfflineNoteWalletNote(
              chainId,
              accountId,
              changeAssetId,
              changeAmountString,
              changeCertificate,
              changeCommitment,
              changeSecret,
              changeOrigin,
              OfflineNoteWalletNoteState.SPENDABLE,
              createdAtMs,
              createdAtMs);
      outputClaims.add(
          new OfflineNote.AuditOutputClaim(
              changeCommitment, changeCertificate, changeAssetId, changeNote.canonicalAmount()));
    }
    final List<byte[]> outputCommitments = new ArrayList<>();
    for (final OfflineNote.AuditOutputClaim claim : outputClaims) {
      outputCommitments.add(claim.noteCommitment());
    }
    final byte[] tokenId =
        OfflineNote.derivePaymentTokenId(
            new OfflineNote.PaymentTokenIdPreimage(
                chainId,
                receiveRequest.paymentRequestId(),
                createdAtMs,
                tokenNonce,
                senderCertificateHash,
                inputNullifiers,
                outputCommitments));
    final List<OfflineNote.IssuedClaim> outputIssuedClaims = new ArrayList<>();
    for (final OfflineNote.AuditOutputClaim claim : outputClaims) {
      outputIssuedClaims.add(claim.issuedClaim());
    }
    final OfflineNote.AuditPublicInputs auditPublicInputs =
        new OfflineNote.AuditPublicInputs(
            tokenId,
            senderCertificateHash,
            inputNullifiers,
            inputClaims,
            outputCommitments,
            outputIssuedClaims);
    final OfflineNote.AuditBundle draft =
        new OfflineNote.AuditBundle(
            tokenId,
            senderCertificate,
            inputNullifiers,
            inputClaims,
            outputCommitments,
            outputClaims,
            draftPlaceholderProof(auditPublicInputs.publicInputsHash()));
    final OfflineNote.AuditBundle audit =
        draft.replacingRecursiveProof(proofProvider.proveAudit(draft));
    audit.validateProofBinding();
    requireTrustedAuditCertificates(audit);
    if (!proofVerifier.verifyAudit(audit)) {
      throw new IllegalArgumentException("Offline Note recursive audit proof verification failed");
    }
    final List<OfflineNote.AuditBundle> outputBearerAuditTrail =
        bearerAuditTrail(selected, audit);
    bearerCashPolicy.validateAuditTrail(outputBearerAuditTrail, audit);
    final OfflineNoteWalletNote finalChangeNote = changeNote;
    store.mutateNotes(notes -> {
      for (final OfflineNoteWalletNote note : selected) {
        final OfflineNoteWalletNote current = notes.get(note.noteCommitmentHex());
        if (current == null || current.state() != OfflineNoteWalletNoteState.SPENDABLE) {
          throw new IllegalArgumentException("selected Offline Note input changed state");
        }
      }
      if (finalChangeNote != null && notes.containsKey(finalChangeNote.noteCommitmentHex())) {
        throw new IllegalArgumentException("Offline Note change note already exists");
      }
      for (final OfflineNoteWalletNote note : selected) {
        notes.put(
            note.noteCommitmentHex(),
            note.withState(OfflineNoteWalletNoteState.SPENT, createdAtMs)
                .withSpentPaymentRequestId(receiveRequest.paymentRequestId(), createdAtMs));
      }
      if (finalChangeNote != null) {
        notes.put(
            finalChangeNote.noteCommitmentHex(),
            finalChangeNote.withBearerAuditTrail(outputBearerAuditTrail, createdAtMs));
      }
      return null;
    });
    return new OfflineNotePaymentToken(
        chainId,
        receiveRequest.paymentRequestId(),
        tokenNonce,
        tokenId,
        audit,
        outputBearerAuditTrail,
        createdAtMs);
  }

  private void rejectReusedReceiveRequest(final String paymentRequestId) {
    for (final OfflineNoteWalletNote note : store.listNotes()) {
      if (paymentRequestId.equals(note.spentPaymentRequestId())) {
        throw new IllegalArgumentException(
            "Offline Note receive request has already been used locally");
      }
      if (note.state() == OfflineNoteWalletNoteState.RECEIVE_PENDING) {
        continue;
      }
      if (note.origin() instanceof OfflineNote.CommitmentOrigin.P2pOutput origin
          && origin.paymentRequestId().equals(paymentRequestId)) {
        throw new IllegalArgumentException(
            "Offline Note receive request has already been used locally");
      }
    }
  }

  private List<OfflineNote.AuditBundle> bearerAuditTrail(final OfflineNoteWalletNote note) {
    if (note.origin() instanceof OfflineNote.CommitmentOrigin.IssuerLoad) {
      return Collections.emptyList();
    }
    if (note.origin() instanceof OfflineNote.CommitmentOrigin.P2pOutput) {
      if (note.bearerAuditTrail().isEmpty()) {
        throw new IllegalArgumentException(
            "Offline Note bearer note is missing the audit trail required for defunding");
      }
      return note.bearerAuditTrail();
    }
    throw new IllegalArgumentException("unsupported Offline Note commitment origin");
  }

  private List<OfflineNote.AuditBundle> bearerAuditTrail(
      final List<OfflineNoteWalletNote> inputNotes, final OfflineNote.AuditBundle audit) {
    final LinkedHashSet<String> seen = new LinkedHashSet<>();
    final List<OfflineNote.AuditBundle> result = new ArrayList<>();
    for (final OfflineNoteWalletNote note : inputNotes) {
      for (final OfflineNote.AuditBundle inputAudit : bearerAuditTrail(note)) {
        if (seen.add(OfflineNoteWallet.hexLower(inputAudit.tokenId()))) {
          result.add(inputAudit);
        }
      }
    }
    if (seen.add(OfflineNoteWallet.hexLower(audit.tokenId()))) {
      result.add(audit);
    }
    return result;
  }

  public OfflineNoteWalletNote accept(final OfflineNotePaymentToken paymentToken) {
    validatePaymentToken(Objects.requireNonNull(paymentToken, "paymentToken"));
    if (!proofVerifier.verifyAudit(paymentToken.audit())) {
      throw new IllegalArgumentException("Offline Note recursive audit proof verification failed");
    }
    return store.mutateNotes(notes -> {
      for (int index = 0; index < paymentToken.audit().outputClaims().size(); index++) {
        final OfflineNote.AuditOutputClaim output = paymentToken.audit().outputClaims().get(index);
        final OfflineNoteWalletNote pending =
            notes.get(OfflineNoteWallet.hexLower(output.noteCommitment()));
        if (pending == null || pending.state() != OfflineNoteWalletNoteState.RECEIVE_PENDING) {
          continue;
        }
        if (!pending.assetId().equals(output.assetId())
            || !pending.canonicalAmount().equals(output.canonicalAmount())
            || !Arrays.equals(output.keyCertificate().payloadHash(), pending.keyCertificate().payloadHash())) {
          throw new IllegalArgumentException("payment token output does not match receive request");
        }
        if (!(pending.origin() instanceof OfflineNote.CommitmentOrigin.P2pOutput)) {
          throw new IllegalArgumentException("payment token output origin must be P2P");
        }
        final OfflineNote.CommitmentOrigin.P2pOutput origin =
            (OfflineNote.CommitmentOrigin.P2pOutput) pending.origin();
        if (!origin.paymentRequestId().equals(paymentToken.paymentRequestId())
            || origin.outputIndex() != index) {
          throw new IllegalArgumentException("payment token output origin does not match receive request");
        }
        final long now = clock.getAsLong();
        final OfflineNoteWalletNote accepted =
            pending
                .withState(OfflineNoteWalletNoteState.SPENDABLE, now)
                .withBearerAuditTrail(paymentToken.bearerAuditTrail(), now);
        notes.put(pending.noteCommitmentHex(), accepted);
        return accepted;
      }
      throw new IllegalStateException("payment token has no pending output for this wallet");
    });
  }

  public CompletableFuture<ClientResponse> publishAudit(final OfflineNotePaymentToken paymentToken) {
    if (transactionSubmitter == null) {
      return failedFuture(new IllegalStateException(
          "Offline Note transaction submitter is required for audit publication"));
    }
    validatePaymentToken(Objects.requireNonNull(paymentToken, "paymentToken"));
    if (!proofVerifier.verifyAudit(paymentToken.audit())) {
      return failedFuture(new IllegalArgumentException(
          "Offline Note recursive audit proof verification failed"));
    }
    return transactionSubmitter.submitAudit(paymentToken.audit()).thenApply(response -> {
      ensureSuccess(response);
      return response;
    });
  }

  public CompletableFuture<OfflineNoteWalletNote> redeem(final OfflineNoteWalletNote note) {
    return redeem(note, accountId);
  }

  public CompletableFuture<OfflineNoteWalletNote> redeem(
      final OfflineNoteWalletNote note, final String recipient) {
    if (transactionSubmitter == null) {
      return failedFuture(new IllegalStateException(
          "Offline Note transaction submitter is required for redeem"));
    }
    final OfflineNoteWalletNote stored = store.findNote(note.noteCommitment());
    final OfflineNoteWalletNote current =
        stored == null ? note : stored;
    if (current.state() != OfflineNoteWalletNoteState.SPENDABLE) {
      throw new IllegalArgumentException("only spendable Offline Note notes can be redeemed");
    }
    final List<OfflineNote.AuditBundle> bearerAuditTrail = bearerAuditTrail(current);
    requireTrustedCertificateForOrigin(current.keyCertificate(), current.origin(), current.accountId());
    final byte[] inputNullifier = deriveInputNullifier(current);
    final OfflineNote.RedeemPublicInputs redeemPublicInputs =
        new OfflineNote.RedeemPublicInputs(
            current.noteCommitment(),
            Collections.singletonList(inputNullifier),
            current.keyCertificate().payloadHash(),
            recipient,
            current.assetId(),
            current.canonicalAmount());
    final OfflineNote.Redeem draft =
        new OfflineNote.Redeem(
            current.noteCommitment(),
            Collections.singletonList(inputNullifier),
            current.keyCertificate(),
            recipient,
            current.assetId(),
            current.canonicalAmount(),
            draftPlaceholderProof(redeemPublicInputs.publicInputsHash()));
    final OfflineNote.Redeem redemption =
        draft.replacingRecursiveProof(proofProvider.proveRedeem(draft));
    redemption.validateProofBinding();
    requireTrustedCertificateForOrigin(
        redemption.senderKeyCertificate(), current.origin(), current.accountId());
    if (!proofVerifier.verifyRedeem(redemption)) {
      throw new IllegalArgumentException("Offline Note recursive redeem proof verification failed");
    }
    final OfflineNoteWalletNote pending =
        store.mutateNotes(notes -> {
          final OfflineNoteWalletNote latest =
              notes.containsKey(current.noteCommitmentHex())
                  ? notes.get(current.noteCommitmentHex())
                  : current;
          if (latest.state() != OfflineNoteWalletNoteState.SPENDABLE) {
            throw new IllegalArgumentException("only spendable Offline Note notes can be redeemed");
          }
          final OfflineNoteWalletNote updated =
              latest.withState(OfflineNoteWalletNoteState.REDEEM_PENDING, clock.getAsLong());
          notes.put(latest.noteCommitmentHex(), updated);
          return updated;
        });
    final CompletableFuture<ClientResponse> submitted;
    try {
      submitted = transactionSubmitter.submitDefund(redemption, bearerAuditTrail);
    } catch (final Throwable error) {
      rollbackRedeemReservation(pending);
      return failedFuture(error);
    }
    return submitted.thenApply(response -> {
      try {
        ensureSuccess(response);
      } catch (final RuntimeException error) {
        rollbackRedeemReservation(pending);
        throw error;
      }
      return pending;
    });
  }

  private void rollbackRedeemReservation(final OfflineNoteWalletNote reserved) {
    store.mutateNotes(notes -> {
      final OfflineNoteWalletNote latest = notes.get(reserved.noteCommitmentHex());
      if (latest != null
          && latest.state() == OfflineNoteWalletNoteState.REDEEM_PENDING
          && latest.updatedAtMs() == reserved.updatedAtMs()) {
        notes.put(
            latest.noteCommitmentHex(),
            latest.withState(OfflineNoteWalletNoteState.SPENDABLE, clock.getAsLong()));
      }
      return null;
    });
  }

  public CompletableFuture<List<OfflineNoteWalletNote>> sync() {
    if (syncResolver == null) {
      return CompletableFuture.completedFuture(store.listNotes());
    }
    CompletableFuture<Void> chain = CompletableFuture.completedFuture(null);
    for (final OfflineNoteWalletNote snapshot : store.listNotes()) {
      if (!isPendingState(snapshot.state())) {
        continue;
      }
      chain = chain.thenCompose(ignored -> {
        final OfflineNoteWalletNote current = store.findNote(snapshot.noteCommitment());
        if (current == null || !isPendingState(current.state())) {
          return CompletableFuture.completedFuture(null);
        }
        return syncResolver.resolvePendingNote(current).thenApply(resolution -> {
          if (resolution != null && resolution.state() != current.state()) {
            store.upsert(current.withState(resolution.state(), clock.getAsLong()));
          }
          return null;
        });
      });
    }
    return chain.thenApply(ignored -> store.listNotes());
  }

  private List<OfflineNoteWalletNote> selectSpendableNotes(
      final String assetDefinitionId, final BigDecimal requestedAmount) {
    final List<OfflineNoteWalletNote> selected = new ArrayList<>();
    BigDecimal total = BigDecimal.ZERO;
    for (final OfflineNoteWalletNote note : store.listNotes()) {
      if (note.state() != OfflineNoteWalletNoteState.SPENDABLE) {
        continue;
      }
      if (!assetDefinition(note.assetId()).equals(assetDefinition(assetDefinitionId))) {
        continue;
      }
      selected.add(note);
      total = total.add(decimal(note.canonicalAmount()));
      if (total.compareTo(requestedAmount) >= 0) {
        break;
      }
      if (selected.size() >= 4) {
        throw new IllegalArgumentException("Offline Note payments support at most 4 input notes");
      }
    }
    if (selected.isEmpty() || total.compareTo(requestedAmount) < 0) {
      throw new IllegalArgumentException("insufficient spendable Offline Note balance");
    }
    return selected;
  }

  private byte[] deriveNoteCommitment(
      final OfflineNote.KeyCertificate keyCertificate,
      final String assetId,
      final String amount,
      final byte[] noteSecret,
      final OfflineNote.CommitmentOrigin origin) {
    return OfflineNote.deriveNoteCommitment(
        new OfflineNote.NoteCommitmentPreimage(
            chainId, keyCertificate.payloadHash(), assetId, amount, noteSecret, origin));
  }

  private byte[] deriveInputNullifier(final OfflineNoteWalletNote note) {
    return OfflineNote.deriveInputNullifier(
        new OfflineNote.InputNullifierPreimage(
            chainId,
            note.noteCommitment(),
            note.keyCertificate().payloadHash(),
            note.noteSecret()));
  }

  private void validatePaymentToken(final OfflineNotePaymentToken paymentToken) {
    if (!chainId.equals(paymentToken.chainId())) {
      throw new IllegalArgumentException("payment token chainId does not match wallet chainId");
    }
    paymentToken.audit().validateProofBinding();
    final byte[] expectedTokenId =
        OfflineNote.derivePaymentTokenId(
            new OfflineNote.PaymentTokenIdPreimage(
                paymentToken.chainId(),
                paymentToken.paymentRequestId(),
                paymentToken.createdAtMs(),
                paymentToken.tokenNonce(),
                paymentToken.audit().senderKeyCertificate().payloadHash(),
                paymentToken.audit().inputNullifiers(),
                paymentToken.audit().outputCommitments()));
    if (!Arrays.equals(paymentToken.audit().tokenId(), paymentToken.tokenId())
        || !Arrays.equals(paymentToken.tokenId(), expectedTokenId)) {
      throw new IllegalArgumentException("Offline Note payment token id does not match bound token metadata");
    }
    requireTrustedAuditCertificates(paymentToken.audit());
    validateBearerAuditTrail(paymentToken.bearerAuditTrail(), paymentToken.audit());
  }

  private void validateBearerAuditTrail(
      final List<OfflineNote.AuditBundle> audits,
      final OfflineNote.AuditBundle terminalAudit) {
    if (audits.isEmpty()
        || !Arrays.equals(
            audits.get(audits.size() - 1).noritoEncoded(), terminalAudit.noritoEncoded())) {
      throw new IllegalArgumentException(
          "Offline Note bearer audit trail must end with the payment token audit");
    }
    bearerCashPolicy.validateAuditTrail(audits, terminalAudit);
    final LinkedHashSet<String> tokenIds = new LinkedHashSet<>();
    final LinkedHashSet<String> nullifiers = new LinkedHashSet<>();
    final LinkedHashSet<String> outputs = new LinkedHashSet<>();
    final Map<String, Integer> outputProducerIndex = new LinkedHashMap<>();
    for (int index = 0; index < audits.size(); index++) {
      for (final byte[] output : audits.get(index).outputCommitments()) {
        final String key = OfflineNoteWallet.hexLower(output);
        if (outputProducerIndex.containsKey(key)) {
          throw new IllegalArgumentException(
              "Offline Note bearer audit trail has duplicate output commitment");
        }
        outputProducerIndex.put(key, index);
      }
    }
    for (int index = 0; index < audits.size(); index++) {
      final OfflineNote.AuditBundle audit = audits.get(index);
      audit.validateProofBinding();
      if (!tokenIds.add(OfflineNoteWallet.hexLower(audit.tokenId()))) {
        throw new IllegalArgumentException(
            "Offline Note bearer audit trail has duplicate token id");
      }
      for (final byte[] nullifier : audit.inputNullifiers()) {
        if (!nullifiers.add(OfflineNoteWallet.hexLower(nullifier))) {
          throw new IllegalArgumentException(
              "Offline Note bearer audit trail has duplicate input nullifier");
        }
      }
      for (final byte[] output : audit.outputCommitments()) {
        if (!outputs.add(OfflineNoteWallet.hexLower(output))) {
          throw new IllegalArgumentException(
              "Offline Note bearer audit trail has duplicate output commitment");
        }
      }
      for (final OfflineNote.IssuedClaim claim : audit.inputClaims()) {
        final Integer producerIndex =
            outputProducerIndex.get(OfflineNoteWallet.hexLower(claim.noteCommitment()));
        if (producerIndex != null && producerIndex >= index) {
          throw new IllegalArgumentException(
              "Offline Note bearer audit trail input claims are out of order");
        }
      }
      requireTrustedAuditCertificates(audit);
      if (!proofVerifier.verifyAudit(audit)) {
        throw new IllegalArgumentException(
            "Offline Note recursive audit proof verification failed");
      }
    }
  }

  private void requireTrustedAuditCertificates(final OfflineNote.AuditBundle audit) {
    requireTrustedEitherCertificate(audit.senderKeyCertificate(), null);
    final byte[] senderCertificateHash = audit.senderKeyCertificate().payloadHash();
    for (final OfflineNote.IssuedClaim input : audit.inputClaims()) {
      if (!Arrays.equals(input.keyCertificatePayloadHash(), senderCertificateHash)) {
        throw new IllegalArgumentException(
            "Offline Note input claim is not bound to the sender certificate");
      }
      requireTrustedEitherCertificate(audit.senderKeyCertificate(), assetAccount(input.assetId()));
    }
    for (final OfflineNote.AuditOutputClaim output : audit.outputClaims()) {
      requireTrustedOwnerCertificate(output.keyCertificate(), assetAccount(output.assetId()));
    }
  }

  private void requireTrustedCertificateForOrigin(
      final OfflineNote.KeyCertificate certificate,
      final OfflineNote.CommitmentOrigin origin,
      final String expectedAccountId) {
    if (origin instanceof OfflineNote.CommitmentOrigin.IssuerLoad) {
      requireTrustedIssuerCertificate(certificate, expectedAccountId);
    } else if (origin instanceof OfflineNote.CommitmentOrigin.P2pOutput) {
      requireTrustedOwnerCertificate(certificate, expectedAccountId);
    } else {
      throw new IllegalArgumentException("unknown Offline Note commitment origin");
    }
  }

  private void requireTrustedIssuerCertificate(
      final OfflineNote.KeyCertificate certificate, final String expectedAccountId) {
    requireMatchingAccount(certificate, expectedAccountId);
    if (!certificateVerifier.verifyIssuerCertificate(certificate)) {
      throw new IllegalArgumentException("Offline Note key certificate verification failed");
    }
  }

  private void requireTrustedOwnerCertificate(
      final OfflineNote.KeyCertificate certificate, final String expectedAccountId) {
    requireMatchingAccount(certificate, expectedAccountId);
    if (!certificateVerifier.verifyOwnerCertificate(certificate)) {
      throw new IllegalArgumentException("Offline Note key certificate verification failed");
    }
  }

  private void requireTrustedEitherCertificate(
      final OfflineNote.KeyCertificate certificate, final String expectedAccountId) {
    requireMatchingAccount(certificate, expectedAccountId);
    if (!certificateVerifier.verifyIssuerCertificate(certificate)
        && !certificateVerifier.verifyOwnerCertificate(certificate)) {
      throw new IllegalArgumentException("Offline Note key certificate verification failed");
    }
  }

  private void requireMatchingAccount(
      final OfflineNote.KeyCertificate certificate, final String expectedAccountId) {
    if (expectedAccountId != null && !expectedAccountId.equals(certificate.accountId())) {
      throw new IllegalArgumentException("Offline Note key certificate account mismatch");
    }
  }

  private OfflineNoteOwnerCertificateSigner requireOwnerCertificateSigner() {
    if (ownerCertificateSigner == null) {
      throw new IllegalStateException(
          "Offline Note owner certificate signer is required for P2P outputs");
    }
    return ownerCertificateSigner;
  }

  private static String assetAccount(final String assetId) {
    final int marker = assetId.indexOf('#');
    if (marker < 0 || marker == assetId.length() - 1) {
      return null;
    }
    final String suffix = assetId.substring(marker + 1);
    final int dataspaceMarker = suffix.indexOf("#dataspace:");
    return dataspaceMarker < 0 ? suffix : suffix.substring(0, dataspaceMarker);
  }

  private byte[] random32() {
    final byte[] bytes = randomSource.nextBytes(32);
    if (bytes.length != 32) {
      throw new IllegalArgumentException(
          "Offline Note random source must return exactly 32 bytes");
    }
    return bytes;
  }

  private static OfflineNote.RecursiveProof draftPlaceholderProof(final byte[] publicInputsHash) {
    return new OfflineNote.RecursiveProof(
        publicInputsHash,
        new OfflineNote.ProofBox(
            "offline-note/draft-placeholder",
            new byte[] {0}));
  }

  private static void ensureSuccess(final ClientResponse response) {
    if (response.statusCode() < 200 || response.statusCode() > 299) {
      throw new IllegalArgumentException(
          "Offline Note transaction rejected with HTTP "
              + response.statusCode()
              + ": "
              + response.message());
    }
  }

  private static boolean isPendingState(final OfflineNoteWalletNoteState state) {
    return state == OfflineNoteWalletNoteState.REDEEM_PENDING;
  }

  private static String walletAssetId(final String assetDefinitionId, final String accountId) {
    return assetDefinition(assetDefinitionId) + "#" + accountId;
  }

  private static String assetDefinition(final String assetIdOrDefinition) {
    final String definition = assetIdOrDefinition.split("#", 2)[0];
    return requireNonBlank(definition, "asset definition id");
  }

  private static BigDecimal decimal(final String value) {
    return new BigDecimal(value);
  }

  private static String canonicalDecimal(final BigDecimal value) {
    BigDecimal normalized = value.stripTrailingZeros();
    if (normalized.scale() < 0) {
      normalized = normalized.setScale(0);
    }
    return normalized.toPlainString();
  }

  static <T> CompletableFuture<T> failedFuture(final Throwable error) {
    final CompletableFuture<T> future = new CompletableFuture<>();
    future.completeExceptionally(error);
    return future;
  }

  private static Throwable unwrapCompletion(final Throwable error) {
    if (error instanceof CompletionException && error.getCause() != null) {
      return error.getCause();
    }
    return error;
  }

  private static String requireNonBlank(final String value, final String field) {
    if (value == null || value.trim().isEmpty()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    return value;
  }

  static String hexLower(final byte[] bytes) {
    final char[] out = new char[bytes.length * 2];
    final char[] alphabet = "0123456789abcdef".toCharArray();
    for (int i = 0; i < bytes.length; i++) {
      final int value = bytes[i] & 0xff;
      out[i * 2] = alphabet[value >>> 4];
      out[i * 2 + 1] = alphabet[value & 0x0f];
    }
    return new String(out);
  }
}
