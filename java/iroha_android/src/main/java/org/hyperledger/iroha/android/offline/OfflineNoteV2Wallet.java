package org.hyperledger.iroha.android.offline;

import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.function.LongSupplier;
import org.hyperledger.iroha.android.client.ClientResponse;

/** One-call Offline Note V2 wallet facade for load, receive, pay, accept, redeem, and sync. */
public final class OfflineNoteV2Wallet {
  private final String chainId;
  private final String accountId;
  private final OfflineNoteV2AttestationProvider attestationProvider;
  private final OfflineNoteV2Store store;
  private final OfflineNoteV2IssuerClient issuerClient;
  private final OfflineNoteV2TransactionSubmitter transactionSubmitter;
  private final OfflineNoteV2SyncResolver syncResolver;
  private final OfflineNoteV2ProofProvider proofProvider;
  private final OfflineNoteV2RandomSource randomSource;
  private final OfflineNoteV2IdGenerator idGenerator;
  private final LongSupplier clock;

  public OfflineNoteV2Wallet(
      final String chainId,
      final String accountId,
      final OfflineNoteV2AttestationProvider attestationProvider) {
    this(
        chainId,
        accountId,
        attestationProvider,
        new InMemoryOfflineNoteV2Store(),
        null,
        null,
        null,
        new NativeOfflineNoteV2ProofProvider(),
        new SecureOfflineNoteV2RandomSource(),
        new UuidOfflineNoteV2IdGenerator(),
        System::currentTimeMillis);
  }

