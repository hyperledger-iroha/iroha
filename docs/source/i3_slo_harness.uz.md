---
lang: uz
direction: ltr
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2025-12-29T18:16:35.966039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Iroha 3 SLO jabduqlar

Iroha 3 chiqarish liniyasi muhim Nexus yo'llari uchun aniq SLOlarni o'z ichiga oladi:

- yakuniy slot davomiyligi (NX‑18 kadans)
- dalillarni tekshirish (tasdiqlash sertifikatlari, JDG attestatsiyalari, ko'prik dalillari)
- so'nggi nuqtani tekshirish (tasdiqlash kechikishi orqali Axum yo'li proksi-server)
- to'lov va staking yo'llari (to'lovchi/homiy va obligatsiyalar/slash oqimlari)

## Byudjetlar

Byudjetlar `benchmarks/i3/slo_budgets.json` da yashaydi va to'g'ridan-to'g'ri skameykaga ko'rsatiladi
I3 to'plamidagi stsenariylar. Maqsadlar har bir qo'ng'iroq uchun p99 maqsadlari:

- Toʻlov/qoʻngʻiroq uchun 50ms (`fee_payer`, `fee_sponsor`, `staking_bond`, `staking_slash`)
- Sertifikat / JDG / ko'prikni tekshirish: 80ms (`commit_cert_verify`, `jdg_attestation_verify`,
  `bridge_proof_verify`)
- Sertifikat yig'ilishini bajarish: 80ms (`commit_cert_assembly`)
- Kirish rejalashtiruvchisi: 50ms (`access_scheduler`)
- So'nggi nuqta proksi-server: 120ms (`torii_proof_endpoint`)

Yonish tezligi bo'yicha maslahatlar (`burn_rate_fast`/`burn_rate_slow`) 14,4/6,0 ni kodlaydi
peyjing va chipta ogohlantirishlari uchun ko'p oynali nisbatlar.

## Jabduqlar

Jabduqni `cargo xtask i3-slo-harness` orqali ishga tushiring:

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

Chiqishlar:

- `bench_report.json|csv|md` - xom I3 dastgoh to'plami natijalari (git hash + stsenariylar)
- `slo_report.json|md` - maqsad uchun o'tish / muvaffaqiyatsiz / byudjet nisbati bilan SLO baholash

Jabduqlar byudjet faylini iste'mol qiladi va `benchmarks/i3/slo_thresholds.json` ni amalga oshiradi
skameykada yugurish paytida maqsad regressga tushganda tez muvaffaqiyatsiz bo'ladi.

## Telemetriya va asboblar paneli

- Yakuniylik: `histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- Isbotni tekshirish: `histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

Grafana boshlang'ich panellari `dashboards/grafana/i3_slo.json` da yashaydi. Prometheus
kuyish tezligi haqida ogohlantirishlar `dashboards/alerts/i3_slo_burn.yml` da taqdim etiladi
yuqoridagi byudjetlar (yakuniylik 2s, 80ms tasdiqlash, oxirgi nuqta proksi
120ms).

## Operatsion qaydlar

- Kechasi jabduqni yugurish; nashr qilish `artifacts/i3_slo/<stamp>/slo_report.md`
  boshqaruv dalillari uchun dastgoh artefaktlari bilan bir qatorda.
- Agar byudjet muvaffaqiyatsiz bo'lsa, stsenariyni aniqlash uchun dastgoh belgisidan foydalaning, so'ngra matkap qiling
  jonli ko'rsatkichlar bilan bog'lanish uchun mos keladigan Grafana paneli/ogohlantirishga.
- Proof endpoint SLOs har bir marshrutdan qochish uchun proksi sifatida tekshirish kechikishidan foydalanadi
  kardinallik zarbasi; benchmark maqsadi (120ms) saqlash/DoS ga mos keladi
  proof API-dagi himoya panjaralari.