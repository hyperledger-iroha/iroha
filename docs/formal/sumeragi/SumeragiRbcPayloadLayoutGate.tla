---- MODULE SumeragiRbcPayloadLayoutGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `RbcPayloadLayout`.

This slice captures the deterministic helper contract used by RBC payload
reconstruction: invalid layouts are rejected, legacy plain layouts keep payload
size unknown, plain layouts map chunks by identity, RS16 layouts separate data
and parity shards, and expected encoded-chunk lengths preserve tail, parity,
and out-of-payload data-slot behavior exactly.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

LegacyPlain == "legacy_plain"
NewZeroChunkPlain == "new_zero_chunk_plain"
NewPlainWithErasure == "new_plain_with_erasure"
NewRs16OddChunk == "new_rs16_odd_chunk"
NewRs16ZeroData == "new_rs16_zero_data"
NewRs16ZeroParity == "new_rs16_zero_parity"
NewPlainValid == "new_plain_valid"
NewRs16Valid == "new_rs16_valid"
PlainCounts == "plain_counts"
Rs16Counts == "rs16_counts"
PlainIndexInRange == "plain_index_in_range"
PlainIndexOutOfRange == "plain_index_out_of_range"
Rs16DataIndexFirst == "rs16_data_index_first"
Rs16ParityIndex == "rs16_parity_index"
Rs16TailDataIndex == "rs16_tail_data_index"
Rs16DataBeyondPayloadIndex == "rs16_data_beyond_payload_index"
Rs16ParityAfterPartialStripe == "rs16_parity_after_partial_stripe"
EncodedPlainIndex == "encoded_plain_index"
EncodedRs16PayloadIndex == "encoded_rs16_payload_index"
EncodedPayloadOutOfRange == "encoded_payload_out_of_range"

Cases == {
  LegacyPlain,
  NewZeroChunkPlain,
  NewPlainWithErasure,
  NewRs16OddChunk,
  NewRs16ZeroData,
  NewRs16ZeroParity,
  NewPlainValid,
  NewRs16Valid,
  PlainCounts,
  Rs16Counts,
  PlainIndexInRange,
  PlainIndexOutOfRange,
  Rs16DataIndexFirst,
  Rs16ParityIndex,
  Rs16TailDataIndex,
  Rs16DataBeyondPayloadIndex,
  Rs16ParityAfterPartialStripe,
  EncodedPlainIndex,
  EncodedRs16PayloadIndex,
  EncodedPayloadOutOfRange
}

InvalidConstructionCases == {
  NewZeroChunkPlain,
  NewPlainWithErasure,
  NewRs16OddChunk,
  NewRs16ZeroData,
  NewRs16ZeroParity
}

ConstructOk == 1
ConstructErr == 2
PayloadKnown == 3
PayloadUnknown == 4
PayloadSizeSome == 5
PayloadSizeNone == 6
PlainProfileRequired == 7
Rs16EvenChunkRequired == 8
Rs16ShardProfileRequired == 9
StripeWidthOne == 10
StripeWidthDataParity == 11
PayloadChunksThree == 12
PayloadChunksNone == 13
StripeCountOne == 14
StripeCountTwo == 15
StripeCountThree == 16
StripeCountNone == 17
TotalChunksThree == 18
TotalChunksSix == 19
TotalChunksNone == 20
PayloadIndexSame == 21
PayloadIndexNone == 22
PayloadIndexZero == 23
PayloadIndexTwo == 24
EncodedIndexSame == 25
EncodedIndexThree == 26
EncodedIndexNone == 27
ExpectedLenFour == 28
ExpectedLenTwo == 29
ExpectedLenZero == 30
ExpectedLenNone == 31
ParityLenChunk == 32
TailLenShort == 33
DataBeyondPayloadLenZero == 34
PlainOutOfRangeNone == 35

ActionUniverse == 1..35

