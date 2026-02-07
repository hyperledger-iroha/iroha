---
lang: az
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 887123dcafff50fb243d9788415b759da3691876e44b3cd7c800eede25a5ab09
source_last_modified: "2026-01-05T09:28:11.916414+00:00"
translation_last_reviewed: 2026-02-07
id: testnet-rollout
title: SoraNet testnet rollout (SNNet-10)
sidebar_label: Testnet Rollout (SNNet-10)
description: Phased activation plan, onboarding kit, and telemetry gates for SoraNet testnet promotions.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
:::

SNNet-10 şəbəkə daxilində SoraNet anonimlik örtüyünün mərhələli aktivləşdirilməsini əlaqələndirir. SoraNet defolt nəqliyyata çevrilməzdən əvvəl hər bir operator gözləntiləri başa düşsün, beləliklə, yol xəritəsi gülləsini konkret çatdırılmalara, runbook-lara və telemetriya qapılarına çevirmək üçün bu plandan istifadə edin.

## Başlatma mərhələləri

| Faza | Zaman qrafiki (hədəf) | Əhatə dairəsi | Tələb olunan artefaktlar |
|-------|-------------------|-------|--------------------|
| **T0 — Qapalı Testnet** | Q4 2026 | Əsas töhfəçilər tərəfindən idarə olunan ≥3 ASN-də 20-50 rele. | Testnet-ə qoşulma dəsti, qoruyucu tüstü dəsti, əsas gecikmə + PoW ölçüləri, qazma jurnalı. |
| **T1 — İctimai Beta** | rüb 2027 | ≥100 rele, qoruyucu fırlanma aktivləşdirilib, çıxış bağlanması tətbiq edilib, `anon-guard-pq` ilə SoraNet üçün defolt SDK beta. | Yenilənmiş bort dəsti, operatorun yoxlanış siyahısı, kataloq nəşri SOP, telemetriya tablosunun paketi, insident sınaq hesabatları. |
| **T2 — Əsas Şəbəkə Defolt** | Q2 2027 (SNNet-6/7/9-da tamamlandı) | İstehsal şəbəkəsi defoltları SoraNet-ə uyğundur; obfs/MASQUE nəqliyyatları və PQ ratchet tətbiqi aktivləşdirilib. | İdarəetmə təsdiq protokolları, birbaşa geriyə qaytarma proseduru, aşağı səviyyəli həyəcan siqnalları, imzalanmış müvəffəqiyyət göstəriciləri hesabatı. |

**keçmə yolu yoxdur**—hər bir mərhələ irəliləmədən əvvəlki mərhələdən telemetriya və idarəetmə artefaktlarını göndərməlidir.

## Testnet onboarding dəsti

Hər bir relay operatoru aşağıdakı faylları olan deterministik paket alır:

| Artefakt | Təsvir |
|----------|-------------|
| `01-readme.md` | Baxış, əlaqə nöqtələri və vaxt qrafiki. |
| `02-checklist.md` | Uçuşdan əvvəl yoxlama siyahısı (avadanlıq, şəbəkəyə əlçatanlıq, mühafizə siyasətinin yoxlanılması). |
| `03-config-example.toml` | Minimal SoraNet relay + orkestr konfiqurasiyası SNNet-9 uyğunluq blokları ilə uyğunlaşdırılmışdır, o cümlədən ən son qoruyucu snapshot hash-i birləşdirən `guard_directory` bloku. |
| `04-telemetry.md` | SoraNet məxfilik ölçüləri tablosuna və xəbərdarlıq hədlərinə qoşulmaq üçün təlimatlar. |
| `05-incident-playbook.md` | Eskalasiya matrisi ilə Brownout/downgrade cavab proseduru. |
| `06-verification-report.md` | Şablon operatorları tüstü testləri keçdikdən sonra tamamlayır və geri qayıdırlar. |

Göstərilən nüsxə `docs/examples/soranet_testnet_operator_kit/`-də yaşayır. Hər bir təşviqat dəsti yeniləyir; versiya nömrələri mərhələni izləyir (məsələn, `testnet-kit-vT0.1`).

İctimai beta (T1) operatorları üçün `docs/source/soranet/snnet10_beta_onboarding.md`-dəki qısa ilkin məlumat deterministik dəst və validator köməkçilərinə işarə edərək ilkin şərtləri, telemetriya nəticələrini və təqdimetmə işini ümumiləşdirir.

`cargo xtask soranet-testnet-feed` mərhələ qapısı şablonu tərəfindən istinad edilən təşviqat pəncərəsini, relay siyahısı, metrik hesabatı, qazma sübutu və qoşma heşlərini birləşdirən JSON lentini yaradır. Lentin `drill_log.signed = true` qeyd edə bilməsi üçün əvvəlcə `cargo xtask soranet-testnet-drill-bundle` ilə qazma qeydlərini və əlavələri imzalayın.

## Uğur göstəriciləri

Fazalar arasında irəliləyiş minimum iki həftə ərzində toplanan aşağıdakı telemetriyada aparılır:

