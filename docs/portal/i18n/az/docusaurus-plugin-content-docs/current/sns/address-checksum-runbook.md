---
id: address-checksum-runbook
lang: az
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Account Address Checksum Incident Runbook
sidebar_label: Checksum incidents
description: Operational response for IH58 (preferred) / compressed (`sora`, second-best) checksum failures (ADDR-7).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
Bu səhifə `docs/source/sns/address_checksum_failure_runbook.md`-i əks etdirir. Yeniləyin
əvvəlcə mənbə faylını seçin, sonra bu nüsxəni sinxronlaşdırın.
:::

Yoxlama məbləğində uğursuzluqlar `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`) kimi görünür
Torii, SDK-lar və pul kisəsi/kəşfiyyatçı müştəriləri. ADDR-6/ADDR-7 yol xəritəsi elementləri indi
operatorlardan yoxlama məbləği xəbərdarlığı və ya dəstək olduqda bu runbook-a əməl etmələrini tələb edir
biletlər yandı.

## Tamaşa nə vaxt veriləcək

- **Xəbərdarlıqlar:** `AddressInvalidRatioSlo` (
  `dashboards/alerts/address_ingest_rules.yml`) səfərlər və qeydlər siyahısı
  `reason="ERR_CHECKSUM_MISMATCH"`.
- **Fiksturun sürüşməsi:** `account_address_fixture_status` Prometheus mətn faylı və ya
  Grafana idarə paneli istənilən SDK nüsxəsi üçün yoxlama məbləğinin uyğunsuzluğunu bildirir.
- **Dəstək eskalasiyaları:** Pulqabı/kəşfiyyatçı/SDK komandaları yoxlama məbləği səhvlərini, IME-ni göstərir
  korrupsiya və ya artıq deşifrə olunmayan panoya skanlar.
- **Əllə müşahidə:** Torii qeydləri təkrarlanan `address_parse_error=checksum_mismatch` göstərir
  istehsal son nöqtələri üçün.

Hadisə xüsusilə Lokal-8/Yerli-12 toqquşmalarına aiddirsə, izləyin
Əvəzinə `AddressLocal8Resurgence` və ya `AddressLocal12Collision` oyun kitabları.

## Sübut yoxlama siyahısı

| Sübut | Komanda / Yer | Qeydlər |
|----------|-------------------|-------|
| Grafana snapshot | `dashboards/grafana/address_ingest.json` | Etibarsız səbəb qəzalarını və təsirlənmiş son nöqtələri çəkin. |
| Xəbərdarlıq yükü | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | Kontekst etiketləri və vaxt ştamplarını daxil edin. |
| Qurğu sağlamlıq | `artifacts/account_fixture/address_fixture.prom` + Grafana | SDK nüsxələrinin `fixtures/account/address_vectors.json`-dən sürükləndiyini sübut edir. |
| PromQL sorğusu | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | Hadisə sənədi üçün CSV ixrac edin. |
| Qeydlər | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` (yaxud log aqreqasiyası) | Paylaşmadan əvvəl PII-ni silin. |
| Quraşdırma yoxlanışı | `cargo xtask address-vectors --verify` | Kanonik generatoru təsdiqləyir və qəbul edilmiş JSON razılaşır. |
| SDK paritet yoxlanışı | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | Xəbərdarlıqlarda/biletlərdə bildirilmiş hər SDK üçün işləyin. |
| Panoya/IME ağlı başında olma | `iroha tools address inspect <literal>` | Gizli simvolları və ya IME-nin yenidən yazmasını aşkar edir; sitat `address_display_guidelines.md`. |

## Dərhal cavab

1. Xəbərdarlığı qəbul edin, insidentdə Grafana anlıq görüntüləri + PromQL çıxışını əlaqələndirin
   mövzu və qeyd Torii kontekstlərinə təsir etdi.
2. Ünvan təhlilinə toxunan manifest promosyonlarını / SDK relizlərini dondurun.
3. İdarə panelinin şəkillərini və yaradılan Prometheus mətn faylı artefaktlarını burada saxlayın
   hadisə qovluğu (`docs/source/sns/incidents/YYYY-MM/<ticket>/`).
4. `checksum_mismatch` faydalı yükləri göstərən jurnal nümunələrini çəkin.
5. SDK sahiblərini (`#sdk-parity`) nümunə yükləri ilə xəbərdar edin ki, onlar sınaqdan keçirə bilsinlər.

