---
lang: az
direction: ltr
source: docs/source/ministry/transparency_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2639f4f7692e13ed61cc6246c87b047dc7415a6c9243ca7c046e6ccea8b55e9a
source_last_modified: "2025-12-29T18:16:35.983537+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Transparency & Audit Plan
summary: Implementation plan for roadmap item MINFO-8 covering quarterly transparency reports, privacy guardrails, dashboards, and automation.
translator: machine-google-reviewed
---

# Şəffaflıq və Audit Hesabatları (MINFO-8)

Yol xəritəsi arayışı: **MINFO-8 — Şəffaflıq və audit hesabatları** və **MINFO-8a — Məxfiliyi qoruyan buraxılış prosesi**

İnformasiya Nazirliyi deterministik şəffaflıq artefaktlarını dərc etməlidir ki, icma moderasiyanın effektivliyini, müraciətlərin idarə edilməsini və qara siyahının pozulmasını yoxlaya bilsin. Bu sənəd MINFO-8-i Q32026 hədəfindən əvvəl bağlamaq üçün tələb olunan əhatə dairəsini, artefaktları, məxfiliyə nəzarəti və əməliyyat iş axınını müəyyən edir.

## Məqsədlər və Nəticələr

- Süni intellekt moderasiyasının dəqiqliyini, müraciət nəticələrini, rədd cavabını, könüllülər panelinin fəaliyyətini və MINFO büdcələri ilə əlaqəli xəzinə hərəkətlərini ümumiləşdirən rüblük şəffaflıq paketləri hazırlayın.
- Xam məlumat paketlərini (Norito JSON + CSV) və idarə panellərini müşayiət edin ki, vətəndaşlar statik PDF-ləri gözləmədən ölçüləri kəsə bilsinlər.
- Hər hansı məlumat toplusu dərc edilməzdən əvvəl məxfilik zəmanətlərini (diferensial məxfilik + minimum hesablama qaydaları) və imzalanmış sertifikatları tətbiq edin.
- Hər bir nəşri DAG və SoraFS idarəetmə sənədlərində qeyd edin ki, tarixi əsərlər dəyişməz və müstəqil olaraq yoxlanıla bilsin.

### Artefakt matrisi

| Artefakt | Təsvir | Format | Saxlama |
|----------|-------------|--------|---------|
| Şəffaflıq xülasəsi | İcra xülasəsi, diqqət çəkən məqamlar, risk maddələri ilə insan tərəfindən oxuna bilən hesabat | Markdown → PDF | `docs/source/ministry/reports/<YYYY-Q>.md` → `artifacts/ministry/transparency/<YYYY-Q>/summary.pdf` |
| Məlumat əlavəsi | Canonical Norito paketi təmizlənmiş masalarla (`ModerationLedgerBlockV1`, müraciətlər, qara siyahı deltaları) | `.norito` + `.json` | `artifacts/ministry/transparency/<YYYY-Q>/data` (SoraFS CID-ə əks olunur) |
| Metrik CSV-lər | İdarə panelləri üçün rahatlıq CSV ixracı (AI FP/FN, müraciət SLA, rədd cavabı) | `.csv` | Eyni kataloq, hashed və imzalanmış |
| İdarə panelinin şəkli | `ministry_transparency_overview` panellərinin Grafana JSON ixracı + xəbərdarlıq qaydaları | `.json` | `dashboards/grafana/ministry_transparency_overview.json` / `dashboards/alerts/ministry_transparency_rules.yml` |
| Provenance manifest | Norito manifest bağlama həzmləri, SoraFS CID, imzalar, buraxılış vaxt damğası | `.json` + ayrılmış imza | `artifacts/ministry/transparency/<YYYY-Q>/manifest.json(.sig)` (həmçinin idarəetmə səsinə əlavə olunur) |

## Məlumat Mənbələri və Boru Kəməri

