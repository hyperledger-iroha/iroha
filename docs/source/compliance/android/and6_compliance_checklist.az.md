---
lang: az
direction: ltr
source: docs/source/compliance/android/and6_compliance_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0ce1be46f9c468915f50de5e38e2f34657b26bf4243fb5ea45dab175789393
source_last_modified: "2026-01-05T09:28:12.002460+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android AND6 Uyğunluq Yoxlama Siyahısı

Bu yoxlama siyahısı mühüm mərhələyə çatan uyğunluq nəticələrini izləyir **AND6 -
CI & Compliance Hardening**. O, tələb olunan tənzimləyici artefaktları birləşdirir
`roadmap.md`-də və altında saxlama sxemini müəyyən edir
`docs/source/compliance/android/` beləliklə Mühəndislik, Dəstək və Hüququ buraxın
Android buraxılışlarını təsdiq etməzdən əvvəl eyni sübut dəstinə istinad edə bilər.

## Əhatə və Sahiblər

| Ərazi | Çatdırılma | Əsas Sahib | Yedək / Rəyçi |
|------|--------------|---------------|-------------------|
| Aİ tənzimləmə paketi | ETSI EN 319 401 təhlükəsizlik hədəfi, GDPR DPIA xülasəsi, SBOM attestasiyası, sübut jurnalı | Uyğunluq və Hüquq (Sofia Martins) | Release Engineering (Aleksey Morozov) |
| Yaponiya tənzimləmə paketi | FISC təhlükəsizlik nəzarət yoxlama siyahısı, ikidilli StrongBox attestasiya paketləri, sübut jurnalı | Uyğunluq və Hüquq (Daniel Park) | Android Proqram Rəhbəri |
| Cihaz laboratoriyasının hazırlığı | Bacarıqların izlənməsi, fövqəladə vəziyyət tetikleyicileri, eskalasiya qeydi | Hardware Laboratoriyası Rəhbəri | Android Observability TL |

## Artefakt matrisi| Artefakt | Təsvir | Saxlama Yolu | Cadensi yeniləyin | Qeydlər |
|----------|-------------|--------------|-----------------|-------|
| ETSI EN 319 401 təhlükəsizlik hədəfi | Android SDK ikili faylları üçün təhlükəsizlik məqsədlərini/fərziyyələrini təsvir edən hekayə. | `docs/source/compliance/android/eu/security_target.md` | Hər GA + LTS buraxılışını yenidən təsdiqləyin. | Buraxılış qatarı üçün qurulma mənşəli hashlərə istinad edilməlidir. |
| GDPR DPIA xülasəsi | Telemetriya/girişi əhatə edən məlumatların qorunması təsirinin qiymətləndirilməsi. | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | İllik + maddi telemetriya dəyişikliklərindən əvvəl. | `sdk/android/telemetry_redaction.md`-də istinad redaksiya siyasəti. |
| SBOM attestasiyası | Gradle/Maven artefaktları üçün imzalanmış SBOM plus SLSA mənşəyi. | `docs/source/compliance/android/eu/sbom_attestation.md` | Hər GA buraxılışı. | CycloneDX hesabatları, kosign paketləri və yoxlama məbləğləri yaratmaq üçün `scripts/android_sbom_provenance.sh <version>`-i işə salın. |
| FISC təhlükəsizlik nəzarət yoxlama siyahısı | SDK nəzarətlərini FISC tələblərinə uyğunlaşdırmaq üçün tamamlanmış yoxlama siyahısı. | `docs/source/compliance/android/jp/fisc_controls_checklist.md` | İllik + JP tərəfdaş pilotlarından əvvəl. | İkidilli başlıqlar təqdim edin (EN/JP). |
| StrongBox attestasiya paketi (JP) | Cihaz başına attestasiya xülasəsi + JP tənzimləyiciləri üçün zəncir. | `docs/source/compliance/android/jp/strongbox_attestation.md` | Yeni avadanlıq hovuza daxil olduqda. | `artifacts/android/attestation/<device>/` altında xam artefaktlara işarə edin. |
| Hüquqi imzalanma memo | ETSI/GDPR/FISC əhatə dairəsini, məxfilik mövqeyini və əlavə edilmiş artefaktlar üçün nəzarət zəncirini əhatə edən məsləhət xülasəsi. | `docs/source/compliance/android/eu/legal_signoff_memo.md` | Hər dəfə artefakt paketi dəyişir və ya yeni yurisdiksiya əlavə olunur. | Memo sübut jurnalından hashlərə və cihaz-laboratoriya ehtiyat paketinə keçidlərə istinad edir. |
| Sübut jurnalı | Hash/zaman damğası metadatası ilə təqdim edilmiş artefaktların indeksi. | `docs/source/compliance/android/evidence_log.csv` | Yuxarıdakı hər hansı giriş dəyişdikdə yenilənir. | Buildkite linki əlavə edin + rəyçinin qeydiyyatı. |
| Cihaz-laboratoriya cihazları paketi | `device_lab_instrumentation.md`-də müəyyən edilmiş proseslə qeydə alınmış yuvaya məxsus telemetriya, növbə və attestasiya sübutu. | `artifacts/android/device_lab/<slot>/` (bax: `docs/source/compliance/android/device_lab_instrumentation.md`) | Hər qorunan yuva + əvəzetmə qazması. | SHA-256 manifestlərini çəkin və sübut jurnalında + yoxlama siyahısında yuva ID-sinə istinad edin. |
| Cihaz-laboratoriya rezervasiyası jurnalı | StrongBox hovuzlarını dondurma zamanı ≥80% saxlamaq üçün rezervasiya iş axını, təsdiqlər, tutum snapshotları və eskalasiya nərdivanı istifadə olunur. | `docs/source/compliance/android/device_lab_reservation.md` | Rezervasyonlar yaradıldıqda/dəyişdirildikdə yeniləyin. | Prosedurda qeyd olunan `_android-device-lab` bilet ID-lərinə və həftəlik təqvim ixracına istinad edin. |
| Cihaz-laboratoriya uğursuzluq runbook & drill bundle | Rüblük məşq planı və artefakt geri dönüş zolaqlarını, Firebase partlayış növbəsini və xarici StrongBox saxlayıcı hazırlığını nümayiş etdirir. | `docs/source/compliance/android/device_lab_failover_runbook.md` + `artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` | Rüblük (və ya hardware siyahısı dəyişikliklərindən sonra). | Qazma ID-lərini sübut jurnalına daxil edin və runbook-da qeyd olunan manifest hash + PagerDuty ixracını əlavə edin. |

