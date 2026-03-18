---
lang: uz
direction: ltr
source: docs/source/ministry/emergency_canon_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afd5db8a761f8cc56dcd8f67f047f06b45c3211738fedb6dfff326d84b4e8a68
source_last_modified: "2026-01-22T14:45:02.098548+00:00"
translation_last_reviewed: 2026-02-07
title: Emergency Canon & TTL Policy
summary: Reference implementation notes for roadmap item MINFO-6a covering denylist tiers, TTL enforcement, and governance evidence requirements.
translator: machine-google-reviewed
---

# Favqulodda Canon va TTL siyosati (MINFO-6a)

Yo'l xaritasi ma'lumotnomasi: **MINFO-6a — Favqulodda vaziyat kanon va TTL siyosati**.

Ushbu hujjat hozirda Torii va CLI da joʻnatiladigan rad etish roʻyxatini tartiblash qoidalari, TTL ijrosi va boshqaruv majburiyatlarini belgilaydi. Operatorlar yangi yozuvlarni nashr etishdan yoki favqulodda vaziyatlarni chaqirishdan oldin ushbu qoidalarga amal qilishlari kerak.

## Daraja ta'riflari

| Darajasi | Standart TTL | Ko'rib chiqish oynasi | Talablar |
|------|-------------|---------------|--------------|
| Standart | 180 kun (`torii.sorafs_gateway.denylist.standard_ttl`) | yo'q | `issued_at` taqdim etilishi kerak. `expires_at` o'tkazib yuborilishi mumkin; Torii sukut bo'yicha `issued_at + standard_ttl` ga o'rnatiladi va uzunroq oynalarni rad etadi. |
| Favqulodda | 30 kun (`torii.sorafs_gateway.denylist.emergency_ttl`) | 7 kun (`torii.sorafs_gateway.denylist.emergency_review_window`) | Oldindan tasdiqlangan kanonga ishora qiluvchi bo'sh bo'lmagan `emergency_canon` yorlig'ini talab qiladi (masalan, `csam-hotline`). `issued_at` + `expires_at` 30 kunlik oyna ichiga tushishi kerak va ko'rib chiqish dalillari avtomatik yaratilgan muddatni keltirishi kerak (`issued_at + review_window`). |
| Doimiy | Muddati yo'q | yo'q | Ko'pchilik boshqaruv qarorlari uchun ajratilgan. Yozuvlarda boʻsh boʻlmagan `governance_reference` (ovoz identifikatori, manifest xesh va boshqalar) keltirilishi kerak. `expires_at` rad etildi. |

Standartlar `torii.sorafs_gateway.denylist.*` orqali sozlanishi qoladi va `iroha_cli` Torii faylni qayta yuklashdan oldin yaroqsiz yozuvlarni ushlash chegaralarini aks ettiradi.

## Ish jarayoni

1. **Metamaʼlumotlarni tayyorlang:** `policy_tier`, `issued_at`, `expires_at` (agar mavjud boʻlsa) va `emergency_canon`/`governance_reference` (har bir JSON8030 yozuvi ichida) kiradi.
2. **Mahalliy ravishda tekshirish:** `iroha app sorafs gateway lint-denylist --path <denylist.json>` ni ishga tushiring, shunda CLI faylni oʻrnatish yoki mahkamlashdan oldin darajaga xos TTL va kerakli maydonlarni qoʻllashi mumkin.
3. **Dalillarni e'lon qilish:** auditorlar qarorni kuzatishi uchun GAR holatlar to'plamiga (kun tartibi paketi, referendum bayonnomalari va boshqalar) kirishda keltirilgan kanon identifikatorini yoki boshqaruv ma'lumotnomasini ilova qiling.
4. **Favqulodda yozuvlarni ko‘rib chiqing:** favqulodda vaziyatlar qoidalari 30 kun ichida avtomatik ravishda tugaydi. Operatorlar 7 kunlik oynada post-fakto ko'rib chiqishni yakunlashlari va natijani vazirlik kuzatuvchisi/SoraFS dalillar do'konida qayd etishlari kerak.
5. **Torii-ni qayta yuklang:** tasdiqlangandan so'ng, `torii.sorafs_gateway.denylist.path` orqali rad etish yo'lini o'rnating va Torii-ni qayta ishga tushiring/qayta yuklang; ish vaqti yozuvlarni qabul qilishdan oldin bir xil cheklovlarni amalga oshiradi.

## Asboblar va havolalar