| Mənbə | Yem | Qeydlər |
|--------|------|-------|
| Moderasiya kitabçası (`docs/source/sorafs_transparency_plan.md`) | CAR fayllarında saxlanılan saatlıq `ModerationLedgerBlockV1` ixracı | Artıq SFM-4c üçün yaşayır; rüblük toplama üçün təkrar istifadə olunur. |
| AI kalibrləmə + yanlış müsbət dərəcələr | `docs/source/sorafs_ai_moderation_plan.md` qurğular + kalibrləmə manifestləri (`docs/examples/ai_moderation_calibration_*.json`) | Siyasət, region və model profili üzrə ümumiləşdirilmiş ölçülər. |
| Apellyasiya reyestri | MINFO-7 xəzinə alətləri tərəfindən yayılan Norito `AppealCaseV1` hadisələri | Tərkibində pay köçürmələri, panel siyahısı, SLA taymerləri var. |
| İnkar edənlər | Merkle reyestrindən `MinistryDenylistChangeV1` hadisələri (MINFO-6) | Hash ailələri, TTL, təcili canon bayraqları daxildir. |
| Xəzinə axını | `MinistryTreasuryTransferV1` hadisələri (apellyasiya depozitləri, panel mükafatları) | `finance/mminfo_gl.csv`-ə qarşı balanslaşdırılmışdır. |Fövqəladə halların idarə edilməsi, TTL limitləri və nəzərdən keçirmə tələbləri artıq mövcuddur
[`docs/source/ministry/emergency_canon_policy.md`](emergency_canon_policy.md), təmin edilməsi
qaçırma metriklərinin səviyyəni tutduğunu (`standard`, `emergency`, `permanent`), canon id,
və Torii-in yükləmə zamanı tətbiq etdiyi son tarixləri nəzərdən keçirin.

Emal mərhələləri:
1. **İngest** xam hadisələri `ministry_transparency_ingest` (şəffaflıq kitabçası qəbul edəni əks etdirən Rust xidməti). Gecələr işləyir, gücsüzdür.
2. `ministry_transparency_builder` ilə rüb üzrə **Aqreqat**. Məxfilik filtrlərindən əvvəl Norito məlumat əlavəsini və hər metrik cədvəli çıxarır.
3. `cargo xtask ministry-transparency sanitize` (və ya `scripts/ministry/dp_sanitizer.py`) vasitəsilə **Sanitarizasiya** ölçülərini həyata keçirin və metadata ilə CSV/JSON dilimlərini buraxın.
4. **Paket** artefaktları, onları `ministry_release_signer` ilə imzalayın və SoraFS + idarəetmə DAG-a yükləyin.

## 2026-3-cü rüb İstinad Buraxılışı

- İlk idarəetmə qapalı paketi (2026-3-cü rüb) 2026-10-07-də `make check-ministry-transparency` vasitəsilə istehsal edilmişdir. Artefaktlar `artifacts/ministry/transparency/2026-Q3/`-də yaşayır, o cümlədən `sanitized_metrics.json`, `dp_report.json`, `summary.md`, `checksums.sha256`, `transparency_manifest.json` və I1060X və güzgüyə çevrildi. SoraFS CID `7f4c2d81a6b13579ccddeeff00112233`.
- Nəşr təfərrüatları, ölçülər cədvəlləri və təsdiqlər `docs/source/ministry/reports/2026-Q3.md`-də ələ keçirilir ki, bu da indi Q3 pəncərəsini nəzərdən keçirən auditorlar üçün kanonik istinad kimi xidmət edir.
- CI buraxılışlar səhnələşdirməni tərk etməzdən əvvəl `make check-ministry-transparency` tətbiq edir, artefakt həzmlərini, Grafana/xəbərdarlıq heşlərini və manifest metadatasını yoxlayır, beləliklə hər gələcək rüb eyni sübut izini izləyir.

## Metriklər və İdarə Panelləri

Grafana idarə paneli (`dashboards/grafana/ministry_transparency_overview.json`) aşağıdakı panelləri nümayiş etdirir:

