---
lang: az
direction: ltr
source: docs/source/compliance/android/device_lab_reservation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05dc578338882ddfcdf2410b0643774ceb8212f28739ba94ac83edf087b9b5dc
source_last_modified: "2025-12-29T18:16:35.924530+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Cihaz Laboratoriyasının Rezervasiyası Proseduru (AND6/AND7)

Bu dərslik Android komandasının cihazı necə sifariş etdiyini, təsdiq etdiyini və yoxladığını təsvir edir
**AND6** (CI və uyğunluğun sərtləşdirilməsi) və **AND7** mərhələləri üçün laboratoriya vaxtı
(müşahidə etməyə hazır olmaq). O, ehtiyat girişini tamamlayır
Gücü təmin etməklə `docs/source/compliance/android/device_lab_contingency.md`
ilk növbədə çatışmazlıqların qarşısı alınır.

## 1. Məqsədlər və əhatə dairəsi

- StrongBox + ümumi cihaz hovuzlarını yol xəritəsində nəzərdə tutulmuş 80% yuxarıda saxlayın
  dondurulmuş pəncərələr boyunca tutum hədəfi.
- CI, attestasiya taramaları və xaos üçün deterministik bir təqvim təqdim edin
  məşqlər heç vaxt eyni aparat üçün yarışmır.
- Qidalanan yoxlanıla bilən izi (sorğular, təsdiqlər, post-run qeydləri) çəkin
  AND6 uyğunluq yoxlama siyahısı və sübut jurnalı.

Bu prosedur xüsusi Pixel zolaqlarını, paylaşılan ehtiyat hovuzunu və
yol xəritəsində istinad edilən xarici StrongBox laboratoriya qoruyucusu. Ad-hoc emulyator
istifadə əhatə dairəsi xaricindədir.

## 2. Rezervasiya pəncərələri

| Hovuz / Zolaq | Avadanlıq | Defolt Yuva Uzunluğu | Rezervasiya Təqdimat Vaxtı | Sahibi |
|-------------|----------|---------------------|-------------------|-------|
| `pixel8pro-strongbox-a` | Pixel8Pro (StrongBox) | 4 saat | 3 iş günü | Hardware Laboratoriyası Rəhbəri |
| `pixel8a-ci-b` | Pixel8a (CI ümumi) | 2saat | 2 iş günü | Android Foundations TL |
| `pixel7-fallback` | Pixel7 paylaşılan hovuz | 2saat | 1 iş günü | Release Engineering |
| `firebase-burst` | Firebase Test Laboratoriyası tüstü növbəsi | 1 saat | 1 iş günü | Android Foundations TL |
| `strongbox-external` | Xarici StrongBox laboratoriyası | 8 saat | 7 təqvim günü | Proqram rəhbəri |

Slotlar UTC-də bron edilir; üst-üstə düşən qeyd-şərtlər açıq təsdiq tələb edir
Hardware Laboratoriya Rəhbərindən.

## 3. İş axını tələb edin

1. **Kontekst hazırlayın**
   - `docs/source/sdk/android/android_strongbox_device_matrix.md` ilə yeniləyin
     məşq etməyi planlaşdırdığınız cihazlar və hazırlıq etiketi
     (`attestation`, `ci`, `chaos`, `partner`).
   - Ən son tutumlu görüntünü toplayın
     `docs/source/sdk/android/android_strongbox_capture_status.md`.
2. **Sorğu göndərin**
   - Şablondan istifadə edərək `_android-device-lab` növbəsinə bilet göndərin
     `docs/examples/android_device_lab_request.md` (sahibi, tarixlər, iş yükləri,
     geri qaytarma tələbi).
   - İstənilən tənzimləyici asılılıqları əlavə edin (məsələn, AND6 attestasiya taraması, AND7
     telemetriya təlimi) və müvafiq yol xəritəsi girişinə keçid edin.
3. **Təsdiq**
   - Hardware Lab Lead bir iş günü ərzində nəzərdən keçirir, slotu təsdiqləyir
     paylaşılan təqvim (`Android Device Lab – Reservations`) və yeniləyir
     `device_lab_capacity_pct` sütununda
     `docs/source/compliance/android/evidence_log.csv`.
