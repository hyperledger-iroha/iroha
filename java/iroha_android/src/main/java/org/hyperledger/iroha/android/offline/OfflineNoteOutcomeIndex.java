package org.hyperledger.iroha.android.offline;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/** Outcome index that maps committed/rejected Offline Note instructions to note states. */
public final class OfflineNoteOutcomeIndex {
  public static final String KIND_ISSUE = "IssueOfflineNote";
  public static final String KIND_REDEEM = "RedeemOfflineNote";
  public static final String KIND_AUDIT = "AuditOfflineNote";

  private final Map<String, String> committedRedeems = new LinkedHashMap<>();
  private final Map<String, String> rejectedRedeems = new LinkedHashMap<>();

  public OfflineNoteOutcomeIndex recordCommittedAudit(
      final OfflineNote.AuditBundle audit, final String transactionHashHex) {
    return this;
  }

  public OfflineNoteOutcomeIndex recordRejectedAudit(
      final OfflineNote.AuditBundle audit, final String transactionHashHex) {
    return this;
  }

  public OfflineNoteOutcomeIndex recordCommittedRedeem(
      final OfflineNote.Redeem redeem, final String transactionHashHex) {
    putFirst(committedRedeems, redeem.sourceNoteCommitment(), transactionHashHex);
    return this;
  }

  public OfflineNoteOutcomeIndex recordRejectedRedeem(
      final OfflineNote.Redeem redeem, final String transactionHashHex) {
    putFirst(rejectedRedeems, redeem.sourceNoteCommitment(), transactionHashHex);
    return this;
  }

  public OfflineNoteSyncResolution resolve(final OfflineNoteWalletNote note) {
    return switch (note.state()) {
      case REDEEM_PENDING -> resolveRedeemPending(note);
      default -> null;
    };
  }

  private OfflineNoteSyncResolution resolveRedeemPending(final OfflineNoteWalletNote note) {
    final String commitmentKey = note.noteCommitmentHex();
    if (committedRedeems.containsKey(commitmentKey)) {
      return new OfflineNoteSyncResolution(
          OfflineNoteWalletNoteState.REDEEMED, committedRedeems.get(commitmentKey));
    }
    if (rejectedRedeems.containsKey(commitmentKey)) {
      return new OfflineNoteSyncResolution(
          OfflineNoteWalletNoteState.SPENDABLE, rejectedRedeems.get(commitmentKey));
    }
    return null;
  }

  private static void putFirst(
      final Map<String, String> target, final byte[] bytes, final String transactionHashHex) {
    final String key = OfflineNoteWallet.hexLower(bytes);
    if (!target.containsKey(key)) {
      target.put(key, transactionHashHex);
    }
  }

  public static OfflineNoteOutcomeIndex fromExplorerOutcomes(
      final List<OfflineNoteExplorerInstructionOutcome> outcomes) {
    final OfflineNoteOutcomeIndex index = new OfflineNoteOutcomeIndex();
    for (final OfflineNoteExplorerInstructionOutcome outcome : outcomes) {
      final String normalized = outcome.transactionStatus().toLowerCase(Locale.ROOT);
      final boolean committed = "committed".equals(normalized);
      final boolean rejected = "rejected".equals(normalized);
      if (!committed && !rejected) {
        continue;
      }
      if (KIND_AUDIT.equalsIgnoreCase(outcome.kind())) {
        final OfflineNote.AuditBundle audit =
            OfflineNote.decodeAuditInstruction(outcome.encodedInstruction());
        if (committed) {
          index.recordCommittedAudit(audit, outcome.transactionHashHex());
        } else {
          index.recordRejectedAudit(audit, outcome.transactionHashHex());
        }
      } else if (KIND_REDEEM.equalsIgnoreCase(outcome.kind())) {
        final OfflineNote.Redeem redeem =
            OfflineNote.decodeRedeemInstruction(outcome.encodedInstruction());
        if (committed) {
          index.recordCommittedRedeem(redeem, outcome.transactionHashHex());
        } else {
          index.recordRejectedRedeem(redeem, outcome.transactionHashHex());
        }
      }
    }
    return index;
  }
}
