package org.hyperledger.iroha.android.offline;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/** Outcome index that maps committed/rejected Offline Note V2 instructions to note states. */
public final class OfflineNoteV2OutcomeIndex {
  public static final String KIND_ISSUE = "IssueOfflineNoteV2";
  public static final String KIND_REDEEM = "RedeemOfflineNoteV2";
  public static final String KIND_AUDIT = "AuditOfflineNoteV2";

  private final Map<String, String> spentInputNullifiers = new LinkedHashMap<>();
  private final Map<String, String> rejectedAuditInputs = new LinkedHashMap<>();
  private final Map<String, String> committedAuditOutputs = new LinkedHashMap<>();
  private final Map<String, String> rejectedAuditOutputs = new LinkedHashMap<>();
  private final Map<String, String> committedRedeems = new LinkedHashMap<>();
  private final Map<String, String> rejectedRedeems = new LinkedHashMap<>();

  public OfflineNoteV2OutcomeIndex recordCommittedAudit(
      final OfflineNoteV2.AuditBundleV2 audit, final String transactionHashHex) {
    for (final byte[] nullifier : audit.inputNullifiers()) {
      putFirst(spentInputNullifiers, nullifier, transactionHashHex);
    }
    for (final byte[] commitment : audit.outputCommitments()) {
      putFirst(committedAuditOutputs, commitment, transactionHashHex);
    }
    return this;
  }

  public OfflineNoteV2OutcomeIndex recordRejectedAudit(
      final OfflineNoteV2.AuditBundleV2 audit, final String transactionHashHex) {
    for (final OfflineNoteV2.IssuedClaimV2 claim : audit.inputClaims()) {
      putFirst(rejectedAuditInputs, claim.noteCommitment(), transactionHashHex);
    }
    for (final byte[] commitment : audit.outputCommitments()) {
      putFirst(rejectedAuditOutputs, commitment, transactionHashHex);
    }
    return this;
  }

  public OfflineNoteV2OutcomeIndex recordCommittedRedeem(
      final OfflineNoteV2.RedeemV2 redeem, final String transactionHashHex) {
    putFirst(committedRedeems, redeem.sourceNoteCommitment(), transactionHashHex);
    return this;
  }

  public OfflineNoteV2OutcomeIndex recordRejectedRedeem(
      final OfflineNoteV2.RedeemV2 redeem, final String transactionHashHex) {
    putFirst(rejectedRedeems, redeem.sourceNoteCommitment(), transactionHashHex);
    return this;
  }

  public OfflineNoteV2SyncResolution resolve(final OfflineNoteV2WalletNote note) {
    return switch (note.state()) {
      case SPEND_PENDING -> resolveSpendPending(note);
      case CHANGE_PENDING, RECEIVE_PENDING -> resolveOutputPending(note);
      case REDEEM_PENDING -> resolveRedeemPending(note);
      default -> null;
    };
  }

  private OfflineNoteV2SyncResolution resolveSpendPending(final OfflineNoteV2WalletNote note) {
    final byte[] inputNullifier =
        OfflineNoteV2.deriveInputNullifier(
            new OfflineNoteV2.InputNullifierPreimageV2(
                note.chainId(),
                note.noteCommitment(),
                note.keyCertificate().payloadHash(),
                note.noteSecret()));
    final String nullifierKey = OfflineNoteV2Wallet.hexLower(inputNullifier);
    if (spentInputNullifiers.containsKey(nullifierKey)) {
      return new OfflineNoteV2SyncResolution(
          OfflineNoteV2WalletNoteState.SPENT, spentInputNullifiers.get(nullifierKey));
    }
    final String commitmentKey = note.noteCommitmentHex();
    if (rejectedAuditInputs.containsKey(commitmentKey)) {
      return new OfflineNoteV2SyncResolution(
          OfflineNoteV2WalletNoteState.SPENDABLE, rejectedAuditInputs.get(commitmentKey));
    }
    return null;
  }

  private OfflineNoteV2SyncResolution resolveOutputPending(final OfflineNoteV2WalletNote note) {
    final String commitmentKey = note.noteCommitmentHex();
    if (committedAuditOutputs.containsKey(commitmentKey)) {
      return new OfflineNoteV2SyncResolution(
          OfflineNoteV2WalletNoteState.SPENDABLE, committedAuditOutputs.get(commitmentKey));
    }
    if (rejectedAuditOutputs.containsKey(commitmentKey)) {
      return new OfflineNoteV2SyncResolution(
          OfflineNoteV2WalletNoteState.CANCELLED, rejectedAuditOutputs.get(commitmentKey));
    }
    return null;
  }

  private OfflineNoteV2SyncResolution resolveRedeemPending(final OfflineNoteV2WalletNote note) {
    final String commitmentKey = note.noteCommitmentHex();
    if (committedRedeems.containsKey(commitmentKey)) {
      return new OfflineNoteV2SyncResolution(
          OfflineNoteV2WalletNoteState.REDEEMED, committedRedeems.get(commitmentKey));
    }
    if (rejectedRedeems.containsKey(commitmentKey)) {
      return new OfflineNoteV2SyncResolution(
          OfflineNoteV2WalletNoteState.SPENDABLE, rejectedRedeems.get(commitmentKey));
    }
    return null;
  }

  private static void putFirst(
      final Map<String, String> target, final byte[] bytes, final String transactionHashHex) {
    final String key = OfflineNoteV2Wallet.hexLower(bytes);
    if (!target.containsKey(key)) {
      target.put(key, transactionHashHex);
    }
  }

  public static OfflineNoteV2OutcomeIndex fromExplorerOutcomes(
      final List<OfflineNoteV2ExplorerInstructionOutcome> outcomes) {
    final OfflineNoteV2OutcomeIndex index = new OfflineNoteV2OutcomeIndex();
    for (final OfflineNoteV2ExplorerInstructionOutcome outcome : outcomes) {
      final String normalized = outcome.transactionStatus().toLowerCase(Locale.ROOT);
      final boolean committed = "committed".equals(normalized);
      final boolean rejected = "rejected".equals(normalized);
      if (!committed && !rejected) {
        continue;
      }
      if (KIND_AUDIT.equalsIgnoreCase(outcome.kind())) {
        final OfflineNoteV2.AuditBundleV2 audit =
            OfflineNoteV2.decodeAuditInstruction(outcome.encodedInstruction());
        if (committed) {
          index.recordCommittedAudit(audit, outcome.transactionHashHex());
        } else {
          index.recordRejectedAudit(audit, outcome.transactionHashHex());
        }
      } else if (KIND_REDEEM.equalsIgnoreCase(outcome.kind())) {
        final OfflineNoteV2.RedeemV2 redeem =
            OfflineNoteV2.decodeRedeemInstruction(outcome.encodedInstruction());
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
