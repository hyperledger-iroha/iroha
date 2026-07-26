---
lang: az
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

:::Qeyd Kanonik Mənbə
:::

## Məqsəd

Bu runbook SoraNet-in mərhələli post-kvant (PQ) anonimlik siyasəti üçün yanğın-qazma ardıcıllığına bələdçilik edir. Operatorlar həm yüksəlişi (Mərhələ A -> Mərhələ B -> Mərhələ C), həm də PQ təchizatı azaldıqda B/A Mərhələsinə nəzarət edilən endirməni məşq edir. Qazma telemetriya qarmaqlarını (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) doğrulayır və hadisənin məşq jurnalı üçün artefaktları toplayır.

## İlkin şərtlər

- Qabiliyyət çəkisi ilə ən son `sorafs_orchestrator` binar (`docs/source/soranet/reports/pq_ratchet_validation.md`-də göstərilən qazma istinadında və ya ondan sonra yerinə yetirin).
- `dashboards/grafana/soranet_pq_ratchet.json` xidmət göstərən Prometheus/Grafana yığınına giriş.
- Nominal qoruyucu kataloq snapshot. Təlimdən əvvəl bir nüsxə götürün və yoxlayın:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Mənbə kataloqu yalnız JSON-u dərc edirsə, fırlanma köməkçilərini işə salmazdan əvvəl onu Norito ikili formatına `soranet-directory build` ilə yenidən kodlayın.

- CLI ilə metadata və mərhələ öncəsi emitent fırlanma artefaktlarını çəkin:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Şəbəkə və müşahidə edilə bilən çağırış qrupları tərəfindən təsdiqlənmiş pəncərəni dəyişdirin.

## Tanıtım addımları

1. **Mərhələli audit**

   Başlanğıc mərhələsini qeyd edin:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Təqdimatdan əvvəl `anon-guard-pq` gözləyin.

2. **B Mərhələsinə yüksəldin (Əksəriyyət PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Manifestlərin təzələnməsi üçün >=5 dəqiqə gözləyin.
   - Grafana (`SoraNet PQ Ratchet Drill` idarə panelində) "Siyasət hadisələri" panelində `stage=anon-majority-pq` üçün `outcome=met` göstərilir.
   - Ekran görüntüsünü və ya JSON panelini çəkin və hadisə jurnalına əlavə edin.

3. **Mərhələ C (Strict PQ)-a yüksəldin**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - `sorafs_orchestrator_pq_ratio_*` histoqramlarının 1.0 tendensiyasını yoxlayın.
   - Qaranlıq sayğacın düz qaldığını təsdiqləyin; əks halda aşağı salınma addımlarını izləyin.

## İşdən çıxarma / işdən çıxarma

1. **Sintetik PQ çatışmazlığına səbəb olun**

   Mühafizə qovluğunu yalnız klassik girişlərə kəsərək oyun meydançasında PQ relelərini söndürün, sonra orkestratorun keşini yenidən yükləyin:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Böyük telemetriyanı müşahidə edin**

   - İdarə paneli: "Brownout Rate" paneli 0-dan yuxarı qalxır.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` `anonymity_outcome="brownout"` ilə `anonymity_reason="missing_majority_pq"` məlumat verməlidir.

3. **Mərhələ B / Mərhələ A-ya endirin**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   PQ təchizatı hələ də kifayət deyilsə, `anon-guard-pq` səviyyəsinə endirin. Qazma sayğacları yerləşdikdən və təşviqlər yenidən tətbiq oluna bildikdən sonra məşq tamamlanır.

4. **Mühafizə kataloqunu bərpa edin**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetriya və artefaktlar

- ** İdarə paneli:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus xəbərdarlıqları:** `sorafs_orchestrator_policy_events_total` qaralma xəbərdarlığının konfiqurasiya edilmiş SLO-nun altında qalmasını təmin edin (hər hansı 10 dəqiqəlik pəncərədə <5%).
- **Hadisə jurnalı:** çəkilmiş telemetriya parçaları və operator qeydlərini `docs/examples/soranet_pq_ratchet_fire_drill.log`-ə əlavə edin.
- **İmzalı tutma:** `cargo xtask soranet-rollout-capture`-dən istifadə edərək qazma jurnalını və tablosunu `artifacts/soranet_pq_rollout/<timestamp>/`-ə köçürün, BLAKE3 həzmlərini hesablayın və imzalanmış `rollout_capture.json` çıxarın.

Misal:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

Yaradılmış metadata və imzanı idarəetmə paketinə əlavə edin.

## Geriyə qayıt

Əgər məşq real PQ çatışmazlıqlarını aşkar edərsə, A Mərhələsində qalın, Şəbəkə TL-ni xəbərdar edin və toplanmış ölçüləri və mühafizə kataloqu fərqlərini hadisə izləyicisinə əlavə edin. Normal xidməti bərpa etmək üçün əvvəllər çəkilmiş qoruyucu kataloq ixracından istifadə edin.

:::tip Reqressiya Əhatəsi
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` bu qazmağı dəstəkləyən sintetik doğrulama təmin edir.
:::