- AI moderasiya dəqiqliyi: hər model üçün FP/FN dərəcəsi, sürüşmə və kalibrləmə hədəfi və `docs/source/sorafs/reports/ai_moderation_calibration_*.md` ilə əlaqəli xəbərdarlıq hədləri.
- Müraciətin həyat dövrü: təqdimatlar, SLA uyğunluğu, geri çevrilmələr, istiqrazların yanması, hər səviyyə üzrə geriləmə.
- İnkarçı çaxnaşma: hər hash ailəsi üçün əlavələr/çıxarılmalar, TTL müddətinin bitməsi, təcili qaydalara çağırışlar.
- Könüllü brifinqləri və panel müxtəlifliyi: hər dil üzrə təqdimatlar, maraqların toqquşması ilə bağlı açıqlamalar, nəşrin gecikməsi. Balanslaşdırılmış qısa sahələr `docs/source/ministry/volunteer_brief_template.md`-də göstərilib, fakt cədvəllərinin və moderasiya teqlərinin maşın oxunaqlı olmasını təmin edir.
- Xəzinədarlıq qalıqları: depozitlər, ödənişlər, ödənilməmiş öhdəlik (MINFO-7-ni qidalandırır).

Xəbərdarlıq qaydaları (`dashboards/alerts/ministry_transparency_rules.yml`-də kodlaşdırılmış) aşağıdakıları əhatə edir:
- FP/FN sapması kalibrləmə bazası ilə müqayisədə >25%.
- Müraciət SLA buraxılış dərəcəsi rübdə >5%.
- Siyasətdən daha köhnə təcili canon TTL-lər.
- Rüb bağlandıqdan sonra nəşrin gecikməsi >14 gün.

## Məxfilik və Buraxılış Qoruyucuları (MINFO-8a)| Metrik sinif | Mexanizm | Parametrlər | Əlavə mühafizəçilər |
|-------------|-----------|------------|-------------------|
| Saylar (müraciətlər, qara siyahıya dəyişikliklər, könüllü brifinqlər) | Laplas səs-küyü | ε=0,75 rüb, δ=1e-6 | Səs-küydən sonrakı dəyəri <5 olan vedrələri sıxışdırın; rübdə aktyor başına 1 klip töhfəsi. |
| AI dəqiqliyi | Numerator/məxrəcdə Qauss səs-küyü | ε=0,5, δ=1e-6 | Yalnız təmizlənmiş nümunə sayı ≥50 (`min_accuracy_samples` mərtəbəsi) olduqda buraxın və etimad intervalını dərc edin. |
| Xəzinə axını | Səs-küy yoxdur (artıq ictimai zəncirdə) | — | Xəzinə şəxsiyyət vəsiqələri istisna olmaqla, maska ​​adları; Merkle sübutları daxildir. |

Buraxılış tələbləri:
- Diferensial məxfilik hesabatlarına epsilon/delta kitabçası və RNG toxum öhdəliyi (`blake3(seed)`) daxildir.
- Həssas nümunələr (dəlil hashləri) artıq ictimai Merkle qəbzləri olmadığı təqdirdə redaktə edilmişdir.
- Bütün silinmiş sahələri və əsaslandırmanı təsvir edən xülasəyə əlavə edilmiş redaksiya jurnalı.

## Nəşriyyat İş Akışı və Xronologiya

| T-Pəncərə | Tapşırıq | Sahib(lər) | Sübut |
|----------|------|----------|----------|
| Rüb bağlandıqdan sonra T+3d | Xam ixracı dondurun, toplama işini işə salın | Nazirliyin Müşahidə Edilməsi TL | `ministry_transparency_ingest.log`, boru kəməri işinin ID-si |
| T+7d | Xam ölçüləri nəzərdən keçirin, DP sanitizer quru run | Data Trust komandası | Sanitizer hesabatı (`artifacts/.../dp_report.json`) |
| T+10d | Qaralama xülasəsi + məlumat əlavəsi | Sənədlər/DevRel + Siyasət analitiki | `docs/source/ministry/reports/<YYYY-Q>.md` |
| T+12d | Artefaktları imzalayın, manifest hazırlayın, SoraFS | ünvanına yükləyin Əməliyyatlar / İdarəetmə Katibliyi | `manifest.json(.sig)`, SoraFS CID |
| T+14d | İdarəetmə panelləri + xəbərdarlıqları, idarəetmədən sonrakı elanı dərc edin | Müşahidə qabiliyyəti + Əlaqələr | Grafana ixrac, xəbərdarlıq qayda hash, idarəetmə səs keçidi |

