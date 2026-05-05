package org.hyperledger.iroha.android.model.instructions;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Typed Android builders and constants for native numeric asset escrow instructions. */
public final class NativeEscrowInstructions {

  private static final String ARG_ACTION = "action";
  private static final String ARG_ESCROW_ID = "escrow_id";
  private static final String ARG_ASSET_DEFINITION = "asset_definition";
  private static final String ARG_AMOUNT = "amount";
  private static final String ARG_BUYER_AMOUNT = "buyer_amount";
  private static final String ARG_SELLER_AMOUNT = "seller_amount";
  private static final String ARG_EVIDENCE_HASHES = "evidence_hashes";
  private static final String ARG_FUNDING_NULLIFIERS = "funding_nullifiers";
  private static final String ARG_ESCROW_COMMITMENT = "escrow_commitment";
  private static final String ARG_ESCROW_NULLIFIERS = "escrow_nullifiers";
  private static final String ARG_BUYER_OUTPUT_COMMITMENTS = "buyer_output_commitments";
  private static final String ARG_SELLER_OUTPUT_COMMITMENTS = "seller_output_commitments";
  private static final String ARG_PROOF = "proof";
  private static final String ARG_ROOT_HINT = "root_hint";

  /** Permission allowing a court account or role to resolve disputed native escrows. */
  public static final String CAN_RESOLVE_ESCROW_DISPUTE = "CanResolveEscrowDispute";

  private NativeEscrowInstructions() {}

  /** Native escrow lifecycle statuses returned by query/event surfaces. */
  public enum Status {
    OPEN("Open"),
    ACCEPTED("Accepted"),
    PAYMENT_SENT("PaymentSent"),
    DISPUTED("Disputed"),
    RELEASED("Released"),
    CANCELLED("Cancelled"),
    RESOLVED("Resolved");

    private final String wireName;

    Status(final String wireName) {
      this.wireName = wireName;
    }

    public String wireName() {
      return wireName;
    }

    public static Status fromWireName(final String value) {
      final String normalized = requireValue(value, "status");
      for (final Status status : values()) {
        if (status.wireName.equals(normalized)) {
          return status;
        }
      }
      throw new IllegalArgumentException("Unknown native escrow status: " + value);
    }
  }

  public static OpenAssetEscrow openAssetEscrow(
      final String escrowId,
      final String assetDefinition,
      final String amount,
      final List<String> evidenceHashes) {
    return new OpenAssetEscrow(escrowId, assetDefinition, amount, evidenceHashes);
  }

  public static AcceptAssetEscrow acceptAssetEscrow(final String escrowId) {
    return new AcceptAssetEscrow(escrowId);
  }

  public static MarkEscrowPaymentSent markPaymentSent(final String escrowId) {
    return new MarkEscrowPaymentSent(escrowId);
  }

  public static ReleaseAssetEscrow releaseAssetEscrow(final String escrowId) {
    return new ReleaseAssetEscrow(escrowId);
  }

  public static CancelAssetEscrow cancelAssetEscrow(final String escrowId) {
    return new CancelAssetEscrow(escrowId);
  }

  public static OpenEscrowDispute openEscrowDispute(
      final String escrowId, final List<String> evidenceHashes) {
    return new OpenEscrowDispute(escrowId, evidenceHashes);
  }

  public static ResolveEscrowDispute resolveEscrowDispute(
      final String escrowId,
      final String buyerAmount,
      final String sellerAmount,
      final List<String> evidenceHashes) {
    return new ResolveEscrowDispute(escrowId, buyerAmount, sellerAmount, evidenceHashes);
  }