> **İpucu:** PDF-ləri və ya xaricdən imzalanmış artefaktları əlavə edərkən, qısa mətni saxlayın
> Dəyişməz artefaktla əlaqə saxlayan cədvəlli yolda Markdown sarğı
> idarəetmə payı. Bu qoruyarkən repo yüngül saxlayır
> audit izi.

## Aİ Tənzimləmə Paketi (ETSI/GDPR)Aİ paketi yuxarıda göstərilən üç artefaktı və hüquqi memoanı birləşdirir:

- `security_target.md`-i buraxılış identifikatoru, Torii manifest hash ilə yeniləyin,
  və SBOM həzm edir ki, auditorlar ikili faylları elan edilmiş əhatə dairəsinə uyğunlaşdıra bilsinlər.
- DPIA xülasəsini ən son telemetriya redaktəsi siyasətinə uyğun saxlayın və
  `docs/source/sdk/android/telemetry_redaction.md`-də istinad edilən Norito fərq çıxarışını əlavə edin.
- SBOM attestasiyasına aşağıdakılar daxil edilməlidir: CycloneDX JSON hash, mənşə
  paket hash, kosign bəyanatı və onları yaradan Buildkite iş URL-i.
- `legal_signoff_memo.md` məsləhəti/tarixi tutmalı, hər artefaktı sadalamalıdır +
  SHA-256, hər hansı kompensasiya nəzarətini təsvir edin və sübut jurnalı sırasına keçid edin
  üstəgəl təsdiqi izləyən PagerDuty bilet ID-si.

## Yaponiya Tənzimləmə Paketi (FISC/StrongBox)

Yaponiya tənzimləyiciləri ikidilli sənədlərlə paralel paket gözləyir:

- `fisc_controls_checklist.md` rəsmi cədvəli əks etdirir; hər ikisini doldurun
  EN və JA sütunları və `sdk/android/security.md` xüsusi bölməsinə istinad edin
  və ya hər bir nəzarəti təmin edən StrongBox attestasiya paketi.
- `strongbox_attestation.md` ən son buraxılışlarını ümumiləşdirir
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  (bir cihaz üçün JSON + Norito zərfləri). Dəyişməz artefaktlara bağlantılar daxil edin
  `artifacts/android/attestation/<device>/` altında və fırlanma kadansını qeyd edin.
