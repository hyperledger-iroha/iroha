---
lang: hy
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e28e5c38283ad6be40a0fc48e0312797f490542a143f4cefdd209aaf8099ac5
source_last_modified: "2026-07-11T20:38:35.470900+00:00"
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

Sumeragi v2 apply path-ը ստուգում է artifact-ը և պահում որպես անփոփոխ Kura sidecar։
Proof builder-ը կարդում է կանոնական block-ը և նրա sidecar-ը, ու պատմական PoP կամ
certificate չի վերականգնում փոփոխական ընթացիկ world state-ից։ Բացակայող, վնասված,
հակասական կամ չստուգվող sidecar-ը մերժվում է fail-closed կերպով, իսկ հասանելիությունը
չի սահմանափակվում վերջին in-memory history window-ով։

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

Եթե block-ը կամ ճշգրիտ տևական v2 artifact-ը բացակայում է կամ անվավեր է, երկու endpoint-ն
էլ fail closed են։ Անհայտ դաշտերը, չաջակցվող version-ները և հնացած proof shape-երը
պետք է մերժվեն։
