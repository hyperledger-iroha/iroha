package org.hyperledger.iroha.android.offline;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Outcome index that maps committed/rejected Offline Note instructions to note states. */
public final class OfflineNoteOutcomeIndex {
  public static final String KIND_ISSUE = "IssueOfflineNote";
  public static final String KIND_REDEEM = "RedeemOfflineNote";
  public static final String KIND_AUDIT = "AuditOfflineNote";
  public static final String STATUS_COMMITTED = "Committed";
  public static final String STATUS_REJECTED = "Rejected";

  private final Map<String, String> committedIssues = new LinkedHashMap<>();
  private final Map<String, String> rejectedIssues = new LinkedHashMap<>();
  private final Map<String, String> committedRedeems = new LinkedHashMap<>();
  private final Map<String, String> rejectedRedeems = new LinkedHashMap<>();

  public OfflineNoteOutcomeIndex recordCommittedIssue(
      final OfflineNote.Issue issue, final String transactionHashHex) {
    putFirst(committedIssues, issue.noteCommitment(), transactionHashHex);
    return this;
  }

  public OfflineNoteOutcomeIndex recordRejectedIssue(
      final OfflineNote.Issue issue, final String transactionHashHex) {
    putFirst(rejectedIssues, issue.noteCommitment(), transactionHashHex);
    return this;
  }

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
      case ISSUE_PENDING -> resolveIssuePending(note);
      case REDEEM_PENDING -> resolveRedeemPending(note);
      default -> null;
    };
  }

  private OfflineNoteSyncResolution resolveIssuePending(final OfflineNoteWalletNote note) {
    final String commitmentKey = note.noteCommitmentHex();
    if (committedIssues.containsKey(commitmentKey)) {
      return new OfflineNoteSyncResolution(
          OfflineNoteWalletNoteState.SPENDABLE, committedIssues.get(commitmentKey));
    }
    if (rejectedIssues.containsKey(commitmentKey)) {
      return new OfflineNoteSyncResolution(
          OfflineNoteWalletNoteState.CANCELLED, rejectedIssues.get(commitmentKey));
    }
    return null;
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
      final boolean committed = STATUS_COMMITTED.equals(outcome.transactionStatus());
      final boolean rejected = STATUS_REJECTED.equals(outcome.transactionStatus());
      if (!committed && !rejected) {
        continue;
      }
      if (KIND_ISSUE.equals(outcome.kind())) {
        final OfflineNote.Issue issue =
            OfflineNote.decodeIssueInstruction(outcome.encodedInstruction());
        if (committed) {
          index.recordCommittedIssue(issue, outcome.transactionHashHex());
        } else {
          index.recordRejectedIssue(issue, outcome.transactionHashHex());
        }
      } else if (KIND_AUDIT.equals(outcome.kind())) {
        final OfflineNote.AuditBundle audit =
            OfflineNote.decodeAuditInstruction(outcome.encodedInstruction());
        if (committed) {
          index.recordCommittedAudit(audit, outcome.transactionHashHex());
        } else {
          index.recordRejectedAudit(audit, outcome.transactionHashHex());
        }
      } else if (KIND_REDEEM.equals(outcome.kind())) {
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
