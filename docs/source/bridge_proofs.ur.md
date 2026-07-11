---
lang: ur
direction: rtl
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 69c9a740261d0c367d52870fc1f48775ae48307056ba9b79d2f811e0c0849f20
source_last_modified: "2026-07-11T15:09:39+04:00"
translation_last_reviewed: 2026-07-11
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

## ایک بار verification اور deterministic limits

- ہر native یا destination proof canonical طور پر ایک بار decode ہوتا ہے اور
  مہنگی cryptographic verification ایک بار چلتی ہے۔ اس سے پہلے consensus ایک
  conservative، hardware-independent work estimate reserve کرتا ہے۔
- `[zk.sccp]` proof count/bytes، native headers، Ethereum light-client updates،
  header bytes، secp256k1 recoveries، BLS aggregate checks/signing
  contributions اور BN254 pairing-product checks کے لیے لازمی nonzero
  per-proof، per-transaction اور per-block limits مقرر کرتا ہے۔ یہ admission
  limits consensus-bound ہیں اور تمام validators پر یکساں ہونی چاہییں۔

## Torii کی حدود

`/v1/bridge/proofs/submit` اور `/v1/bridge/messages` endpoint-specific HTTP body
limits نافذ کرتے ہیں۔ Authentication، rate limit اور `Content-Length` body
پڑھنے سے پہلے check ہوتے ہیں؛ chunked body صرف سخت حد تک پڑھی جاتی ہے۔ حد سے
بڑی request `413` جبکہ malformed transport/JSON الگ `400` واپس کرتی ہے۔
Detached transaction payload کی حد 16 MiB اور signature payload کی 16 KiB ہے۔

</div>
