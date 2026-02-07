---
lang: az
direction: ltr
source: docs/source/compliance/android/device_lab_failover_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 473b2b49d32c32d2b884b670ba35e9aa3d0606cfd451d441a7ca927c1160311d
source_last_modified: "2025-12-29T18:16:35.923579+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Device Lab Failover Drill Runbook (AND6/AND7)

Bu runbook proseduru, sübut tələblərini və əlaqə matrisini əks etdirir
istinad edilən **cihaz-laboratoriya fövqəladə hal planını** həyata keçirərkən istifadə olunur
`roadmap.md` (§“Tənzimləyici artefakt təsdiqləri və laboratoriya şəraiti”). Tamamlayır
rezervasiya iş axını (`device_lab_reservation.md`) və hadisə jurnalı
(`device_lab_contingency.md`) beləliklə, uyğunluğu nəzərdən keçirənlər, hüquq məsləhətçiləri və SRE
uğursuzluğa hazırlığı necə təsdiqlədiyimizə dair tək bir həqiqət mənbəyimiz var.

## Məqsəd və Sürət

- Android StrongBox + ümumi cihaz hovuzlarının uğursuz ola biləcəyini nümayiş etdirin
  ehtiyat Pixel zolaqlarına, paylaşılan hovuza, Firebase Test Lab partlayış növbəsinə və
  AND6/AND7 SLA-larını əskik etmədən xarici StrongBox saxlayıcısı.
- Qanunun ETSI/FISC təqdimatlarına əlavə edə biləcəyi sübut paketi hazırlayın
  Fevral uyğunluq baxışından əvvəl.
- Hər rübdə ən azı bir dəfə, üstəlik, laboratoriya avadanlıqlarının siyahısı dəyişdikdə işləyin
  (yeni cihazlar, təqaüdə çıxma və ya 24 saatdan artıq texniki xidmət).

| Qazma ID | Tarix | Ssenari | Evidence Bundle | Status |
|----------|------|----------|-----------------|--------|
| DR-2026-02-Q1 | 2026-02-20 | Simulyasiya edilmiş Pixel8Pro zolağı kəsilməsi + AND7 telemetriya məşqi ilə attestasiya gecikməsi | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | ✅ Tamamlandı — paket hashləri `docs/source/compliance/android/evidence_log.csv`-də qeydə alınıb. |
| DR-2026-05-Q2 | 22-05-2026 (planlaşdırılıb) | StrongBox baxım üst-üstə düşür + Nexus məşq | `artifacts/android/device_lab_contingency/20260522-failover-drill/` *(gözləyən)* — `_android-device-lab` bilet **AND6-DR-202605** rezervasiyaları saxlayır; paket təlimdən sonra doldurulacaq. | 🗓 Planlaşdırılmış — təqvim bloku AND6 kadansı üzrə “Android Cihaz Laboratoriyası – Rezervasyonlar”a əlavə edildi. |

## Prosedur

### 1. Qazmadan əvvəl hazırlıq

1. `docs/source/sdk/android/android_strongbox_capture_status.md`-də baza qabiliyyətini təsdiqləyin.
2. Hədəf ISO həftəsi üçün rezervasiya təqvimini vasitəsilə ixrac edin
   `python3 scripts/android_device_lab_export.py --week <ISO week>`.
3. Fayl `_android-device-lab` bileti
   `AND6-DR-<YYYYMM>` əhatə dairəsi (“failover drill”), planlaşdırılmış yuvalar və təsirə məruz qalan
   iş yükləri (attestasiya, CI tüstüsü, telemetriya xaosu).
4. `device_lab_contingency.md`-də fövqəladə hallar jurnalı şablonunu yeniləyin
   qazma tarixi üçün yertutan sıra.

### 2. Uğursuzluq şəraitini simulyasiya edin

1. Laboratoriya daxilində əsas zolağı (`pixel8pro-strongbox-a`) söndürün və ya boşaldın
   planlaşdırıcı və rezervasiya girişini "drill" olaraq qeyd edin.
2. PagerDuty-də (`AND6-device-lab` xidməti) saxta kəsilmə xəbərdarlığını işə salın və
   sübut paketi üçün bildiriş ixracını əldə edin.
3. Normal olaraq zolaqdan istifadə edən Buildkite işlərinə şərh yazın
   (`android-strongbox-attestation`, `android-ci-e2e`) qazma ID ilə.

### 3. Failover icrası1. Əsas CI hədəfi üçün ehtiyat Pixel7 zolağı irəliləyin və planlaşdırın
   qarşı planlaşdırılmış iş yükü.
2. `firebase-burst` zolağı vasitəsilə Firebase Test Lab partlama dəstini işə salın
   StrongBox əhatə dairəsi paylaşılana keçərkən pərakəndə pul kisəsi tüstü testləri
   zolaq. Audit üçün biletdə CLI çağırışını (və ya konsol ixracını) çəkin
   paritet.