## Kök səbəb izolyasiyası

### Armatur və ya generatorun sürüşməsi

- `cargo xtask address-vectors --verify`-i yenidən işə salın; uğursuz olarsa bərpa edin.
- `ci/account_fixture_metrics.sh` (və ya fərdi
  Paketi təsdiqləmək üçün hər bir SDK üçün `scripts/account_fixture_helper.py check`).
  qurğular kanonik JSON-a uyğun gəlir.

### Müştəri kodlayıcıları / IME reqressiyaları

- Sıfır genişliyi tapmaq üçün `iroha tools address inspect` vasitəsilə istifadəçi tərəfindən verilən hərfləri yoxlayın
  birləşmələr, kana çevrilmələri və ya kəsilmiş yüklər.
- Cross-check pul kisəsi/explorer ilə axır
  `docs/source/sns/address_display_guidelines.md` (iki nüsxə hədəfləri, xəbərdarlıqlar,
  QR köməkçiləri) təsdiq edilmiş UX-ə əməl etmələrini təmin etmək.

### Manifest və ya qeydiyyat problemləri

- Ən son manifest paketini yenidən doğrulamaq üçün `address_manifest_ops.md`-i izləyin və
  heç bir Local-8 seçicisinin yenidən üzə çıxmamasını təmin edin.
  faydalı yüklərdə görünür.

### Zərərli və ya səhv formalaşdırılmış trafik

- Torii qeydləri və `torii_http_requests_total` vasitəsilə təhqiredici IP-ləri/tətbiq identifikatorlarını parçalayın.
- Təhlükəsizlik/İdarəetmə təqibi üçün ən azı 24 saat qeydləri qoruyun.

## Təsirlərin azaldılması və bərpası

| Ssenari | Fəaliyyətlər |
|----------|---------|
| Fikstür sürüşməsi | `fixtures/account/address_vectors.json`-i bərpa edin, `cargo xtask address-vectors --verify`-i yenidən işə salın, SDK paketlərini yeniləyin və biletə `address_fixture.prom` anlıq görüntüləri əlavə edin. |
| SDK/müştəri reqresiyası | Kanonik qurğuya + `iroha tools address inspect` çıxışına və SDK pariteti CI (məsələn, `ci/check_address_normalize.sh`) arxasındakı qapı buraxılışlarına istinad edən fayl problemləri. |
| Zərərli təqdimatlar | Tənqid edən əsasları məhdudlaşdırın və ya bloklayın, məzar daşları seçiciləri tələb olunarsa, İdarəetməyə çatdırın. |

Təsirə məruz qalandan sonra təsdiq etmək üçün yuxarıdakı PromQL sorğusunu təkrar edin
`ERR_CHECKSUM_MISMATCH` ən azı sıfırda qalır (`/tests/*` istisna olmaqla)
Hadisəni endirməzdən 30 dəqiqə əvvəl.

## Bağlama

1. Arxiv Grafana anlıq görüntüləri, PromQL CSV, jurnaldan çıxarışlar və `address_fixture.prom`.
2. `status.md` (ADDR bölməsi) və alətlər/sənədlər varsa, yol xəritəsi cərgəsini yeniləyin
   dəyişdi.
3. Yeni dərslər açıldıqda `docs/source/sns/incidents/` altında insidentdən sonrakı qeydləri fayl edin
   meydana çıxmaq.
4. Mümkün olduqda SDK buraxılış qeydlərində yoxlama məbləğinin düzəlişlərinin qeyd olunduğundan əmin olun.
5. Xəbərdarlığın 24 saat yaşıl qaldığını və armatur yoxlamalarının əvvəl yaşıl qaldığını təsdiq edin
   həll edir.