---
lang: az
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9774dca76eff9ff13fcad9bf1fa7f084b95a987c392727cf0e6a74a4844e2b8e
source_last_modified: "2026-01-22T14:45:01.375247+00:00"
translation_last_reviewed: 2026-02-07
id: pq-rollout-plan
title: SNNet-16G Post-Quantum Rollout Playbook
sidebar_label: PQ Rollout Plan
description: Operational guide for promoting the SoraNet hybrid X25519+ML-KEM handshake from canary to default across relays, clients, and SDKs.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
:::

SNNet-16G SoraNet nəqliyyatı üçün post-kvant buraxılışını tamamlayır. `rollout_phase` düymələri operatorlara hər bir səth üçün xam JSON/TOML redaktə etmədən mövcud Mərhələ A qoruyucu tələbindən Mərhələ B çoxluq əhatəsinə və C Stage C ciddi PQ duruşuna qədər deterministik təşviqi əlaqələndirməyə imkan verir.

Bu oyun kitabı əhatə edir:

- Faza tərifləri və kod bazasında simli yeni konfiqurasiya düymələri (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- SDK və CLI bayraq xəritəsi, beləliklə hər bir müştəri yayımı izləyə bilsin.
- Relay/müştəri kanarya planlaşdırma gözləntiləri üstəgəl, təşviqi təmin edən idarəetmə panelləri (`dashboards/grafana/soranet_pq_ratchet.json`).
- Geri qaytarma qarmaqları və yanğınsöndürmə dəftərinə istinadlar ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Faza xəritəsi

| `rollout_phase` | Effektiv anonimlik mərhələsi | Defolt effekt | Tipik istifadə |
|----------------|--------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Mərhələ A) | Donanma istiləşərkən dövrə başına ən azı bir PQ qoruyucusu tələb edin. | Əsas və erkən kanar həftələr. |
| `ramp` | `anon-majority-pq` (Mərhələ B) | >= əhatə dairəsinin üçdə ikisi üçün PQ relelərinə istiqamətli seçim; klassik relelər ehtiyat kimi qalır. | Regionlar üzrə relay kanareykaları; SDK önizləməsini dəyişdirir. |
| `default` | `anon-strict-pq` (Mərhələ C) | Yalnız PQ sxemlərini tətbiq edin və aşağı səviyyəli siqnalları sıxın. | Telemetriya və idarəetmənin imzalanması tamamlandıqdan sonra son təşviqat. |

Səth həm də açıq-aydın `anonymity_policy` təyin edərsə, o, həmin komponent üçün mərhələni ləğv edir. Açıq mərhələnin buraxılması indi `rollout_phase` dəyərinə uyğundur ki, operatorlar fazanı hər mühitə bir dəfə çevirə və müştərilərə onu miras qoya bilsinlər.

## Konfiqurasiya arayışı

### Orkestr (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Orkestr yükləyicisi iş vaxtında (`crates/sorafs_orchestrator/src/lib.rs:2229`) ehtiyat mərhələsini həll edir və onu `sorafs_orchestrator_policy_events_total` və `sorafs_orchestrator_pq_ratio_*` vasitəsilə üzə çıxarır. Tətbiq etməyə hazır fraqmentlər üçün `docs/examples/sorafs_rollout_stage_b.toml` və `docs/examples/sorafs_rollout_stage_c.toml`-ə baxın.

### Rust müştəri / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` indi təhlil edilmiş fazanı (`crates/iroha/src/client.rs:2315`) qeyd edir, beləliklə, köməkçi əmrlər (məsələn, `iroha_cli app sorafs fetch`) defolt anonimlik siyasəti ilə yanaşı cari mərhələ haqqında məlumat verə bilər.

## Avtomatlaşdırma

İki `cargo xtask` köməkçisi cədvəlin yaradılmasını və artefaktın ələ keçirilməsini avtomatlaşdırır.

1. **Region cədvəlini yaradın**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   Müddətlər `s`, `m`, `h` və ya `d` şəkilçilərini qəbul edir. Komanda dəyişiklik sorğusu ilə göndərilə bilən `artifacts/soranet_pq_rollout_plan.json` və Markdown xülasəsini (`artifacts/soranet_pq_rollout_plan.md`) yayır.

2. **İmzalarla qazma artefaktlarını çəkin**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   Komanda təchiz edilmiş faylları `artifacts/soranet_pq_rollout/<timestamp>_<label>/`-ə köçürür, hər artefakt üçün BLAKE3 həzmlərini hesablayır və metadata və faydalı yükün üzərində Ed25519 imzası olan `rollout_capture.json` yazır. Yanğınsöndürmə məşq protokollarını imzalayan eyni şəxsi açardan istifadə edin ki, idarəetmə ələ keçirməni tez bir zamanda təsdiq etsin.

## SDK və CLI bayraq matrisi

