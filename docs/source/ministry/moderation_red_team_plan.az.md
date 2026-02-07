---
lang: az
direction: ltr
source: docs/source/ministry/moderation_red_team_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7ecf637fa28f68a997367118163224caecc6c71c604d5e7ece409941d5374f44
source_last_modified: "2025-12-29T18:16:35.978869+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Red-Team & Chaos Drill Plan
summary: Execution plan for roadmap item MINFO-9 covering recurring adversarial campaigns, telemetry hooks, and reporting requirements.
translator: machine-google-reviewed
---

# Moderasiya Qırmızı Komanda və Xaos Təlimləri (MINFO-9)

Yol xəritəsi arayışı: **MINFO-9 — Moderasiya qırmızı komanda və xaos təlimləri** (hədəflər Q32026)

İnformasiya Nazirliyi AI moderasiya boru kəmərlərini, mükafat proqramlarını, şlüzləri və idarəetmə nəzarətlərini vurğulayan təkrarlana bilən rəqib kampaniyaları həyata keçirməlidir. Bu plan, mövcud moderasiya tablosuna (`dashboards/grafana/ministry_moderation_overview.json`) və fövqəladə hallar üzrə iş axınlarına birbaşa bağlı olan əhatə dairəsi, tempi, məşq şablonları və sübut tələblərini müəyyən etməklə yol xəritəsində qeyd olunan "🈳 Başlanmadı" boşluğunu bağlayır.

## Məqsədlər və Nəticələr

- Əlavə təhlükə siniflərinə əhatə dairəsini genişləndirməzdən əvvəl çox hissəli qaçaqmalçılıq cəhdlərini, rüşvətxorluğu/müraciət saxtakarlığını və şlüz yan kanal araşdırmalarını əhatə edən rüblük qırmızı komanda təlimləri keçirin.
- Hər qazma üçün deterministik artefaktları (CLI qeydləri, Norito manifestləri, Grafana ixracları, SoraFS CID-lər) çəkin və onları `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` vasitəsilə fayllayın.
- Qazma məhsulunu yenidən kalibrləmə manifestlərinə (`docs/examples/ai_moderation_calibration_*.json`) və denilist siyasətinə köçürün ki, remediasiya tapşırıqları izlənilə bilən yol xəritəsi biletlərinə çevrilsin.
- Tel siqnalı/runbook inteqrasiyası beləliklə, nasazlıqlar MINFO-1 idarə panelləri və Alertmanager paketləri (`dashboards/alerts/ministry_moderation_rules.yml`) ilə yanaşı görünür.

## Əhatə və Asılılıqlar

- **Sınaqda olan sistemlər:** SoraFS qəbul/orkestrator yolları, `docs/source/sorafs_ai_moderation_plan.md`-də müəyyən edilmiş süni intellekt nəzarətçisi, rədd siyahısı/Merkle tətbiqi, müraciət xəzinəsi alətləri və şlüz dərəcəsi limitləri.
- **Tələblər:** Fövqəladə canon və TTL siyasəti (`docs/source/ministry/emergency_canon_policy.md`), moderasiya kalibrləmə qurğuları və Torii saxta qoşqu pariteti beləliklə, təkrarlana bilən faydalı yüklər xaos qaçışları zamanı təkrar oxuna bilsin.
- **Əhatə dairəsindən kənar:** SoraDNS siyasətləri, Kaigi konfransı və ya nazirliyə aid olmayan kommunikasiya kanalları (SNNet və DOCS-SORA proqramları altında ayrıca izlənilir).

## Rol və Məsuliyyətlər

| Rol | Məsuliyyətlər | Əsas Sahib | Yedək |
|------|------------------|---------------|--------|
| Qazma Direktoru | Ssenari siyahısını təsdiq edir, qırmızı komandalar təyin edir, runbooks-da imzalayır | Nazirliyin Təhlükəsizlik Rəhbəri | Moderator müavini |
| Düşmən Hüceyrə | Faydalı yüklər hazırlayır, hücumlar edir, sübutları qeyd edir | Təhlükəsizlik Mühəndisliyi Gildiyası | Könüllü operatorlar |
| Müşahidə Aparıcısı | Tablolara/silahlara nəzarət edir, Grafana ixracını, faylları insident qrafiklərini çəkir | SRE / Müşahidə edilə bilən TL | Zəng üzrə SRE |
| Növbətçi Moderator | Artan axını idarə edir, ləğvetmə sorğularını təsdiqləyir, təcili canon qeydlərini yeniləyir | Hadisə komandiri | Ehtiyat komandir |
| Hesabat Yazıçısı | `docs/source/ministry/reports/moderation_red_team_template.md` altında şablonu doldurur, artefaktları əlaqələndirir, təqib məsələlərini açır | Sənədlər/DevRel | Məhsulla əlaqə |