  public static AnonymousEscrowInstruction openAnonymousAssetEscrow(
      final String escrowId,
      final String assetDefinition,
      final List<String> fundingNullifiers,
      final String escrowCommitment,
      final String proof,
      final String rootHint,
      final List<String> evidenceHashes) {
    final Map<String, String> args = new LinkedHashMap<>();
    args.put(ARG_ACTION, AnonymousEscrowInstruction.ACTION_OPEN);
    args.put(ARG_ESCROW_ID, requireValue(escrowId, "escrowId"));
    args.put(ARG_ASSET_DEFINITION, requireValue(assetDefinition, "assetDefinition"));
    appendList(args, ARG_FUNDING_NULLIFIERS, normalizedList(fundingNullifiers, "fundingNullifiers"));
    args.put(ARG_ESCROW_COMMITMENT, requireValue(escrowCommitment, "escrowCommitment"));
    args.put(ARG_PROOF, requireValue(proof, "proof"));
    appendOptional(args, ARG_ROOT_HINT, rootHint);
    appendEvidence(args, normalizedEvidenceHashes(evidenceHashes));
    return new AnonymousEscrowInstruction(AnonymousEscrowInstruction.ACTION_OPEN, escrowId, args);
  }

  public static AnonymousEscrowInstruction acceptAnonymousAssetEscrow(final String escrowId) {
    return anonymousEscrowOnly(AnonymousEscrowInstruction.ACTION_ACCEPT, escrowId);
  }

  public static AnonymousEscrowInstruction markAnonymousPaymentSent(final String escrowId) {
    return anonymousEscrowOnly(AnonymousEscrowInstruction.ACTION_MARK_PAYMENT_SENT, escrowId);
  }

  public static AnonymousEscrowInstruction releaseAnonymousAssetEscrow(
      final String escrowId,
      final List<String> escrowNullifiers,
      final List<String> buyerOutputCommitments,
      final String proof,
      final String rootHint) {
    final Map<String, String> args = anonymousProofArguments(
        AnonymousEscrowInstruction.ACTION_RELEASE, escrowId, escrowNullifiers, proof, rootHint);
    appendList(
        args,
        ARG_BUYER_OUTPUT_COMMITMENTS,
        normalizedList(buyerOutputCommitments, "buyerOutputCommitments"));
    return new AnonymousEscrowInstruction(AnonymousEscrowInstruction.ACTION_RELEASE, escrowId, args);
  }

  public static AnonymousEscrowInstruction cancelAnonymousAssetEscrow(
      final String escrowId,
      final List<String> escrowNullifiers,
      final List<String> sellerOutputCommitments,
      final String proof,
      final String rootHint) {
    final Map<String, String> args = anonymousProofArguments(
        AnonymousEscrowInstruction.ACTION_CANCEL, escrowId, escrowNullifiers, proof, rootHint);
    appendList(
        args,
        ARG_SELLER_OUTPUT_COMMITMENTS,
        normalizedList(sellerOutputCommitments, "sellerOutputCommitments"));
    return new AnonymousEscrowInstruction(AnonymousEscrowInstruction.ACTION_CANCEL, escrowId, args);
  }

  public static AnonymousEscrowInstruction openAnonymousEscrowDispute(
      final String escrowId, final List<String> evidenceHashes) {
    final Map<String, String> args = escrowOnlyArguments(
        AnonymousEscrowInstruction.ACTION_OPEN_DISPUTE, requireValue(escrowId, "escrowId"));
    appendEvidence(args, normalizedEvidenceHashes(evidenceHashes));
    return new AnonymousEscrowInstruction(
        AnonymousEscrowInstruction.ACTION_OPEN_DISPUTE, escrowId, args);
  }

  public static AnonymousEscrowInstruction resolveAnonymousEscrowDispute(
      final String escrowId,
      final List<String> escrowNullifiers,
      final List<String> buyerOutputCommitments,
      final List<String> sellerOutputCommitments,
      final String proof,
      final String rootHint,
      final List<String> evidenceHashes) {
    final Map<String, String> args = anonymousProofArguments(
        AnonymousEscrowInstruction.ACTION_RESOLVE, escrowId, escrowNullifiers, proof, rootHint);
    appendList(
        args,
        ARG_BUYER_OUTPUT_COMMITMENTS,
        normalizedList(buyerOutputCommitments, "buyerOutputCommitments"));
    appendList(
        args,
        ARG_SELLER_OUTPUT_COMMITMENTS,
        normalizedList(sellerOutputCommitments, "sellerOutputCommitments"));
    appendEvidence(args, normalizedEvidenceHashes(evidenceHashes));
    return new AnonymousEscrowInstruction(AnonymousEscrowInstruction.ACTION_RESOLVE, escrowId, args);
  }

