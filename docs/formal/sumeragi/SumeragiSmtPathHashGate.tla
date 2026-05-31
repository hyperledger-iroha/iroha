---- MODULE SumeragiSmtPathHashGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the deterministic SMT path and hash contracts.

This slice captures the consensus-relevant helper rules in
`sumeragi::smt`: `compute_post_state_root(...)`, `node_hash(...)`,
`parent_prefix(...)`, `child_prefix(...)`, `truncate_prefix(...)`, and
`mask_tail_bits(...)`. The concrete Rust implementation hashes with
`iroha_crypto::Hash`; the model abstracts hash bytes to the structural
obligations that keep roots deterministic across peers:

- empty inputs use `Hash::new([])`,
- reads feed the tree only when there are no writes,
- writes feed the tree whenever any write is present,
- leaves and internal nodes use distinct domain tags and ordered fields,
- absent siblings use the fixed empty hash,
- duplicate keys follow the last inserted value and final nodes are key-sorted,
- bit prefixes use the low-bit ordering implemented by `bit_idx % 8`, and
- child/parent/truncation helpers mask tail bits deterministically.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

EmptyTree == 1
InputNoWrites == 2
InputWithWrites == 3
LeafPreimage == 4
NodePreimage == 5
MissingChild == 6
DuplicateKey == 7
CanonicalOrder == 8
TruncateZero == 9
TruncateByteBoundary == 10
TruncateNonByte == 11
BitOrder == 12
ParentPrefix == 13
ChildPrefixLeft == 14
ChildPrefixRight == 15
ChildPrefixTail == 16

Cases == 1..16

EmptyHashForNoLeaves == 1
ReadsSelectedWhenNoWrites == 2
WritesSelectedWhenPresent == 3
ReadsIgnoredWhenWritesPresent == 4
LeafDomainTagZero == 5
LeafBindsKeyHash == 6
LeafBindsValueHash == 7
NodeDomainTagOne == 8
NodeLeftBeforeRight == 9
MissingChildUsesEmptyHash == 10
DuplicateKeyLastWins == 11
NodesSortedByKey == 12
TruncateZeroReturnsEmpty == 13
ByteBoundaryDropsTailBytes == 14
NonByteKeepsLowBitsOnly == 15
BitIndexUsesLowBitOrder == 16
ParentDropsOneBit == 17
ChildExtendsParent == 18
ChildLeftClearsTargetBit == 19
ChildRightSetsTargetBit == 20
ChildMasksTailBits == 21

Actions == 1..21

