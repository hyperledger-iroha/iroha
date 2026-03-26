---
lang: uz
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a583af55cf8b4cf5070828bfb52146be88f92937c8d7887ab37a2056bf55ec9e
source_last_modified: "2026-01-22T16:26:46.515965+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
id: ommaviy-onboarding-vositalar to'plami
sarlavha: SNS Bulk Onboarding Toolkit
sidebar_label: Ommaviy ishga tushirish asboblar to'plami
tavsif: SN-3b registratori uchun CSV to RegisterNameRequestV1 avtomatizatsiyasi.
---

::: Eslatma Kanonik manba
Nometall `docs/source/sns/bulk_onboarding_toolkit.md` tashqi operatorlar ko'radi
omborni klonlashsiz bir xil SN-3b ko'rsatmalari.
:::

# SNS Bulk Onboarding Toolkit (SN-3b)

**Yo'l xaritasi ma'lumotnomasi:** SN-3b "Yopma ravishda ishga tushirish asboblari"  
**Artifaktlar:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Katta registratorlar ko'pincha yuzlab `.sora` yoki `.nexus` ro'yxatga olishlarini oldindan bosqichma-bosqich amalga oshiradilar.
bir xil boshqaruv tasdiqlari va hisob-kitob relslari bilan. JSONni qo'lda yaratish
foydali yuklar yoki CLI-ni qayta ishga tushirish masshtablashmaydi, shuning uchun SN-3b deterministikni yuboradi.
`RegisterNameRequestV1` tuzilmalarini tayyorlovchi CSV - Norito quruvchisi
Torii yoki CLI. Yordamchi har bir satrni oldinda tasdiqlaydi, ikkalasini ham chiqaradi
jamlangan manifest va ixtiyoriy yangi qator bilan ajratilgan JSON va yuborishi mumkin
audit uchun tuzilgan tushumlarni yozib olishda avtomatik yuklaydi.

## 1. CSV sxemasi

Tahlil qiluvchi quyidagi sarlavha qatorini talab qiladi (tartib moslashuvchan):

