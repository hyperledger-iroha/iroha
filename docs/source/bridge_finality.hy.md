---
lang: hy
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

# Կամրջի վերջնականության ապացույցներ

Այս փաստաթուղթը սահմանում է առաջին թողարկման կամրջի վերջնականության ձևաչափը։
Ապացույցը փոխանցում է Sumeragi v2-ի ստեղծած և տևականորեն պահպանած ճշգրիտ
վերջնականության տվյալները։ Ապացույցի պատյանի schema version-ը `1` է, իսկ ներսում
գտնվող consensus protocol version-ը՝ `2`։ Sumeragi v1 certificate-ի projection,
decoder կամ fallback ուղի չկա։

## Ապացույցի ճշգրիտ ձևաչափը

Norito կամ Norito JSON կոդավորմամբ `BridgeFinalityProof`-ն ունի միայն երեք դաշտ․

```text
{ version, block_header, finality_artifact }
```

- `version`-ը պետք է լինի `1`;
- `block_header`-ը պահանջվող բարձրության կանոնական `BlockHeader`-ն է;
- `finality_artifact`-ը տվյալ բլոկի համար պահված ճշգրիտ `V2FinalityArtifact`-ն է։ Այն
  height-context roster-ի հերթականությամբ տևականորեն ներառում է յուրաքանչյուր
  validator-ի BLS-normal PoP-ը (`validator_set_pops`)։

Artifact-ը պարունակում է ամբողջական ու անփոփոխ `HeightContext`, ճշգրիտ
`BlockSubject`, block hash, CommitQC և roster-ին համապատասխան PoP-երը։ Height context-ը
սառեցնում է chain-ը, epoch-ը, roster-ը, `DualQuorum`-ը, DA layout-ը, leader seed-ը և
մյուս consensus տվյալները։ Epoch-ն ավարտող parent block-ի context-ը ներառում է նաև
optional `next_epoch_snapshot`; քանի որ այս դաշտը context id-ի մաս է, parent CommitQC-ն
այն վավերացնում է՝ նախքան այն child roster-ը թույլատրելու իրավասություն կստանա։
Finalized snapshot-ը հաջորդ epoch-ի պարամետրերի հետ վավերացնում է նաև
`epoch_end_height`-ը և հաջորդ roster-ին համահունչ `validator_set_pops`-ը։

## Տևական պահպանում և ստուգում

Մինչ finality-ի հրապարակումը կամ block body-ի հեռացումը Kura-ն ճշգրիտ canonical
header-ը և root-authenticated SCCP archive-ը գրում է immutable retained-block
record-ում, ապա exact V2 artifact-ը պահում է առանձին immutable finality record-ում։
Երկու գրառումներն էլ idempotent են և նույն height-ի հակասությունը մերժում են։
`build_finality_proof`-ը կարդում է միայն retained header-ը և verified finality record-ը՝
երբեք չկարդալով historical block body կամ PoP-ը mutable world state-ով չփոխարինելով։
Restart-ի ժամանակ header/archive/artifact/hash կապը կրկին ստուգվում է։ Body eviction-ը
ճիշտ proof-ը անհասանելի չի դարձնում, իսկ բացակայող, վնասված, հակասական կամ չստուգվող
record-ը fail closed է։

Stateless verifier-ը ճշգրտորեն համադրում է version, chain, height, header hash, header-ի
canonical predecessor և view, context, subject և CommitQC դաշտերը և ստուգում artifact-ի բոլոր PoP-երը։
Signer index-ները պետք
է լինեն խիստ աճող ու սահմաններում։ CommitQC-ն պետք է բավարարի և՛ validator count, և՛
voting power quorum-ը, իսկ ճշգրիտ Sumeragi v2 vote preimage-ի BLS aggregate signature-ը
պետք է վավեր լինի։

## Վստահության հենակետ և հաջորդների ստուգում

Առանձին ապացույցը ցույց է տալիս միայն իր բերած roster-ի ներքին համահունչությունը։
`BridgeFinalityVerifier`-ը առաջին ապացույցից առաջ պահանջում է հստակ վստահված
`HeightContextId`։ Այնուհետև ընդունում է միայն անմիջական հաջորդ բարձրությունը և child
context-ի parent CommitQC-ն ստուգում նախորդ սառեցված roster-ով ու PoP-ով։ Epoch-ի ներսում
child artifact-ը պատճենում է նախորդ artifact-ի PoP-երը, իսկ սահմանին epoch-ը, roster-ը,
quorum-ը, seed-ը և PoP-երը պետք է համապատասխանեն parent CommitQC-ով վավերացված
`next_epoch_snapshot`-ին՝ ներառյալ `epoch_end_height`-ը։ Հին, բաց թողնված և չկապված
բարձրությունները մերժվում են։

SCCP-ն օգտագործում է նույն `BridgeFinalityProof`-ը։ Միայն message-ի տրամադրած roster-ի
տակ ստորագրությանը վստահելը բավարար չէ. governance-ով ամրացված checkpoint
context/artifact-ից մինչև message artifact պետք է ստուգվի յուրաքանչյուր անմիջական
successor-ը։

## Bundle և API

`BridgeFinalityBundle`-ը ճշգրտորեն `{ commitment, finality_proof }` է։ Commitment-ը
`{ chain_id, height_context_id, block_height, block_hash }` է։

- `GET /v1/bridge/finality/{height}`-ը վերադարձնում է `BridgeFinalityProof`;
- `GET /v1/bridge/finality/bundle/{height}`-ը վերադարձնում է `BridgeFinalityBundle`։

Եթե retained canonical header-ը կամ ճշգրիտ տևական v2 artifact-ը բացակայում է կամ
անվավեր է, երկու endpoint-ն էլ fail closed են։ Historical block body eviction-ը ճիշտ
proof-ը անհասանելի չի դարձնում։ Անհայտ դաշտերը, չաջակցվող version-ները և հնացած proof
shape-երը պետք է մերժվեն։
