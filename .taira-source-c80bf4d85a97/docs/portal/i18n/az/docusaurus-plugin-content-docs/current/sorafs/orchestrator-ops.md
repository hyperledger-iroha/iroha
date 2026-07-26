---
id: orchestrator-ops
lang: az
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Orchestrator Operations Runbook
sidebar_label: Orchestrator Ops Runbook
description: Step-by-step operational guide for rolling out, monitoring, and rolling back the multi-source orchestrator.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
:::

Bu runbook çox mənbəli gətirmə orkestratorunu hazırlamaq, yaymaq və işlətməklə SRE-lərə rəhbərlik edir. O, inkişaf etdirici bələdçisini mərhələli aktivləşdirmə və həmyaşıdların qara siyahısı daxil olmaqla istehsalın buraxılması üçün tənzimlənmiş prosedurlarla tamamlayır.

> **Həmçinin bax:** [Multi-Source Rollout Runbook](./multi-source-rollout.md) donanmanın geniş yayılması dalğalarına və fövqəladə hallar təminatçısının rədd edilməsinə diqqət yetirir. Gündəlik orkestr əməliyyatları üçün bu sənəddən istifadə edərkən idarəetmə / səhnələşdirmə koordinasiyası üçün ona istinad edin.

## 1. Uçuşdan əvvəl Yoxlama Siyahısı

1. **Provayder daxiletmələrini toplayın**
   - Ən son provayder reklamları (`ProviderAdvertV1`) və hədəf donanması üçün telemetriya şəkli.
   - Test edilən manifestdən əldə edilən faydalı yük planı (`plan.json`).
2. **Deterministik hesab tablosunu göstərin**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - `artifacts/scoreboard.json`-in hər bir istehsal provayderini `eligible` olaraq siyahıya aldığını təsdiqləyin.
   - Xülasə JSON-u tablonun yanında arxivləşdirin; auditorlar dəyişiklik sorğusunu təsdiq edərkən yığın təkrar sınaq sayğaclarına etibar edirlər.
3. **Aparatlarla quru qaçış** — İstehsal yüklərinə toxunmazdan əvvəl orkestrator binarının gözlənilən versiyaya uyğun olmasını təmin etmək üçün `docs/examples/sorafs_ci_sample/`-də ictimai qurğulara qarşı eyni əmri yerinə yetirin.

## 2. Mərhələli Yayılma Proseduru

1. **Kanar mərhələsi (≤2 provayder)**
   - Hesab tablosunu yenidən qurun və orkestratoru kiçik alt çoxluğa bağlamaq üçün `--max-peers=2` ilə işləyin.
   - Monitor:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - Tam manifest əldə etmək üçün təkrar cəhd dərəcələri 1%-dən aşağı qaldıqdan sonra davam edin və heç bir provayder uğursuzluqlar toplamır.
2. **Eniş mərhələsi (50% təminatçılar)**
   - `--max-peers`-i artırın və yeni telemetriya snapshot ilə təkrar işə salın.
   - `--provider-metrics-out` və `--chunk-receipts-out` ilə hər qaçışda davam edin. Artefaktları ≥7 gün saxlayın.
3. **Tam təqdimat**
   - `--max-peers`-i çıxarın (və ya tam uyğun sayı təyin edin).
   - Müştəri yerləşdirmələrində orkestr rejimini aktivləşdirin: konfiqurasiya idarəetmə sisteminiz vasitəsilə davamlı hesab tablosunu və konfiqurasiya JSON-u paylayın.
   - `sorafs_orchestrator_fetch_duration_ms` p95/p99-u göstərmək üçün idarə panellərini yeniləyin və hər bölgə üzrə histoqramları yenidən sınayın.

## 3. Peer Blacklisting & Boosting

İdarəetmə yeniləmələrini gözləmədən qeyri-sağlam provayderləri yoxlamaq üçün CLI-nin qiymətləndirmə siyasətindən istifadə edin.

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```

- `--deny-provider` sadalanan ləqəbi cari sessiya üçün nəzərdən silir.
- `--boost-provider=<alias>=<weight>` provayderin planlaşdırıcı çəkisini artırır. Dəyərlər normallaşdırılmış cədvəl çəkisinə əlavədir və yalnız yerli qaçışa aiddir.
- Hadisə biletində ləğvetmələri qeyd edin və JSON çıxışlarını əlavə edin ki, əsas problem həll edildikdən sonra sahibi komanda vəziyyəti barışdırsın.

Daimi dəyişikliklər üçün mənbə telemetriyasını dəyişdirin (cinayətkarın cəzalandırıldığını qeyd edin) və ya CLI ləğvetmələrini silməzdən əvvəl reklamı yenilənmiş axın büdcələri ilə yeniləyin.

## 4. Uğursuzluq Triajı

Alma uğursuz olduqda:

1. Yenidən işə başlamazdan əvvəl aşağıdakı artefaktları çəkin:
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. `session.summary.json`-də insan tərəfindən oxuna bilən xəta sətrini yoxlayın:
   - `no providers were supplied` → provayder yollarını və reklamlarını yoxlayın.
   - `retry budget exhausted ...` → `--retry-budget` artırın və ya qeyri-sabit həmyaşıdları çıxarın.
   - `no compatible providers available ...` → pozan provayderin diapazon imkanları metadatasını yoxlayın.
3. Provayder adını `sorafs_orchestrator_provider_failures_total` ilə əlaqələndirin və metrik sıçrayışlar olarsa, izləmə bileti yaradın.
4. Uğursuzluğu deterministik şəkildə təkrar etmək üçün `--scoreboard-json` və çəkilmiş telemetriya ilə gətirməni oflayn rejimdə təkrarlayın.

## 5. Geriyə qayıt

Orkestratorun təqdimatını geri qaytarmaq üçün:

2. Hesab lövhəsinin neytral çəkiyə qayıtması üçün hər hansı `--boost-provider` ləğvini silin.
3. Uçuşda heç bir qalıq gətirmə olmadığını təsdiq etmək üçün ən azı bir gün orkestr göstəricilərini silməyə davam edin.

Artefaktların nizam-intizamlı tutulması və mərhələli buraxılışların saxlanması çoxmənbəli orkestratorun müşahidə qabiliyyətini və audit tələblərini toxunulmaz saxlayaraq, heterojen provayder donanmalarında təhlükəsiz şəkildə idarə olunmasını təmin edir.