## Sürət və Zaman Qrafiki| Faza | Hədəf Pəncərəsi | Əsas Fəaliyyətlər | Artefaktlar |
|-------|---------------|----------------|-----------|
| **Plan** | T−4 həftə | `scripts/ministry/scaffold_red_team_drill.py` vasitəsilə ssenariləri seçin, faydalı yük qurğularını, iskele artefaktlarını yeniləyin və matkapların güzgüyə çevirdiyi telemetriya köməkçilərini (`scripts/telemetry/check_redaction_status.py`, `ci/run_android_telemetry_chaos_prep.sh`) qurutun | Ssenari xülasələri, bilet izləyicisi |
| **Hazır** | T−1 həftə | İştirakçıları kilidləyin, mərhələ SoraFS/Torii qum qutuları, idarə panellərini/xəbərdarlıq heşlərini dondurun | Hazır yoxlama siyahısı, tablosuna həzmlər |
| **İcra et** | Qazma günü (4 saat) | Düşmən axınlarını işə salın, Alertmanager bildirişlərini toplayın, Torii/CLI izlərini çəkin, ləğvetmə təsdiqlərini tətbiq edin | Canlı jurnal, Grafana anlıq görüntülər |
| **Bərpa** | T+1 gün | `artifacts/ministry/red-team/<YYYY-MM>/` və SoraFS-ə ləğvetmələri, skrab məlumat dəstlərini, arxiv artefaktlarını geri qaytarın | Sübut paketi, manifest |
| **Hesabat** | T+1 həftə | Şablondan Markdown hesabatını dərc edin, remediasiya biletlərini qeyd edin, roadmap/status.md-ni yeniləyin | Hesabat faylı, Jira/GitHub bağlantıları |

Rüblük təlimlər (Mar/İyun/Sentyabr/Dekabr) minimum həyata keçirilir; yüksək riskli tapıntılar eyni sübut iş prosesini izləyən ad-hoc qaçışları tetikler.

## Ssenari Kitabxanası (İlkin)

| Ssenari | Təsvir | Uğur siqnalları | Sübut Girişləri |
|----------|-------------|-----------------|-----------------|
| Çox hissəli qaçaqmalçılıq | Zamanla süni intellekt filtrlərindən yan keçməyə çalışan polimorfik faydalı yükləri olan SoraFS provayderləri arasında parça zənciri yayılmışdır. Orkestr diapazonunun əldə edilməsi, moderasiya TTL-ləri və inkarçıların yayılması məşqləri. | Qaçaqmalçılıq istifadəçinin çatdırılmasından əvvəl aşkar edilmişdir; yayılan inkarçı delta; `ministry_moderation_overview` SLA daxilində yanğın xəbərdar edir. | `sorafs.fetch.*` tablosundan CLI təkrar oxutma qeydləri, yığın manifestləri, rədd edilmiş fərqlər, İz ID-ləri. |
| Rüşvət və müraciətin saxtalaşdırılması | Zərərli moderator cütləri rüşvətxorluqla bağlı dəyişiklikləri təsdiqləməyə çalışır; xəzinə axınını yoxlayır, təsdiqləri ləğv edir və audit qeydini aparır. | Məcburi dəlillərlə qeyd edilən ləğv, xəzinə köçürmələri qeyd edildi, idarəçilik səsi qeyd edildi. | Norito qeydləri ləğv edir, `docs/source/ministry/volunteer_brief_template.md` yeniləmələri, xəzinə dəftəri qeydləri. |
| Gateway yan kanal araşdırması | Moderasiya edilmiş məzmunu çıxarmaq üçün keş vaxtını və TTL-ləri ölçən saxta naşirləri simulyasiya edir. SNNet-15-dən əvvəl CDN/gateway sərtləşdirməni həyata keçirir. | Rate-limit & anomaliya panelləri zondları vurğulayır; admin CLI siyasətin icrasını göstərir; məzmun sızması yoxdur. | Şlüz giriş qeydləri, `ministry_gateway_observability` panellərinin Grafana sıyrılması, oflayn nəzərdən keçirmək üçün paket izlərini (pcap) çəkin. |

İlk üç ssenari öyrənmə kadansını bitirdikdən sonra gələcək iterasiyalar `honey-payload beacons`, `AI adversarial prompt floods` və `SoraFS metadata poisoning` əlavə edəcək.

## İcra yoxlama siyahısı1. **Ön qazma**
   - Runbook + ssenari sənədinin dərc edildiyini və təsdiqləndiyini təsdiqləyin.
   - Snapshot panelləri (`dashboards/grafana/ministry_moderation_overview.json`) və `artifacts/ministry/red-team/<YYYY-MM>/dashboards/` üçün xəbərdarlıq qaydaları.
   - Hesabat + artefakt kataloqları yaratmaq üçün `scripts/ministry/scaffold_red_team_drill.py`-dən istifadə edin, sonra qazma üçün hazırlanmış hər hansı qurğu paketləri üçün SHA256 həzmlərini qeyd edin.
   - Müxalif faydalı yükləri yeritməzdən əvvəl inkarçı Merkle köklərini və təcili qanun qeydlərini yoxlayın.
