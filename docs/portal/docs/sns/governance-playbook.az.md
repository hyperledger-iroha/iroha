---
lang: az
direction: ltr
source: docs/portal/docs/sns/governance-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aa4560c51d066ce5c63581dd102aef4e70786d140790fb157323df2553b15f4b
source_last_modified: "2026-01-28T17:11:30.700959+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
id: idarəçilik-oyun kitabı
başlıq: Sora Name Service Governance Playbook
sidebar_label: İdarəetmə kitabçası
təsvir: SN-1/SN-6 tərəfindən istinad edilən şura, qəyyum, stüard və qeydiyyatçı iş axınları üçün Runbook.
---

:::Qeyd Kanonik Mənbə
Bu səhifə `docs/source/sns/governance_playbook.md`-i əks etdirir və indi kimi xidmət edir
kanonik portal nüsxəsi. Mənbə faylı tərcümə PR-ləri üçün saxlanılır.
:::

# Sora Name Service Governance Playbook (SN-6)

**Statusu:** 24-03-2026-cı il tarixdə tərtib edilib — SN-1/SN-6 hazırlığı üçün canlı arayış  
**Yol xəritəsi bağlantıları:** SN-6 “Uyğunluq və Mübahisələrin Həlli”, SN-7 “Həlledici və Gateway Sync”, ADDR-1/ADDR-5 ünvan siyasəti  
**Tələblər:** [`registry-schema.md`](./registry-schema.md]-də reyestr sxemi, [`registrar-api.md`](./registrar-api.md)-də registrator API müqaviləsi, UX təlimatına ünvan [`address-display-guidelines.md`](./address-display-guidelines.md) və [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md) daxilində hesab strukturu qaydaları.

Bu dərslik Sora Name Service (SNS) idarəetmə orqanlarının necə qəbul etdiyini təsvir edir
nizamnamələr, qeydiyyatları təsdiqləmək, mübahisələri gücləndirmək və sübut etmək ki, həlledici və
Gateway dövlətləri sinxron qalır. Bu, yol xəritəsinin tələbini yerinə yetirir
`sns governance ...` CLI, Norito manifestləri və audit artefaktları vahidi paylaşır
N1-dən əvvəl operatorla əlaqəli istinad (ictimai işə salınma).

## 1. Əhatə və Auditoriya

Sənəd hədəfləri:

- Nizamnamələr, şəkilçi siyasətlər və mübahisələr üzrə səs verən İdarəetmə Şurasının üzvləri
  nəticələr.
- Qəyyumlar şurasının üzvləri fövqəladə dondurma verir və geri dönüşləri nəzərdən keçirir.
- Qeydiyyatçı növbələrini idarə edən, auksionları təsdiqləyən və idarə edən şəkilçi stüardlar
  gəlir bölgüsü.
- SoraDNS yayılması, GAR yeniləmələri üçün cavabdeh olan həlledici/şluz operatorları,
  və telemetriya qoruyucuları.
- Uyğunluq, xəzinə və dəstək qrupları bunu hər bir nümayiş etdirməlidir
  idarəetmə fəaliyyəti audit edilə bilən Norito artefaktlarını buraxdı.

O, qapalı beta (N0), ictimai buraxılış (N1) və genişləndirmə (N2) mərhələlərini əhatə edir.
hər bir iş axını tələb olunan sübutlarla əlaqələndirməklə `roadmap.md`-də sadalanan,
idarə panelləri və eskalasiya yolları.

## 2. Rollar və Əlaqə Xəritəsi

