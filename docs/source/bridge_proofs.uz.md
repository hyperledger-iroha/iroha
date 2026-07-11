---
lang: uz
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 465d8cf704022986b169ab93133517428f8cf2ffe01a498cbda458f4a5b2e69b
source_last_modified: "2026-07-11T15:09:39+04:00"
translation_last_reviewed: 2026-07-11
translator: machine-assisted
---

> Bu 2026-07-11 holatiga ko‘ra qisqartirilgan mahalliylashtirilgan sharh bo‘lib,
> to‘liq normativ tarjima emas. Aniq turlar, API shartnomalari va reliz talablari
> uchun [inglizcha kanonik sahifadan](bridge_proofs.md) foydalaning.

# SCCP V1 ko‘prik dalillari — qisqa sharh

## Birinchi reliz chegarasi

- SCCP V1 yopiq sirtga ega: faqat Ethereum mainnet, BSC mainnet va TRON mainnet
  qo‘llanadi; SORA tomonidagi yagona endpoint — `sora-taira`. Boshqa tarmoq
  profili yoki SORA identifikatori rad etiladi.
- `SubmitBridgeProof` faqat marshrutga bog‘langan typed `NativeProtocol` va
  `SccpDestination` dalillarini qabul qiladi. Umumiy `Ics` va `TransparentZk`
  payload yuborish mavjud emas va fail-closed tarzda rad etiladi.

## Typed reyestr va tarix

- `SccpRegistryV1` typed va append-only. Har bir lane ko‘pi bilan 64 ta route
  revision va 4 096 ta native trust anchor saqlaydi. Yozuvlar yashirincha
  chiqarilmaydi; limitdan keyingi qo‘shish atomik ravishda rad etiladi.
- Anchor interval tasdiqlangan consensus coordinate ishlatadi: Ethereum uchun
  finalized beacon slot, BSC/TRON uchun finalized native block height. Eski
  anchor keyingi checkpoint-ni ham qo‘shib olgan holda amal qiladi, undan keyin
  esa amal qilmaydi.
- Durable inbound record event/finality height va `anchor_interval_height` ni
  alohida saqlaydi. lane+anchor high-water faqat oshadi; keyingi checkpoint
  undan past bo‘la olmaydi. Snapshot hydration indeksni to‘liq qayta hisoblab,
  yetishmayotgan, eskirgan yoki ortiqcha qiymatni rad etadi. Message id qayta
  ishlatilishi va replay ham rad etiladi.

## Bir martalik tekshiruv va deterministik limitlar

- Har bir native yoki destination proof kanonik tarzda bir marta decode qilinadi
  va qimmat cryptographic verification bir marta bajariladi. Undan oldin
  consensus konservativ, hardware-independent ish bahosini zaxiralaydi.
- `[zk.sccp]` proof count/bytes, native headers, Ethereum light-client updates,
  header bytes, secp256k1 recoveries, BLS aggregate checks/signing contributions
  va BN254 pairing-product checks uchun majburiy noldan katta per-proof,
  per-transaction va per-block limitlarini belgilaydi. Bu admission limitlari
  consensus-bound bo‘lib, barcha validatorlarda bir xil bo‘lishi kerak.

## Torii chegaralari

`/v1/bridge/proofs/submit` va `/v1/bridge/messages` endpoint-specific HTTP body
limitlaridan foydalanadi. Authentication, rate limit va `Content-Length` body
o‘qilishidan oldin tekshiriladi; chunked body faqat qat’iy limitgacha o‘qiladi.
Haddan katta so‘rov `413`, malformed transport/JSON esa alohida `400` qaytaradi.
Detached transaction payload 16 MiB, signature payload 16 KiB bilan cheklangan.
