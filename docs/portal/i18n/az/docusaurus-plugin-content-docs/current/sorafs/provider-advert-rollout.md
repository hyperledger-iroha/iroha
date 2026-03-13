---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: "SoraFS Provider Advert Rollout Plan"
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md)-dən uyğunlaşdırılıb.

# SoraFS Provayder Reklamının Yayım Planı

Bu plan icazə verən provayder reklamlarından kəsilməsini əlaqələndirir
çoxmənbəli yığın üçün tələb olunan tam idarə olunan `ProviderAdvertV1` səthi
axtarış. O, üç nəticəyə diqqət yetirir:

- **Operator təlimatı.** Yaddaş təminatçıları addım-addım hərəkətləri tamamlamalıdır
  hər qapı fırlanmadan əvvəl.
- **Telemetriya əhatə dairəsi.** Müşahidəetmə və Əməliyyatların istifadə etdiyi idarə panelləri və xəbərdarlıqlar
  təsdiq etmək üçün şəbəkə yalnız uyğun reklamları qəbul edir.
Yayım [SoraFS miqrasiyasında SF-2b/2c mərhələləri ilə uyğunlaşır.
yol xəritəsi](./migration-roadmap) və qəbul siyasətini qəbul edir
[provayderin qəbul siyasəti](./provider-admission-policy) artıq mövcuddur
təsiri.

## Cari Tələblər

SoraFS yalnız idarəetmə ilə örtülmüş `ProviderAdvertV1` faydalı yükləri qəbul edir. The
qəbul zamanı aşağıdakı tələblər yerinə yetirilir:

- `profile_id=sorafs.sf1@1.0.0`, kanonik `profile_aliases` mövcuddur.
- `chunk_range_fetch` qabiliyyətinin faydalı yükləri çoxmənbəli üçün daxil edilməlidir
  axtarış.
- `signature_strict=true` elana əlavə edilmiş şura imzaları ilə
  zərf.
- `allow_unknown_capabilities` yalnız açıq YAĞLAMA məşqləri zamanı icazə verilir
  və qeyd edilməlidir.

## Operator Yoxlama Siyahısı

1. **İnventar reklamları.** Hər dərc olunmuş reklamı siyahıya salın və qeyd edin:
   - İdarəedici zərf yolu (`defaults/nexus/sorafs_admission/...` və ya istehsal ekvivalenti).
   - Reklam `profile_id` və `profile_aliases`.
   - Qabiliyyət siyahısı (ən azı `torii_gateway` və `chunk_range_fetch` gözləyin).
   - `allow_unknown_capabilities` bayrağı (satıcı tərəfindən qorunan TLV-lər mövcud olduqda tələb olunur).
2. **Provayder alətləri ilə bərpa edin.**
   - Provayderinizin reklam yayımçısı ilə faydalı yükü yenidən qurun:
     - `profile_id=sorafs.sf1@1.0.0`
     - müəyyən edilmiş `max_span` ilə `capability=chunk_range_fetch`
     - GREASE TLV-ləri mövcud olduqda `allow_unknown_capabilities=<true|false>`
   - `/v2/sorafs/providers` və `sorafs_fetch` vasitəsilə doğrulayın; bilinməyənlərə dair xəbərdarlıqlar
     imkanları sınaqdan keçirilməlidir.
3. **Çox mənbəli hazırlığı təsdiq edin.**
   - `sorafs_fetch`-i `--provider-advert=<path>` ilə icra edin; CLI indi uğursuz olur
     `chunk_range_fetch` olmadıqda və nəzərə alınmayan naməlum üçün xəbərdarlıqları çap etdikdə
     imkanlar. JSON hesabatını çəkin və əməliyyat qeydləri ilə arxivləşdirin.
4. **Mərhələ yeniləmələri.**
   - `ProviderAdmissionRenewalV1` zərflərini ən azı 30 gün əvvəl təqdim edin
     son istifadə müddəti. Yeniləmələr kanonik tutacaq və qabiliyyət dəstini saxlamalıdır;
     yalnız pay, son nöqtələr və ya metadata dəyişməlidir.