SpecActions(c) ==
  CASE c = LegacyPlain ->
      {ConstructOk, PayloadUnknown, PayloadSizeNone, PayloadChunksNone,
       StripeCountNone, TotalChunksNone, PayloadIndexNone, EncodedIndexNone,
       ExpectedLenNone}
    [] c = NewZeroChunkPlain ->
      {ConstructErr}
    [] c = NewPlainWithErasure ->
      {ConstructErr, PlainProfileRequired}
    [] c = NewRs16OddChunk ->
      {ConstructErr, Rs16EvenChunkRequired}
    [] c \in {NewRs16ZeroData, NewRs16ZeroParity} ->
      {ConstructErr, Rs16ShardProfileRequired}
    [] c = NewPlainValid ->
      {ConstructOk, PayloadKnown, PayloadSizeSome, StripeWidthOne}
    [] c = NewRs16Valid ->
      {ConstructOk, PayloadKnown, PayloadSizeSome, StripeWidthDataParity}
    [] c = PlainCounts ->
      {PayloadChunksThree, StripeCountThree, TotalChunksThree}
    [] c = Rs16Counts ->
      {PayloadChunksThree, StripeCountTwo, TotalChunksSix,
       StripeWidthDataParity}
    [] c = PlainIndexInRange ->
      {PayloadIndexSame, EncodedIndexSame, ExpectedLenFour}
    [] c = PlainIndexOutOfRange ->
      {PayloadIndexNone, EncodedIndexNone, ExpectedLenNone,
       PlainOutOfRangeNone}
    [] c = Rs16DataIndexFirst ->
      {PayloadIndexZero, EncodedIndexSame, ExpectedLenFour}
    [] c = Rs16ParityIndex ->
      {PayloadIndexNone, EncodedIndexNone, ExpectedLenFour, ParityLenChunk}
    [] c = Rs16TailDataIndex ->
      {PayloadIndexTwo, EncodedIndexThree, ExpectedLenTwo, TailLenShort}
    [] c = Rs16DataBeyondPayloadIndex ->
      {PayloadIndexNone, EncodedIndexNone, ExpectedLenZero,
       DataBeyondPayloadLenZero}
    [] c = Rs16ParityAfterPartialStripe ->
      {PayloadIndexNone, EncodedIndexNone, ExpectedLenFour, ParityLenChunk}
    [] c = EncodedPlainIndex ->
      {PayloadIndexSame, EncodedIndexSame}
    [] c = EncodedRs16PayloadIndex ->
      {PayloadIndexTwo, EncodedIndexThree}
    [] c = EncodedPayloadOutOfRange ->
      {EncodedIndexNone}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "legacy_claims_known" /\ c = LegacyPlain ->
      (spec \ {PayloadUnknown, PayloadSizeNone}) \cup
        {PayloadKnown, PayloadSizeSome}
    [] Bug = "zero_chunk_accepted" /\ c = NewZeroChunkPlain ->
      {ConstructOk, PayloadKnown, PayloadSizeSome}
    [] Bug = "plain_accepts_erasure" /\ c = NewPlainWithErasure ->
      (spec \ {ConstructErr, PlainProfileRequired}) \cup {ConstructOk}
    [] Bug = "rs16_accepts_odd_chunk" /\ c = NewRs16OddChunk ->
      (spec \ {ConstructErr, Rs16EvenChunkRequired}) \cup {ConstructOk}
    [] Bug = "rs16_accepts_zero_data" /\ c = NewRs16ZeroData ->
      (spec \ {ConstructErr, Rs16ShardProfileRequired}) \cup {ConstructOk}
    [] Bug = "rs16_accepts_zero_parity" /\ c = NewRs16ZeroParity ->
      (spec \ {ConstructErr, Rs16ShardProfileRequired}) \cup {ConstructOk}
    [] Bug = "plain_total_uses_rs_width" /\ c = PlainCounts ->
      (spec \ {StripeCountThree, TotalChunksThree}) \cup
        {StripeCountOne, TotalChunksSix}
    [] Bug = "rs16_stripes_floor" /\ c = Rs16Counts ->
      (spec \ {StripeCountTwo, TotalChunksSix}) \cup
        {StripeCountOne, TotalChunksThree}
    [] Bug = "rs16_total_omits_parity" /\ c = Rs16Counts ->
      (spec \ {TotalChunksSix}) \cup {TotalChunksThree}
    [] Bug = "rs16_parity_maps_payload" /\ c = Rs16ParityIndex ->
      (spec \ {PayloadIndexNone, EncodedIndexNone}) \cup
        {PayloadIndexTwo, EncodedIndexThree}
    [] Bug = "rs16_tail_len_full" /\ c = Rs16TailDataIndex ->
      (spec \ {ExpectedLenTwo, TailLenShort}) \cup {ExpectedLenFour}
    [] Bug = "rs16_data_beyond_returns_chunk" /\
       c = Rs16DataBeyondPayloadIndex ->
      (spec \ {ExpectedLenZero, DataBeyondPayloadLenZero}) \cup
        {ExpectedLenFour}
    [] Bug = "rs16_parity_len_zero" /\
       c \in {Rs16ParityIndex, Rs16ParityAfterPartialStripe} ->
      (spec \ {ExpectedLenFour, ParityLenChunk}) \cup {ExpectedLenZero}
    [] Bug = "encoded_idx_identity_for_rs16" /\
       c = EncodedRs16PayloadIndex ->
      (spec \ {EncodedIndexThree}) \cup {EncodedIndexSame}
    [] Bug = "encoded_idx_allows_oob" /\ c = EncodedPayloadOutOfRange ->
      (spec \ {EncodedIndexNone}) \cup {EncodedIndexThree}
    [] Bug = "plain_oob_maps" /\ c = PlainIndexOutOfRange ->
      (spec \ {PayloadIndexNone, EncodedIndexNone, ExpectedLenNone,
               PlainOutOfRangeNone}) \cup
        {PayloadIndexSame, EncodedIndexSame, ExpectedLenFour}
    [] OTHER -> spec