2. **Qazma zamanı**
   - Hər bir hərəkəti (vaxt damğası, operator, əmr) canlı jurnala (paylaşılan sənəd və ya `docs/source/ministry/reports/tmp/<timestamp>.md`) daxil edin.
   - Sorğu identifikatorları, model adları və risk xalları daxil olmaqla, Torii cavablarını və AI moderasiya qərarlarını çəkin.
   - Eskalasiya iş prosesini həyata keçirin (istəyi ləğv edin → komandirin təsdiqi → `emergency_canon_policy` yeniləməsi).
   - Ən azı bir xəbərdarlıq-təmizləmə məşqini və sənəd cavab gecikməsini işə salın.
3. **Post-Drill**
   - Ləğv etmələri təmizləyin, rədd edilmiş siyahıdakı qeydləri istehsal dəyərlərinə qaytarın və siqnalların səssizliyini yoxlayın.
   - Grafana/Alertmanager tarixçəsini, CLI qeydlərini, Norito manifestlərini ixrac edin və onları sübut paketinə əlavə edin.
   - Sahibləri və son tarixləri ilə faylların düzəldilməsi məsələləri (yüksək/orta/aşağı); yekun hesabata keçid.

## Telemetriya, Metriklər və Sübutlar

- ** İdarə panelləri:** `dashboards/grafana/ministry_moderation_overview.json` + gələcək `ministry_red_team_heatmap.json` (yer tutucu) canlı siqnalları tutur. Hər qazma üçün JSON snapshotlarını ixrac edin.
- **Xəbərdarlıq:** `dashboards/alerts/ministry_moderation_rules.yml` üstəgəl qarşıdan gələn `ministry_red_team_rules.yml` auditləri sadələşdirmək üçün qazma ID və ssenariyə istinad edən annotasiyaları daxil etməlidir.
- **Norito artefaktları:** hər məşqi `RedTeamDrillV1` hadisələri (xüsusi qarşıdan gələn) kimi kodlayır, buna görə də Torii/CLI ixracı deterministikdir və idarəetmə ilə paylaşıla bilər.
- **Hesabat şablonu:** `docs/source/ministry/reports/moderation_red_team_template.md` surətini çıxarın və ssenari, ölçülər, sübutlar həzmləri, remediasiya statusu və idarəetmənin imzalanması ilə doldurun.
- **Arxiv:** Artefaktları `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`-də (loqlar, CLI çıxışı, Norito paketləri, tablosunun ixracı) saxlayın və idarəetmə təsdiq etdikdə ictimai nəzərdən keçirmək üçün uyğun SoraFS CAR manifestini dərc edin.

## Avtomatlaşdırma və Növbəti Addımlar1. `scripts/ministry/` altında köməkçi skriptləri tətbiq edin, faydalı yük qurğularını əkmək, rədd edilmiş siyahı daxiletmələrini dəyişdirmək və CLI/Grafana ixracını toplamaq. (`scaffold_red_team_drill.py`, `moderation_payload_tool.py` və `check_red_team_reports.py` indi iskele, faydalı yük paketi, rədd edilmiş siyahı yamaqları və yer tutucunun tətbiqini əhatə edir; `export_red_team_evidence.py` çatışmayan tablosunu/logun ixracını əlavə edir0 API00 ən çox API0 ilə dəstək00 sübut determinist olun.)
2. Hesabatları birləşdirməzdən əvvəl şablonun tamlığını və sübut həzmlərini yoxlamaq üçün CI-ni `ci/check_ministry_red_team.sh` ilə genişləndirin. ✅ (`scripts/ministry/check_red_team_reports.py` bütün yerinə yetirilən məşq hesabatlarında yer tutucunun çıxarılmasını təmin edir.)
3. Qarşıdan gələn təlimləri, açıq remediasiya elementlərini və son icra göstəricilərini üzə çıxarmaq üçün `ministry_red_team_status` bölməsini `status.md`-ə əlavə edin.
4. Qazma metadatasını şəffaflıq boru kəmərinə inteqrasiya edin ki, rüblük hesabatlar ən son xaos nəticələrinə istinad etsin.
5. Yem qazma hesabatlarını birbaşa `cargo xtask ministry-transparency ingest --red-team-report <path>...`-ə daxil edin, beləliklə dezinfeksiya edilmiş rüblük ölçülər və idarəetmə manifestləri mövcud kitab kitabçası/müraciət/inkar siyahısı ilə yanaşı qazma ID-lərini, sübut paketlərini və idarə paneli SHA-ları daşıyır.

Bu addımlar yerə endikdən sonra, MINFO-9 izlənilə bilən artefaktlar və ölçülə bilən uğur meyarları ilə 🈳 Başlanmamış vəziyyətdən 🈺 Davam edir vəziyyətinə keçir.