- İçərisində təqdimatlarla göndərilən ikidilli örtük məktubu şablonunu qeyd edin
  `docs/source/compliance/android/jp/README.md` beləliklə, Dəstək onu təkrar istifadə edə bilər.
- Sübut jurnalını yoxlama siyahısına istinad edən bir sıra ilə yeniləyin
  attestasiya paketi hash və çatdırılma ilə əlaqəli hər hansı JP tərəfdaş bilet ID-ləri.

## Təqdimat İş axını

1. **Qaralama** - Sahib artefaktı hazırlayır, planlaşdırılan fayl adını qeyd edir
   yuxarıdakı cədvələ baxın və yenilənmiş Markdown kötüyü üstəgəl a olan PR açır
   xarici əlavənin yoxlama cəmi.
2. **İcmal** - Buraxılış Mühəndisliyi mənbə hashlərinin səhnələşdirilənlərə uyğun olduğunu təsdiqləyir
   ikili; Uyğunluq tənzimləmə dilini yoxlayır; Dəstək SLA-ları təmin edir və
   telemetriya siyasətlərinə düzgün istinad edilir.
3. **Sign-off** - Təsdiq edənlər öz adlarını və tarixlərini `Sign-off` cədvəlinə əlavə edirlər.
   aşağıda. Sübut jurnalı PR URL və Buildkite proqramı ilə yenilənir.
4. **Yayımla** - SRE idarəçiliyi imzalandıqdan sonra artefaktı daxil edin
   `status.md` və Android Support Playbook istinadlarını yeniləyin.

### Qeydiyyatdan çıxmaq jurnalı

| Artefakt | Nəzərdən keçirən | Tarix | PR / Sübut |
|----------|-------------|------|---------------|
| *(gözləyir)* | - | - | - |

## Cihaz Laboratoriyası Rezervasiyası və Fövqəladə Hallar Planı

Yol xəritəsində **cihaz laboratoriyasının mövcudluğu** riskini azaltmaq üçün:- `docs/source/compliance/android/evidence_log.csv`-də həftəlik tutumu izləyin
  (sütun `device_lab_capacity_pct`). Mövcud olduqda Alert Release Engineering
  iki həftə ardıcıl olaraq 70%-dən aşağı düşür.
- Aşağıdakı StrongBox/ümumi zolaqları qoruyun
  `docs/source/compliance/android/device_lab_reservation.md` hər birini qabaqlayır
  dondurma, məşq və ya uyğunluq süpürülməsi üçün sorğular, təsdiqlər və artefaktlar
  `_android-device-lab` növbəsində tutulur. Yaranan bilet identifikatorlarını əlaqələndirin
  tutumlu anlıq görüntüləri qeyd edərkən sübut jurnalında.
- **Reklam hovuzları:** əvvəlcə paylaşılan Pixel hovuzuna daxil olun; hələ doymuşsa,
  CI təsdiqi üçün Firebase Test Laboratoriyasının tüstüsünü planlaşdırın.
- **Xarici laboratoriya saxlayıcısı:** saxlayıcını StrongBox tərəfdaşı ilə saxlayın
  lab. beləliklə, pəncərələrin dondurulması zamanı (minimum 7 gün müddətində) avadanlığı rezerv edə bilək.
- **Eskalasiya:** PagerDuty-də `AND6-device-lab` insidentinin artması
  ilkin və ehtiyat hovuzlar tutumun 50%-dən aşağı düşür. Hardware Laboratoriyası Rəhbəri
  cihazları yenidən prioritetləşdirmək üçün SRE ilə əlaqələndirir.
- **Yüksəlmə sübut paketləri:** hər məşqi altında saxlayın
  Rezervasiya ilə `artifacts/android/device_lab_contingency/<YYYYMMDD>/`
  sorğu, PagerDuty ixracı, hardware manifest və bərpa transkripti. İstinad
  paketini `device_lab_contingency.md`-dən əldə edin və SHA-256-nı sübut jurnalına əlavə edin
  beləliklə, hüquqi, fövqəladə iş axınının həyata keçirildiyini sübut edə bilər.
- **Rüblük məşqlər:** Runbook-da istifadə edin
  `docs/source/compliance/android/device_lab_failover_runbook.md`, əlavə edin
  nəticədə paket yolu + `_android-device-lab` biletinə manifest hash və
  qazma identifikatorunu həm fövqəladə hallar jurnalında, həm də sübut jurnalında əks etdirin.

