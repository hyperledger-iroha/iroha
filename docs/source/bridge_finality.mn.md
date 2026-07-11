---
lang: mn
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

# Bridge finality нотолгоо

Энэ баримт бичиг анхны хувилбарын bridge finality форматыг тодорхойлно. Нотолгоо нь
Sumeragi v2-ийн үүсгэж, тогтвортой хадгалсан яг таг finality evidence-ийг дамжуулна.
Proof envelope-ийн schema version нь `1`, харин доторх consensus protocol version нь
`2`. Sumeragi v1 certificate projection, decoder эсвэл fallback зам байхгүй.

## Нотолгооны яг таг формат

Norito эсвэл Norito JSON-оор кодлосон `BridgeFinalityProof` зөвхөн гурван талбартай:

```text
{ version, block_header, finality_artifact }
```

- `version` заавал `1` байна;
- `block_header` нь хүссэн өндөр дэх canonical `BlockHeader`;
- `finality_artifact` нь тухайн блокт хадгалсан яг таг `V2FinalityArtifact`. Энэ нь
  height-context roster-ийн дарааллаар validator бүрийн BLS-normal PoP-ийг
  (`validator_set_pops`) тогтвортой дотроо агуулна.

Artifact нь бүрэн, өөрчлөгдөшгүй `HeightContext`, яг таг `BlockSubject`, block hash,
CommitQC болон roster-т таарсан PoP-уудыг агуулна. Height context нь chain, epoch,
roster, `DualQuorum`, DA layout, leader seed болон бусад consensus өгөгдлийг царцаана.
Epoch-ийг дуусгаж буй parent block-ийн context нь optional `next_epoch_snapshot`-ийг мөн
агуулна; уг талбар context id-ийн хэсэг тул child roster-ийг зөвшөөрөхөөс өмнө parent
CommitQC түүнийг баталгаажуулна. Finalized snapshot нь дараагийн epoch parameters-аас
гадна `epoch_end_height` болон дараагийн roster-т таарсан `validator_set_pops`-ийг баталгаажуулна.

## Тогтвортой хадгалалт ба шалгалт

Sumeragi v2 apply path artifact-ийг шалгаад өөрчлөгдөшгүй Kura sidecar болгон хадгална.
Proof builder canonical block болон sidecar-ийг уншиж, түүхэн PoP эсвэл certificate-ийг
өөрчлөгдөж болох одоогийн world state-ээс дахин бүтээдэггүй. Байхгүй, эвдэрсэн,
зөрчилтэй эсвэл баталгаажихгүй sidecar-ийг fail closed байдлаар татгалзана; хүртээмж нь
сүүлийн үеийн in-memory history window-оор хязгаарлагдахгүй.

Stateless verifier нь version, chain, height, header hash, header-ийн canonical predecessor
ба view, context, subject, CommitQC-г яг тааруулж, artifact доторх бүх PoP-ийг шалгана.
Signer index-үүд эрс өсөх дараалалтай,
хязгаарт байх ёстой. CommitQC нь validator count болон voting power хоёр quorum-ыг
зэрэг хангаж, яг таг Sumeragi v2 vote preimage дээрх BLS aggregate signature хүчинтэй
байх ёстой.

## Итгэлийн anchor ба successor шалгалт

Ганц нотолгоо зөвхөн өөрийн авч явсан roster дорх дотоод нийцлийг харуулна.
`BridgeFinalityVerifier` эхний нотолгооноос өмнө илэрхий итгэмжлэгдсэн
`HeightContextId` шаарддаг. Дараа нь зөвхөн шууд дараагийн өндрийг хүлээн авч, child
context-ийн parent CommitQC-г өмнөх царцаасан roster ба PoP-оор шалгана. Epoch дотор child
artifact нь өмнөх artifact-ийн PoP-ийг хуулна; boundary дээр epoch, roster, quorum, seed
ба PoP нь parent CommitQC-ээр баталгаажсан `next_epoch_snapshot`-тай, түүний
`epoch_end_height`-ийг оролцуулан, таарах ёстой. Хуучин, алгассан, холбоогүй өндрийг татгалзана.

SCCP ижил `BridgeFinalityProof`-ийг ашиглана. Message-ийн өгсөн roster дорх гарын үсэгт
дангаар нь итгэж болохгүй; governance-ээр тогтоосон checkpoint context/artifact-аас
message artifact хүртэлх шууд successor бүрийг шалгана.

## Bundle ба API

`BridgeFinalityBundle` яг `{ commitment, finality_proof }` байна. Commitment нь
`{ chain_id, height_context_id, block_height, block_hash }`.

- `GET /v1/bridge/finality/{height}` нь `BridgeFinalityProof` буцаана;
- `GET /v1/bridge/finality/bundle/{height}` нь `BridgeFinalityBundle` буцаана.

Block эсвэл яг таг тогтвортой v2 artifact байхгүй буюу хүчингүй бол хоёр endpoint хоёулаа
fail closed болно. Үл мэдэгдэх талбар, дэмжигдээгүй version, хуучирсан proof shape-ийг
татгалзах ёстой.