  public OfflineNoteV2Wallet(
      final String chainId,
      final String accountId,
      final OfflineNoteV2AttestationProvider attestationProvider,
      final OfflineNoteV2Store store,
      final OfflineNoteV2IssuerClient issuerClient,
      final OfflineNoteV2TransactionSubmitter transactionSubmitter,
      final OfflineNoteV2ProofProvider proofProvider,
      final OfflineNoteV2RandomSource randomSource,
      final OfflineNoteV2IdGenerator idGenerator,
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
        randomSource,
        idGenerator,
        clock);
  }

  public OfflineNoteV2Wallet(
      final String chainId,
      final String accountId,
      final OfflineNoteV2AttestationProvider attestationProvider,
      final OfflineNoteV2Store store,
      final OfflineNoteV2IssuerClient issuerClient,
      final OfflineNoteV2TransactionSubmitter transactionSubmitter,
      final OfflineNoteV2SyncResolver syncResolver,
      final OfflineNoteV2ProofProvider proofProvider,
      final OfflineNoteV2RandomSource randomSource,
      final OfflineNoteV2IdGenerator idGenerator,
      final LongSupplier clock) {
    this.chainId = requireNonBlank(chainId, "chainId");
    this.accountId = requireNonBlank(accountId, "accountId");
    this.attestationProvider = Objects.requireNonNull(attestationProvider, "attestationProvider");
    this.store = Objects.requireNonNull(store, "store");
    this.issuerClient = issuerClient;
    this.transactionSubmitter = transactionSubmitter;
    this.syncResolver = syncResolver;
    this.proofProvider = Objects.requireNonNull(proofProvider, "proofProvider");
    this.randomSource = Objects.requireNonNull(randomSource, "randomSource");
    this.idGenerator = Objects.requireNonNull(idGenerator, "idGenerator");
    this.clock = Objects.requireNonNull(clock, "clock");
  }

  public List<OfflineNoteV2WalletNote> listNotes() {
    return store.listNotes();
  }

  public CompletableFuture<OfflineNoteV2WalletNote> load(
      final String assetDefinitionId, final String amount) {
    if (issuerClient == null) {
      return failedFuture(new IllegalStateException("Offline Note V2 issuer client is required for load"));
    }
    final String assetId = walletAssetId(assetDefinitionId, accountId);
    return issuerClient.prepareLoad(chainId, accountId, assetDefinition(assetId), amount)
        .thenCompose(context -> {
          final byte[] noteSecret = random32();
          final OfflineNoteV2.CommitmentOriginV2.IssuerLoad origin =
              new OfflineNoteV2.CommitmentOriginV2.IssuerLoad(
                  context.operationId(), context.lineageId(), context.localRevision());
          final byte[] noteCommitment =
              deriveNoteCommitment(context.keyCertificate(), assetId, amount, noteSecret, origin);
          final OfflineNoteV2IssueRequest request =
              new OfflineNoteV2IssueRequest(
                  chainId,
                  accountId,
                  assetDefinition(assetId),
                  assetId,
                  amount,
                  context,
                  noteCommitment);
          return issuerClient.issueNote(request).thenApply(response -> {
            if (!Arrays.equals(response.noteCommitment(), noteCommitment)) {
              throw new IllegalStateException("issuer returned a different Offline Note V2 commitment");
            }
            final OfflineNoteV2.KeyCertificateV2 certificate =
                response.keyCertificate() == null ? context.keyCertificate() : response.keyCertificate();
            final long now = clock.getAsLong();
            final OfflineNoteV2WalletNote note =
                new OfflineNoteV2WalletNote(
                    chainId,
                    accountId,
                    assetId,
                    amount,
                    certificate,
                    noteCommitment,
                    noteSecret,
                    origin,
                    OfflineNoteV2WalletNoteState.SPENDABLE,
                    now,
                    now);
            store.upsert(note);
            return note;
          });
        });
  }

  public OfflineNoteV2ReceiveRequest prepareReceive(
      final String assetDefinitionId, final String amount) {
    final String paymentRequestId = idGenerator.nextId("payment-request");
    final OfflineNoteV2.KeyCertificateV2 keyCertificate =
        attestationProvider.currentKeyCertificate();
    final String assetId = walletAssetId(assetDefinitionId, accountId);
    final byte[] noteSecret = random32();
    final OfflineNoteV2.CommitmentOriginV2.P2pOutput origin =
        new OfflineNoteV2.CommitmentOriginV2.P2pOutput(paymentRequestId, 0);
    final byte[] outputCommitment =
        deriveNoteCommitment(keyCertificate, assetId, amount, noteSecret, origin);
    final long now = clock.getAsLong();
    final OfflineNoteV2WalletNote pending =
        new OfflineNoteV2WalletNote(
            chainId,
            accountId,
            assetId,
            amount,
            keyCertificate,
            outputCommitment,
            noteSecret,
            origin,
            OfflineNoteV2WalletNoteState.RECEIVE_PENDING,
            now,
            now);
    store.upsert(pending);
    return new OfflineNoteV2ReceiveRequest(
        chainId,
        paymentRequestId,
        accountId,
        assetDefinition(assetId),
        assetId,
        pending.canonicalAmount(),
        keyCertificate,
        outputCommitment);
  }

  public OfflineNoteV2PaymentToken pay(final OfflineNoteV2ReceiveRequest receiveRequest) {
    Objects.requireNonNull(receiveRequest, "receiveRequest");
    if (!chainId.equals(receiveRequest.chainId())) {
      throw new IllegalArgumentException("receive request chainId does not match wallet chainId");
    }
    final BigDecimal requestedAmount = decimal(receiveRequest.canonicalAmount());
    final List<OfflineNoteV2WalletNote> selected =
        selectSpendableNotes(receiveRequest.assetDefinitionId(), requestedAmount);
    BigDecimal inputAmount = BigDecimal.ZERO;
    for (final OfflineNoteV2WalletNote note : selected) {
      inputAmount = inputAmount.add(decimal(note.canonicalAmount()));
    }
    final BigDecimal changeAmount = inputAmount.subtract(requestedAmount);
    if (changeAmount.signum() < 0) {
      throw new IllegalArgumentException("selected input amount is below requested amount");
    }

    final OfflineNoteV2.KeyCertificateV2 senderCertificate = selected.get(0).keyCertificate();
    final byte[] senderCertificateHash = senderCertificate.payloadHash();
    for (final OfflineNoteV2WalletNote note : selected) {
      if (!Arrays.equals(note.keyCertificate().payloadHash(), senderCertificateHash)) {
        throw new IllegalArgumentException("selected input notes must use the same key certificate");
      }
    }

    final List<byte[]> inputNullifiers = new ArrayList<>();
    final List<OfflineNoteV2.IssuedClaimV2> inputClaims = new ArrayList<>();
    for (final OfflineNoteV2WalletNote note : selected) {
      inputNullifiers.add(deriveInputNullifier(note));
      inputClaims.add(note.issuedClaim());
    }
    final List<OfflineNoteV2.AuditOutputClaimV2> outputClaims = new ArrayList<>();
    outputClaims.add(
        new OfflineNoteV2.AuditOutputClaimV2(
            receiveRequest.outputCommitment(),
            receiveRequest.keyCertificate(),
            receiveRequest.assetId(),
            receiveRequest.canonicalAmount()));
    final byte[] tokenNonce = random32();
    OfflineNoteV2WalletNote changeNote = null;
    if (changeAmount.signum() > 0) {
      final byte[] changeSecret = random32();
      final String changeAmountString = canonicalDecimal(changeAmount);
      final String changeAssetId = walletAssetId(receiveRequest.assetDefinitionId(), accountId);
      final OfflineNoteV2.CommitmentOriginV2.P2pOutput changeOrigin =
          new OfflineNoteV2.CommitmentOriginV2.P2pOutput(receiveRequest.paymentRequestId(), 1);
      final byte[] changeCommitment =
          deriveNoteCommitment(
              senderCertificate, changeAssetId, changeAmountString, changeSecret, changeOrigin);
      final long now = clock.getAsLong();
      changeNote =
          new OfflineNoteV2WalletNote(
              chainId,
              accountId,
              changeAssetId,
              changeAmountString,
              senderCertificate,
              changeCommitment,
              changeSecret,
              changeOrigin,
              OfflineNoteV2WalletNoteState.CHANGE_PENDING,
              now,
              now);
      outputClaims.add(
          new OfflineNoteV2.AuditOutputClaimV2(
              changeCommitment, senderCertificate, changeAssetId, changeNote.canonicalAmount()));
    }
    final List<byte[]> outputCommitments = new ArrayList<>();
    for (final OfflineNoteV2.AuditOutputClaimV2 claim : outputClaims) {
      outputCommitments.add(claim.noteCommitment());
    }
    final byte[] tokenId =
        OfflineNoteV2.derivePaymentTokenId(
            new OfflineNoteV2.PaymentTokenIdPreimageV2(
                chainId, tokenNonce, senderCertificateHash, inputNullifiers, outputCommitments));
    final OfflineNoteV2.AuditBundleV2 draft =
        new OfflineNoteV2.AuditBundleV2(
            tokenId,
            senderCertificate,
            inputNullifiers,
            inputClaims,
            outputCommitments,
            outputClaims,
            placeholderProof());
    final OfflineNoteV2.AuditBundleV2 audit =
        draft.replacingRecursiveProof(proofProvider.proveAudit(draft));
    audit.validateProofBinding();
    final long now = clock.getAsLong();
    for (final OfflineNoteV2WalletNote note : selected) {
      store.upsert(note.withState(OfflineNoteV2WalletNoteState.SPEND_PENDING, now));
    }
    if (changeNote != null) {
      store.upsert(changeNote);
    }
    return new OfflineNoteV2PaymentToken(
        receiveRequest.paymentRequestId(), tokenId, audit, now);
  }

  public CompletableFuture<OfflineNoteV2WalletNote> accept(
      final OfflineNoteV2PaymentToken paymentToken) {
    if (transactionSubmitter == null) {
      return failedFuture(new IllegalStateException(
          "Offline Note V2 transaction submitter is required for accept"));
    }
    Objects.requireNonNull(paymentToken, "paymentToken").audit().validateProofBinding();
    OfflineNoteV2.AuditOutputClaimV2 matched = null;
    for (final OfflineNoteV2.AuditOutputClaimV2 claim : paymentToken.audit().outputClaims()) {
      final OfflineNoteV2WalletNote note = store.findNote(claim.noteCommitment());
      if (note != null && note.state() == OfflineNoteV2WalletNoteState.RECEIVE_PENDING) {
        matched = claim;
        break;
      }
    }
    if (matched == null) {
      return failedFuture(new IllegalStateException("payment token has no pending output for this wallet"));
    }
    final OfflineNoteV2WalletNote pending = store.findNote(matched.noteCommitment());
    if (pending == null) {
      return failedFuture(new IllegalStateException("pending receive note is missing"));
    }
    if (!pending.assetId().equals(matched.assetId())) {
      throw new IllegalArgumentException("payment token output asset does not match receive request");
    }
    if (!pending.canonicalAmount().equals(matched.canonicalAmount())) {
      throw new IllegalArgumentException("payment token output amount does not match receive request");
    }
    if (!Arrays.equals(matched.keyCertificate().payloadHash(), pending.keyCertificate().payloadHash())) {
      throw new IllegalArgumentException("payment token output key certificate does not match receive request");
    }
    return transactionSubmitter.submitAudit(paymentToken.audit()).thenApply(response -> {
      ensureSuccess(response);
      final OfflineNoteV2WalletNote accepted =
          pending.withState(OfflineNoteV2WalletNoteState.SPENDABLE, clock.getAsLong());
      store.upsert(accepted);
      return accepted;
    });
  }

  public CompletableFuture<OfflineNoteV2WalletNote> redeem(final OfflineNoteV2WalletNote note) {
    return redeem(note, accountId);
  }

  public CompletableFuture<OfflineNoteV2WalletNote> redeem(
      final OfflineNoteV2WalletNote note, final String recipient) {
    if (transactionSubmitter == null) {
      return failedFuture(new IllegalStateException(
          "Offline Note V2 transaction submitter is required for redeem"));
    }
    final OfflineNoteV2WalletNote current =
        store.findNote(note.noteCommitment()) == null ? note : store.findNote(note.noteCommitment());
    if (current.state() != OfflineNoteV2WalletNoteState.SPENDABLE) {
      throw new IllegalArgumentException("only spendable Offline Note V2 notes can be redeemed");
    }
    final byte[] inputNullifier = deriveInputNullifier(current);
    final OfflineNoteV2.RedeemV2 draft =
        new OfflineNoteV2.RedeemV2(
            current.noteCommitment(),
            Collections.singletonList(inputNullifier),
            current.keyCertificate(),
            recipient,
            current.assetId(),
            current.canonicalAmount(),
            placeholderProof());
    final OfflineNoteV2.RedeemV2 redemption =
        draft.replacingRecursiveProof(proofProvider.proveRedeem(draft));
    redemption.validateProofBinding();
    final OfflineNoteV2WalletNote pending =
        current.withState(OfflineNoteV2WalletNoteState.REDEEM_PENDING, clock.getAsLong());
    store.upsert(pending);
    return transactionSubmitter.submitRedeem(redemption).thenApply(response -> {
      ensureSuccess(response);
      return pending;
    });
  }

  public CompletableFuture<List<OfflineNoteV2WalletNote>> sync() {
    if (syncResolver == null) {
      return CompletableFuture.completedFuture(store.listNotes());
    }
    CompletableFuture<Void> chain = CompletableFuture.completedFuture(null);
    for (final OfflineNoteV2WalletNote snapshot : store.listNotes()) {
      if (!isPendingState(snapshot.state())) {
        continue;
      }
      chain = chain.thenCompose(ignored -> {
        final OfflineNoteV2WalletNote current = store.findNote(snapshot.noteCommitment());
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

  private List<OfflineNoteV2WalletNote> selectSpendableNotes(
      final String assetDefinitionId, final BigDecimal requestedAmount) {
    final List<OfflineNoteV2WalletNote> selected = new ArrayList<>();
    BigDecimal total = BigDecimal.ZERO;
    for (final OfflineNoteV2WalletNote note : store.listNotes()) {
      if (note.state() != OfflineNoteV2WalletNoteState.SPENDABLE) {
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
        throw new IllegalArgumentException("Offline Note V2 payments support at most 4 input notes");
      }
    }
    if (selected.isEmpty() || total.compareTo(requestedAmount) < 0) {
      throw new IllegalArgumentException("insufficient spendable Offline Note V2 balance");
    }
    return selected;
  }

  private byte[] deriveNoteCommitment(
      final OfflineNoteV2.KeyCertificateV2 keyCertificate,
      final String assetId,
      final String amount,
      final byte[] noteSecret,
      final OfflineNoteV2.CommitmentOriginV2 origin) {
    return OfflineNoteV2.deriveNoteCommitment(
        new OfflineNoteV2.NoteCommitmentPreimageV2(
            chainId, keyCertificate.payloadHash(), assetId, amount, noteSecret, origin));
  }

  private byte[] deriveInputNullifier(final OfflineNoteV2WalletNote note) {
    return OfflineNoteV2.deriveInputNullifier(
        new OfflineNoteV2.InputNullifierPreimageV2(
            chainId,
            note.noteCommitment(),
            note.keyCertificate().payloadHash(),
            note.noteSecret()));
  }

  private byte[] random32() {
    final byte[] bytes = randomSource.nextBytes(32);
    if (bytes.length != 32) {
      throw new IllegalArgumentException(
          "Offline Note V2 random source must return exactly 32 bytes");
    }
    return bytes;
  }

  private static OfflineNoteV2.RecursiveProofV2 placeholderProof() {
    return new OfflineNoteV2.RecursiveProofV2(
        OfflineNoteV2.hash("offline-note-v2-draft-proof".getBytes(java.nio.charset.StandardCharsets.UTF_8)),
        new OfflineNoteV2.ProofBox(OfflineNoteV2.RECURSIVE_BACKEND, new byte[] {1}));
  }

  private static void ensureSuccess(final ClientResponse response) {
    if (response.statusCode() < 200 || response.statusCode() > 299) {
      throw new IllegalArgumentException(
          "Offline Note V2 transaction rejected with HTTP "
              + response.statusCode()
              + ": "
              + response.message());
    }
  }

  private static boolean isPendingState(final OfflineNoteV2WalletNoteState state) {
    return state == OfflineNoteV2WalletNoteState.RECEIVE_PENDING
        || state == OfflineNoteV2WalletNoteState.CHANGE_PENDING
        || state == OfflineNoteV2WalletNoteState.SPEND_PENDING
        || state == OfflineNoteV2WalletNoteState.REDEEM_PENDING;
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