5. **Asılı komandalarla ünsiyyət qurun.**
   - SDK sahibləri operatorlara xəbərdarlıq edən versiyaları buraxmalıdırlar
     reklamlar rədd edilir.
   - DevRel hər bir faza keçidini elan edir; tablosuna bağlantılar və
     eşik məntiqi aşağıdadır.
6. **İdarə panelləri və xəbərdarlıqları quraşdırın.**
   - Grafana ixracını idxal edin və onu **SoraFS / Provayder altında yerləşdirin
     İdarə paneli UID `sorafs-provider-admission` ilə yayım**.
   - Xəbərdarlıq qaydalarının paylaşılan `sorafs-advert-rollout`-ə işarə etdiyinə əmin olun
     səhnələşdirmə və istehsalda bildiriş kanalı.

## Telemetriya və İdarə Panelləri

Aşağıdakı göstəricilər artıq `iroha_telemetry` vasitəsilə ifşa olunub:

- `torii_sorafs_admission_total{result,reason}` — qəbul edilmiş, rədd edilmiş saylar,
  və xəbərdarlıq nəticələri. Səbəblərə `missing_envelope`, `unknown_capability`,
  `stale` və `policy_violation`.

Grafana ixracı: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Faylı paylaşılan tablosuna (`observability/dashboards`) idxal edin
və dərc etməzdən əvvəl yalnız məlumat mənbəyi UID-ni yeniləyin.

Şura Grafana qovluğu altında **SoraFS / Provider Rollout** ilə dərc edir.
sabit UID `sorafs-provider-admission`. Xəbərdarlıq qaydaları
`sorafs-admission-warn` (xəbərdarlıq) və `sorafs-admission-reject` (kritik)
`sorafs-advert-rollout` bildiriş siyasətindən istifadə etmək üçün əvvəlcədən konfiqurasiya edilmişdir; tənzimləmək
Əgər təyinat siyahısı redaktə etmək əvəzinə dəyişərsə, həmin əlaqə nöqtəsi
idarə paneli JSON.

Tövsiyə olunan Grafana panelləri:

| Panel | Sorğu | Qeydlər |
|-------|-------|-------|
| **Qəbul nəticəsi dərəcəsi** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Qəbul etmə ilə xəbərdarlıq etmə və rədd etməni vizuallaşdırmaq üçün yığın diaqramı. Xəbərdarlıq > 0.05 * cəmi (xəbərdarlıq) və ya rədd > 0 (kritik) olduqda xəbərdar olun. |
| **Xəbərdarlıq nisbəti** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Peycer həddini təmin edən tək sətirli vaxt seriyası (5% xəbərdarlıq dərəcəsi 15 dəqiqə). |
| **Rədd etmə səbəbləri** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Sürücülər runbook triage; azaltma addımlarına keçidlər əlavə edin. |
| **Borcunuzu yeniləyin** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Yeniləmə müddətini itirmiş provayderləri göstərir; kəşf keş qeydləri ilə çarpaz istinad. |

Əl ilə idarə olunan tablolar üçün CLI artefaktları:

- `sorafs_fetch --provider-metrics-out` `failures`, `successes` yazır və
  Provayder üçün `disabled` sayğacları. Nəzarət etmək üçün ad-hoc tablosuna idxal edin
  istehsal təminatçılarını dəyişməzdən əvvəl orkestr quru çalışır.
- JSON hesabatının `chunk_retry_rate` və `provider_failure_rate` sahələri
  tez-tez qəbuldan əvvəl olan tənzimləmə və ya köhnə yük əlamətlərini vurğulayın
  rəddlər.

### Grafana tablosunun tərtibatı

Observability xüsusi lövhə dərc edir — **SoraFS Provayder Qəbulu
Yayım** (`sorafs-provider-admission`) — **SoraFS / Provayderin Yayımlanması** altında
aşağıdakı kanonik panel identifikatorları ilə:

- Panel 1 — *Qəbulun nəticəsi dərəcəsi* (yığılmış sahə, vahid “ops/dəq”).
- Panel 2 — *Xəbərdarlıq nisbəti* (tək seriya), ifadəni yayan
  `cəmi(dərəcə(torii_sorafs_qəbul_cəmi{nəticə="xəbərdar et"}[5dq])) /
   məbləğ(dərəcə(torii_sorafs_qəbul_cəmi[5m]))`.
- Panel 3 — *Rədd etmə səbəbləri* (`reason` ilə qruplaşdırılmış vaxt seriyası), sıralama ilə
  `rate(...[5m])`.
- Panel 4 — *Borcu təzələyin* (stat), yuxarıdakı cədvəldəki sorğunun əks olunması və
  miqrasiya kitabçasından çıxarılan reklam yeniləmə müddətləri ilə şərh edilir.

İnfrastruktur tablosunun repo-da JSON skeletini kopyalayın (və ya yaradın).
`observability/dashboards/sorafs_provider_admission.json`, sonra yalnız yeniləyin
məlumat mənbəyi UID; panel identifikatorları və xəbərdarlıq qaydaları runbooks tərəfindən istinad edilir
aşağıda, ona görə də bu sənədlərə yenidən baxmadan onları yenidən nömrələməkdən çəkinin.

Rahatlıq üçün depo indi istinad tablosunun tərifini göndərir
`docs/source/grafana_sorafs_admission.json`; əgər varsa onu Grafana qovluğuna kopyalayın
yerli test üçün başlanğıc nöqtəsinə ehtiyacınız var.

### Prometheus xəbərdarlıq qaydaları

Aşağıdakı qayda qrupunu `observability/prometheus/sorafs_admission.rules.yml`-ə əlavə edin
(bu ilk SoraFS qayda qrupudursa faylı yaradın) və onu daxil edin
Prometheus konfiqurasiyanız. `<pagerduty>`-i faktiki marşrutlaşdırma ilə əvəz edin
zəng zamanı dönüşünüz üçün etiket.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`-i işə salın
sintaksisin `promtool check rules` keçməsini təmin etmək üçün dəyişikliklərə basmadan əvvəl.

## Qəbul Nəticələri

- Çatışmayan `chunk_range_fetch` qabiliyyəti → `reason="missing_capability"` ilə rədd edin.
- `allow_unknown_capabilities=true` olmadan naməlum qabiliyyət TLV-ləri → ilə rədd edin
  `reason="unknown_capability"`.
- `signature_strict=false` → rədd et (təcrid olunmuş diaqnostika üçün qorunur).
- Vaxtı keçmiş `refresh_deadline` → rədd edin.

## Ünsiyyət və Hadisələrin İdarə Edilməsi

- **Həftəlik status poçtu.** DevRel qəbulun qısa xülasəsini təqdim edir
  ölçülər, gözlənilməz xəbərdarlıqlar və qarşıdan gələn son tarixlər.
- **Hadisə cavabı.** Əgər `reject` yanğın xəbərdar edirsə, çağırış üzrə mühəndislər:
  1. Təhqiredici reklamı Torii kəşfi (`/v2/sorafs/providers`) vasitəsilə əldə edin.
  2. Provayder boru kəmərində reklamın yoxlanmasını yenidən işə salın və onunla müqayisə edin
     Xətanı təkrarlamaq üçün `/v2/sorafs/providers`.
  3. Növbəti yeniləmədən əvvəl reklamı çevirmək üçün provayderlə koordinasiya edin
     son tarix.
- **Dəyişiklik donur.** Heç bir qabiliyyət sxemi R1/R2 zamanı torpağı dəyişdirmir
  yayma komissiyası imzalanır; GREASE sınaqları zamanı planlaşdırılmalıdır
  həftəlik texniki xidmət pəncərəsi və miqrasiya kitabçasına daxil edilmişdir.

## İstinadlar

- [SoraFS Node/Müştəri Protokolu](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Provayderin Qəbul Siyasəti](./provider-admission-policy)
- [Miqrasiya Yol Xəritəsi](./migration-roadmap)
- [Provayder Reklamı Çox Mənbəli Artırmaları](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)