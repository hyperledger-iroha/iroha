package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;

/** App-facing offline-cash lifecycle helpers that keep online sync out of local exchange paths. */
public final class OfflineCashLifecycle {
  public static final String TRANSPORT_QR = "qr";
  public static final String TRANSPORT_NFC = "nfc";
  public static final String TRANSPORT_NEARBY = "nearby";

  private OfflineCashLifecycle() {}

  public static final class NfcCapability {
    private final boolean supported;
    private final String reason;

    private NfcCapability(final boolean supported, final String reason) {
      this.supported = supported;
      this.reason = reason;
    }

    public static NfcCapability supported() {
      return new NfcCapability(true, null);
    }

    public static NfcCapability unavailable(final String reason) {
      return new NfcCapability(false, Objects.requireNonNull(reason, "reason"));
    }

    public boolean supportedFlag() {
      return supported;
    }

    public String reason() {
      return reason;
    }
  }

  public static final class TransportCapabilities {
    private final boolean qrStreaming;
    private final NfcCapability nfc;
    private final boolean nearby;

    public TransportCapabilities(
        final boolean qrStreaming, final NfcCapability nfc, final boolean nearby) {
      this.qrStreaming = qrStreaming;
      this.nfc = Objects.requireNonNull(nfc, "nfc");
      this.nearby = nearby;
    }

    public List<String> supportedTransportKinds() {
      final List<String> kinds = new ArrayList<>();
      if (qrStreaming) {
        kinds.add(TRANSPORT_QR);
      }
      if (nfc.supportedFlag()) {
        kinds.add(TRANSPORT_NFC);
      }
      if (nearby) {
        kinds.add(TRANSPORT_NEARBY);
      }
      return Collections.unmodifiableList(kinds);
    }
  }

  public static final class ConfigurationSnapshot {
    private final String chainId;
    private final String assetDefinitionId;
    private final boolean offlinePaymentsEnabled;
    private final String issuerPublicKeyBase64;
    private final Integer nativeBridgeAbiVersion;
    private final String artifactSetId;
    private final String circuitId;
    private final long createdAtMs;
    private final Long expiresAtMs;

    public ConfigurationSnapshot(
        final boolean offlinePaymentsEnabled,
        final String issuerPublicKeyBase64,
        final Integer nativeBridgeAbiVersion,
        final Long expiresAtMs) {
      this(
          null,
          null,
          offlinePaymentsEnabled,
          issuerPublicKeyBase64,
          nativeBridgeAbiVersion,
          null,
          null,
          0L,
          expiresAtMs);
    }

    public ConfigurationSnapshot(
        final String chainId,
        final String assetDefinitionId,
        final boolean offlinePaymentsEnabled,
        final String issuerPublicKeyBase64,
        final Integer nativeBridgeAbiVersion,
        final String artifactSetId,
        final String circuitId,
        final long createdAtMs,
        final Long expiresAtMs) {
      this.chainId = chainId;
      this.assetDefinitionId = assetDefinitionId;
      this.offlinePaymentsEnabled = offlinePaymentsEnabled;
      this.issuerPublicKeyBase64 = issuerPublicKeyBase64;
      this.nativeBridgeAbiVersion = nativeBridgeAbiVersion;
      this.artifactSetId = artifactSetId;
      this.circuitId = circuitId;
      this.createdAtMs = createdAtMs;
      this.expiresAtMs = expiresAtMs;
    }

    public String chainId() {
      return chainId;
    }

    public String assetDefinitionId() {
      return assetDefinitionId;
    }

    public boolean offlinePaymentsEnabled() {
      return offlinePaymentsEnabled;
    }

    public String issuerPublicKeyBase64() {
      return issuerPublicKeyBase64;
    }

    public Integer nativeBridgeAbiVersion() {
      return nativeBridgeAbiVersion;
    }

    public String artifactSetId() {
      return artifactSetId;
    }

    public String circuitId() {
      return circuitId;
    }

    public long createdAtMs() {
      return createdAtMs;
    }

    public Long expiresAtMs() {
      return expiresAtMs;
    }

