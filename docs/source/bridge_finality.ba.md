---
lang: ba
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

# Күпер финаллылығы дәлилдәре

Был документ тәүге сығарылыш өсөн күпер финаллылығы форматын билдәләй. Дәлил Sumeragi
v2 булдырған һәм даими һаҡлаған теүәл финаллылыҡ мәғлүмәтен ташый. Дәлил тышлығының
schema version-ы `1`, ә эсендәге consensus protocol version-ы `2`. Sumeragi v1
certificate-ына projection, decoder йәки fallback юл юҡ.

## Дәлилдең теүәл форматы

Norito йәки Norito JSON менән кодланған `BridgeFinalityProof` өс кенә яландан тора:

```text
{ version, block_header, finality_artifact }
```

- `version` мотлаҡ `1` булырға тейеш;
- `block_header` — һоралған бейеклектең каноник `BlockHeader`-ы;
- `finality_artifact` — ошо блок өсөн һаҡланған теүәл `V2FinalityArtifact`. Ул
  height-context roster тәртибендә һәр validator-ҙың BLS-normal PoP-ын
  (`validator_set_pops`) даими эсендә һаҡлай.

Artifact тулы һәм үҙгәрмәҫ `HeightContext`, теүәл `BlockSubject`, block hash, CommitQC һәм
roster-ға тура килгән PoP-тарҙы үҙ эсенә ала. Height context chain, epoch, roster,
`DualQuorum`, DA layout, leader seed һәм башҡа consensus мәғлүмәтен нығыта. Epoch-ты
тамамлаған parent block context-е optional `next_epoch_snapshot`-ты ла үҙ эсенә ала;
был яландар context id өлөшө булғанға, child roster-ға рөхсәт бирер алдынан parent
CommitQC уны аутентификациялай. Finalized snapshot киләһе epoch parameters менән бергә
`epoch_end_height` һәм киләһе roster-ға тура килгән `validator_set_pops`-ты ла нығыта.

## Даими һаҡлау һәм тикшереү

Kura finality баҫтырылғанға йәки block body сығарылғанға тиклем теүәл canonical header һәм
`commitment_index` тәртибендәге SCCP archive-ты immutable retained record-та һаҡлай.
Finality artifact шунан һуң шул уҡ header менән айырым immutable record-та һаҡлана. Proof
builder retained header һәм finality record-ты ғына уҡый; тарихи block body йәки үҙгәреүсән
WSV payload кәрәкмәй. Юғалған, боҙолған, ҡапма-ҡаршы йәки тикшерелмәгән record fail-closed
рәүештә кире ҡағыла.

Stateless verifier version, chain, height, header hash, header-ҙың canonical predecessor-ы
һәм view-ы, context, subject һәм CommitQC-ны
теүәл сағыштыра һәм artifact эсендәге бөтә PoP-ты тикшерә. Signer index-тар ҡәтғи үҫеүсе
һәм сик эсендә булырға тейеш. CommitQC validator count һәм voting power буйынса ике
quorum-ды ла үтәп, теүәл Sumeragi v2 vote preimage өсөн BLS aggregate signature дөрөҫ
булырға тейеш.

## Ышаныс anchor-ы һәм successor тикшереүе

Айырым дәлил үҙе килтергән roster аҫтындағы эске ярашлылыҡты ғына күрһәтә.
`BridgeFinalityVerifier` тәүге дәлилгә тиклем асыҡтан-асыҡ ышаныслы `HeightContextId`
талап итә. Артабан ул тик шунда уҡ киләһе бейеклекте ҡабул итә һәм child context-тың
parent CommitQC-ын алдағы нығытылған roster һәм PoP менән тикшерә. Epoch эсендә child
artifact алдағы artifact PoP-тарын күсерә; сиктә epoch, roster, quorum, seed һәм PoP
parent CommitQC аутентификациялаған `next_epoch_snapshot`-ҡа, шул иҫәптән
`epoch_end_height`-ҡа, тура килергә тейеш. Иҫке, үткәреп ебәрелгән һәм бәйләнмәгән
бейеклектәр кире ҡағыла.

SCCP шул уҡ `BridgeFinalityProof`-ты ҡуллана. Message биргән roster аҫтындағы ҡултамғаға
ғына ышаныу етмәй; governance нығытҡан checkpoint context/artifact-ынан message
artifact-ына тиклем һәр тура successor тикшерелергә тейеш.

## Bundle һәм API

`BridgeFinalityBundle` теүәл `{ commitment, finality_proof }` формаһында. Commitment:
`{ chain_id, height_context_id, block_height, block_hash }`.

- `GET /v1/bridge/finality/{height}` `BridgeFinalityProof` ҡайтара;
- `GET /v1/bridge/finality/bundle/{height}` `BridgeFinalityBundle` ҡайтара.

Retained canonical header йәки теүәл даими v2 artifact юҡ йә яраҡһыҙ булһа, ике endpoint
та fail closed була. Block-body eviction дөрөҫ proof-ты юғалтмай. Билдәһеҙ яландар,
хупланмаған version-дар һәм иҫкергән proof shape-тар кире ҡағылырға тейеш.
