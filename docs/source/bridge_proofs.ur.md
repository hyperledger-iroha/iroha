---
lang: ur
direction: rtl
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 74e29801129deccb6d5640d414289c47cf13fa9e0229fb55212b6c7710d7c5f7
source_last_modified: "2026-07-12T07:38:49.568351+00:00"
translation_last_reviewed: 2026-07-12
translator: machine-assisted
---

<div dir="rtl">

> یہ 2026-07-11 تک کا مختصر مقامی خلاصہ ہے، مکمل معیاری ترجمہ نہیں۔ درست
> types، API contracts اور release requirements کے لیے
> [انگریزی canonical صفحہ](bridge_proofs.md) استعمال کریں۔

# SCCP V1 برج proofs — مختصر جائزہ

## پہلے release کی حد

- SCCP V1 ایک بند سطح ہے: صرف Ethereum mainnet، BSC mainnet اور TRON mainnet
  دستیاب ہیں، اور SORA کا واحد endpoint `sora-taira` ہے۔ کسی دوسرے network
  profile یا SORA identity کو مسترد کیا جاتا ہے۔
- `SubmitBridgeProof` صرف route سے بندھے ہوئے typed `NativeProtocol` اور
  `SccpDestination` proofs قبول کرتا ہے۔ عمومی `Ics` اور `TransparentZk`
  payload submission دستیاب نہیں اور fail-closed انداز میں مسترد ہوتی ہے۔

## Typed registry اور تاریخ

- `SccpRegistryV1` typed اور append-only ہے۔ ہر lane زیادہ سے زیادہ 64 route
  revisions اور 4,096 native trust anchors محفوظ رکھتی ہے۔ records کو ضمنی
  طور پر نہیں نکالا جاتا؛ حد کے بعد اگلا append جوہری طور پر مسترد ہوتا ہے۔
- Anchor interval ایک authenticated consensus coordinate استعمال کرتا ہے:
  Ethereum کے لیے finalized beacon slot اور BSC/TRON کے لیے finalized native
  block height۔ پرانا anchor اگلے checkpoint سمیت معتبر رہتا ہے، اس کے بعد
  نہیں۔
- Durable inbound record، event/finality height اور `anchor_interval_height`
  الگ الگ محفوظ کرتا ہے۔ lane+anchor high-water صرف بڑھتا ہے؛ successor
  checkpoint اس سے کم نہیں ہو سکتا۔ Snapshot hydration پورا index دوبارہ
  بناتی ہے اور missing، stale یا extra value مسترد کرتی ہے۔ Message id کا
  دوبارہ استعمال اور replay بھی مسترد ہوتے ہیں۔

TRON کا source route عین
`transferToTaira(bytes,uint256,uint64 expectedNonce)` ABI استعمال کرتا ہے۔ کامیاب
execution کے لیے `expectedNonce == transferNonce` لازم ہے؛ پھر storage بڑھانے سے
پہلے یہی قدر canonical payload میں لکھی جاتی ہے۔ Native admission، payload کے
recipient، scaled amount اور nonce سے مکمل ABI call دوبارہ بناتی ہے۔ اس لیے پرانا
دو-argument selector، stale یا future nonce اور ختم شدہ `uint64` nonce سب
fail-closed انداز میں مسترد ہوتے ہیں۔

## ایک بار verification اور deterministic limits

- ہر native یا destination proof canonical طور پر ایک بار decode ہوتا ہے اور
  مہنگی cryptographic verification ایک بار چلتی ہے۔ اس سے پہلے consensus ایک
  conservative، hardware-independent work estimate reserve کرتا ہے۔
- `[zk.sccp]` proof count/bytes، native headers، Ethereum light-client updates،
  header bytes، secp256k1 recoveries، BLS aggregate checks/signing
  contributions اور BN254 pairing-product checks کے لیے لازمی nonzero
  per-proof، per-transaction اور per-block limits مقرر کرتا ہے۔ یہ admission
  limits consensus-bound ہیں اور تمام validators پر یکساں ہونی چاہییں۔

## Outbound commitment، retention اور discovery

ہر کامیاب outbound message کو block execution order کے مطابق dense
`commitment_index` (`0..=511`) ملتا ہے۔ V1 کی fixed limits فی block 512 messages اور
فی message 4,096 canonical payload bytes ہیں۔ `[zk.sccp]` pending payload state کو
`max_pending_outbound_messages` (default `65536`) اور
`max_pending_outbound_payload_bytes` (default `268435456`) دونوں سے محدود کرتا ہے۔

Finality publish ہونے یا block body evict ہونے سے پہلے Kura exact canonical header
اور root-authenticated SCCP archive کو immutable طور پر محفوظ کرتا ہے۔ Proof، bundle،
proof request اور recent history reconstruction کو historical block body یا mutable
WSV payload copy درکار نہیں۔ Destination proof قبول ہونے پر pending payload اور اس
کا charge atomically حذف ہوتے ہیں اور locator/index کے ساتھ fixed terminal descriptor
رہتا ہے۔ Pending state bounded ہے؛ terminal records اور immutable Kura history مستقل
replay protection کے لیے جان بوجھ کر بڑھتے ہیں۔ `GET /v1/sccp/messages/recent`
compound cursor `{ from, after_index }` استعمال کرتا ہے۔ Immutable evidence
total/operator disk usage میں شمار ہوتا ہے مگر evictable-body budget میں نہیں۔

## Torii کی حدود

`/v1/bridge/proofs/submit` اور `/v1/bridge/messages` endpoint-specific HTTP body
limits نافذ کرتے ہیں۔ Authentication، rate limit اور `Content-Length` body
پڑھنے سے پہلے check ہوتے ہیں؛ chunked body صرف سخت حد تک پڑھی جاتی ہے۔ حد سے
بڑی request `413` جبکہ malformed transport/JSON الگ `400` واپس کرتی ہے۔
Detached transaction payload کی حد 16 MiB اور signature payload کی 16 KiB ہے۔

</div>