    public void requireUsableForOfflineExchange(
        final long nowMs, final Integer requiredNativeBridgeAbiVersion) {
      if (!offlinePaymentsEnabled) {
        throw new ConfigurationSnapshotException(
            "offline_payments_disabled",
            "Offline cash is disabled in the cached configuration snapshot.");
      }
      if (!isCanonicalSnapshotText(issuerPublicKeyBase64)) {
        throw new ConfigurationSnapshotException(
            "missing_issuer_public_key",
            "Offline cash requires a cached issuer public key before offline exchange.");
      }
      if (expiresAtMs != null && expiresAtMs <= nowMs) {
        throw new ConfigurationSnapshotException(
            "expired",
            "Offline cash configuration snapshot expired at " + expiresAtMs + ".");
      }
      final Integer checkedNativeBridgeAbiVersion =
          positiveNativeBridgeAbiVersion(nativeBridgeAbiVersion, "nativeBridgeAbiVersion");
      final Integer checkedRequiredNativeBridgeAbiVersion =
          positiveNativeBridgeAbiVersion(
              requiredNativeBridgeAbiVersion, "requiredNativeBridgeAbiVersion");
      if (checkedRequiredNativeBridgeAbiVersion != null
          && (checkedNativeBridgeAbiVersion == null
              || checkedNativeBridgeAbiVersion < checkedRequiredNativeBridgeAbiVersion)) {
        throw new ConfigurationSnapshotException(
            "unsupported_native_bridge_abi",
            "Offline cash requires native bridge ABI " + checkedRequiredNativeBridgeAbiVersion + ".");
      }
    }
  }

  private static Integer positiveNativeBridgeAbiVersion(final Integer value, final String fieldName) {
    if (value == null) {
      return null;
    }
    if (value.intValue() <= 0) {
      throw new ConfigurationSnapshotException(
          "malformed_snapshot",
          "Offline cash configuration snapshot field " + fieldName + " must be a positive integer.");
    }
    return value;
  }

