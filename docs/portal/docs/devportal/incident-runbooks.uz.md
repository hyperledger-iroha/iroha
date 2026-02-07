---
lang: uz
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8599dbc1a8e4fe846965eed90af128deb5950f83dc61838fea583b326b92a011
source_last_modified: "2025-12-29T18:16:35.104300+00:00"
translation_last_reviewed: 2026-02-07
id: incident-runbooks
title: Incident Runbooks & Rollback Drills
sidebar_label: Incident Runbooks
description: Response guides for failed portal deployments, SoraFS replication degradation, analytics outages, and the quarterly rehearsal cadence required by DOCS-9.
translator: machine-google-reviewed
---

## Maqsad

“Yo‘l xaritasi” bandi **DOCS-9** amaldagi o‘yin kitoblari hamda mashq rejasini talab qiladi.
portal operatorlari taxmin qilmasdan yuk tashishdagi nosozliklarni tiklashlari mumkin. Ushbu eslatma
uchta yuqori signalli hodisani qamrab oladi - muvaffaqiyatsiz joylashtirish, replikatsiya
degradatsiya va tahliliy uzilishlar - va choraklik mashqlarni hujjatlashtiradi
taxallusni orqaga qaytarish va sintetik tekshirish hali ham oxirigacha ishlaydi.

### Tegishli materiallar

- [`devportal/deploy-guide`](./deploy-guide) — qadoqlash, imzolash va taxallus
  rag'batlantirish ish jarayoni.
- [`devportal/observability`](./observability) — chiqarish teglari, tahlillar va
  problar quyida keltirilgan.
