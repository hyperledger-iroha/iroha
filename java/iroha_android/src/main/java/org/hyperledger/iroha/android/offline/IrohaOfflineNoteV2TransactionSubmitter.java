package org.hyperledger.iroha.android.offline;

import java.util.Collections;
import java.util.concurrent.CompletableFuture;
import java.util.function.LongSupplier;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.IrohaClient;
import org.hyperledger.iroha.android.crypto.Signer;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoCodecAdapter;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.TransactionBuilder;

/** Transaction submitter that wraps Offline V2 instructions in signed Iroha transactions. */
public final class IrohaOfflineNoteV2TransactionSubmitter implements OfflineNoteV2TransactionSubmitter {
  private final IrohaClient client;
  private final Signer signer;
  private final String chainId;
  private final String authority;
  private final LongSupplier clock;
  private final TransactionBuilder transactionBuilder;

  public IrohaOfflineNoteV2TransactionSubmitter(
      final IrohaClient client,
      final Signer signer,
      final String chainId,
      final String authority) {
    this(client, signer, chainId, authority, new NoritoJavaCodecAdapter(), System::currentTimeMillis);
  }

  public IrohaOfflineNoteV2TransactionSubmitter(
      final IrohaClient client,
      final Signer signer,
      final String chainId,
      final String authority,
      final NoritoCodecAdapter codecAdapter,
      final LongSupplier clock) {
    this.client = client;
    this.signer = signer;
    this.chainId = chainId;
    this.authority = authority;
    this.clock = clock;
    this.transactionBuilder =
        new TransactionBuilder(codecAdapter, IrohaKeyManager.withSoftwareProvider());
  }

  @Override
  public CompletableFuture<ClientResponse> submitAudit(final OfflineNoteV2.AuditBundleV2 audit) {
    return submit(OfflineNoteV2.auditInstruction(audit));
  }

  @Override
  public CompletableFuture<ClientResponse> submitRedeem(final OfflineNoteV2.RedeemV2 redemption) {
    return submit(OfflineNoteV2.redeemInstruction(redemption));
  }

  private CompletableFuture<ClientResponse> submit(final InstructionBox instruction) {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId(chainId)
            .setAuthority(authority)
            .setCreationTimeMs(clock.getAsLong())
            .setInstructions(Collections.singletonList(instruction))
            .build();
    try {
      return client.submitTransaction(transactionBuilder.encodeAndSign(payload, signer));
    } catch (final NoritoException | SigningException ex) {
      return OfflineNoteV2Wallet.failedFuture(ex);
    }
  }
}
