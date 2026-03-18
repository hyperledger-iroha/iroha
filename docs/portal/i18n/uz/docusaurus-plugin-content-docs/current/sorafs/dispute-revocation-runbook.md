---
id: dispute-revocation-runbook
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Dispute & Revocation Runbook
sidebar_label: Dispute & Revocation Runbook
description: Governance workflow for filing SoraFS capacity disputes, coordinating revocations, and evacuating data deterministically.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

## Maqsad

Ushbu runbook boshqaruv operatorlariga SoraFS sig'imiga oid nizolarni topshirish, bekor qilishni muvofiqlashtirish va ma'lumotlarni evakuatsiya qilishning aniq bajarilishini ta'minlash orqali rahbarlik qiladi.

## 1. Hodisani baholang

- **Trigger shartlari:** SLA buzilishini aniqlash (ish vaqti/PoR ishlamay qolishi), replikatsiya etishmovchiligi yoki hisob-kitob kelishmovchiligi.
- **Telemetriyani tasdiqlang:** provayder uchun `/v1/sorafs/capacity/state` va `/v1/sorafs/capacity/telemetry` suratlarini oling.
- **Manfaatdor tomonlarni xabardor qilish:** Saqlash guruhi (provayder operatsiyalari), Boshqaruv kengashi (qaror qabul qiluvchi organ), Kuzatish imkoniyati (boshqaruv paneli yangilanishlari).

## 2. Dalillar to'plamini tayyorlang

1. Xom artefaktlarni to'plang (temetriya JSON, CLI jurnallari, auditor eslatmalari).
2. Deterministik arxivga normallashtirish (masalan, tarbol); rekord:
   - BLAKE3-256 dayjest (`evidence_digest`)
   - Media turi (`application/zip`, `application/jsonl` va boshqalar)
   - Xosting URI (ob'ektni saqlash, SoraFS pin yoki Torii kirish nuqtasi)
3. To'plamni bir marta yozish huquqiga ega boshqaruv dalillarini yig'ish paqirida saqlang.

## 3. Eʼtiroz bildiring

1. `sorafs_manifest_stub capacity dispute` uchun maxsus JSON yarating:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. CLI-ni ishga tushiring:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=i105... \
     --private-key=ed25519:<key>
   ```

3. `dispute_summary.json` ni ko'rib chiqing (turni tasdiqlang, dalillar dayjesti, vaqt belgilari).
4. JSON so‘rovini Torii `/v1/sorafs/capacity/dispute` raqamiga boshqaruv tranzaksiya navbati orqali yuboring. `dispute_id_hex` javob qiymatini oling; u keyingi bekor qilish harakatlari va audit hisobotlarini belgilaydi.

## 4. Evakuatsiya va bekor qilish

1. **Foydali oyna:** provayderni kutilayotgan bekor qilish haqida xabardor qilish; siyosat ruxsat berganda qadalgan ma'lumotlarni evakuatsiya qilishga ruxsat bering.
2. **`ProviderAdmissionRevocationV1` yarating:**
   - Tasdiqlangan sabab bilan `sorafs_manifest_stub provider-admission revoke` dan foydalaning.
   - Imzolar va bekor qilish dayjestini tekshiring.
3. **Bekor qilishni nashr etish:**
   - Torii ga bekor qilish so'rovini yuboring.
   - Provayder reklamalari bloklanganligiga ishonch hosil qiling (`torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` ko'tarilishini kuting).
4. **Boshqaruv panelini yangilash:** provayderni bekor qilingan deb belgilang, nizo identifikatoriga havola qiling va dalillar to‘plamini bog‘lang.

## 5. O'limdan keyingi va kuzatuv

- Boshqaruv hodisalari kuzatuvchisida vaqt jadvalini, asosiy sabab va tuzatish harakatlarini yozib oling.
- Qaytarilishni aniqlang (ulush ulushini qisqartirish, to'lovlarni qaytarish, mijozlarga to'lovlarni qaytarish).
- Hujjatlarni o'rganish; agar kerak bo'lsa, SLA chegaralarini yoki monitoring ogohlantirishlarini yangilang.

## 6. Ma'lumotnomalar

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (nizo bo'limi)
- `docs/source/sorafs/provider_admission_policy.md` (bekor qilish ish jarayoni)
- Kuzatuv paneli: `SoraFS / Capacity Providers`

## Tekshirish ro'yxati

- [ ] Dalillar toʻplami qoʻlga olindi va xeshlandi.
- [ ] Eʼtirozli foydali yuk mahalliy darajada tasdiqlangan.
- [ ] Torii bahsli tranzaksiya qabul qilindi.
- [ ] Bekor qilish amalga oshirildi (tasdiqlangan bo'lsa).
- [ ] Boshqaruv paneli/runbooks yangilandi.
- [ ] O'limdan keyin boshqaruv kengashiga topshirildi.