4. **İcra**
   - Planlaşdırılmış işləri yerinə yetirmək; Buildkite run ID-lərini və ya alət jurnallarını qeyd edin.
   - Hər hansı sapmalara diqqət yetirin (avadanlığın dəyişdirilməsi, aşırma).
5. **Bağlama**
   - Artefaktlar/linklərlə biletə şərh verin.
   - Əgər icra uyğunluqla bağlı idisə, yeniləyin
     `docs/source/compliance/android/and6_compliance_checklist.md` və bir sıra əlavə edin
     `evidence_log.csv` üçün.

Partnyor demolarına (AND8) təsir edən sorğular Partner Engineering-ə daxil edilməlidir.

## 4. Dəyişiklik və Ləğv- **Yenidən planlaşdırma:** orijinal bileti yenidən açın, yeni slot təklif edin və bileti yeniləyin
  təqvim girişi. Yeni slot 24 saat ərzində olarsa, Hardware Lab Lead + SRE-ni pingləyin
  birbaşa.
- **Fövqəladə ləğv:** ehtiyat planına əməl edin
  (`device_lab_contingency.md`) və tətik/fəaliyyət/izləmə sətirlərini qeyd edin.
- **Həddini aşmaq:** əgər qaçış öz yuvasını >15 dəqiqə keçirsə, yeniləmə göndərin və təsdiqləyin
  növbəti rezervasiyanın davam etdirilə biləcəyi; əks halda geriyə təhvil verin
  hovuz və ya Firebase partlayış zolağı.

## 5. Sübut və Audit

| Artefakt | Məkan | Qeydlər |
|----------|----------|-------|
| Biletlərin bron edilməsi | `_android-device-lab` növbəsi (Jira) | İxrac həftəlik xülasəsi; sübut jurnalında bilet identifikatorlarını əlaqələndirin. |
| Təqvim ixracı | `artifacts/android/device_lab/<YYYY-WW>-calendar.{ics,json}` | Hər cümə günü `scripts/android_device_lab_export.py --ics-url <calendar_ics_feed>` işləyin; köməkçi süzülmüş `.ics` faylını üstəgəl ISO həftəsi üçün JSON xülasəsini saxlayır ki, auditlər hər iki artefaktı əl ilə yükləmədən əlavə edə bilsin. |
| Tutum anlıq görüntüləri | `docs/source/compliance/android/evidence_log.csv` | Hər rezervasiyadan/bağlandıqdan sonra yeniləyin. |
| İşdən sonrakı qeydlər | `docs/source/compliance/android/device_lab_contingency.md` (fövqəladə hallarda) və ya bilet şərhi | Auditlər üçün tələb olunur. |

Rüblük uyğunluq yoxlamaları zamanı təqvim ixracını, bilet xülasəsini,
və AND6 yoxlama siyahısına təqdim edilən sübut jurnalından çıxarış.

### Təqvim ixracının avtomatlaşdırılması

1. “Android Cihaz Laboratoriyası – Rezervasyonlar” üçün ICS lentinin URL-ni əldə edin (və ya `.ics` faylını endirin).
2. İcra etmək

   ```bash
   python3 scripts/android_device_lab_export.py \
     --ics-url "https://calendar.example/ical/export" \
     --week <ISO week, defaults to current>
   ```

   Skript həm `artifacts/android/device_lab/<YYYY-WW>-calendar.ics` yazır
   və `...-calendar.json`, seçilmiş ISO həftəsini çəkir.
3. Yaradılmış faylları həftəlik sübut paketi ilə yükləyin və istinad edin
   `docs/source/compliance/android/evidence_log.csv`-də JSON xülasəsi nə zaman
   giriş cihazı-laboratoriya tutumu.

## 6. Eskalasiya Nərdivanı

1. Aparat Laboratoriyası Rəhbəri (əsas)
2. Android Foundations TL
3. Proqram rəhbəri/buraxılış mühəndisliyi (dondurulmuş pəncərələr üçün)
4. Xarici StrongBox laboratoriya əlaqəsi (tutucu işə salındıqda)