- `docs/source/sorafs_node_client_protocol.md`
  va [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — registr telemetriyasi va eskalatsiya chegaralari.
- `docs/portal/scripts/sorafs-pin-release.sh` va `npm run probe:*` yordamchilari
  nazorat varaqlarida havola qilingan.

### Umumiy telemetriya va asboblar

| Signal / Asbob | Maqsad |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (uchrashgan/o'tkazib yuborilgan/kutishda) | Replikatsiya to'xtash joylari va SLA buzilishini aniqlaydi. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Triaj uchun kechikish chuqurligi va tugallanish kechikishini aniqlaydi. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Ko'pincha noto'g'ri joylashtirishdan keyin shlyuz tomonidagi nosozliklarni ko'rsatadi. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Sintetik problar o'tish joylarini chiqaradi va tasdiqlaydi. |
| `npm run check:links` | Buzilgan bog'lanish eshigi; har bir yumshatishdan keyin ishlatiladi. |
| `sorafs_cli manifest submit … --alias-*` (`scripts/sorafs-pin-release.sh` tomonidan o'ralgan) | Taxallusni ilgari surish/qaytarish mexanizmi. |
| `Docs Portal Publishing` Grafana taxtasi (`dashboards/grafana/docs_portal.json`) | Rad etish/taxallus/TLS/replikatsiya telemetriyasini jamlaydi. PagerDuty ogohlantirishlari dalillar uchun ushbu panellarga havola qiladi. |

## Runbook - Muvaffaqiyatsiz tarqatish yoki yomon artefakt

### Trigger shartlari

- Ko'rib chiqish/ishlab chiqarish problari muvaffaqiyatsiz tugadi (`npm run probe:portal -- --expect-release=…`).
- `torii_sorafs_gateway_refusals_total` da Grafana ogohlantirishlari yoki
  Chiqarishdan keyin `torii_sorafs_manifest_submit_total{status="error"}`.
- Qo'lda QA buzilgan marshrutlar yoki Try-It proksi-serveridagi nosozliklar haqida darhol xabar beradi
  taxallus reklamasi.

### Darhol cheklash

1. **Oʻrnatishlarni muzlatish:** CI quvur liniyasini `DEPLOY_FREEZE=1` (GitHub) bilan belgilang
   ish oqimini kiritish) yoki Jenkins ishini to'xtatib turing, shunda qo'shimcha artefaktlar o'chmaydi.
2. **Artefaktlarni suratga olish:** `build/checksums.sha256` ishlamay qolgan qurilmani yuklab oling,
   `portal.manifest*.{json,to,bundle,sig}` va prob chiqishini orqaga qaytarish mumkin
   aniq dayjestlarga murojaat qiling.
3. **Manfaatdor tomonlarni xabardor qiling:** saqlash SRE, Docs/DevRel yetakchisi va boshqaruv
   xabardorlik uchun navbatchi (ayniqsa, `docs.sora` ta'sir qilganda).

### Orqaga qaytarish tartibi

1. Oxirgi yaxshi ma'lum (LKG) manifestini aniqlang. Ishlab chiqarish ish oqimi saqlanadi
   ularni `artifacts/devportal/<release>/sorafs/portal.manifest.to` ostida.
2. Yuboruvchi yordamchi bilan manifestga taxallusni qayta bog'lang:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. Qayta tiklash xulosasini LKG va bilan birgalikda voqea chiptasiga yozing
   muvaffaqiyatsiz manifest hazm qilish.

### Tasdiqlash

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature …` va `sorafs_cli proof verify …`
   qayta targ'ib qilingan manifest hali mos kelishini tasdiqlash uchun (o'rnatish qo'llanmasiga qarang).
   arxivlangan CAR.
4. Try-It staging proksi-serverining qaytib kelishini ta'minlash uchun `npm run probe:tryit-proxy`.

### Hodisadan keyingi

1. Asosiy sabab tushunilgandan keyingina joylashtirish quvurini qayta yoqing.
2. To‘ldirish [`devportal/deploy-guide`](./deploy-guide) “O‘rganilgan saboqlar”
   yangi gotchalar bilan yozuvlar, agar mavjud bo'lsa.
3. Muvaffaqiyatsiz sinov to'plami uchun fayl nuqsonlari (prob, havola tekshiruvi va boshqalar).

## Runbook — Replikatsiya degradatsiyasi

### Trigger shartlari

- Ogohlantirish: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_jami{natija=~"uchrashuv|o'tkazib yuborilgan"}), 1) <
  10 daqiqa uchun 0,95`.
- 10 daqiqa davomida `torii_sorafs_replication_backlog_total > 10` (qarang
  `pin-registry-ops.md`).
- Boshqaruv nashrdan keyin taxallusning sekin mavjudligi haqida hisobot beradi.

### Triaj

1. Tasdiqlash uchun [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) asboblar panelini tekshiring
   kechikish saqlash sinfiga yoki provayderlar parkiga mahalliylashtirilganmi.
2. `sorafs_registry::submit_manifest` ogohlantirishlari uchun Torii jurnallarini oʻzaro tekshiring.
   taqdimnomalarning o'zi muvaffaqiyatsiz ekanligini aniqlash.
3. `sorafs_cli manifest status --manifest …` (roʻyxatlar
   provayder uchun replikatsiya natijalari).

### Yumshatish

1. Manifestni takrorlash soni yuqori (`--pin-min-replicas 7`) yordamida qayta chiqaring.
   `scripts/sorafs-pin-release.sh`, shuning uchun rejalashtiruvchi yukni kattaroq joyga tarqatadi
   provayder to'plami. Hodisalar jurnaliga yangi manifest dayjestini yozib oling.
2. Agar kechikishlar bitta provayderga bog'langan bo'lsa, uni quyidagi orqali vaqtincha o'chirib qo'ying
   replikatsiya rejalashtiruvchisi (`pin-registry-ops.md` da hujjatlashtirilgan) va yangisini yuboring
   manifest boshqa provayderlarni taxallusni yangilashga majbur qiladi.
3. Taxallusning yangiligi replikatsiya paritetidan muhimroq bo'lsa, ni qayta bog'lang
   sahnalashtirilgan manifestga taxallusni (`docs-preview`), keyin nashr eting
   SRE orqada qolganlarni tozalagandan keyin keyingi manifest.

### Qayta tiklash va yopish

1. Ta'minlash uchun `torii_sorafs_replication_sla_total{outcome="missed"}` monitor
   platolarni hisoblash.
2. `sorafs_cli manifest status` chiqishini har bir replika ekanligini tasdiqlang
   muvofiqlikka qaytish.
3. Keyingi qadamlar bilan o'limdan keyingi replikatsiyalar to'plamini fayllang yoki yangilang
   (provayder miqyosi, chunker sozlash va boshqalar).

## Runbook — Analitika yoki telemetriyadagi uzilish

### Trigger shartlari

- `npm run probe:portal` muvaffaqiyatli bo'ldi, lekin asboblar paneli qabul qilishni to'xtatadi
  >15 daqiqa davomida `AnalyticsTracker` hodisalari.
- Maxfiylik tekshiruvi to'xtatilgan hodisalarning kutilmagan ko'payishini belgilaydi.
- `npm run probe:tryit-proxy` `/probe/analytics` yo'llarida muvaffaqiyatsiz tugadi.

### Javob

1. Qurilish vaqti kiritishlarini tekshiring: `DOCS_ANALYTICS_ENDPOINT` va
   `DOCS_ANALYTICS_SAMPLE_RATE` muvaffaqiyatsiz reliz artefaktida (`build/release.json`).
2. `DOCS_ANALYTICS_ENDPOINT` bilan `npm run probe:portal` ni qayta ishga tushiring.
   treker hali ham foydali yuklarni chiqarishini tasdiqlash uchun staging kollektori.
3. Agar kollektorlar ishlamay qolsa, `DOCS_ANALYTICS_ENDPOINT=""` ni o'rnating va shunday qilib qayta tiklang.
   kuzatuvchining qisqa tutashuvi; voqea vaqt jadvalida uzilish oynasini yozib oling.
4. `scripts/check-links.mjs` barmoq izlarini tasdiqlang `checksums.sha256`
   (analitik uzilishlar sayt xaritasini tekshirishni * bloklamasligi kerak).
5. Kollektor tiklangandan so'ng, mashq qilish uchun `npm run test:widgets` ni ishga tushiring
   qayta nashr qilishdan oldin tahliliy yordamchi birlik sinovlari.

### Hodisadan keyingi

1. [`devportal/observability`](./observability) ni istalgan yangi kollektor bilan yangilang
   cheklovlar yoki namuna olish talablari.
2. Tashqarida har qanday tahliliy ma'lumotlar o'chirilgan yoki tahrirlangan bo'lsa, fayl boshqaruvi haqida xabarnoma
   siyosat.

## Har chorakda chidamlilik mashqlari

Har ikki mashqni **har chorakning birinchi seshanbasida** (yanvar/aprel/iyul/oktabr) bajaring.
yoki har qanday yirik infratuzilma o'zgarishidan so'ng darhol. Artefaktlarni ostida saqlang
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Matkap | Qadamlar | Dalil |
| ----- | ----- | -------- |
| Taxallusni qaytarish repetisiyasi | 1. Eng so‘nggi ishlab chiqarish manifestidan foydalanib, “Muvaffaqiyatsiz o‘rnatish” ni qayta o‘ynang.<br/>2. Problar o‘tgandan keyin qayta ishlab chiqarishga ulang.<br/>3. Matkap papkasida `portal.manifest.submit.summary.json` va prob jurnallarini yozib oling. | `rollback.submit.json`, prob chiqishi va repetisiyaning yorlig'i. |
| Sintetik tekshirish auditi | 1. Ishlab chiqarish va sahnalashtirishga qarshi `npm run probe:portal` va `npm run probe:tryit-proxy` ni ishga tushiring.<br/>2. `npm run check:links` ishga tushiring va `build/link-report.json` arxivini saqlang.<br/>3. Tekshiruv muvaffaqiyatini tasdiqlovchi Grafana panellarining skrinshotlari/eksportlarini ilova qiling. | Prob jurnallari + `link-report.json` manifest barmoq iziga havola. |

O'tkazib yuborilgan mashqlarni Docs/DevRel menejeriga va SRE boshqaruvini tekshirishga yetkazing,
chunki yo'l xaritasi deterministik, har chorakda ikkala taxallusni isbotlashni talab qiladi
orqaga qaytarish va portal problari sog'lom bo'lib qoladi.

## PagerDuty va qo'ng'iroq bo'yicha muvofiqlashtirish

- PagerDuty xizmati **Docs Portal Publishing** tomonidan yaratilgan ogohlantirishlarga egalik qiladi.
  `dashboards/grafana/docs_portal.json`. Qoidalar `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` va `DocsPortal/TLSExpiry` Docs/DevRel sahifalarida
  asosiy, ikkinchi darajali saqlash SRE.
- Sahifalanganda, `DOCS_RELEASE_TAG` ni qo'shing, ta'sirlanganlarning skrinshotlarini ilova qiling
  Grafana panellari va oldin voqea qaydlarida prob/link-check chiqishini bog'lang
  yumshatish boshlanadi.
- Yumshatgandan so'ng (orqaga qaytarish yoki qayta joylashtirish), `npm run probe:portal` ni qayta ishga tushiring,
  `npm run check:links` va ko'rsatkichlarni ko'rsatadigan yangi Grafana suratlarini oling
  chegaralar ichida qaytib. Oldin PagerDuty hodisasiga barcha dalillarni biriktiring
  uni hal qilish.
- Agar ikkita ogohlantirish bir vaqtning o'zida yonsa (masalan, TLS muddati tugashi va kechikishlar), triage
  avval rad etadi (nashr qilishni to'xtating), orqaga qaytarish protsedurasini bajaring, keyin tozalang
  Ko'prikda SRE xotirasi bo'lgan TLS/ortiqcha ob'ektlar.