Hər buraxılış aşağıdakılar tərəfindən təsdiqlənməlidir:
1. Nazirliyin Müşahidə Edilməsi TL (məlumat bütövlüyü)
2. İdarəetmə Şurası ilə əlaqə (siyasət)
3. Sənədlər/Comms aparıcısı (ictimai ifadə)

## Avtomatlaşdırma və Sübut Saxlama- Xam lentlərdən rüblük snapshot yaratmaq üçün `cargo xtask ministry-transparency ingest` istifadə edin (mühasibat kitabçası, müraciətlər, rədd siyahısı, xəzinədarlıq, könüllü). `cargo xtask ministry-transparency build` ilə izləyin və dərc etməzdən əvvəl idarə paneli ölçülərini JSON və imzalanmış manifestləri buraxın.
- Qırmızı komanda əlaqəsi: bir və ya daha çox `--red-team-report docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` faylını qəbul etmə mərhələsinə ötürün ki, şəffaflıq snapshot və sanitarlaşdırılmış ölçülər jurnal/müraciət/inkar siyahısı məlumatı ilə yanaşı qazma identifikatorları, ssenari sinifləri, sübut paketi yolları və idarə paneli SHA-ları daşısın. Bu, hər bir şəffaflıq paketində əl ilə redaktə edilmədən əks olunan MINFO-9 qazma kadansını saxlayır.
- Könüllü təqdimatlar `docs/source/ministry/volunteer_brief_template.md`-ə uyğun olmalıdır (məsələn: `docs/examples/ministry/volunteer_brief_template.json`). Alma addımı həmin obyektlərin JSON massivini gözləyir, `moderation.off_topic` daxiletmələrini avtomatik süzgəcdən keçirir, açıqlama sertifikatlarını tətbiq edir və fakt cədvəlinin əhatə dairəsini qeyd edir ki, idarə panelləri çatışmayan sitatları vurğulaya bilsin.
- Əlavə avtomatlaşdırma `scripts/ministry/` altında yaşayır. `dp_sanitizer.py` `cargo xtask ministry-transparency sanitize` əmrini əhatə edir, `transparency_release.py` (mənbə alətləri ilə yanaşı əlavə olunur) isə artefaktları paketləyir, SoraFS CID-ni Norito və ya izahatdan əldə edir `--sorafs-cid`) və həm `transparency_manifest.json`, həm də `transparency_release_action.json` (manifest digesti, SoraFS CID-i və tablosunu tutan `TransparencyReleaseV1` idarəetmə yükü) yazır. Norito faydalı yükünü kodlaşdırmaq üçün `--governance-dir <path>`-i `transparency_release.py`-ə ötürün (və ya `cargo xtask ministry-transparency anchor --action artifacts/.../transparency_release_action.json --governance-dir <path>`-i işə salın) və dərc etməzdən əvvəl onu (üstəlik JSON xülasəsi) idarəetmə DAG kataloquna buraxın. Eyni bayraq həmçinin rüb, SoraFS CID, manifest yolları və IPNS açar ləqəbini (`--ipns-key` vasitəsilə ləğv edilə bilər) istinad edən `<governance-dir>/publisher/head_requests/ministry_transparency/` altında `MinistryTransparencyHeadUpdateV1` sorğusu verir. `--auto-head-update`-ni `publisher_head_updater.py` vasitəsilə dərhal həmin sorğunu emal etmək üçün təmin edin, isteğe bağlı olaraq IPNS-nin eyni zamanda dərc edilməsi lazım olduqda `--head-update-ipns-template '/usr/local/bin/ipfs name publish --key {ipns_key} /ipfs/{cid}'`-i keçin. Əks halda, növbəni boşaltmaq, `publisher/head_updates.log` əlavə etmək, `publisher/ipns_heads/<key>.json`-i yeniləmək və işlənmiş JSON-u `head_requests/ministry_transparency/processed/` altında arxivləşdirmək üçün daha sonra `scripts/ministry/publisher_head_updater.py --governance-dir <path>` proqramını işə salın (lazım olduqda eyni şablonla).
- Buraxılış açarı ilə imzalanmış `checksums.sha256` faylı ilə `artifacts/ministry/transparency/<YYYY-Q>/` altında saxlanılan artefaktlar. Ağac indi `artifacts/ministry/transparency/2026-Q3/`-də istinad paketi daşıyır (sanitarlaşdırılmış ölçülər, DP hesabatı, xülasə, manifest, idarəetmə fəaliyyəti) beləliklə mühəndislər alətləri oflayn olaraq sınaqdan keçirə bilsinlər və `scripts/ministry/check_transparency_release.py` yerli olaraq həzmləri/rüb metaməlumatlarını yoxlayır, Grafana-də isə eyni sübutlar etibarlıdır. yüklənmişdir. Yoxlayıcı indi sənədləşdirilmiş DP büdcələrini tətbiq edir (hesablar üçün ε≤0,75, dəqiqlik üçün ε≤0,5, δ≤1e−6) və `min_accuracy_samples` və ya bastırma həddinin sürüşməsi və ya bu mərtəbədən aşağı qiymət sızması zamanı qurma uğursuz olur. Skripti yol xəritəsi (MINFO‑8) və CI arasında müqavilə kimi qəbul edin: məxfilik parametrləri nə vaxtsa dəyişərsə, həm yuxarıdakı cədvəli, həm də yoxlayıcını birlikdə tənzimləyin.- İdarəetmə lövbəri: manifest həzminə, SoraFS CID-ə və idarə panelinə git SHA (`iroha_data_model::ministry::TransparencyReleaseV1` kanonik faydalı yükü müəyyən edir) istinad edərək `TransparencyReleaseV1` əməliyyatı yaradın.

