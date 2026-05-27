package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.ClientResponse;

/** App-facing Offline Bearer Cash wallet over the ZK Offline Note engine. */
public final class OfflineBearerCashWallet {
  private final OfflineNoteWallet delegate;

  public OfflineBearerCashWallet(final OfflineNoteWallet delegate) {
    this.delegate = Objects.requireNonNull(delegate, "delegate");
  }

  public OfflineNoteWallet unwrap() {
    return delegate;
  }

  public List<OfflineBearerCashNote> listNotes() {
    return wrapNotes(delegate.listNotes());
  }

  public CompletableFuture<OfflineBearerCashNote> load(
      final String assetDefinitionId, final String amount) {
    return delegate.load(assetDefinitionId, amount).thenApply(OfflineBearerCashNote::new);
  }

  public OfflineBearerCashReceiveRequestV1 prepareReceive(
      final String assetDefinitionId, final String amount) {
    return new OfflineBearerCashReceiveRequestV1(delegate.prepareReceive(assetDefinitionId, amount));
  }

  public OfflineBearerCashPaymentTokenV1 pay(
      final OfflineBearerCashReceiveRequestV1 receiveRequest) {
    return new OfflineBearerCashPaymentTokenV1(
        delegate.pay(Objects.requireNonNull(receiveRequest, "receiveRequest").unwrap()));
  }

  public OfflineBearerCashNote accept(final OfflineBearerCashPaymentTokenV1 paymentToken) {
    return new OfflineBearerCashNote(
        delegate.accept(Objects.requireNonNull(paymentToken, "paymentToken").unwrap()));
  }

  public CompletableFuture<ClientResponse> publishAudit(
      final OfflineBearerCashPaymentTokenV1 paymentToken) {
    return delegate.publishAudit(Objects.requireNonNull(paymentToken, "paymentToken").unwrap());
  }

  public CompletableFuture<OfflineBearerCashNote> redeem(final OfflineBearerCashNote note) {
    return delegate.redeem(Objects.requireNonNull(note, "note").unwrap())
        .thenApply(OfflineBearerCashNote::new);
  }

  public CompletableFuture<OfflineBearerCashNote> redeem(
      final OfflineBearerCashNote note, final String recipient) {
    return delegate.redeem(Objects.requireNonNull(note, "note").unwrap(), recipient)
        .thenApply(OfflineBearerCashNote::new);
  }

  public CompletableFuture<List<OfflineBearerCashNote>> sync() {
    return delegate.sync().thenApply(OfflineBearerCashWallet::wrapNotes);
  }

  private static List<OfflineBearerCashNote> wrapNotes(final List<OfflineNoteWalletNote> notes) {
    final ArrayList<OfflineBearerCashNote> result = new ArrayList<>(notes.size());
    for (final OfflineNoteWalletNote note : notes) {
      result.add(new OfflineBearerCashNote(note));
    }
    return result;
  }
}