- Ish vaqti siyosati ijrosi `sorafs::gateway::denylist` (`crates/iroha_torii/src/sorafs/gateway/denylist.rs`) da ishlaydi va yuklovchi endi `torii.sorafs_gateway.denylist.*` kirishlarini tahlil qilishda darajali metamaʼlumotlarni qoʻllaydi.
- CLI tekshiruvi `GatewayDenylistRecord::validate` (`crates/iroha_cli/src/commands/sorafs.rs`) ichidagi ish vaqti semantikasini aks ettiradi. TTLlar sozlangan oynadan oshib ketganda yoki majburiy kanon/boshqaruv havolalari yoʻq boʻlganda linter ishlamay qoladi.
- Konfiguratsiya tugmalari `torii.sorafs_gateway.denylist` (`crates/iroha_config/src/parameters/{defaults,actual.rs,user.rs}`) ostida aniqlangan, shuning uchun operatorlar boshqaruv turli chegaralarni tasdiqlagan bo'lsa, TTLlarni o'zgartirishi/ko'rib chiqish muddatlarini o'zgartirishi mumkin.
- Ommaviy rad etish ro'yxati (`docs/source/sorafs_gateway_denylist_sample.json`) endi barcha uch darajani ko'rsatadi va yangi yozuvlar uchun kanonik shablon sifatida ishlatilishi kerak.Ushbu to'siqlar favqulodda vaziyatlar ro'yxatini kodlash, cheksiz TTLlarning oldini olish va doimiy bloklar uchun aniq boshqaruv dalillarini majburlash orqali yo'l xaritasi **MINFO-6a** bandini qondiradi.

## Registrni avtomatlashtirish va dalillar eksporti

Favqulodda kanon tasdiqlashlari deterministik registr suratini va a
diff to'plami Torii rad etish ro'yxatini yuklashdan oldin. Asboblar ostida
`xtask/src/sorafs.rs` va CI jabduqlari `ci/check_sorafs_gateway_denylist.sh`
butun ish jarayonini qamrab oladi.

### Kanonik to'plamni yaratish

1. Xom yozuvlarni (odatda boshqaruv tomonidan ko'rib chiqiladigan fayl) ish jarayonida bosqichma-bosqich o'tkazing.
   katalog.
2. JSONni kanoniklashtiring va muhrlang:
   ```bash
   cargo xtask sorafs-gateway denylist pack \
     --input path/to/denylist.json \
     --out artifacts/ministry/denylist_registry/$(date +%Y%m%dT%H%M%SZ) \
     --label ministry-emergency \
     --force
   ```
   Buyruq imzolangan `.json`, Norito `.to` paketini chiqaradi
   konvert va Merkle-root matn fayli boshqaruv sharhlovchilari tomonidan kutilgan.
   Katalogni `artifacts/ministry/denylist_registry/` (yoki sizning
   tanlangan dalil paqir) shuning uchun `scripts/ministry/transparency_release.py` mumkin
   `--artifact denylist_bundle=<path>` bilan keyinroq oling.
3. Yaratilgan `checksums.sha256` ni bosishdan oldin uning yonida saqlang.
   SoraFS/GAR ga. CI `ci/check_sorafs_gateway_denylist.sh` xuddi shunday mashqlarni bajaradi
   `pack` asbob-uskunalar ishlashini kafolatlash uchun namunaviy rad etish ro'yxatiga qarshi yordamchi.
   har bir nashr.

### Diff + audit to'plami

1. Yangi paketni oldingi ishlab chiqarish surati bilan solishtiring
   xtask diff yordamchisi:
   ```bash
   cargo xtask sorafs-gateway denylist diff \
     --old artifacts/ministry/denylist_registry/2026-05-01/denylist_old.json \
     --new artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --report-json artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
   JSON hisobotida barcha qo'shimchalar/olib tashlashlar ro'yxati keltirilgan va dalillarni aks ettiradi
   `MinistryDenylistChangeV1` tomonidan iste'mol qilingan tuzilma (murojaat qilingan
   `docs/source/sorafs_gateway_self_cert.md` va muvofiqlik rejasi).
2. Har bir kanon so'roviga `denylist_diff.json` qo'shing (bu qancha ekanligini isbotlaydi
   yozuvlar tegdi, qaysi daraja o'zgardi va qanday dalil xesh xaritalari
   kanonik to'plam).
3. Farqlar avtomatik ravishda yaratilganda (CI yoki chiqarish quvurlari), eksport qiling
   `denylist_diff.json` yo'li `--artifact denylist_diff=<path>` orqali, shuning uchun
   shaffoflik manifestida uni tozalangan ko'rsatkichlar bilan bir qatorda qayd etadi. Xuddi shu CI
   yordamchi `--evidence-out <path>` ni qabul qiladi, bu esa CLI xulosa bosqichini ishga tushiradi va
   natijada olingan JSONni keyinroq nashr qilish uchun so'ralgan joyga nusxa ko'chiradi.

### Nashr va oshkoralik1. To'plamni + farq artefaktlarini har choraklik shaffoflik katalogiga tashlang
   (`artifacts/ministry/transparency/<YYYY-Q>/denylist/`). Shaffoflik
   reliz yordamchisi keyin ularni o'z ichiga olishi mumkin:
   ```bash
   scripts/ministry/transparency_release.py \
     --quarter 2026-Q3 \
     --output-dir artifacts/ministry/transparency/2026-Q3 \
     --sanitized artifacts/ministry/transparency/2026-Q3/sanitized_metrics.json \
     --dp-report artifacts/ministry/transparency/2026-Q3/dp_report.json \
     --artifact denylist_bundle=artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --artifact denylist_diff=artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
2. Choraklik hisobotda yaratilgan to'plamga/farqga havola
   (`docs/source/ministry/reports/<YYYY-Q>.md`) va bir xil yo'llarni biriktiring
   GAR ovoz paketi, shuning uchun auditorlar dalillar iziga kirish imkonisiz qayta ko'rishlari mumkin
   ichki CI. `ci/check_sorafs_gateway_denylist.sh --evidence-out \
   artefacts/ministry/denylist_registry//denylist_evidence.json` hozir
   paket/diff/dalil quruq ishga tushirishni amalga oshiradi (`iroha_cli ilovasi sorafs shlyuziga qo'ng'iroq qilish)
   dalil` kaput ostida) shuning uchun avtomatlashtirish bilan birga xulosani saqlab qolishi mumkin
   kanonik to'plamlar.
3. Nashr qilingandan so'ng, boshqaruv yukini orqali bog'lang
   `cargo xtask ministry-transparency anchor` (avtomatik ravishda chaqiriladi
   `transparency_release.py`, `--governance-dir` taqdim etilganda) shuning uchun
   denylist registrining dayjesti shaffoflik bilan bir xil DAG daraxtida ko'rinadi
   ozod qilish.

Ushbu jarayondan so'ng "ro'yxatga olish kitobini avtomatlashtirish va dalillarni eksport qilish" yopiladi.
bo'shliq `roadmap.md:450` da chaqiriladi va har bir favqulodda vaziyat kanonini ta'minlaydi
qaror takrorlanadigan artefaktlar, JSON farqlari va shaffoflik jurnali bilan keladi
yozuvlar.

### TTL va Canon Evidence Helper

Bundle/diff juftligi ishlab chiqarilgandan so'ng, qo'lga olish uchun CLI dalillar yordamchisini ishga tushiring
boshqaruv talab qiladigan TTL xulosalari va favqulodda vaziyatlarni ko'rib chiqish muddatlari:

```bash
iroha app sorafs gateway evidence \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --out artifacts/ministry/denylist_registry/2026-05-14/denylist_evidence.json \
  --label csam-canon-2026-05
```

Buyruq JSON manbasini xeshlaydi, har bir yozuvni tasdiqlaydi va ixchamni chiqaradi
o'z ichiga olgan xulosa:

- `kind` va eng erta/oxirgi siyosat darajasi boʻyicha jami yozuvlar
  vaqt belgilari kuzatildi.
- `emergency_reviews[]` ro'yxati har bir favqulodda kanonni o'zi bilan sanab o'tadi
  deskriptor, samarali amal qilish muddati, ruxsat etilgan maksimal TTL va hisoblangan
  `review_due_by` oxirgi muddati.

`denylist_evidence.json` ni oʻralgan paket/diff bilan birga biriktiring, shunda auditorlar yordam berishi mumkin
CLI-ni qayta ishga tushirmasdan TTL muvofiqligini tasdiqlang. Allaqachon yaratilgan CI ishlari
to'plamlar yordamchini chaqirishi va dalil artefaktini nashr qilishi mumkin (masalan
qo'ng'iroq qilish `ci/check_sorafs_gateway_denylist.sh --evidence-out <path>`), ta'minlash
har bir kanon so'rovi izchil xulosa bilan keladi.