Bugs == {
  "none",
  "legacy_claims_known",
  "zero_chunk_accepted",
  "plain_accepts_erasure",
  "rs16_accepts_odd_chunk",
  "rs16_accepts_zero_data",
  "rs16_accepts_zero_parity",
  "plain_total_uses_rs_width",
  "rs16_stripes_floor",
  "rs16_total_omits_parity",
  "rs16_parity_maps_payload",
  "rs16_tail_len_full",
  "rs16_data_beyond_returns_chunk",
  "rs16_parity_len_zero",
  "encoded_idx_identity_for_rs16",
  "encoded_idx_allows_oob",
  "plain_oob_maps"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

InvalidLayoutsAreRejected ==
  /\ \A c \in InvalidConstructionCases:
       ConstructErr \in ImplementationActions(c)
  /\ PlainProfileRequired \in ImplementationActions(NewPlainWithErasure)
  /\ Rs16EvenChunkRequired \in ImplementationActions(NewRs16OddChunk)
  /\ Rs16ShardProfileRequired \in ImplementationActions(NewRs16ZeroData)
  /\ Rs16ShardProfileRequired \in ImplementationActions(NewRs16ZeroParity)

LegacyPlainHasUnknownPayloadSize ==
  /\ ConstructOk \in ImplementationActions(LegacyPlain)
  /\ PayloadUnknown \in ImplementationActions(LegacyPlain)
  /\ PayloadSizeNone \in ImplementationActions(LegacyPlain)
  /\ PayloadChunksNone \in ImplementationActions(LegacyPlain)
  /\ StripeCountNone \in ImplementationActions(LegacyPlain)
  /\ TotalChunksNone \in ImplementationActions(LegacyPlain)

ChunkCountsMatchEncoding ==
  /\ StripeWidthOne \in ImplementationActions(NewPlainValid)
  /\ StripeWidthDataParity \in ImplementationActions(NewRs16Valid)
  /\ PayloadChunksThree \in ImplementationActions(PlainCounts)
  /\ StripeCountThree \in ImplementationActions(PlainCounts)
  /\ TotalChunksThree \in ImplementationActions(PlainCounts)
  /\ PayloadChunksThree \in ImplementationActions(Rs16Counts)
  /\ StripeCountTwo \in ImplementationActions(Rs16Counts)
  /\ TotalChunksSix \in ImplementationActions(Rs16Counts)

EncodedPayloadIndicesRoundTrip ==
  /\ PayloadIndexSame \in ImplementationActions(PlainIndexInRange)
  /\ EncodedIndexSame \in ImplementationActions(PlainIndexInRange)
  /\ PlainOutOfRangeNone \in ImplementationActions(PlainIndexOutOfRange)
  /\ PayloadIndexNone \in ImplementationActions(PlainIndexOutOfRange)
  /\ EncodedIndexNone \in ImplementationActions(PlainIndexOutOfRange)
  /\ PayloadIndexZero \in ImplementationActions(Rs16DataIndexFirst)
  /\ EncodedIndexSame \in ImplementationActions(Rs16DataIndexFirst)
  /\ PayloadIndexNone \in ImplementationActions(Rs16ParityIndex)
  /\ EncodedIndexNone \in ImplementationActions(Rs16ParityIndex)
  /\ PayloadIndexTwo \in ImplementationActions(Rs16TailDataIndex)
  /\ EncodedIndexThree \in ImplementationActions(Rs16TailDataIndex)
  /\ PayloadIndexNone \in ImplementationActions(Rs16DataBeyondPayloadIndex)
  /\ PayloadIndexTwo \in ImplementationActions(EncodedRs16PayloadIndex)
  /\ EncodedIndexThree \in ImplementationActions(EncodedRs16PayloadIndex)
  /\ EncodedIndexNone \in ImplementationActions(EncodedPayloadOutOfRange)

ExpectedLengthsMatchLayout ==
  /\ ExpectedLenFour \in ImplementationActions(PlainIndexInRange)
  /\ ExpectedLenNone \in ImplementationActions(PlainIndexOutOfRange)
  /\ ExpectedLenFour \in ImplementationActions(Rs16DataIndexFirst)
  /\ ExpectedLenFour \in ImplementationActions(Rs16ParityIndex)
  /\ ParityLenChunk \in ImplementationActions(Rs16ParityIndex)
  /\ ExpectedLenTwo \in ImplementationActions(Rs16TailDataIndex)
  /\ TailLenShort \in ImplementationActions(Rs16TailDataIndex)
  /\ ExpectedLenZero \in ImplementationActions(Rs16DataBeyondPayloadIndex)
  /\ DataBeyondPayloadLenZero \in
       ImplementationActions(Rs16DataBeyondPayloadIndex)
  /\ ExpectedLenFour \in ImplementationActions(Rs16ParityAfterPartialStripe)
  /\ ParityLenChunk \in
       ImplementationActions(Rs16ParityAfterPartialStripe)

RbcPayloadLayoutCoreSafety ==
  /\ ActionsMatchSpec
  /\ InvalidLayoutsAreRejected
  /\ LegacyPlainHasUnknownPayloadSize
  /\ ChunkCountsMatchEncoding
  /\ EncodedPayloadIndicesRoundTrip
  /\ ExpectedLengthsMatchLayout

SafetyFast ==
  RbcPayloadLayoutCoreSafety

====