| Ustun | Majburiy | Tavsif |
|--------|----------|-------------|
| `label` | Ha | Talab qilingan yorliq (aralash holatda qabul qilinadi; asbob v1 va UTS-46 normalariga muvofiq normallashadi). |
| `suffix_id` | Ha | Raqamli qo'shimcha identifikator (o'nlik yoki `0x` hex). |
| `owner` | Ha | AccountId string (domainless encoded literal; canonical i105 only; no `@<domain>` suffix). |
| `term_years` | Ha | Butun son `1..=255`. |
| `payment_asset_id` | Ha | Hisob-kitob aktivi (masalan, `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Ha | Obyektning asl birliklarini ifodalovchi belgisiz butun sonlar. |
| `settlement_tx` | Ha | JSON qiymati yoki toʻlov tranzaksiyasini yoki xeshni tavsiflovchi literal satr. |
| `payment_payer` | Ha | Toʻlovga ruxsat bergan hisob identifikatori. |
| `payment_signature` | Ha | JSON yoki styuard yoki xazina imzosi isbotini o'z ichiga olgan literal qator. |
| `controllers` | Ixtiyoriy | Kontroller hisob manzillarining nuqta-vergul yoki vergul bilan ajratilgan roʻyxati. Oʻtkazib yuborilganda birlamchi `[owner]`. |
| `metadata` | Ixtiyoriy | Inline JSON yoki `@path/to/file.json` hal qiluvchi maslahatlar, TXT yozuvlari va hokazolarni taqdim etadi. Standart `{}`. |
| `governance` | Ixtiyoriy | Inline JSON yoki `@path` `GovernanceHookV1` ga ishora qiladi. `--require-governance` bu ustunni qo'llaydi. |

Har qanday ustun hujayra qiymatini `@` bilan oldindan belgilash orqali tashqi faylga murojaat qilishi mumkin.
Yo'llar CSV fayliga nisbatan hal qilinadi.

## 2. Yordamchini ishga tushirish

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Asosiy variantlar:

- `--require-governance` boshqaruv ilgagisiz qatorlarni rad etadi (foydali).
  premium auktsionlar yoki zaxiralangan topshiriqlar).
- `--default-controllers {owner,none}` boshqaruvchi katakchalar bo'sh yoki yo'qligini hal qiladi
  egasining hisobiga qayting.
- `--controllers-column`, `--metadata-column` va `--governance-column` nomini o'zgartirish
  yuqori oqim eksporti bilan ishlashda ixtiyoriy ustunlar.

Muvaffaqiyatli skript yig'ilgan manifestni yozadi:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "i105...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"i105...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"i105...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

Agar `--ndjson` taqdim etilsa, har bir `RegisterNameRequestV1` ham shunday yoziladi.
bitta qatorli JSON hujjati, shuning uchun avtomatlashtirish so'rovlarni to'g'ridan-to'g'ri ichiga yuborishi mumkin
Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Avtomatlashtirilgan taqdimotlar

### 3.1 Torii REST rejimi

`--submit-torii-url` va `--submit-token` yoki
`--submit-token-file` har bir manifest yozuvni to'g'ridan-to'g'ri Torii ga surish uchun:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Yordamchi har bir so'rov uchun bitta `POST /v1/sns/names` chiqaradi va uni bekor qiladi
  birinchi HTTP xatosi. Javoblar jurnal yo'liga NDJSON sifatida qo'shiladi
  yozuvlar.
- `--poll-status` har bir so'rovdan keyin `/v1/sns/names/{namespace}/{literal}` so'rovlarini qayta so'raydi
  yozuv mavjudligini tasdiqlash uchun topshirish (`--poll-attempts` gacha, standart 5)
  ko'rinadigan. `--suffix-map` (JSON `suffix_id` dan `"suffix"` qiymatlarigacha) taqdim eting.
  asbob so'rov uchun `{label}.{suffix}` literallarini olishi mumkin.
- Sozlanishi: `--submit-timeout`, `--poll-attempts` va `--poll-interval`.

### 3.2 iroha CLI rejimi

Har bir manifest yozuvini CLI orqali yo'naltirish uchun ikkilik yo'lni taqdim eting:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Kontrollerlar `Account` yozuvlari bo'lishi kerak (`controller_type.kind = "Account"`)
  chunki CLI hozirda faqat hisobga asoslangan kontrollerlarni ochib beradi.
- Metadata va boshqaruv bloklari har bir so'rov bo'yicha vaqtinchalik fayllarga yoziladi va
  `iroha sns register --metadata-json ... --governance-json ...` ga yuborildi.
- CLI stdout va stderr plus chiqish kodlari qayd qilinadi; nolga teng bo'lmagan chiqish kodlari bekor qilinadi
  yugurish.

Ikkala yuborish rejimi ham ro'yxatga oluvchini o'zaro tekshirish uchun birgalikda ishlashi mumkin (Torii va CLI).
joylashtirish yoki qayta tiklashni takrorlash.

### 3.3 Taqdim etish kvitansiyalari

`--submission-log <path>` taqdim etilganda, skript NDJSON yozuvlarini qo'shadi
qo'lga olish:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Muvaffaqiyatli Torii javoblari tarkibidan olingan tuzilgan maydonlarni oʻz ichiga oladi
`NameRecordV1` yoki `RegisterNameResponseV1` (masalan, `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) asboblar paneli va boshqaruv
hisobotlar jurnalni erkin shakldagi matnni tekshirmasdan tahlil qilishi mumkin. Ushbu jurnalga biriktiring
takrorlanuvchi dalillar uchun manifest bilan birga registrator chiptalari.

## 4. Docs portalini chiqarishni avtomatlashtirish

CI va portal vazifalari `docs/portal/scripts/sns_bulk_release.sh` deb nomlanadi, bu esa o'raladi
yordamchi va artefaktlarni `artifacts/sns/releases/<timestamp>/` ostida saqlaydi:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

Ssenariy:

1. `registrations.manifest.json`, `registrations.ndjson` tuzadi va nusxa oladi
   asl CSVni relizlar katalogiga kiriting.