| Rol | Əsas vəzifələr | İlkin artefaktlar və telemetriya | Eskalasiya |
|------|----------------------|----------------------------------------|------------|
| İdarəetmə Şurası | Nizamnamələr, şəkilçi siyasətlər, mübahisələrə dair qərarlar və stüard rotasiyalarını hazırlayın və ratifikasiya edin. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, şura bülletenləri `sns governance charter submit` vasitəsilə saxlanılır. | Şura sədri + idarəetmə sənəd izləyicisi. |
| Qəyyumlar Şurası | Yumşaq/sərt dondurma, fövqəladə hallar və 72 saatlıq rəylər verin. | `sns governance freeze` tərəfindən buraxılan qəyyum biletləri, `artifacts/sns/guardian/*` altında daxil edilmiş manifestləri ləğv edir. | Qəyyumun çağırış fırlanması (≤15 dəq ACK). |
| Suffiks Stüardlar | Qeydiyyatçı növbələrini, auksionları, qiymət səviyyələrini və müştəri məlumatlarını idarə etmək; uyğunluğu etiraf etmək. | `SuffixPolicyV1`-də stüard siyasətləri, qiymət arayış vərəqləri, tənzimləyici qeydlərin yanında saxlanılan stüard təsdiqləri. | Stüard proqram rəhbəri + şəkilçiyə məxsus PagerDuty. |
| Qeydiyyatçı və Faturalandırma Əməliyyatları | `/v1/sns/*` son nöqtələrini idarə edin, ödənişləri uzlaşdırın, telemetriya buraxın və CLI anlık görüntülərini qoruyun. | Qeydiyyatçı API ([`registrar-api.md`](./registrar-api.md)), `sns_registrar_status_total` ölçüləri, `artifacts/sns/payments/*` altında arxivləşdirilmiş ödəniş sübutları. | Qeydiyyatın növbətçi meneceri və xəzinədarlıq əlaqəsi. |
| Resolver & Gateway Operators | SoraDNS, GAR və şlüz vəziyyətini registrator hadisələri ilə uyğunlaşdırın; axın şəffaflığı ölçüləri. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Resolver SRE on-call + Gateway əməliyyat körpüsü. |
| Xəzinədarlıq və Maliyyə | 70/30 gəlir bölgüsü, müraciətlərin ayrılması, vergi/xəzinə sənədləri və SLA sertifikatları tətbiq edin. | Gəlir hesablama manifestləri, Şerit/xəzinə ixracı, `docs/source/sns/regulatory/` altında rüblük KPI əlavələri. | Maliyyə nəzarətçisi + uyğunluq məmuru. |
| Uyğunluq və Tənzimləmə Əlaqəsi | Qlobal öhdəlikləri (Aİ DSA və s.) izləyin, KPI müqavilələrini yeniləyin və açıqlamalar verin. | `docs/source/sns/regulatory/`-də tənzimləyici qeydlər, istinad lövhələri, stolüstü məşqlər üçün `ops/drill-log.md` qeydləri. | Uyğunluq proqramı rəhbəri. |
| Dəstək / SRE On-zəng | İnsidentləri idarə edin (toqquşmalar, faktura sürüşməsi, həlledicinin kəsilməsi), müştəri mesajlaşmasını koordinasiya edin və öz runbook-larına sahib olun. | Hadisə şablonları, `ops/drill-log.md`, mərhələli laboratoriya sübutları, `incident/` altında arxivləşdirilmiş Slack/müharibə otağı transkriptləri. | SNS çağırış fırlanması + SRE idarəetməsi. |

## 3. Kanonik Artefaktlar və Məlumat Mənbələri