## Tapşırıqları və Növbəti Addımları açın

| Tapşırıq | Status | Qeydlər |
|------|--------|-------|
| `ministry_transparency_ingest` + inşaatçı işlərini həyata keçirin | 🈺 Davam edir | `cargo xtask ministry-transparency ingest|build` indi kitab/apellyasiya/inkar siyahısı/xəzinədarlıq sənədlərini tikir; qalan iş telləri DP sanitizer + buraxılış skript boru kəməri. |
| Grafana tablosunu + xəbərdarlıq paketini dərc edin | 🈴 Tamamlandı | Dashboard + xəbərdarlıq faylları `dashboards/grafana/ministry_transparency_overview.json` və `dashboards/alerts/ministry_transparency_rules.yml` altında yaşayır; buraxılış zamanı onları PagerDuty `ministry-transparency`-ə bağlayın. |
| DP təmizləyicisini avtomatlaşdırın + mənşəli manifest | 🈴 Tamamlandı | `cargo xtask ministry-transparency sanitize` (qablaşdırma: `scripts/ministry/dp_sanitizer.py`) sanitarlaşdırılmış göstəricilər + DP hesabatı verir və `scripts/ministry/transparency_release.py` indi mənşəyə görə `checksums.sha256` və `transparency_manifest.json` yazır. |
| Rüblük hesabat şablonu yaradın (`reports/<YYYY-Q>.md`) | 🈴 Tamamlandı | Şablon əlavə edildi `docs/source/ministry/reports/2026-Q3-template.md`; rübdə kopyalayın/adını dəyişin və dərc etməzdən əvvəl `{{...}}` nişanlarını dəyişdirin. |
| Tel idarəetmə DAG anker | 🈴 Tamamlandı | `TransparencyReleaseV1` `iroha_data_model::ministry`-də yaşayır, `scripts/ministry/transparency_release.py` JSON yükünü yayır və `cargo xtask ministry-transparency anchor` `.to` artefaktını konfiqurasiya edilmiş idarəetmə DAG direktoruna avtomatik olaraq kodlaşdırır. |

Sənədin, tablosunun xüsusiyyətlərinin və iş axınının çatdırılması MINFO-8-i 🈳-dən 🈺-ə köçürür. Qalan mühəndislik tapşırıqları (işlər, skriptlər, xəbərdarlıq naqilləri) yuxarıdakı cədvəldə izlənilir və ilk Q32026 nəşrindən əvvəl bağlanmalıdır.