  private static AnonymousEscrowInstruction anonymousEscrowOnly(
      final String action, final String escrowId) {
    return new AnonymousEscrowInstruction(
        action,
        escrowId,
        escrowOnlyArguments(action, requireValue(escrowId, "escrowId")));
  }

  private static Map<String, String> anonymousProofArguments(
      final String action,
      final String escrowId,
      final List<String> escrowNullifiers,
      final String proof,
      final String rootHint) {
    final Map<String, String> args = escrowOnlyArguments(action, requireValue(escrowId, "escrowId"));
    appendList(args, ARG_ESCROW_NULLIFIERS, normalizedList(escrowNullifiers, "escrowNullifiers"));
    args.put(ARG_PROOF, requireValue(proof, "proof"));
    appendOptional(args, ARG_ROOT_HINT, rootHint);
    return args;
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static String requireValue(final String value, final String fieldName) {
    final String trimmed = Objects.requireNonNull(value, fieldName).trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(fieldName + " must not be blank");
    }
    return trimmed;
  }

  private static List<String> normalizedEvidenceHashes(final List<String> evidenceHashes) {
    if (evidenceHashes == null || evidenceHashes.isEmpty()) {
      return Collections.emptyList();
    }
    final List<String> normalized = new ArrayList<>(evidenceHashes.size());
    for (int i = 0; i < evidenceHashes.size(); i++) {
      normalized.add(requireValue(evidenceHashes.get(i), "evidenceHashes[" + i + "]"));
    }
    return Collections.unmodifiableList(normalized);
  }

  private static List<String> normalizedList(final List<String> values, final String fieldName) {
    if (values == null || values.isEmpty()) {
      return Collections.emptyList();
    }
    final List<String> normalized = new ArrayList<>(values.size());
    for (int i = 0; i < values.size(); i++) {
      normalized.add(requireValue(values.get(i), fieldName + "[" + i + "]"));
    }
    return Collections.unmodifiableList(normalized);
  }

  private static List<String> parseEvidenceHashes(final String raw) {
    if (raw == null || raw.isBlank()) {
      return Collections.emptyList();
    }
    final List<String> values = new ArrayList<>();
    for (final String part : raw.split(",")) {
      final String trimmed = part.trim();
      if (!trimmed.isEmpty()) {
        values.add(trimmed);
      }
    }
    return Collections.unmodifiableList(values);
  }

  private static void appendEvidence(
      final Map<String, String> arguments, final List<String> evidenceHashes) {
    if (evidenceHashes != null && !evidenceHashes.isEmpty()) {
      arguments.put(ARG_EVIDENCE_HASHES, String.join(",", evidenceHashes));
    }
  }

  private static void appendList(
      final Map<String, String> arguments, final String key, final List<String> values) {
    arguments.put(key, String.join(",", values));
  }

  private static void appendOptional(
      final Map<String, String> arguments, final String key, final String value) {
    if (value != null) {
      arguments.put(key, requireValue(value, key));
    }
  }

  private static Map<String, String> immutable(final Map<String, String> arguments) {
    return Collections.unmodifiableMap(new LinkedHashMap<>(arguments));
  }

  private static Map<String, String> escrowOnlyArguments(
      final String action, final String escrowId) {
    final Map<String, String> args = new LinkedHashMap<>();
    args.put(ARG_ACTION, action);
    args.put(ARG_ESCROW_ID, escrowId);
    return args;
  }

  /** Typed representation of `OpenAssetEscrow`. */
  public static final class OpenAssetEscrow implements InstructionTemplate {
    public static final String ACTION = "OpenAssetEscrow";

    private final String escrowId;
    private final String assetDefinition;
    private final String amount;
    private final List<String> evidenceHashes;
    private final Map<String, String> arguments;

    public OpenAssetEscrow(
        final String escrowId,
        final String assetDefinition,
        final String amount,
        final List<String> evidenceHashes) {
      this(
          requireValue(escrowId, "escrowId"),
          requireValue(assetDefinition, "assetDefinition"),
          requireValue(amount, "amount"),
          normalizedEvidenceHashes(evidenceHashes),
          null);
    }