Fövqəladə hallar planının hər aktivləşdirilməsini sənədləşdirin
`docs/source/compliance/android/device_lab_contingency.md` (tarixi daxil edin,
tetikleyici, hərəkətlər və təqiblər).

## Statik Analiz Prototipi

- `make android-lint` `ci/check_android_javac_lint.sh` sarar, tərtib edir
  `java/iroha_android` və paylaşılan `java/norito_java` mənbələri
  `javac --release 21 -Xlint:all -Werror` (bayraqlanmış kateqoriyalar ilə
- Kompilyasiyadan sonra skript AND6 asılılıq siyasətini tətbiq edir
  `jdeps --summary`, təsdiq edilmiş icazə siyahısından kənar hər hansı bir modul uğursuz olarsa
  (`java.base`, `java.net.http`, `jdk.httpserver`) görünür. Bu saxlayır
  Android səthi SDK şurasının "gizli JDK asılılığı yoxdur" ilə uyğunlaşdırılıb
  StrongBox uyğunluğunun nəzərdən keçirilməsindən əvvəl tələb.
- CI indi eyni qapıdan keçir
  `.github/workflows/android-lint.yml`, çağırır
  Android və ya toxunan hər təkan/PR-də `ci/check_android_javac_lint.sh`
  paylaşılan Norito Java mənbələri və yükləmələri `artifacts/android/lint/jdeps-summary.txt`
  beləliklə, uyğunluq rəyləri yenidən işə salınmadan imzalanmış modul siyahısına istinad edə bilər
  yerli skript.
- Müvəqqəti saxlamaq lazım olduqda `ANDROID_LINT_KEEP_WORKDIR=1` seçin
  iş sahəsi. Skript artıq yaradılan modul xülasəsini kopyalayır
  `artifacts/android/lint/jdeps-summary.txt`; təyin edin
  `ANDROID_LINT_SUMMARY_OUT=docs/source/compliance/android/evidence/android_lint_jdeps.txt`
  (və ya oxşar) yoxlamalar üçün əlavə, versiyalı artefakt tələb etdikdə.
  Mühəndislər Android PR-lərini təqdim etməzdən əvvəl komandanı hələ də yerli olaraq icra etməlidirlər
  Java mənbələrinə toxunun və qeydə alınmış xülasəni/loqu uyğunluğa əlavə edin
  rəylər. Buraxılış qeydlərindən onu “Android javac lint + asılılığı” kimi istinad edin
  skan edin”.

## CI Sübutları (Lint, Testlər, Attestasiya)- `.github/workflows/android-and6.yml` indi bütün AND6 qapılarını idarə edir (javac lint +
  asılılıq skanı, Android test dəsti, StrongBox attestasiya təsdiqləyicisi və
  Android səthinə toxunan hər bir PR/pushda cihaz-laboratoriya yuvasının yoxlanılması).
- `ci/run_android_tests.sh` `ci/run_android_tests.sh`-i sarar və yayır
  isə `artifacts/android/tests/test-summary.json`-də deterministik xülasə
  konsol jurnalını `artifacts/android/tests/test.log`-ə saxlamaq. Hər ikisini bağlayın
  faylları CI işlərinə istinad edərkən uyğunluq paketlərinə.
- `scripts/android_strongbox_attestation_ci.sh --summary-out` istehsal edir
  `artifacts/android/attestation/ci-summary.json`, paketi təsdiqləyir
  StrongBox və üçün `artifacts/android/attestation/**` altında attestasiya zəncirləri və
  TEE hovuzları.
- `scripts/check_android_device_lab_slot.py --root fixtures/android/device_lab`
  CI-də istifadə edilən nümunə yuvasını (`slot-sample/`) yoxlayır və işarələnə bilər
  real ilə `artifacts/android/device_lab/<slot-id>/` altında çalışır
  `--require-slot --json-out <dest>` cihaz paketlərini sübut etmək üçün izləyin
  sənədləşdirilmiş tərtibat. CI doğrulama xülasəsini yazır
  `artifacts/android/device_lab/summary.json`; nümunə yuvası daxildir
  yer tutucu telemetriya/attestasiya/növbə/log çıxarışları üstəgəl qeydə alınmışdır
  Təkrarlana bilən hashlər üçün `sha256sum.txt`.

