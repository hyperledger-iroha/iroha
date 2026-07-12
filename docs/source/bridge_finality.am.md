---
lang: am
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1cbd248fe14e63d00f002f09e1663181f3ab9bd99124ffeb89c56763b784046b
source_last_modified: "2026-07-12"
translation_last_reviewed: 2026-07-12
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# የድልድይ መጨረሻነት ማረጋገጫዎች

ይህ ሰነድ ለመጀመሪያው ልቀት የድልድይ መጨረሻነት ቅርጸትን ይገልጻል። ማረጋገጫው Sumeragi v2
የፈጠረውንና በቋሚነት ያከማቸውን ትክክለኛ finality evidence ይይዛል። የማረጋገጫ
envelope schema version `1` ሲሆን በውስጡ ያለው consensus protocol version `2` ነው።
የSumeragi v1 certificate projection፣ decoder ወይም fallback መንገድ የለም።

## ትክክለኛው የማረጋገጫ ቅርጸት

በNorito ወይም Norito JSON የተመሰጠረው `BridgeFinalityProof` ሦስት መስኮች ብቻ አሉት፦

```text
{ version, block_header, finality_artifact }
```

- `version` የግድ `1` መሆን አለበት፤
- `block_header` በተጠየቀው ከፍታ ያለው canonical `BlockHeader` ነው፤
- `finality_artifact` ለዚያ block የተቀመጠው ትክክለኛ `V2FinalityArtifact` ነው። በheight-context
  roster ቅደም ተከተል የእያንዳንዱን validator BLS-normal PoP (`validator_set_pops`)
  በቋሚነት በውስጡ ይይዛል።

Artifact-ው ሙሉና የማይለወጥ `HeightContext`፣ ትክክለኛ `BlockSubject`፣ block hash፣
CommitQC እና roster-aligned PoP-ዎችን ይይዛል። Height context chain፣ epoch፣ roster፣
`DualQuorum`፣ DA layout፣ leader seed እና ሌሎች consensus data ያስርቃል። Epoch-ን የሚያበቃው
parent block context optional `next_epoch_snapshot` ደግሞ ይይዛል፤ መስኩ የcontext id ክፍል
ስለሆነ parent CommitQC ለchild roster ሥልጣን ከመስጠቱ በፊት ያረጋግጠዋል።
Finalized snapshot-ው የቀጣዩ epoch parameters ብቻ ሳይሆን `epoch_end_height` እና ለቀጣዩ roster
የተስተካከሉ `validator_set_pops`-ዎችንም ያረጋግጣል።

## ቋሚ ማከማቻ እና ማረጋገጥ

Kura finality ከማተሙ ወይም block body ከማስወገዱ በፊት ትክክለኛውን canonical header እና
በ`commitment_index` ቅደም ተከተል ያለውን SCCP archive የያዘ immutable retained record
ያስቀምጣል። Finality artifact ከዚያ ከዚያው header ጋር በተለየ immutable record ይቀመጣል።
Proof builder retained header እና finality record ብቻ ያነባል፤ historical block body ወይም
mutable WSV payload አይፈልግም። የጎደለ፣ የተበላሸ፣ የሚጋጭ ወይም የማይረጋገጥ record
fail closed ይሆናል።

Stateless verifier version፣ chain፣ height፣ header hash፣ canonical predecessor፣ view፣ context፣
subject እና CommitQC-ን
በትክክል ያዛምዳል እና በartifact ውስጥ ያሉትን PoP-ዎች ሁሉ ያረጋግጣል። Signer index-ዎች
በጥብቅ እየጨመሩና በወሰን ውስጥ መሆን አለባቸው። CommitQC የvalidator count እና voting
power quorum ሁለቱንም ማሟላት፣ በትክክለኛው Sumeragi v2 vote preimage ላይ BLS aggregate
signature-ው ትክክለኛ መሆን አለበት።

## የእምነት anchor እና successor ማረጋገጥ

አንድ ማረጋገጫ በራሱ roster ሥር ያለውን ውስጣዊ ወጥነት ብቻ ያሳያል።
`BridgeFinalityVerifier` የመጀመሪያውን ማረጋገጫ ከመቀበሉ በፊት በግልጽ የታመነ
`HeightContextId` ይፈልጋል። ከዚያ በኋላ ወዲያውኑ የሚቀጥለውን height ብቻ ተቀብሎ በchild
context ያለውን parent CommitQC በቀድሞው frozen roster እና PoP ያረጋግጣል። በepoch ውስጥ child
artifact የቀድሞውን artifact PoP ይቀዳል፤ በboundary ላይ epoch፣ roster፣ quorum፣ seed እና PoP
በparent CommitQC ከተረጋገጠው `next_epoch_snapshot` ጋር፣ የተረጋገጠውን
`epoch_end_height` ጨምሮ፣ መዛመድ አለባቸው። የቆዩ፣ የተዘለሉ እና ያልተገናኙ heights ይከለከላሉ።

SCCP ይህንኑ `BridgeFinalityProof` ይጠቀማል። በmessage የቀረበ roster ሥር signature-ን ብቻ
ማመን በቂ አይደለም፤ በgovernance ከተቆለፈ checkpoint context/artifact እስከ message artifact
ያለው እያንዳንዱ immediate successor መረጋገጥ አለበት።

## Bundle እና API

`BridgeFinalityBundle` በትክክል `{ commitment, finality_proof }` ነው። Commitment-ው
`{ chain_id, height_context_id, block_height, block_hash }` ነው።

- `GET /v1/bridge/finality/{height}` `BridgeFinalityProof` ይመልሳል፤
- `GET /v1/bridge/finality/bundle/{height}` `BridgeFinalityBundle` ይመልሳል።

Retained canonical header ወይም ትክክለኛው ቋሚ v2 artifact ከጎደለ ወይም invalid ከሆነ
ሁለቱም endpoint-ዎች fail closed ይሆናሉ። Block-body eviction ትክክለኛ proof-ን አያጠፋም።
ያልታወቁ መስኮች፣ ያልተደገፉ versions እና retired proof shapes መከልከል አለባቸው።