| Artefakt | Məkan | Məqsəd |
|----------|----------|---------|
| Nizamnamə + KPI əlavəsi | `docs/source/sns/governance_addenda/` | Versiya ilə idarə olunan imzalanmış nizamnamələr, KPI müqavilələri və CLI səsləri ilə istinad edilən idarəetmə qərarları. |
| Reyestr sxemi | [`registry-schema.md`](./registry-schema.md) | Kanonik Norito strukturları (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Qeydiyyatçı müqaviləsi | [`registrar-api.md`](./registrar-api.md) | REST/gRPC yükləri, `sns_registrar_status_total` ölçüləri və idarəetmə çəngəl gözləntiləri. |
| Ünvan UX bələdçisi | [`address-display-guidelines.md`](./address-display-guidelines.md) | Kanonik i105 (üstünlük verilir) + pul kisələri/kəşfiyyatçılar tərəfindən əks olunmuş sıxılmış (`sora`, ikinci ən yaxşı) renderlər. |
| SoraDNS / GAR sənədləri | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Deterministik host törəməsi, şəffaflıq təminatçısı iş axını və xəbərdarlıq qaydaları. |
| Tənzimləyici qeydlər | `docs/source/sns/regulatory/` | Yurisdiksiyaya aid qəbul qeydləri (məsələn, AB DSA), stüard təsdiqləri, şablon əlavələri. |
| Qazma jurnalı | `ops/drill-log.md` | Faza çıxışlarından əvvəl tələb olunan xaos və IR məşqlərinin qeydi. |
| Artefakt saxlama | `artifacts/sns/` | `sns governance ...` tərəfindən istehsal edilən ödəniş sübutları, qəyyum biletləri, həlledici fərqlər, KPI ixracları və imzalanmış CLI çıxışı. |

Bütün idarəetmə tədbirləri yuxarıdakı cədvəldə ən azı bir artefakt istinad etməlidir
beləliklə, auditorlar 24 saat ərzində qərar yolunu yenidən qura bilərlər.

## 4. Lifecycle Playbooks

### 4.1 Nizamnamə və Stüard Hərəkətləri

| Addım | Sahibi | CLI / Sübut | Qeydlər |
|------|-------|----------------|-------|
| Əlavə layihəsi və KPI deltaları | Şura məruzəçisi + stüard aparıcı | Markdown şablonu `docs/source/sns/governance_addenda/YY/` | altında saxlanılır KPI müqavilə identifikatorlarını, telemetriya qarmaqlarını və aktivləşdirmə şərtlərini daxil edin. |
| Təklif göndər | Şura sədri | `sns governance charter submit --input SN-CH-YYYY-NN.md` (`CharterMotionV1` istehsal edir) | CLI `artifacts/sns/governance/<id>/charter_motion.json` altında saxlanılan Norito manifestini yayır. |
| Səs və qəyyumun təsdiqi | Şura + qəyyumlar | `sns governance ballot cast --proposal <id>` və `sns governance guardian-ack --proposal <id>` | Hashed protokolları və kvorum sübutlarını əlavə edin. |
| Stüard qəbulu | Stüard proqramı | `sns governance steward-ack --proposal <id> --signature <file>` | Suffiks siyasəti dəyişməzdən əvvəl tələb olunur; `artifacts/sns/governance/<id>/steward_ack.json` altında qeyd zərfi. |
| Aktivləşdirmə | Qeydiyyatçı əməliyyatları | `SuffixPolicyV1`-i yeniləyin, qeydiyyatçı keşlərini yeniləyin, qeydi `status.md`-də dərc edin. | Aktivləşdirmə vaxt damğası `sns_governance_activation_total`-ə daxil edildi. |
| Audit jurnalı | Uyğunluq | Girişi `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`-ə əlavə edin və masa üstü yerinə yetirilirsə, qazma jurnalı. | Telemetriya panellərinə və siyasət fərqlərinə istinadlar daxil edin. |

### 4.2 Qeydiyyat, Hərrac və Qiymət Təsdiqləri

1. **Ön uçuş:** Qiymət səviyyəsini təsdiqləmək üçün qeydiyyatçı `SuffixPolicyV1` sorğusu göndərir,
   mövcud şərtlər və lütf/ödəniş pəncərələri. Qiymət cədvəllərini sinxronlaşdırın
   sənədləşdirilmiş 3/4/5/6–9/10+ səviyyəli cədvəl (əsas səviyyə + şəkilçi əmsalları)
   yol xəritəsi.
2. **Möhürlənmiş təklif auksionları:** Premium hovuzlar üçün 72 saatlıq öhdəlik / 24 saat aşkar edin
   dövrü `sns governance auction commit` / `... reveal` vasitəsilə. Öhdəliyi dərc edin
   `artifacts/sns/auctions/<name>/commit.json` altında siyahı (yalnız hashlər).
   auditorlar təsadüfiliyi yoxlaya bilərlər.
3. **Ödənişin yoxlanılması:** Qeydiyyatçılar `PaymentProofV1` ilə təsdiqləyir
   xəzinə bölgüsü (70% xəzinədarlıq / 30% stüard, ≤10% göndəriş bölgüsü).
   Norito JSON-u `artifacts/sns/payments/<tx>.json` altında saxlayın və onu birləşdirin
   qeydiyyatçı cavabı (`RevenueAccrualEventV1`).
4. **İdarəetmə qarmağı:** Premium/mühafizə olunan adlar üçün `GovernanceHookV1` əlavə edin
   şura təklifinin şəxsiyyət vəsiqələrinə və stüard imzalarına istinad edir. Çatışmayan qarmaqlar nəticəsi
   `sns_err_governance_missing`-də.
5. **Aktivləşdirmə + həlledici sinxronizasiya:** Torii reyestr hadisəsini yaydıqdan sonra işə salın
   yayılan yeni GAR/zona vəziyyətini təsdiqləmək üçün həlledici şəffaflıq tayları
   (bax §4.5).
6. **Müştəri açıqlaması:** Müştəri ilə bağlı kitabçanı yeniləyin (pul kisəsi/kəşfiyyatçı)
   paylaşılan qurğular vasitəsilə [`address-display-guidelines.md`](./address-display-guidelines.md), i105 və
   sıxılmış renderlər surət/QR təlimatına uyğun gəlir.

### 4.3 Yeniləmələr, Faturalandırma və Xəzinədarlığın Üzləşdirilməsi- **Yeniləmə iş axını:** Qeydiyyatçılar 30 günlük güzəşt + 60 günlük ödənişi tətbiq edirlər
  `SuffixPolicyV1`-də göstərilən pəncərələr. 60 gündən sonra Hollandiya ardıcıllığı yenidən açır
  (7gün, 10× ödəniş 15%/gün) `sns idarəçiliyi vasitəsilə avtomatik olaraq işə salınır
  yenidən açın`.
- **Gəlir bölgüsü:** Hər bir yeniləmə və ya köçürmə a yaradır
  `RevenueAccrualEventV1`. Xəzinə ixracatı (CSV/Parket) ilə uzlaşmalıdır
  bu hadisələr gündəlik; sübutları `artifacts/sns/treasury/<date>.json`-ə əlavə edin.
- **Referral oymalar:** Könüllü müraciət faizləri hər şəkilçiyə görə izlənilir
  stüard siyasətinə `referral_share` əlavə etməklə. Qeydiyyatçılar finalı verirlər
  Ödəniş sübutunun yanında ayırma və saxlama yönləndirməsi.
- **Hesabat kadansı:** Maliyyə aylıq KPI əlavələrini (qeydiyyatlar,
  yenilənmələr, ARPU, mübahisə/istiqraz istifadəsi) altında
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Tablolar çəkilməlidir
  eyni ixrac edilmiş cədvəllər, buna görə də Grafana nömrələri kitab dəlillərinə uyğun gəlir.
- **Aylıq KPI icmalı:** İlk çərşənbə axşamı yoxlama məntəqəsi maliyyə rəhbərini cütləşdirir,
  növbətçi stüard və proqram PM. [SNS KPI tablosunu] açın (./kpi-dashboard.md)
  (`sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json` portalının yerləşdirilməsi),
  registrator ötürücülük qabiliyyətini + gəlir cədvəllərini, əlavədəki log deltalarını ixrac edin,
  və artefaktları yaddaşa əlavə edin. Nəzərdən keçirmə aşkar edərsə, hadisəyə səbəb olun
  SLA pozuntuları (pəncərələri >72 saat dondurun, qeydiyyatçı xətası sıçrayışları, ARPU sürüşməsi).

### 4.4 Dondurmalar, Mübahisələr və Apellyasiya

| Faza | Sahibi | Fəaliyyət və Sübut | SLA |
|-------|-------|-------------------|-----|
| Yumşaq dondurma sorğusu | Stüard / dəstək | Ödəniş sübutları, mübahisəli istiqraz arayışı və təsirə məruz qalan seçici(lər) ilə `SNS-DF-<id>` fayl bileti. | qəbuldan ≤4 saat. |
| Qəyyum bileti | Mühafizə şurası | `sns governance freeze --selector <i105> --reason <text> --until <ts>` imzalı `GuardianFreezeTicketV1` istehsal edir. JSON biletini `artifacts/sns/guardian/<id>.json` altında saxlayın. | ≤30dəq ACK, ≤2saat icra. |
| Şuranın ratifikasiyası | İdarəetmə şurası | Dondurulmaları təsdiq və ya rədd edin, qəyyum biletinə sənəd qərarının linki və mübahisəli borc həzmi. | Növbəti şura sessiyası və ya asinxron səsvermə. |
| Arbitraj paneli | Uyğunluq + stüard | `sns governance dispute ballot` vasitəsilə təqdim edilmiş bülletenlərlə 7 münsiflər heyətini (yol xəritəsi üzrə) çağırın. Anonim səs qəbzlərini insident paketinə əlavə edin. | İstiqraz depozitindən ≤7 gün sonra qərar. |
| Müraciət | Qəyyum + şura | Apellyasiya şikayəti ikiqat artırır və andlı iclasçı prosesini təkrarlayır; qeyd Norito manifest `DisputeAppealV1` və istinad əsas bilet. | ≤10 gün. |
| Dondurma və remediasiya | Qeydiyyatçı + həlledici əməliyyatlar | `sns governance unfreeze --selector <i105> --ticket <id>`-i icra edin, qeydiyyatçı statusunu yeniləyin və GAR/həlledici fərqləri təbliğ edin. | Qərardan dərhal sonra. |

Fövqəladə hallar (qəyyumun yaratdığı donmalar ≤72 saat) eyni axını izləyir, lakin
retroaktiv şuranın nəzərdən keçirilməsini və şəffaflıq qeydini tələb edir
`docs/source/sns/regulatory/`.

### 4.5 Resolver & Gateway Propagation

1. **Hadisə qarmağı:** Hər bir qeyd hadisəsi həlledici hadisə axınına ötürür
   (`tools/soradns-resolver` SSE). Həlledici əməliyyatlar vasitəsilə abunə olun və fərqləri qeyd edin
   şəffaflıq tədarükçüsü (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **GAR şablon yeniləməsi:** Şlüzlər istinad edilən GAR şablonlarını yeniləməlidir
   `canonical_gateway_suffix()` və `host_pattern` siyahısını yenidən imzalayın. Mağaza fərqləri
   `artifacts/sns/gar/<date>.patch`-də.
3. **Zonefile nəşri:**-da təsvir olunan zona faylı skeletindən istifadə edin
   `roadmap.md` (ad, ttl, cid, sübut) və onu Torii/SoraFS-ə itələyin. Arxiv edin
   `artifacts/sns/zonefiles/<name>/<version>.json` altında Norito JSON.
4. **Şəffaflıq yoxlanışı:** `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`-i işə salın
   siqnalların yaşıl qalmasını təmin etmək üçün. Prometheus mətn çıxışını əlavə edin
   həftəlik şəffaflıq hesabatı.
5. **Gateway auditi:** `Sora-*` başlıq nümunələrini qeyd edin (keş siyasəti, CSP, GAR)
   həzm edin) və onları idarəetmə jurnalına əlavə edin ki, operatorlar sübut edə bilsinlər
   Gateway nəzərdə tutulan qoruyucu barmaqlıqlarla yeni ada xidmət edirdi.

## 5. Telemetriya və Hesabat

| Siqnal | Mənbə | Təsvir / Fəaliyyət |
|--------|--------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Torii registrator işləyiciləri | Qeydiyyatlar, yeniləmələr, dondurmalar, köçürmələr üçün müvəffəqiyyət/səhv sayğacı; hər şəkilçiyə `result="error"` sıçrayış etdikdə xəbərdarlıq edir. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Torii ölçüləri | API işləyiciləri üçün gecikmə SLOları; `torii_norito_rpc_observability.json`-dən qurulmuş yem panelləri. |
| `soradns_bundle_proof_age_seconds` & `soradns_bundle_cid_drift_total` | Resolver transparency tailer | Köhnə sübutları və ya GAR sürüşməsini aşkar edin; `dashboards/alerts/soradns_transparency_rules.yml`-də müəyyən edilmiş qoruyucu barmaqlıqlar. |
| `sns_governance_activation_total` | İdarəetmə CLI | Nizamnamə/əlavə aktivləşdirildikdə sayğac artırılır; şura qərarları ilə dərc edilmiş əlavələri uzlaşdırmaq üçün istifadə olunur. |
| `guardian_freeze_active` ölçü cihazı | Guardian CLI | Seçici başına yumşaq/sərt dondurma pəncərələrini izləyir; dəyər elan edilmiş SLA-dan kənarda `1` qalsa, səhifə SRE. |
| KPI əlavə tabloları | Maliyyə / Sənədlər | Tənzimləyici qeydlərlə yanaşı dərc olunan aylıq yığımlar; portal onları [SNS KPI tablosuna](./kpi-dashboard.md) vasitəsilə daxil edir ki, stüardlar və tənzimləyicilər eyni Grafana görünüşünə daxil ola bilsinlər. |

## 6. Sübut və Audit Tələbləri

| Fəaliyyət | Arxivə dəlil | Saxlama |
|--------|--------------------|---------|
| Nizamnamə / siyasət dəyişikliyi | İmzalanmış Norito manifest, CLI transkripti, KPI fərqi, stüard təsdiqi. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Qeydiyyat / yenilənmə | `RegisterNameRequestV1` faydalı yük, `RevenueAccrualEventV1`, ödəniş sübutu. | `artifacts/sns/payments/<tx>.json`, registrator API qeydləri. |
| Hərrac | Təqdimat/açıqlama manifestləri, təsadüfilik toxumu, qalibin hesablanması cədvəli. | `artifacts/sns/auctions/<name>/`. |
| Dondurmaq / açmaq | Qəyyum bileti, şura səs hashı, insident qeydinin URL-i, müştəri mesajları şablonu. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Həlledicinin yayılması | Zonefile/GAR fərqi, hazırlayıcı JSONL çıxarışı, Prometheus snapshot. | `artifacts/sns/resolver/<date>/` + şəffaflıq hesabatları. |
| Tənzimləyici qəbul | Qəbul qeydi, son tarix izləyicisi, stüard təsdiqi, KPI dəyişikliyinin xülasəsi. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Faza Qapısının Yoxlama Siyahısı

| Faza | Çıxış meyarları | Sübut paketi |
|-------|---------------|-----------------|
| N0 — Qapalı beta | SN-1/SN-2 reyestr sxemi, əl qeydiyyatçısı CLI, qəyyumluq təlimi tamamlandı. | Nizamnamə hərəkəti + stüard ACK, qeydiyyatçı quru iş qeydləri, həlledici şəffaflıq hesabatı, `ops/drill-log.md`-də qazma girişi. |
| N1 — İctimai işə | Hərraclar + sabit qiymət səviyyələri `.sora`/`.nexus`, özünəxidmət registratoru, həlledici avtomatik sinxronizasiya, faktura panelləri üçün canlıdır. | Qiymətləndirmə vərəqi fərqi, qeydiyyatçı CI nəticələri, ödəniş/KPI əlavəsi, şəffaflıq təminatçısı çıxışı, insident məşq qeydləri. |
| N2 — Genişlənmə | `.dao`, reseller API-ləri, mübahisə portalı, stüard xal kartları, analitik tablolar. | Portal skrinşotları, mübahisəli SLA ölçüləri, stüard bal kartı ixracı, reseller siyasətlərinə istinad edən yenilənmiş idarəetmə nizamnaməsi. |

Faza çıxışları qeydə alınmış masa üstü məşqləri tələb edir (qeydiyyat xoşbəxt yol, dondurma,
həlledicinin kəsilməsi) `ops/drill-log.md`-ə əlavə edilmiş artefaktlarla.

## 8. Hadisəyə Cavab və Eskalasiya

| Tətik | Ciddilik | Dərhal sahibi | Məcburi tədbirlər |
|---------|----------|-----------------|-------------------|
| Resolver/GAR drift və ya köhnəlmiş sübutlar | Sev1 | Resolver SRE + qəyyum şurası | Zəng zamanı səhifə həlledicisi, sifarişçi çıxışını ələ keçirin, təsirlənmiş adların dondurulub-dondurulmayacağına qərar verin, statusu hər 30 dəqiqədən bir yeniləyin. |
| Qeydiyyatçı kəsilməsi, faktura xətası və ya geniş yayılmış API xətaları | Sev1 | Qeydiyyatçı növbətçi menecer | Yeni auksionları dayandırın, manual CLI-yə keçin, stüardlara/xəzinədarlığa xəbər verin, insident sənədinə Torii qeydlərini əlavə edin. |
| Tək ad mübahisəsi, ödəniş uyğunsuzluğu və ya müştəri eskalasiyası | Sev2 | Stüard + dəstək aparıcı | Ödəniş sübutlarını toplayın, yumşaq dondurmanın lazım olub olmadığını müəyyənləşdirin, SLA daxilində sorğuçuya cavab verin, nəticəni mübahisə izləyicisində qeyd edin. |
| Uyğunluq auditinin tapılması | Sev2 | Uyğunluq əlaqəsi | Təmir planının layihəsi, `docs/source/sns/regulatory/` altında fayl memo, təqib şura iclasını planlaşdırın. |
| Qazma və ya məşq | Sev3 | Proqram PM | `ops/drill-log.md`-dən skriptli ssenarini, arxiv artefaktlarını, boşluqları yol xəritəsi tapşırıqları kimi etiketləyin. |

Bütün hadisələr sahibliklə `incident/YYYY-MM-DD-sns-<slug>.md` yaratmalıdır
Cədvəllər, əmr jurnalları və bu müddət ərzində hazırlanmış sübutlara istinadlar
oyun kitabı.

## 9. İstinadlar

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (SNS, DG, ADDR bölmələri)

Hər dəfə nizamnamə mətni, CLI səthləri və ya telemetriya zamanı bu kitabçanı yeniləyin
müqavilələrin dəyişdirilməsi; `docs/source/sns/governance_playbook.md`-ə istinad edən yol xəritəsi qeydləri
həmişə ən son versiyaya uyğun olmalıdır.