## Cihaz-Lab Alətləri İş Akışı

Hər bir rezervasiya və ya uğursuzluq məşqi aşağıdakılara əməl etməlidir
`device_lab_instrumentation.md` bələdçi belə telemetriya, növbə və attestasiya
artefaktlar sifariş jurnalı ilə düzülür:

1. **Toxum yuvası artefaktları.** Yaradın
   Standart alt qovluqlarla `artifacts/android/device_lab/<slot>/` və işə salın
   Yuva bağlandıqdan sonra `shasum` (yeni sənədin “Artifact Layout” bölməsinə baxın.
   bələdçi).
2. **Alətlər üzrə əmrləri yerinə yetirin.** Telemetriya/növbə tutmasını yerinə yetirin,
   həzm, StrongBox qoşqu və lint/asılılıq skanını tam olaraq ləğv edin
   sənədləşdirilib ki, çıxışlar CI-i əks etdirir.
3. **Fayl sübutu.** Yeniləmə
   `docs/source/compliance/android/evidence_log.csv` və rezervasiya bileti
   slot ID, SHA-256 manifest yolu və müvafiq idarə paneli/Buildkite ilə
   keçidlər.

Artefakt qovluğunu və hash manifestini AND6 buraxılış paketinə əlavə edin
təsirlənmiş dondurma pəncərəsi. İdarəetmə rəyçiləri bunu edən yoxlama siyahılarını rədd edəcəklər
yuva identifikatoruna və alətlər təlimatına istinad etməyin.

### Rezervasiya və təhvil verməyə hazırlığın sübutu

Yol xəritəsinin “Tənzimləyici artefakt təsdiqləri və laboratoriya vəziyyəti” elementi daha çox şey tələb edir
alətlərdən daha çox. Hər bir AND6 paketi həmçinin proaktivə istinad etməlidir
rezervasiya iş axını və rüblük uğursuzluq məşqi:- **Rezervasiya kitabçası (`device_lab_reservation.md`).** Rezervasiyanı izləyin
  cədvəl (göndərmə vaxtları, sahiblər, yuva uzunluğu), paylaşılan təqvimi vasitəsilə ixrac edin
  `scripts/android_device_lab_export.py` və `_android-device-lab` qeyd edin
  `evidence_log.csv`-də tutum anlıq görüntüləri ilə yanaşı bilet ID-ləri. Oyun kitabı
  eskalasiya nərdivanını və fövqəladə halların tətiklərini izah edir; həmin detalları köçürün
  rezervasiyalar hərəkət etdikdə və ya tutum aşağı düşdükdə yoxlama siyahısına daxil olun
  80% yol xəritəsi hədəfi.
- **Failover drill runbook (`device_lab_failover_runbook.md`).** İcra edin
  rüblük məşq (kesilməni simulyasiya et → ehtiyat zolaqları təşviq et → məşğul
  Firebase partlaması + xarici StrongBox tərəfdaşı) və artefaktları altında saxlayın
  `artifacts/android/device_lab_contingency/<drill-id>/`. Hər bir paket olmalıdır
  manifest, PagerDuty ixracı, Buildkite keçidləri, Firebase partlaması ehtiva edir
  hesabat və iş kitabçasında qeyd olunan işçinin təsdiqi. istinad edin
  qazma ID, SHA-256 manifest və həm sübut jurnalında, həm də izləmə bileti
  bu yoxlama siyahısı.

Bu sənədlər birlikdə sübut edir ki, cihazın tutumunun planlaşdırılması, kəsilmə məşqləri,
və cihaz paketləri tələb etdiyi eyni yoxlanılmış izi paylaşır
yol xəritəsi və hüquqi rəyçilər.

## Təkmilləşdirməni nəzərdən keçirin

- **Rüblük** - Aİ/JP artefaktlarının yeni olduğunu təsdiqləyin; təzələyin
  sübut jurnalının hashləri; mənşəyi ələ keçirməyi məşq edin.
- **Öncədən buraxılış** - Hər GA/LTS kəsilməsi zamanı bu yoxlama siyahısını işə salın və əlavə edin
  buraxılış RFC-yə tamamlanmış jurnal.
- **Hadisədən sonrakı** - Sev 1/2 insident telemetriya, imzalama və ya toxunursa
  attestasiya, remediasiya qeydləri ilə müvafiq artefakt kötüklərini yeniləmək və
  sübut jurnalında arayışı ələ keçirin.