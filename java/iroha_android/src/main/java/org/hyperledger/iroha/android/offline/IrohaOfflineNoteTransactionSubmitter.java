package org.hyperledger.iroha.android.offline;

import java.util.Collections;
import java.util.ArrayList;
import java.util.List;
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

/** Transaction submitter that wraps Offline instructions in signed Iroha transactions. */
public final class IrohaOfflineNoteTransactionSubmitter implements OfflineNoteTransactionSubmitter {
  private final IrohaClient client;
  private final Signer signer;
  private final String chainId;
  private final String authority;
  private final LongSupplier clock;
  private final TransactionBuilder transactionBuilder;

  public IrohaOfflineNoteTransactionSubmitter(
      final IrohaClient client,
      final Signer signer,
      final String chainId,
      final String authority) {
    this(client, signer, chainId, authority, new NoritoJavaCodecAdapter(), System::currentTimeMillis);
  }

  public IrohaOfflineNoteTransactionSubmitter(
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
  public CompletableFuture<ClientResponse> submitAudit(final OfflineNote.AuditBundle audit) {
    return submit(OfflineNote.auditInstruction(audit));
  }

  @Override
  public CompletableFuture<ClientResponse> submitRedeem(final OfflineNote.Redeem redemption) {
    return submit(OfflineNote.redeemInstruction(redemption));
  }

  @Override
  public CompletableFuture<ClientResponse> submitDefund(
      final OfflineNote.Redeem redemption,
      final List<OfflineNote.AuditBundle> bearerAuditTrail) {
    final List<InstructionBox> instructions = new ArrayList<>();
    for (final OfflineNote.AuditBundle audit : bearerAuditTrail) {
      instructions.add(OfflineNote.auditInstruction(audit));
    }
    instructions.add(OfflineNote.redeemInstruction(redemption));
    return submit(instructions);
  }

  private CompletableFuture<ClientResponse> submit(final InstructionBox instruction) {
    return submit(Collections.singletonList(instruction));
  }

  private CompletableFuture<ClientResponse> submit(final List<InstructionBox> instructions) {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId(chainId)
            .setAuthority(authority)
            .setCreationTimeMs(clock.getAsLong())
            .setInstructions(instructions)
            .build();
    try {
      return client.submitTransaction(transactionBuilder.encodeAndSign(payload, signer));
    } catch (final NoritoException | SigningException ex) {
      return OfflineNoteWallet.failedFuture(ex);
    }
  }
}