### Merkle reestrining dalillari

MINFO-6 da joriy qilingan Merkle reestri operatorlardan uni nashr etishni talab qiladi
TTL xulosasi bilan birga ildiz va har bir kirish dalillari. Yugurgandan so'ng darhol
dalil yordamchisi, Merkle artefaktlarini qo'lga kiriting:

```bash
iroha app sorafs gateway merkle snapshot \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.to

iroha app sorafs gateway merkle proof \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --index 12 \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.to
```Snapshot JSON BLAKE3 Merkle ildizini, barglar sonini va har birini qayd qiladi
deskriptor/xesh juftligi, shuning uchun GAR ovozlari xeshlangan aniq daraxtga murojaat qilishi mumkin.
`--norito-out` yetkazib berish `.to` artefaktini JSON bilan birga saqlaydi va
shlyuzlar ro'yxatga olish kitobi yozuvlarini to'g'ridan-to'g'ri Norito orqali qirib tashlamasdan oladi
stdout. `merkle proof` har qanday ma'lumot uchun yo'nalish bitlari va birodar xeshlarini chiqaradi
nolga asoslangan kirish indeksi, bu har biriga qo'shilish isbotini qo'shishni osonlashtiradi
GAR eslatmasida keltirilgan favqulodda vaziyat kanoni - ixtiyoriy Norito nusxasi dalilni saqlaydi
daftarda tarqatishga tayyor. Keyingi JSON va Norito artefaktlarini saqlang
TTL xulosasi va farqlar to'plamiga, shuning uchun shaffoflik relizlar va boshqaruv
langarlar bir xil ildizga murojaat qiladi.