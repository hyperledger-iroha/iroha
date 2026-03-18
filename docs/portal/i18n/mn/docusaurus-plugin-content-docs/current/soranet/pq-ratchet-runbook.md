---
id: pq-ratchet-runbook
lang: mn
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet PQ Ratchet Fire Drill
sidebar_label: PQ Ratchet Runbook
description: On-call rehearsal steps for promoting or demoting the staged PQ anonymity policy with deterministic telemetry validation.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Каноник эх сурвалжийг анхаарна уу
:::

## Зорилго

Энэхүү runbook нь SoraNet-ийн үе шаттай дараах квант (PQ) нэрээ нууцлах бодлогын галын өрөмдлөгийн дарааллыг удирдан чиглүүлдэг. Операторууд PQ нийлүүлэлт буурах үед албан тушаал ахих (A-үе шат -> В шат -> С шат) болон хяналттай бууралтыг B/A шат руу буцаах бэлтгэлийг хийдэг. Өрөмдлөг нь телеметрийн дэгээг (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) баталгаажуулж, ослын давталтын бүртгэлд зориулж олдворуудыг цуглуулдаг.

## Урьдчилсан нөхцөл

- Чадварыг жинлэх хамгийн сүүлийн үеийн `sorafs_orchestrator` хоёртын хувилбар (`docs/source/soranet/reports/pq_ratchet_validation.md`-д үзүүлсэн өрөмдлөгийн лавлагааны дараа эсвэл дараа нь хийх).
- `dashboards/grafana/soranet_pq_ratchet.json`-д үйлчлэх Prometheus/Grafana стек рүү нэвтрэх.
- Нэрлэсэн хамгаалалтын лавлахын хормын хувилбар. Дасгал хийхээс өмнө хуулбарыг авч, баталгаажуулна уу:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Хэрэв эх лавлах нь зөвхөн JSON-г нийтэлдэг бол эргүүлэх туслахуудыг ажиллуулахын өмнө үүнийг Norito хоёртын систем болгон `soranet-directory build`-ээр дахин кодчил.

- CLI ашиглан мета өгөгдөл болон гаргагчийн эргэлтийн өмнөх үе шатыг олж авах:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Сүлжээний болон ажиглалтын багуудаар батлагдсан өөрчлөлтийн цонх.

## Сурталчилгааны алхамууд

1. **Үе шатны аудит**

   Эхлэх үе шатыг тэмдэглэ:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Урамшууллын өмнө `anon-guard-pq` хүлээж байна.

2. **В шат (Majority PQ)-д дэвших**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Манифестийг сэргээх хүртэл >=5 минут хүлээнэ үү.
   - Grafana (`SoraNet PQ Ratchet Drill` хяналтын самбар) "Бодлогын үйл явдал" самбарт `stage=anon-majority-pq`-д зориулсан `outcome=met`-г харуулна.
   - Дэлгэцийн агшин эсвэл JSON самбарыг авч, тохиолдлын бүртгэлд хавсаргана уу.

3. **С шат руу ахиулна (Strict PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - `sorafs_orchestrator_pq_ratio_*` гистограммуудын чиг хандлагыг 1.0 болгож шалгана уу.
   - Борлуулалтын тоолуур хавтгай хэвээр байгааг баталгаажуулах; эс бөгөөс доошилсон алхмуудыг дагана уу.

## Бууруулах / борлуулах өрөм

1. **Синтетик PQ дутагдлыг өдөөх**

   Тоглоомын талбайн орчинд PQ релейг идэвхгүй болгоод хамгаалалтын лавлахыг зөвхөн сонгодог оруулга болгон тохируулж, дараа нь найруулагчийн кэшийг дахин ачаална уу:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Браунтын телеметрийг ажиглах**

   - Хяналтын самбар: "Brownout Rate" самбар 0-ээс дээш өснө.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` `anonymity_outcome="brownout"`-г `anonymity_reason="missing_majority_pq"`-ээр мэдээлэх ёстой.

3. **Б шат / А шат руу доошилно**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Хэрэв PQ нийлүүлэлт хангалтгүй хэвээр байвал `anon-guard-pq` хүртэл бууруулна уу. Борлуулалтын тоолуур тогтсоны дараа сургалт дуусч, урамшууллыг дахин ашиглах боломжтой.

4. **Хамгаалалтын лавлахыг сэргээх**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Телеметр ба олдворууд

- **Хяналтын самбар:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus сэрэмжлүүлэг:** `sorafs_orchestrator_policy_events_total`-ийн дохиолол нь тохируулсан SLO-ээс доогуур байгаа эсэхийг баталгаажуулна уу (ямар ч 10 минутын цонхонд <5%).
- **Ойл явдлын бүртгэл:** авсан телеметрийн хэсэг болон операторын тэмдэглэлийг `docs/examples/soranet_pq_ratchet_fire_drill.log`-д нэмнэ үү.
- **Гарын үсэг зурсан зураг авалт:** `cargo xtask soranet-rollout-capture` ашиглан өрмийн бүртгэл болон онооны самбарыг `artifacts/soranet_pq_rollout/<timestamp>/` руу хуулж, BLAKE3 боловсруулалтыг тооцоолж, `rollout_capture.json` гарын үсэг зурна.

Жишээ:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

Үүсгэсэн мета өгөгдөл болон гарын үсгийг удирдлагын багцад хавсаргана уу.

## Буцах

Хэрэв дасгал сургуулилт нь бодит PQ дутагдлыг илрүүлбэл, А шатанд үлдэж, Сүлжээний TL-д мэдэгдэж, цуглуулсан хэмжигдэхүүнүүд болон хамгаалалтын лавлахын ялгааг осол бүртгэгч рүү хавсаргана уу. Хэвийн үйлчилгээг сэргээхийн тулд өмнө нь авсан хамгаалалтын лавлах экспортыг ашиглана уу.

:::tip Регрессийн хамрах хүрээ
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` нь энэхүү өрөмдлөгийн нийлэг баталгаажуулалтыг өгдөг.
:::