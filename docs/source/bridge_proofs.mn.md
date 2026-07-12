---
lang: mn
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 74e29801129deccb6d5640d414289c47cf13fa9e0229fb55212b6c7710d7c5f7
source_last_modified: "2026-07-12T07:38:49.568351+00:00"
translation_last_reviewed: 2026-07-12
translator: machine-assisted
---

> Энэ нь 2026-07-11-ний товчилсон нутагшуулсан тойм бөгөөд бүрэн норматив
> орчуулга биш. Яг төрөл, API гэрээ, хувилбарын шаардлагыг
> [англи канон хуудаснаас](bridge_proofs.md) үзнэ үү.

# SCCP V1 гүүрийн нотолгоо — товч тойм

## Анхны хувилбарын хил

- SCCP V1 нь хаалттай гадаргуу: зөвхөн Ethereum mainnet, BSC mainnet, TRON
  mainnet дэмжигдэнэ; SORA талын цорын ганц төгсгөлийн цэг нь `sora-taira`.
  Бусад сүлжээний profile эсвэл SORA identity-г татгалзана.
- `SubmitBridgeProof` зөвхөн route-д холбогдсон төрөлжсөн `NativeProtocol` ба
  `SccpDestination` нотолгоог хүлээн авна. Ерөнхий `Ics` болон `TransparentZk`
  payload илгээх боломжгүй бөгөөд fail-closed журмаар татгалзана.

## Төрөлжсөн бүртгэл ба түүх

- `SccpRegistryV1` нь төрөлжсөн, append-only. Lane бүр хамгийн ихдээ 64 route
  revision, 4,096 native trust anchor хадгална. Бичлэгийг далд байдлаар
  устгахгүй; хязгаараас давсан дараагийн нэмэлтийг атомоор татгалзана.
- Anchor interval нь баталгаажсан consensus coordinate ашиглана: Ethereum-д
  finalized beacon slot, BSC/TRON-д finalized native block height. Хуучин
  anchor нь залгамж checkpoint-ийг оролцуулан хүчинтэй, түүнээс цааш хүчингүй.
- Durable inbound record нь event/finality height болон
  `anchor_interval_height`-ийг тусад нь хадгална. lane+anchor high-water зөвхөн
  өснө; дараагийн checkpoint түүнээс бага байж болохгүй. Snapshot hydration
  индексийг бүрэн дахин тооцож, дутуу, хуучирсан эсвэл илүү утгыг татгалзана.
  Message id давтах болон replay-г мөн татгалзана.

TRON эх үүсвэрийн route нь яг
`transferToTaira(bytes,uint256,uint64 expectedNonce)` ABI-г хэрэглэнэ. Амжилттай
гүйцэтгэхийн тулд `expectedNonce == transferNonce` байх ёстой бөгөөд дараа нь
storage-ийг нэмэхээс өмнө уг утгыг canonical payload-д бичнэ. Native admission
нь payload recipient, хэмжээсжүүлсэн дүн болон nonce-оос бүтэн ABI call-ыг сэргээн
байгуулна. Иймээс хуучин хоёр-argument selector, stale эсвэл future nonce, мөн
хязгаар нь дууссан `uint64` nonce бүгд аюулгүйгээр татгалзагдана.

## Нэг удаагийн шалгалт ба детерминист хязгаар

- Native болон destination нотолгоог каноноор нэг удаа decode хийж, үнэтэй
  криптограф шалгалтыг нэг удаа гүйцэтгэнэ. Үүнээс өмнө consensus нь
  консерватив, hardware-independent ажлын тооцоог нөөцөлнө.
- `[zk.sccp]` нь proof count/bytes, native headers, Ethereum light-client
  updates, header bytes, secp256k1 recoveries, BLS aggregate checks/signing
  contributions, BN254 pairing-product checks-д заавал тэгээс их per-proof,
  per-transaction, per-block хязгаар тогтооно. Эдгээр admission limit нь
  consensus-bound тул бүх validator ижил утгатай байна.

## Outbound commitment, хадгалалт ба илрүүлэлт

Амжилттай outbound message бүр block execution order-ийн дагуу нягт
`commitment_index` (`0..=511`) авна. V1-ийн тогтмол хязгаар нь нэг block-д 512
message, нэг message-д 4,096 canonical payload byte байна. `[zk.sccp]` pending
payload state-ийг `max_pending_outbound_messages` (default `65536`) болон
`max_pending_outbound_payload_bytes` (default `268435456`) хоёроор хамтад нь хязгаарлана.

Kura finality нийтлэх эсвэл block body-г eviction хийхээс өмнө яг canonical header
болон root-authenticated SCCP archive-ийг immutable хадгална. Proof, bundle, proof
request, recent history сэргээхдээ түүхэн block body эсвэл mutable WSV payload copy
уншихгүй. Destination proof хүлээн авахад pending payload ба түүний charge atomically
арилж, locator/index-ээ хадгалсан fixed terminal descriptor үлдэнэ. Pending state
хязгаартай; terminal records болон immutable Kura history нь байнгын replay protection-д
зориулан өснө. `GET /v1/sccp/messages/recent` нь `{ from, after_index }` compound
cursor ашиглана. Immutable evidence total/operator disk usage-д тооцогдох боловч
evictable-body budget-д орохгүй.

## Torii-ийн хязгаар

`/v1/bridge/proofs/submit` болон `/v1/bridge/messages` нь endpoint-specific HTTP
body хязгаартай. Authentication, rate limit, `Content-Length`-ийг body уншихаас
өмнө шалгана; chunked body-г зөвхөн хатуу хязгаар хүртэл уншина. Хэт том хүсэлт
`413`, malformed transport/JSON тусдаа `400` буцаана. Detached transaction
payload 16 MiB, signature payload 16 KiB-ээр хязгаарлагдана.