3. Qısa bir attestasiya taraması üçün xarici StrongBox laboratoriya nəzarətçisini işə salın;
   aşağıda təsvir olunduğu kimi əlaqə təsdiqini qeyd edin.
4. Bütün Buildkite icra identifikatorlarını, Firebase iş URL-lərini və saxlama transkriptlərini qeyd edin
   `_android-device-lab` bileti və sübut paketi manifestidir.

### 4. Doğrulama və geri qaytarma

1. Attestasiya/CI iş vaxtlarını ilkin göstərici ilə müqayisə edin; bayraq deltaları >10% -dən
   Hardware Laboratoriyası Rəhbəri.
2. Əsas zolağı bərpa edin və tutum snapshotunu və hazırlığı yeniləyin
   doğrulama keçdikdən sonra matris.
3. Son cərgəni `device_lab_contingency.md`-ə tətik, hərəkətlər
   və təqiblər.
4. `docs/source/compliance/android/evidence_log.csv`-i yeniləyin:
   paket yolu, SHA-256 manifest, Buildkite icra identifikatorları, PagerDuty ixrac hash və
   rəyçinin imzalanması.

## Evidence Bundle Layout

| Fayl | Təsvir |
|------|-------------|
| `README.md` | Xülasə (məşq ID-si, əhatə dairəsi, sahiblər, vaxt qrafiki). |
| `bundle-manifest.json` | Paketdəki hər bir fayl üçün SHA-256 xəritəsi. |
| `calendar-export.{ics,json}` | İxrac skriptindən ISO həftəlik rezervasiya təqvimi. |
| `pagerduty/incident_<id>.json` | PagerDuty insidentinin ixracı xəbərdarlıq + təsdiqləmə qrafikini göstərir. |
| `buildkite/<job>.txt` | Təsirə məruz qalan iş yerləri üçün Buildkite işlətmə URL-ləri və qeydləri. |
| `firebase/burst_report.json` | Firebase Test Laboratoriyası partlama icrası xülasəsi. |
| `retainer/acknowledgement.eml` | Xarici StrongBox laboratoriyasından təsdiq. |
| `photos/` | Avadanlıq yenidən kabelləşdirilibsə, laboratoriya topologiyasının isteğe bağlı fotoşəkilləri/skrinşotları. |

Paketi burada saxlayın
`artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` və qeyd edin
sübut jurnalının içərisindəki manifest yoxlama məbləği və AND6 uyğunluğu yoxlama siyahısı.

## Əlaqə və Eskalasiya Matrisi

| Rol | Əsas Əlaqə | Kanal(lar) | Qeydlər |
|------|-----------------|------------|-------|
| Hardware Laboratoriyası Rəhbəri | Priya Ramanathan | `@android-lab` Slack · +81-3-5550-1234 | Yerində fəaliyyətlərə və təqvim yeniləmələrinə sahibdir. |
| Cihaz Laboratoriyası Əməliyyatları | Mateo Cruz | `_android-device-lab` növbə | Rezervasiya biletləri + paket yükləmələrini əlaqələndirir. |
| Release Engineering | Aleksey Morozov | Eng Slack-i buraxın · `release-eng@iroha.org` | Buildkite sübutlarını təsdiqləyir + hashları dərc edir. |
| Xarici StrongBox Laboratoriyası | Sakura Instruments NOC | `noc@sakura.example` · +81-3-5550-9876 | Saxlayıcı əlaqə; 6 saat ərzində mövcudluğu təsdiqləyin. |
| Firebase Burst Koordinatoru | Tessa Wright | `@android-ci` Slack | Geri qaytarılması lazım olduqda Firebase Test Laboratoriyasının avtomatlaşdırılmasını işə salır. |

Təlim bloklama problemlərini aşkar edərsə, aşağıdakı ardıcıllıqla artırın:
1. Hardware Laboratoriyası Rəhbəri
2. Android Foundations TL
3. Proqram rəhbəri / Buraxılış mühəndisliyi
4. Uyğunluq üzrə Rəhbər + Hüquq Məsləhətçisi (məşq tənzimləmə riskini aşkar edərsə)

## Hesabat və Təqiblər- İstinad edərkən rezervasiya proseduru ilə yanaşı bu runbook-u əlaqələndirin
  `roadmap.md`, `status.md` və idarəetmə paketlərində uğursuzluq hazırlığı.
- Rüblük məşq xülasəsini sübut paketi ilə birlikdə Uyğunluq + Hüquqa e-poçt göndərin
  hash cədvəli və `_android-device-lab` bilet ixracını əlavə edin.
- Əsas ölçüləri əks etdirin (taxılma vaxtı, bərpa olunan iş yükləri, gözlənilməz tədbirlər)
  `status.md` və AND7 qaynar siyahı izləyicisi içərisindədir ki, rəyçiləri izləyə bilsinlər.
  konkret məşqdən asılılıq.