SpecActions(candidate) ==
  CASE candidate = EmptyTree ->
      {EmptyHashForNoLeaves}
    [] candidate = InputNoWrites ->
      {ReadsSelectedWhenNoWrites}
    [] candidate = InputWithWrites ->
      {WritesSelectedWhenPresent, ReadsIgnoredWhenWritesPresent}
    [] candidate = LeafPreimage ->
      {LeafDomainTagZero, LeafBindsKeyHash, LeafBindsValueHash}
    [] candidate = NodePreimage ->
      {NodeDomainTagOne, NodeLeftBeforeRight}
    [] candidate = MissingChild ->
      {MissingChildUsesEmptyHash}
    [] candidate = DuplicateKey ->
      {DuplicateKeyLastWins}
    [] candidate = CanonicalOrder ->
      {NodesSortedByKey}
    [] candidate = TruncateZero ->
      {TruncateZeroReturnsEmpty}
    [] candidate = TruncateByteBoundary ->
      {ByteBoundaryDropsTailBytes}
    [] candidate = TruncateNonByte ->
      {NonByteKeepsLowBitsOnly}
    [] candidate = BitOrder ->
      {BitIndexUsesLowBitOrder}
    [] candidate = ParentPrefix ->
      {ParentDropsOneBit}
    [] candidate = ChildPrefixLeft ->
      {ChildExtendsParent, ChildLeftClearsTargetBit, ChildMasksTailBits}
    [] candidate = ChildPrefixRight ->
      {ChildExtendsParent, ChildRightSetsTargetBit, ChildMasksTailBits}
    [] candidate = ChildPrefixTail ->
      {ChildMasksTailBits}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = EmptyTree /\ Bug = "empty_root_not_empty_hash" ->
      spec \ {EmptyHashForNoLeaves}
    [] candidate = InputNoWrites /\ Bug = "input_no_writes_uses_writes" ->
      spec \ {ReadsSelectedWhenNoWrites}
    [] candidate = InputWithWrites /\ Bug = "input_with_writes_includes_reads" ->
      spec \ {ReadsIgnoredWhenWritesPresent}
    [] candidate = LeafPreimage /\ Bug = "leaf_drops_domain_tag" ->
      spec \ {LeafDomainTagZero}
    [] candidate = LeafPreimage /\ Bug = "leaf_drops_key_hash" ->
      spec \ {LeafBindsKeyHash}
    [] candidate = LeafPreimage /\ Bug = "leaf_drops_value_hash" ->
      spec \ {LeafBindsValueHash}
    [] candidate = NodePreimage /\ Bug = "node_drops_domain_tag" ->
      spec \ {NodeDomainTagOne}
    [] candidate = NodePreimage /\ Bug = "node_swaps_children" ->
      spec \ {NodeLeftBeforeRight}
    [] candidate = MissingChild /\ Bug = "missing_child_nonempty" ->
      spec \ {MissingChildUsesEmptyHash}
    [] candidate = DuplicateKey /\ Bug = "duplicate_key_first_wins" ->
      spec \ {DuplicateKeyLastWins}
    [] candidate = CanonicalOrder /\ Bug = "key_order_input_order" ->
      spec \ {NodesSortedByKey}
    [] candidate = TruncateZero /\ Bug = "truncate_zero_keeps_byte" ->
      spec \ {TruncateZeroReturnsEmpty}
    [] candidate = TruncateByteBoundary /\
          Bug = "truncate_byte_boundary_keeps_tail" ->
      spec \ {ByteBoundaryDropsTailBytes}
    [] candidate = TruncateNonByte /\ Bug = "truncate_nonbyte_keeps_high_bits" ->
      spec \ {NonByteKeepsLowBitsOnly}
    [] candidate = BitOrder /\ Bug = "bit_order_msb_first" ->
      spec \ {BitIndexUsesLowBitOrder}
    [] candidate = ParentPrefix /\ Bug = "parent_keeps_length" ->
      spec \ {ParentDropsOneBit}
    [] candidate \in {ChildPrefixLeft, ChildPrefixRight} /\
          Bug = "child_does_not_extend_parent" ->
      spec \ {ChildExtendsParent}
    [] candidate = ChildPrefixLeft /\ Bug = "child_left_sets_bit" ->
      spec \ {ChildLeftClearsTargetBit}
    [] candidate = ChildPrefixRight /\ Bug = "child_right_clears_bit" ->
      spec \ {ChildRightSetsTargetBit}
    [] candidate \in {ChildPrefixLeft, ChildPrefixRight, ChildPrefixTail} /\
          Bug = "child_skips_tail_mask" ->
      spec \ {ChildMasksTailBits}
    [] OTHER -> spec

Bugs == {
  "none",
  "empty_root_not_empty_hash",
  "input_no_writes_uses_writes",
  "input_with_writes_includes_reads",
  "leaf_drops_domain_tag",
  "leaf_drops_key_hash",
  "leaf_drops_value_hash",
  "node_drops_domain_tag",
  "node_swaps_children",
  "missing_child_nonempty",
  "duplicate_key_first_wins",
  "key_order_input_order",
  "truncate_zero_keeps_byte",
  "truncate_byte_boundary_keeps_tail",
  "truncate_nonbyte_keeps_high_bits",
  "bit_order_msb_first",
  "parent_keeps_length",
  "child_does_not_extend_parent",
  "child_left_sets_bit",
  "child_right_clears_bit",
  "child_skips_tail_mask"
}

Init ==
  checked = 0

Next ==
  /\ checked < 16
  /\ checked' = checked + 1

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..16
  /\ \A candidate \in Cases:
       /\ SpecActions(candidate) \subseteq Actions
       /\ ImplementationActions(candidate) \subseteq Actions

Safety ==
  \A candidate \in Cases:
    ImplementationActions(candidate) = SpecActions(candidate)

====
