package org.hyperledger.iroha.android.offline;

import java.util.concurrent.CompletableFuture;
import java.util.List;
import org.hyperledger.iroha.android.client.ClientResponse;

/** Submits direct Offline Note audit/redeem transactions. */
public interface OfflineNoteTransactionSubmitter {
  CompletableFuture<ClientResponse> submitAudit(OfflineNote.AuditBundle audit);
  CompletableFuture<ClientResponse> submitRedeem(OfflineNote.Redeem redemption);
  CompletableFuture<ClientResponse> submitDefund(
      OfflineNote.Redeem redemption, List<OfflineNote.AuditBundle> bearerAuditTrail);
}