2. Manifestni Torii va/yoki CLI (konfiguratsiya qilinganida), yozish orqali yuboradi
   `submissions.log` yuqorida tuzilgan kvitantsiyalar bilan.
3. Chiqarishni tavsiflovchi `summary.json` chiqaradi (yo‘llar, Torii URL, CLI yo‘li,
   vaqt tamg'asi) shuning uchun portal avtomatizatsiyasi to'plamni artefakt xotirasiga yuklashi mumkin.
4. `metrics.prom` (`--metrics` orqali bekor qilish) ishlab chiqaradi
   Umumiy so'rovlar uchun Prometheus formatidagi hisoblagichlar, qo'shimchalarni taqsimlash,
   jami aktivlar va taqdim etish natijalari. Xulosa JSON bu faylga havola qiladi.

Ish oqimlari relizlar katalogini hozirda bitta artefakt sifatida arxivlaydi
boshqaruv uchun audit uchun zarur bo'lgan hamma narsani o'z ichiga oladi.

## 5. Telemetriya va asboblar paneli

`sns_bulk_release.sh` tomonidan yaratilgan ko'rsatkichlar fayli quyidagilarni ochib beradi
seriya:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

`metrics.prom`ni Prometheus yon aravachasiga (masalan, Promtail yoki
paketli importer) registratorlar, styuardlar va boshqaruv tengdoshlarini bir xilda ushlab turish uchun
ommaviy taraqqiyot. Grafana doska
`dashboards/grafana/sns_bulk_release.json` bir xil ma'lumotlarni panellar bilan ingl
har bir qo'shimchalar soni, to'lov hajmi va topshirishning muvaffaqiyatli/muvaffaqiyatli nisbati uchun.
Kengash `release` tomonidan filtrlanadi, shuning uchun auditorlar bitta CSV ishga tushishi mumkin.

## 6. Validatsiya va nosozlik rejimlari

- **Yorliq kanonizatsiyasi:** kirishlar Python IDNA plus bilan normallashtiriladi
  kichik harflar va Norm v1 belgilar filtrlari. Noto'g'ri yorliqlar barchasidan oldin tezda ishdan chiqadi
  tarmoq qo'ng'iroqlari.
- **Raqamli to'siqlar:** qo'shimcha identifikatorlar, muddatlar yillari va narx bo'yicha maslahatlar tushishi kerak
  `u16` va `u8` chegaralarida. To'lov maydonlari o'nlik yoki olti burchakli butun sonlarni qabul qiladi
  `i64::MAX` gacha.
- **Metamaʼlumotlar yoki boshqaruv tahlili:** inline JSON toʻgʻridan-toʻgʻri tahlil qilinadi; fayl
  murojaatlar CSV manziliga nisbatan hal qilinadi. Ob'ekt bo'lmagan metama'lumotlar
  tekshirish xatosini keltirib chiqaradi.
- **Kontrollerlar:** bo'sh hujayralar `--default-controllers` ni hurmat qiladi. Aniq taqdim eting
  boshqaruvchi ro'yxatlari (masalan, `i105...;i105...`) egasi bo'lmaganlarga topshirilganda
  aktyorlar.

Xatolar kontekstli qator raqamlari bilan xabar qilinadi (masalan
`error: row 12 term_years must be between 1 and 255`). Skript bilan chiqadi
tekshirish xatolarida `1` kodi va CSV yo'li yo'q bo'lganda `2`.

## 7. Sinov va kelib chiqish

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` CSV tahlilini qamrab oladi,
  NDJSON emissiyasi, boshqaruvni tatbiq etish va CLI yoki Torii taqdimoti
  yo'llar.
- Yordamchi sof Python (qo'shimcha bog'liqliklar yo'q) va istalgan joyda ishlaydi
  `python3` mavjud. Ma'muriyat tarixi CLI bilan birga kuzatib boriladi
  takror ishlab chiqarish uchun asosiy ombor.

Ishlab chiqarish jarayonlari uchun yaratilgan manifest va NDJSON to'plamini biriktiring
registrator chiptasi, shuning uchun styuardlar topshirilgan aniq yuklarni takrorlashlari mumkin
Torii gacha.