Eskalasiyalar biletə daxil edilməli və həftəlik Android-də əks olunmalıdır
status poçtu.

## 7. Əlaqədar Sənədlər

- `docs/source/compliance/android/device_lab_contingency.md` — hadisə qeydi
  tutum çatışmazlığı.
- `docs/source/compliance/android/and6_compliance_checklist.md` — ustad
  çatdırılmaların yoxlanış siyahısı.
- `docs/source/sdk/android/android_strongbox_device_matrix.md` — aparat
  əhatə izləyicisi.
- `docs/source/sdk/android/android_strongbox_attestation_run_log.md` -
  AND6/AND7 tərəfindən istinad edilən StrongBox sertifikatı.

Bu rezervasiya prosedurunun saxlanılması yol xəritəsinin “müəyyən et
cihaz-laboratoriya rezervasiya proseduru” və tərəfdaşla üzləşdiyi uyğunluq artefaktlarını saxlayır
Android hazırlıq planının qalan hissəsi ilə sinxronlaşdırılır.

## 8. Failover Drill Proseduru və Əlaqələr

Yol xəritəsinin AND6 maddəsi də rüblük uğursuzluq məşqini tələb edir. Tam,
addım-addım təlimatlar yaşayır
`docs/source/compliance/android/device_lab_failover_runbook.md`, lakin yüksək
səviyyəli iş axını aşağıda ümumiləşdirilmişdir ki, sorğu edənlər təlimləri birlikdə planlaşdıra bilsinlər
rutin rezervasiyalar.1. **Maşını planlaşdırın:** Təsirə məruz qalan zolaqları bağlayın (`pixel8pro-strongbox-a`,
   ehtiyat hovuzu, `firebase-burst`, xarici StrongBox qoruyucusu) paylaşılanda
   təqvim və `_android-device-lab` növbəsi məşqdən ən azı 7 gün əvvəl.
2. **Kəsinti simulyasiya edin:** Əsas zolağı boşaldın, PagerDuty-ni işə salın
   (`AND6-device-lab`) hadisəsi və asılı Buildkite işlərinə şərh yazın
   runbook-da qeyd olunan qazma ID-si.
3. **Uğursuzluq:** Pixel7 ehtiyat zolağı təşviq edin, Firebase partlamasını başladın
   paketi seçin və 6 saat ərzində xarici StrongBox tərəfdaşını cəlb edin. Tutmaq
   Buildkite ilə işləyən URL-lər, Firebase ixracları və saxlanma təsdiqləri.
4. **Təsdiq edin və bərpa edin:** Attestasiya + CI iş vaxtlarını yoxlayın, proqramı bərpa edin
   orijinal zolaqları və `device_lab_contingency.md` üstəgəl sübut jurnalını yeniləyin
   paket yolu + yoxlama məbləğləri ilə.

### Əlaqə və Eskalasiya Referansı

| Rol | Əsas Əlaqə | Kanal(lar) | Eskalasiya Sifarişi |
|------|-----------------|------------|------------------|
| Hardware Laboratoriyası Rəhbəri | Priya Ramanathan | `@android-lab` Slack · +81-3-5550-1234 | 1 |
| Cihaz Laboratoriyası Əməliyyatları | Mateo Cruz | `_android-device-lab` növbə | 2 |
| Android Foundations TL | Elena Vorobeva | `@android-foundations` Slack | 3 |
| Release Engineering | Aleksey Morozov | `release-eng@iroha.org` | 4 |
| Xarici StrongBox Laboratoriyası | Sakura Instruments NOC | `noc@sakura.example` · +81-3-5550-9876 | 5 |

Qazma bloklama problemlərini aşkar edərsə və ya hər hansı bir geriləmə olarsa, ardıcıl olaraq artırın
zolaq 30 dəqiqə ərzində onlayn ola bilməz. Həmişə eskalasiyanı qeyd edin
qeydləri `_android-device-lab` biletində qeyd edin və fövqəladə hallar jurnalında əks etdirin.