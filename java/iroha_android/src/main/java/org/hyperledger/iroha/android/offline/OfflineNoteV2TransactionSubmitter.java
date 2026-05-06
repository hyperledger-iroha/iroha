package org.hyperledger.iroha.android.offline;

import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.ClientResponse;

/** Submits direct Offline Note V2 audit/redeem transactions. */
public interface OfflineNoteV2TransactionSubmitter {
  CompletableFuture<ClientResponse> submitAudit(OfflineNoteV2.AuditBundleV2 audit);
  CompletableFuture<ClientResponse> submitRedeem(OfflineNoteV2.RedeemV2 redemption);
}