- `soranet_privacy_circuit_events_total`: dövrələrin 95%-i qaralma və ya endirmə hadisələri olmadan tamamlandı; qalan 5% PQ təchizatı ilə məhdudlaşdırılır.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: Gündə gətirmə seanslarının <1%-i planlaşdırılmış təlimlərdən kənarda işləməyə səbəb olur.
- `soranet_privacy_gar_reports_total`: gözlənilən GAR kateqoriya qarışığının ±10%-i daxilində dispersiya; sıçrayışlar təsdiq edilmiş siyasət yeniləmələri ilə izah edilməlidir.
- PoW biletinin müvəffəqiyyət dərəcəsi: 3s hədəf pəncərəsində ≥99%; `soranet_privacy_throttles_total{scope="congestion"}` vasitəsilə məlumat verildi.
- Bölgəyə görə gecikmə (95-ci faizlik): sxemlər tam qurulduqdan sonra <200ms, `soranet_privacy_rtt_millis{percentile="p95"}` vasitəsilə çəkilir.

İdarə paneli və xəbərdarlıq şablonları `dashboard_templates/` və `alert_templates/`-də yaşayır; onları telemetriya repozitoriyanıza əks etdirin və CI lint yoxlamalarına əlavə edin. Təqdimat tələb etməzdən əvvəl idarəetmə ilə bağlı hesabat yaratmaq üçün `cargo xtask soranet-testnet-metrics` istifadə edin.

Mərhələ qapısı təqdimatları `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md` altında saxlanılan kopyalanmağa hazır Markdown formasına keçid verən `docs/source/soranet/snnet10_stage_gate_template.md`-ə uyğun olmalıdır.

## Doğrulama yoxlama siyahısı

Operatorlar hər bir mərhələyə daxil olmamışdan əvvəl aşağıdakıları bağlamalıdırlar:

- ✅ Cari qəbul zərfi ilə imzalanmış relay reklamı.
- ✅ Qoruyucu fırlanma tüstü testi (`tools/soranet-relay --check-rotation`) keçir.
- ✅ `guard_directory` ən son `GuardDirectorySnapshotV2` artefaktına işarə edir və `expected_directory_hash_hex` komitə həzminə uyğun gəlir (relenin işə salınması təsdiq edilmiş hashı qeyd edir).
- ✅ PQ ratchet ölçüləri (`sorafs_orchestrator_pq_ratio`) tələb olunan mərhələ üçün hədəf hədlərinin üstündə qalır.
- ✅ GAR uyğunluq konfiqurasiyası ən son etiketə uyğun gəlir (SNNet-9 kataloquna baxın).
- ✅ Siqnal simulyasiyasını aşağı salın (kollektorları söndürün, 5 dəqiqə ərzində xəbərdarlıq gözləyin).
- ✅ PoW/DoS qazması sənədləşdirilmiş yumşaldıcı addımlarla həyata keçirilir.

Əvvəlcədən doldurulmuş şablon onboarding dəstinə daxildir. Operatorlar istehsal etimadnaməsini almadan əvvəl tamamlanmış hesabatı idarəetmənin yardım masasına təqdim edirlər.

## İdarəetmə və hesabat

- **Dəyişikliklərə nəzarət:** yüksəlişlər şuranın protokolunda qeyd edilmiş və status səhifəsinə əlavə edilmiş İdarəetmə Şurasının təsdiqini tələb edir.
- **Status həzmi:** relay sayı, PQ nisbəti, qaralma hadisələri və görkəmli fəaliyyət elementlərini ümumiləşdirən həftəlik yeniləmələri dərc edin (kadans başlayan kimi `docs/source/status/soranet_testnet_digest.md`-də saxlanılır).
- **Geri qaytarmalar:** DNS/mühafizə önbelleği etibarsızlığı və müştəri əlaqəsi şablonları daxil olmaqla, 30 dəqiqə ərzində şəbəkəni əvvəlki mərhələyə qaytaran imzalanmış geri qaytarma planını qoruyun.

## Dəstəkləyici aktivlər

- `cargo xtask soranet-testnet-kit [--out <dir>]` onboarding dəstini `xtask/templates/soranet_testnet/`-dən hədəf kataloquna təqdim edir (defolt olaraq `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` SNNet-10 uğur göstəricilərini qiymətləndirir və idarəetmənin nəzərdən keçirilməsi üçün uyğun strukturlaşdırılmış keçid/uğursuzluq hesabatı verir. Nümunə snapshot `docs/examples/soranet_testnet_metrics_sample.json`-də yaşayır.
- Grafana və Alertmanager şablonları `dashboard_templates/soranet_testnet_overview.json` və `alert_templates/soranet_testnet_rules.yml` altında yaşayır; onları telemetriya repozitoriyanıza köçürün və ya CI lint yoxlamalarına köçürün.
- SDK/portal mesajlaşması üçün aşağı səviyyəli rabitə şablonu `docs/source/soranet/templates/downgrade_communication_template.md`-də yerləşir.
- Həftəlik status həzmləri kanonik forma kimi `docs/source/status/soranet_testnet_weekly_digest.md`-dən istifadə etməlidir.

Çəkmə sorğuları bu səhifəni hər hansı artefakt və ya telemetriya dəyişiklikləri ilə birlikdə güncəlləşdirməlidir ki, yayım planı kanonik qalsın.