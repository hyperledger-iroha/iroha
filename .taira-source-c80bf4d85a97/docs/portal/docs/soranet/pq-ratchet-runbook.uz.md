---
lang: uz
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 370733f000ffa7022cab18931c2697031a225d2ce3ae83382896c0d61f9fe6e2
source_last_modified: "2026-01-05T09:28:11.912569+00:00"
translation_last_reviewed: 2026-02-07
id: pq-ratchet-runbook
title: SoraNet PQ Ratchet Fire Drill
sidebar_label: PQ Ratchet Runbook
description: On-call rehearsal steps for promoting or demoting the staged PQ anonymity policy with deterministic telemetry validation.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
:::

## Maqsad

Ushbu ish kitobi SoraNetning bosqichma-bosqich post-kvant (PQ) anonimlik siyosati uchun yong'in-mashqlar ketma-ketligini boshqaradi. Operatorlar ham yuqoriga ko‘tarilish (A-bosqich -> B bosqich -> C bosqich) va PQ ta’minoti pasayganda B/A bosqichiga boshqariladigan pasaytirishni takrorlaydi. Matkap telemetriya ilgaklarini (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) tasdiqlaydi va voqea takrorlash jurnali uchun artefaktlarni to'playdi.

## Old shartlar

- So'nggi `sorafs_orchestrator` ikkilik qobiliyatini o'lchash bilan (`docs/source/soranet/reports/pq_ratchet_validation.md` da ko'rsatilgan matkap ma'lumotnomasida yoki undan keyin bajaring).
- `dashboards/grafana/soranet_pq_ratchet.json` ga xizmat qiluvchi Prometheus/Grafana stekiga kirish.
- Nominal qo'riqchi katalogining surati. Mashq qilishdan oldin nusxasini oling va tasdiqlang:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Agar manba katalog faqat JSON-ni nashr etsa, aylantirish yordamchilarini ishga tushirishdan oldin uni Norito ikkilik fayliga `soranet-directory build` bilan qayta kodlang.

- CLI yordamida metama'lumotlar va emitent aylanishdan oldingi artefaktlarni yozib oling:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Tarmoq va kuzatuv guruhlari tomonidan tasdiqlangan o'zgartirish oynasi.

## Rag'batlantirish bosqichlari

1. **Bosqichli audit**

   Boshlanish bosqichini yozib oling:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Rag'batlantirishdan oldin `anon-guard-pq` kuting.

2. **B bosqichiga ko'tarilish (ko'pchilik PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Manifestlar yangilanishi uchun >=5 daqiqa kuting.
   - Grafana (`SoraNet PQ Ratchet Drill` asboblar paneli) da "Siyosat hodisalari" panelida `stage=anon-majority-pq` uchun `outcome=met` ko'rsatiladi.
   - Skrinshot yoki JSON panelini oling va uni voqea jurnaliga biriktiring.

3. **S bosqichiga (qattiq PQ) ko'taring**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - `sorafs_orchestrator_pq_ratio_*` gistogrammalarining 1.0 ga o'tishini tekshiring.
   - Qo'ng'irlash hisoblagichining tekisligini tasdiqlang; aks holda pasaytirish bosqichlarini bajaring.

## Pastga tushirish / burg'ulash

1. **Sintetik PQ tanqisligini keltirib chiqaring**

   Qo'riqchi katalogini faqat klassik yozuvlarga qisqartirish orqali o'yin maydonchasi muhitida PQ o'rni o'chiring, keyin orkestr keshini qayta yuklang:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Qoʻngʻirlash telemetriyasini kuzating**

   - Boshqaruv paneli: "Brownout Rate" paneli 0 dan yuqori ko'tariladi.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` `anonymity_outcome="brownout"` bilan `anonymity_reason="missing_majority_pq"` xabar berishi kerak.

3. **B bosqich / A bosqichga pasaytirish**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Agar PQ ta'minoti hali ham etarli bo'lmasa, `anon-guard-pq` ga tushiring. Mashq taymerlari oʻrnashganidan soʻng yakunlanadi va aksiyalar qayta qoʻllanilishi mumkin.

4. **Qo'riqchi katalogini tiklash**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetriya va artefaktlar

- **Boshqaruv paneli:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus ogohlantirishlari:** `sorafs_orchestrator_policy_events_total` ishdan chiqish ogohlantirishi sozlangan SLO (har qanday 10 daqiqali oynada <5%) ostida qolishini taʼminlang.
- **Hodisalar jurnali:** olingan telemetriya parchalari va operator qaydlarini `docs/examples/soranet_pq_ratchet_fire_drill.log` ga qo'shing.
- **Imzolangan suratga olish:** `cargo xtask soranet-rollout-capture` yordamida matkap jurnali va jadvalni `artifacts/soranet_pq_rollout/<timestamp>/` ga nusxalash, BLAKE3 dayjestlarini hisoblash va imzolangan `rollout_capture.json` yaratish.

Misol:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

Yaratilgan metamaʼlumotlar va imzoni boshqaruv paketiga biriktiring.

## Orqaga qaytarish

Agar matkap haqiqiy PQ tanqisligini aniqlasa, A bosqichida qoling, Networking TL-ga xabar bering va yig'ilgan o'lchovlarni va qo'riqchi katalog farqlarini voqea kuzatuvchisiga biriktiring. Oddiy xizmatni tiklash uchun avval olingan qo'riqchi katalogini eksport qilishdan foydalaning.

:::tip Regressiya qamrovi
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` ushbu matkapni qo'llab-quvvatlovchi sintetik tekshirishni ta'minlaydi.
:::