  private static boolean isCanonicalSnapshotText(final String value) {
    if (value == null || value.isEmpty()) {
      return false;
    }
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      if (c <= 0x20 || c > 0x7E) {
        return false;
      }
    }
    return true;
  }

  public static final class ConfigurationSnapshotException extends IllegalStateException {
    private final String code;

    public ConfigurationSnapshotException(final String code, final String message) {
      super(message);
      this.code = code;
    }

    public String code() {
      return code;
    }
  }

  public interface AuditReceiptSynchronizer {
    CompletableFuture<Boolean> hasPendingAuditReceipts();

    CompletableFuture<Void> syncPendingAuditReceipts();
  }

  public interface Wallet {
    CompletableFuture<Object> load(String assetDefinitionId, String amount);

    Object prepareReceive(String assetDefinitionId, String amount);

    Object createPayment(Object receiveRequest);

    Object acceptPayment(Object paymentToken);

    CompletableFuture<Object> redeem(Object note, String recipient);
  }

  public static final class Controller {
    private final Wallet wallet;
    private final AuditReceiptSynchronizer auditReceiptSynchronizer;

    public Controller(final Wallet wallet, final AuditReceiptSynchronizer auditReceiptSynchronizer) {
      this.wallet = Objects.requireNonNull(wallet, "wallet");
      this.auditReceiptSynchronizer = auditReceiptSynchronizer;
    }

    public Controller(
        final OfflineNoteWallet wallet, final AuditReceiptSynchronizer auditReceiptSynchronizer) {
      this(new OfflineNoteWalletAdapter(wallet), auditReceiptSynchronizer);
    }

    public Controller(
        final OfflineBearerCashWallet wallet,
        final AuditReceiptSynchronizer auditReceiptSynchronizer) {
      this(new OfflineBearerCashWalletAdapter(wallet), auditReceiptSynchronizer);
    }

    public CompletableFuture<Boolean> syncPendingAuditReceiptsIfNeeded() {
      if (auditReceiptSynchronizer == null) {
        return CompletableFuture.completedFuture(false);
      }
      return auditReceiptSynchronizer
          .hasPendingAuditReceipts()
          .thenCompose(
              pending -> {
                if (!pending) {
                  return CompletableFuture.completedFuture(false);
                }
                return auditReceiptSynchronizer
                    .syncPendingAuditReceipts()
                    .thenApply(ignored -> true);
              });
    }

    public CompletableFuture<Object> load(final String assetDefinitionId, final String amount) {
      return syncPendingAuditReceiptsIfNeeded()
          .thenCompose(ignored -> wallet.load(assetDefinitionId, amount));
    }

    public Object prepareReceive(final String assetDefinitionId, final String amount) {
      return wallet.prepareReceive(assetDefinitionId, amount);
    }

    public Object createPayment(final Object receiveRequest) {
      return wallet.createPayment(receiveRequest);
    }

    public Object acceptPayment(final Object paymentToken) {
      return wallet.acceptPayment(paymentToken);
    }

    public CompletableFuture<Object> redeem(final Object note, final String recipient) {
      return wallet.redeem(note, recipient);
    }
  }

  private static final class OfflineNoteWalletAdapter implements Wallet {
    private final OfflineNoteWallet wallet;

    private OfflineNoteWalletAdapter(final OfflineNoteWallet wallet) {
      this.wallet = Objects.requireNonNull(wallet, "wallet");
    }

    @Override
    public CompletableFuture<Object> load(final String assetDefinitionId, final String amount) {
      return wallet.load(assetDefinitionId, amount).thenApply(note -> (Object) note);
    }

    @Override
    public Object prepareReceive(final String assetDefinitionId, final String amount) {
      return wallet.prepareReceive(assetDefinitionId, amount);
    }

    @Override
    public Object createPayment(final Object receiveRequest) {
      if (!(receiveRequest instanceof OfflineNoteReceiveRequest)) {
        throw new IllegalArgumentException("receiveRequest must be OfflineNoteReceiveRequest");
      }
      return wallet.pay((OfflineNoteReceiveRequest) receiveRequest);
    }

    @Override
    public Object acceptPayment(final Object paymentToken) {
      if (!(paymentToken instanceof OfflineNotePaymentToken)) {
        throw new IllegalArgumentException("paymentToken must be OfflineNotePaymentToken");
      }
      return wallet.accept((OfflineNotePaymentToken) paymentToken);
    }

    @Override
    public CompletableFuture<Object> redeem(final Object note, final String recipient) {
      if (!(note instanceof OfflineNoteWalletNote)) {
        throw new IllegalArgumentException("note must be OfflineNoteWalletNote");
      }
      final OfflineNoteWalletNote walletNote = (OfflineNoteWalletNote) note;
      final CompletableFuture<OfflineNoteWalletNote> result =
          recipient == null ? wallet.redeem(walletNote) : wallet.redeem(walletNote, recipient);
      return result.thenApply(redeemed -> (Object) redeemed);
    }
  }

  private static final class OfflineBearerCashWalletAdapter implements Wallet {
    private final OfflineBearerCashWallet wallet;

    private OfflineBearerCashWalletAdapter(final OfflineBearerCashWallet wallet) {
      this.wallet = Objects.requireNonNull(wallet, "wallet");
    }

    @Override
    public CompletableFuture<Object> load(final String assetDefinitionId, final String amount) {
      return wallet.load(assetDefinitionId, amount).thenApply(note -> (Object) note);
    }

    @Override
    public Object prepareReceive(final String assetDefinitionId, final String amount) {
      return wallet.prepareReceive(assetDefinitionId, amount);
    }

    @Override
    public Object createPayment(final Object receiveRequest) {
      if (!(receiveRequest instanceof OfflineBearerCashReceiveRequestV1)) {
        throw new IllegalArgumentException(
            "receiveRequest must be OfflineBearerCashReceiveRequestV1");
      }
      return wallet.pay((OfflineBearerCashReceiveRequestV1) receiveRequest);
    }

    @Override
    public Object acceptPayment(final Object paymentToken) {
      if (!(paymentToken instanceof OfflineBearerCashPaymentTokenV1)) {
        throw new IllegalArgumentException("paymentToken must be OfflineBearerCashPaymentTokenV1");
      }
      return wallet.accept((OfflineBearerCashPaymentTokenV1) paymentToken);
    }

    @Override
    public CompletableFuture<Object> redeem(final Object note, final String recipient) {
      if (!(note instanceof OfflineBearerCashNote)) {
        throw new IllegalArgumentException("note must be OfflineBearerCashNote");
      }
      final OfflineBearerCashNote bearerNote = (OfflineBearerCashNote) note;
      final CompletableFuture<OfflineBearerCashNote> result =
          recipient == null ? wallet.redeem(bearerNote) : wallet.redeem(bearerNote, recipient);
      return result.thenApply(redeemed -> (Object) redeemed);
    }
  }
}