| Səthi | Kanarya (Mərhələ A) | Ramp (B Mərhələsi) | Defolt (Mərhələ C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` gətirin | `--anonymity-policy stage-a` və ya faza | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Orkestr konfiqurasiyası JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust müştəri konfiqurasiyası (`iroha.toml`) | `rollout_phase = "canary"` (standart) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` imzalanmış əmrlər | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, istəyə görə `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, istəyə görə `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, istəyə görə `.ANON_STRICT_PQ` |
| JavaScript orkestr köməkçiləri | `rolloutPhase: "canary"` və ya `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Bütün SDK xəritəni orkestr tərəfindən istifadə edilən eyni mərhələ təhlilçisinə keçir (`crates/sorafs_orchestrator/src/lib.rs:365`), beləliklə, qarışıq dilli yerləşdirmələr konfiqurasiya edilmiş faza ilə kilid addımında qalır.

## Canary planlaşdırma yoxlama siyahısı

1. **Öncə uçuş (T mənfi 2 həftə)**

- Əvvəlki iki həftə ərzində A Mərhələsinin tükənmə dərəcəsini <1% və region üzrə PQ əhatə dairəsini >=70% təsdiqləyin (`sorafs_orchestrator_pq_candidate_ratio`).
   - Kanareyka pəncərəsini təsdiqləyən idarəçiliyin nəzərdən keçirilməsini planlaşdırın.
   - Səhnələşdirmədə `sorafs.gateway.rollout_phase = "ramp"`-i yeniləyin (orkestrator JSON-u redaktə edin və yenidən yerləşdirin) və tanıtım boru kəmərini qurudan idarə edin.

2. **Relay kanareyası (T günü)**

   - `rollout_phase = "ramp"` orkestrində və iştirak edən relay manifestlərində təyin etməklə hər dəfə bir bölgəni təşviq edin.
   - TTL qoruyucu keş keşini iki dəfə artırmaq üçün PQ Ratchet idarə panelində (indi buraxılış paneli var) "Nəticə üzrə Siyasət Hadisələri" və "Brownout Rate" monitorinqini aparın.
   - Audit saxlama üçün işə başlamazdan əvvəl və sonra `sorafs_cli guard-directory fetch` anlıq görüntülərini kəsin.

3. **Müştəri/SDK canary (T plus 1 həftə)**

   - Müştəri konfiqurasiyalarında `rollout_phase = "ramp"`-i çevirin və ya təyin edilmiş SDK kohortları üçün `stage-b` ləğvetmələrini keçin.
   - Telemetriya fərqlərini çəkin (`sorafs_orchestrator_policy_events_total` `client_id` və `region` ilə qruplaşdırılıb) və onları yayılma hadisə jurnalına əlavə edin.

4. **Defolt təşviqat (T plus 3 həftə)**

   - İdarəetmə bağlandıqdan sonra həm orkestr, həm də müştəri konfiqurasiyalarını `rollout_phase = "default"`-ə keçirin və imzalanmış hazırlıq yoxlama siyahısını buraxılış artefaktlarına çevirin.

## İdarəetmə və sübut yoxlama siyahısı

| Faza dəyişikliyi | Tanıtım qapısı | Sübut paketi | Panellər və xəbərdarlıqlar |
|-----------------------|----------------|-----------------|---------------------|
| Kanarya → Ramp *(Mərhələ B ön baxışı)* | Mərhələ - Arxadan gələn 14 gün ərzində aşağı düşmə nisbəti <1%, irəli sürülən bölgə üçün `sorafs_orchestrator_pq_candidate_ratio` ≥ 0,7, Argon2 bileti p95 < 50 ms-ni doğrulayır və rezervasiya edilmiş təşviqat üçün idarəetmə sahəsi. | `cargo xtask soranet-rollout-plan` JSON/Markdown cütü, qoşalaşmış `sorafs_cli guard-directory fetch` snapşotları (əvvəl/sonra), imzalanmış `cargo xtask soranet-rollout-capture --label canary` paketi və [PQ ratchet runbook](I180050000) istinad edən kanareyka dəqiqələri. | `dashboards/grafana/soranet_pq_ratchet.json` (Siyasət hadisələri + Qəzəblənmə dərəcəsi), `dashboards/grafana/soranet_privacy_metrics.json` (SN16 aşağı səviyyəli nisbət), `docs/source/soranet/snnet16_telemetry_plan.md`-də telemetriya istinadları. |
| Ramp → Defolt *(Mərhələ C tətbiqi)* | 30 günlük SN16 telemetriya yanması baş verdi, `sn16_handshake_downgrade_total` ilkin xəttdə, `sorafs_orchestrator_brownouts_total` müştəri kanareyası zamanı sıfır və proksi keçid məşqi qeydə alınıb. | `sorafs_cli proxy set-mode --mode gateway|direct` transkript, `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` çıxışı, `sorafs_cli guard-directory verify` jurnalı və imzalanmış `cargo xtask soranet-rollout-capture --label default` paketi. | Eyni PQ Ratchet lövhəsi və `docs/source/sorafs_orchestrator_rollout.md` və `dashboards/grafana/soranet_privacy_metrics.json`-də sənədləşdirilmiş SN16 aşağı səviyyəli panellər. |
| Fövqəladə vəziyyətə düşmə / geri çəkilməyə hazırlıq | Endirmə sayğacları yüksəldikdə, qoruyucu kataloq yoxlanışı uğursuz olduqda və ya `/policy/proxy-toggle` buferi davamlı endirmə hadisələrini qeyd etdikdə işə salınır. | `docs/source/ops/soranet_transport_rollback.md`, `sorafs_cli guard-directory import` / `guard-cache prune` qeydləri, `cargo xtask soranet-rollout-capture --label rollback`, insident biletləri və bildiriş şablonlarından yoxlama siyahısı. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` və hər iki xəbərdarlıq paketi (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Yaradılmış `rollout_capture.json` ilə hər artefaktı `artifacts/soranet_pq_rollout/<timestamp>_<label>/` altında saxlayın ki, idarəetmə paketlərində skorbord, promtool izləri və həzmlər olsun.
- Yüklənmiş sübutların SHA256 həzmlərini (dəqiqələr PDF, çəkmə paketi, qoruyucu snapshotlar) tanıtım protokollarına əlavə edin ki, Parlamentin təsdiqləmələri səhnələşdirmə klasterinə giriş olmadan təkrar oxuna bilsin.
- `docs/source/soranet/snnet16_telemetry_plan.md`-in aşağı səviyyəli lüğətlər və xəbərdarlıq hədləri üçün kanonik mənbə olaraq qaldığını sübut etmək üçün tanıtım biletindəki telemetriya planına istinad edin.

## İdarə paneli və telemetriya yeniləmələri

`dashboards/grafana/soranet_pq_ratchet.json` indi bu kitabçaya keçid edən və cari mərhələni əks etdirən "Təqdimat Planı" annotasiya paneli ilə göndərilir, beləliklə, idarəetmə rəyləri hansı mərhələnin aktiv olduğunu təsdiq edə bilər. Panel təsvirini konfiqurasiya düymələrində gələcək dəyişikliklərlə sinxronlaşdırın.

Xəbərdarlıq üçün mövcud qaydaların `stage` etiketindən istifadə etdiyinə əmin olun ki, kanareyka və defolt mərhələlər ayrı siyasət hədlərini (`dashboards/alerts/soranet_handshake_rules.yml`) işə salsın.

## Geri dönmə qarmaqları

### Defolt → Ramp (Mərhələ C → Mərhələ B)

1. `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` ilə orkestratoru aşağı salın (və eyni fazanı SDK konfiqurasiyaları arasında əks etdirin) beləliklə, Mərhələ B bütün donanma üzrə davam etsin.
2. `/policy/proxy-toggle` remediasiya iş prosesinin yoxlanıla bilən qalması üçün stenoqramı ələ alaraq `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` vasitəsilə müştəriləri təhlükəsiz nəqliyyat profilinə məcbur edin.
3. `artifacts/soranet_pq_rollout/` altında qoruyucu qovluq fərqlərini, promtool çıxışını və tablosuna ekran görüntülərini arxivləşdirmək üçün `cargo xtask soranet-rollout-capture --label rollback-default`-i işə salın.

### Ramp → Kanarya (Mərhələ B → Mərhələ A)

1. `sorafs_cli guard-directory import --guard-directory guards.json` ilə irəliləmədən əvvəl çəkilmiş qoruyucu kataloq snapşotunu idxal edin və `sorafs_cli guard-directory verify`-i yenidən işə salın ki, demosasiya paketinə hashlar olsun.
2. Orkestr və müştəri konfiqurasiyalarında `rollout_phase = "canary"` (və ya `anonymity_policy stage-a` ilə əvəz edin) təyin edin, sonra aşağı səviyyəli boru xəttini sübut etmək üçün [PQ ratchet runbook](./pq-ratchet-runbook.md)-dən PQ mandallı qazmağı təkrar çalın.
3. İdarəetməni xəbərdar etməzdən əvvəl yenilənmiş PQ Ratchet və SN16 telemetriya skrinşotlarını və xəbərdarlıq nəticələrini hadisə jurnalına əlavə edin.

### Qoruyucu xatırlatmalar- Aşağı düşmə baş verdikdə `docs/source/ops/soranet_transport_rollback.md`-ə istinad edin və hər hansı müvəqqəti yumşaltmanı izləmə işləri üçün yayım izləyicisinə `TODO:` elementi kimi daxil edin.
- `dashboards/alerts/soranet_handshake_rules.yml` və `dashboards/alerts/soranet_privacy_rules.yml`-i geri çəkilmədən əvvəl və sonra `promtool test rules` əhatəsi altında saxlayın ki, siqnalın sürüşməsi tutma dəsti ilə birlikdə sənədləşdirilsin.