    private OpenAssetEscrow(
        final String escrowId,
        final String assetDefinition,
        final String amount,
        final List<String> evidenceHashes,
        final Map<String, String> argumentOrder) {
      this.escrowId = escrowId;
      this.assetDefinition = assetDefinition;
      this.amount = amount;
      this.evidenceHashes = evidenceHashes;
      if (argumentOrder == null) {
        final Map<String, String> args = new LinkedHashMap<>();
        args.put(ARG_ACTION, ACTION);
        args.put(ARG_ESCROW_ID, escrowId);
        args.put(ARG_ASSET_DEFINITION, assetDefinition);
        args.put(ARG_AMOUNT, amount);
        appendEvidence(args, evidenceHashes);
        this.arguments = immutable(args);
      } else {
        this.arguments = immutable(argumentOrder);
      }
    }

    public String escrowId() {
      return escrowId;
    }

    public String assetDefinition() {
      return assetDefinition;
    }

    public String amount() {
      return amount;
    }

    public List<String> evidenceHashes() {
      return evidenceHashes;
    }

    @Override
    public InstructionKind kind() {
      return InstructionKind.CUSTOM;
    }

    @Override
    public Map<String, String> toArguments() {
      return arguments;
    }

    public static OpenAssetEscrow fromArguments(final Map<String, String> arguments) {
      return new OpenAssetEscrow(
          require(arguments, ARG_ESCROW_ID),
          require(arguments, ARG_ASSET_DEFINITION),
          require(arguments, ARG_AMOUNT),
          parseEvidenceHashes(arguments.get(ARG_EVIDENCE_HASHES)),
          arguments);
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof OpenAssetEscrow other)) {
        return false;
      }
      return Objects.equals(escrowId, other.escrowId)
          && Objects.equals(assetDefinition, other.assetDefinition)
          && Objects.equals(amount, other.amount)
          && Objects.equals(evidenceHashes, other.evidenceHashes);
    }

    @Override
    public int hashCode() {
      return Objects.hash(escrowId, assetDefinition, amount, evidenceHashes);
    }
  }

  private abstract static class EscrowOnlyInstruction implements InstructionTemplate {
    private final String escrowId;
    private final Map<String, String> arguments;

    EscrowOnlyInstruction(
        final String action, final String escrowId, final Map<String, String> argumentOrder) {
      this.escrowId = requireValue(escrowId, "escrowId");
      this.arguments =
          immutable(argumentOrder == null ? escrowOnlyArguments(action, this.escrowId) : argumentOrder);
    }

    public String escrowId() {
      return escrowId;
    }

    @Override
    public InstructionKind kind() {
      return InstructionKind.CUSTOM;
    }

    @Override
    public Map<String, String> toArguments() {
      return arguments;
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      return obj != null && getClass() == obj.getClass()
          && Objects.equals(escrowId, ((EscrowOnlyInstruction) obj).escrowId);
    }

    @Override
    public int hashCode() {
      return Objects.hash(getClass(), escrowId);
    }
  }

  /** Typed representation of `AcceptAssetEscrow`. */
  public static final class AcceptAssetEscrow extends EscrowOnlyInstruction {
    public static final String ACTION = "AcceptAssetEscrow";

    public AcceptAssetEscrow(final String escrowId) {
      super(ACTION, escrowId, null);
    }

    private AcceptAssetEscrow(final String escrowId, final Map<String, String> arguments) {
      super(ACTION, escrowId, arguments);
    }

    public static AcceptAssetEscrow fromArguments(final Map<String, String> arguments) {
      return new AcceptAssetEscrow(require(arguments, ARG_ESCROW_ID), arguments);
    }
  }

  /** Typed representation of `MarkEscrowPaymentSent`. */
  public static final class MarkEscrowPaymentSent extends EscrowOnlyInstruction {
    public static final String ACTION = "MarkEscrowPaymentSent";

    public MarkEscrowPaymentSent(final String escrowId) {
      super(ACTION, escrowId, null);
    }

    private MarkEscrowPaymentSent(final String escrowId, final Map<String, String> arguments) {
      super(ACTION, escrowId, arguments);
    }

    public static MarkEscrowPaymentSent fromArguments(final Map<String, String> arguments) {
      return new MarkEscrowPaymentSent(require(arguments, ARG_ESCROW_ID), arguments);
    }
  }

  /** Typed representation of `ReleaseAssetEscrow`. */
  public static final class ReleaseAssetEscrow extends EscrowOnlyInstruction {
    public static final String ACTION = "ReleaseAssetEscrow";

    public ReleaseAssetEscrow(final String escrowId) {
      super(ACTION, escrowId, null);
    }

    private ReleaseAssetEscrow(final String escrowId, final Map<String, String> arguments) {
      super(ACTION, escrowId, arguments);
    }

    public static ReleaseAssetEscrow fromArguments(final Map<String, String> arguments) {
      return new ReleaseAssetEscrow(require(arguments, ARG_ESCROW_ID), arguments);
    }
  }

  /** Typed representation of `CancelAssetEscrow`. */
  public static final class CancelAssetEscrow extends EscrowOnlyInstruction {
    public static final String ACTION = "CancelAssetEscrow";

    public CancelAssetEscrow(final String escrowId) {
      super(ACTION, escrowId, null);
    }

    private CancelAssetEscrow(final String escrowId, final Map<String, String> arguments) {
      super(ACTION, escrowId, arguments);
    }

    public static CancelAssetEscrow fromArguments(final Map<String, String> arguments) {
      return new CancelAssetEscrow(require(arguments, ARG_ESCROW_ID), arguments);
    }
  }

  /** Typed representation of `OpenEscrowDispute`. */
  public static final class OpenEscrowDispute implements InstructionTemplate {
    public static final String ACTION = "OpenEscrowDispute";

    private final String escrowId;
    private final List<String> evidenceHashes;
    private final Map<String, String> arguments;

    public OpenEscrowDispute(final String escrowId, final List<String> evidenceHashes) {
      this(requireValue(escrowId, "escrowId"), normalizedEvidenceHashes(evidenceHashes), null);
    }

    private OpenEscrowDispute(
        final String escrowId,
        final List<String> evidenceHashes,
        final Map<String, String> argumentOrder) {
      this.escrowId = escrowId;
      this.evidenceHashes = evidenceHashes;
      if (argumentOrder == null) {
        final Map<String, String> args = escrowOnlyArguments(ACTION, escrowId);
        appendEvidence(args, evidenceHashes);
        this.arguments = immutable(args);
      } else {
        this.arguments = immutable(argumentOrder);
      }
    }

    public String escrowId() {
      return escrowId;
    }

    public List<String> evidenceHashes() {
      return evidenceHashes;
    }

    @Override
    public InstructionKind kind() {
      return InstructionKind.CUSTOM;
    }

    @Override
    public Map<String, String> toArguments() {
      return arguments;
    }

    public static OpenEscrowDispute fromArguments(final Map<String, String> arguments) {
      return new OpenEscrowDispute(
          require(arguments, ARG_ESCROW_ID),
          parseEvidenceHashes(arguments.get(ARG_EVIDENCE_HASHES)),
          arguments);
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof OpenEscrowDispute other)) {
        return false;
      }
      return Objects.equals(escrowId, other.escrowId)
          && Objects.equals(evidenceHashes, other.evidenceHashes);
    }

    @Override
    public int hashCode() {
      return Objects.hash(escrowId, evidenceHashes);
    }
  }

  /** Typed representation of `ResolveEscrowDispute`. */
  public static final class ResolveEscrowDispute implements InstructionTemplate {
    public static final String ACTION = "ResolveEscrowDispute";

    private final String escrowId;
    private final String buyerAmount;
    private final String sellerAmount;
    private final List<String> evidenceHashes;
    private final Map<String, String> arguments;

    public ResolveEscrowDispute(
        final String escrowId,
        final String buyerAmount,
        final String sellerAmount,
        final List<String> evidenceHashes) {
      this(
          requireValue(escrowId, "escrowId"),
          requireValue(buyerAmount, "buyerAmount"),
          requireValue(sellerAmount, "sellerAmount"),
          normalizedEvidenceHashes(evidenceHashes),
          null);
    }

    private ResolveEscrowDispute(
        final String escrowId,
        final String buyerAmount,
        final String sellerAmount,
        final List<String> evidenceHashes,
        final Map<String, String> argumentOrder) {
      this.escrowId = escrowId;
      this.buyerAmount = buyerAmount;
      this.sellerAmount = sellerAmount;
      this.evidenceHashes = evidenceHashes;
      if (argumentOrder == null) {
        final Map<String, String> args = new LinkedHashMap<>();
        args.put(ARG_ACTION, ACTION);
        args.put(ARG_ESCROW_ID, escrowId);
        args.put(ARG_BUYER_AMOUNT, buyerAmount);
        args.put(ARG_SELLER_AMOUNT, sellerAmount);
        appendEvidence(args, evidenceHashes);
        this.arguments = immutable(args);
      } else {
        this.arguments = immutable(argumentOrder);
      }
    }

    public String escrowId() {
      return escrowId;
    }

    public String buyerAmount() {
      return buyerAmount;
    }

    public String sellerAmount() {
      return sellerAmount;
    }

    public List<String> evidenceHashes() {
      return evidenceHashes;
    }

    @Override
    public InstructionKind kind() {
      return InstructionKind.CUSTOM;
    }

    @Override
    public Map<String, String> toArguments() {
      return arguments;
    }

    public static ResolveEscrowDispute fromArguments(final Map<String, String> arguments) {
      return new ResolveEscrowDispute(
          require(arguments, ARG_ESCROW_ID),
          require(arguments, ARG_BUYER_AMOUNT),
          require(arguments, ARG_SELLER_AMOUNT),
          parseEvidenceHashes(arguments.get(ARG_EVIDENCE_HASHES)),
          arguments);
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof ResolveEscrowDispute other)) {
        return false;
      }
      return Objects.equals(escrowId, other.escrowId)
          && Objects.equals(buyerAmount, other.buyerAmount)
          && Objects.equals(sellerAmount, other.sellerAmount)
          && Objects.equals(evidenceHashes, other.evidenceHashes);
    }

    @Override
    public int hashCode() {
      return Objects.hash(escrowId, buyerAmount, sellerAmount, evidenceHashes);
    }
  }

  /** Typed representation for proof-carrying anonymous native escrow instructions. */
  public static final class AnonymousEscrowInstruction implements InstructionTemplate {
    public static final String ACTION_OPEN = "OpenAnonymousAssetEscrow";
    public static final String ACTION_ACCEPT = "AcceptAnonymousAssetEscrow";
    public static final String ACTION_MARK_PAYMENT_SENT = "MarkAnonymousEscrowPaymentSent";
    public static final String ACTION_RELEASE = "ReleaseAnonymousAssetEscrow";
    public static final String ACTION_CANCEL = "CancelAnonymousAssetEscrow";
    public static final String ACTION_OPEN_DISPUTE = "OpenAnonymousEscrowDispute";
    public static final String ACTION_RESOLVE = "ResolveAnonymousEscrowDispute";

    private final String action;
    private final String escrowId;
    private final Map<String, String> arguments;

    private AnonymousEscrowInstruction(
        final String action, final String escrowId, final Map<String, String> arguments) {
      this.action = requireValue(action, "action");
      this.escrowId = requireValue(escrowId, "escrowId");
      this.arguments = immutable(arguments);
    }

    public String action() {
      return action;
    }

    public String escrowId() {
      return escrowId;
    }

    @Override
    public InstructionKind kind() {
      return InstructionKind.CUSTOM;
    }

    @Override
    public Map<String, String> toArguments() {
      return arguments;
    }

    public static AnonymousEscrowInstruction fromArguments(final Map<String, String> arguments) {
      return new AnonymousEscrowInstruction(
          require(arguments, ARG_ACTION), require(arguments, ARG_ESCROW_ID), arguments);
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof AnonymousEscrowInstruction other)) {
        return false;
      }
      return Objects.equals(action, other.action)
          && Objects.equals(escrowId, other.escrowId)
          && Objects.equals(arguments, other.arguments);
    }

    @Override
    public int hashCode() {
      return Objects.hash(action, escrowId, arguments);
    }
  }

  public static List<String> evidenceHashes(final String... hashes) {
    if (hashes == null || hashes.length == 0) {
      return Collections.emptyList();
    }
    return normalizedEvidenceHashes(Arrays.asList(hashes));
  }
}
