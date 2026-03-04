---
lang: az
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/economic_analysis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1b453559c05401edc11894e585c8d5ca4b678d4667c1cef0415582e1f7de8246
source_last_modified: "2025-12-29T18:16:35.087502+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# İqtisadi Təhlil - 2025-10 -> 2025-11 Kölgə Qaçış

Mənbə artefakt: `docs/examples/soranet_incentive_shadow_run.json` (imza +
eyni kataloqda açıq açar). Simulyasiya hər relayda 60 dövr təkrarlandı
`RewardConfig`-də qeyd edilmiş mükafat mühərriki ilə
`reward_config.json`.

## Dağıtım Xülasəsi

- **Ümumi ödənişlər:** 360 mükafatlı dövr ərzində 5160 XOR.
- **Ədalətlilik zərfi:** Cini əmsalı 0,121; üst relay payı 23,26%
  (30% idarəetmə qoruyuculuğundan xeyli aşağı).
- **Mövcudluq:** donanma orta hesabla 96,97%, bütün relelər 94%-dən yuxarı qaldı.
- **Bandwidth:** donanma orta hesabla 91,20%, ən aşağı göstərici isə 87,23%
  planlaşdırılmış təmir zamanı; cəzalar avtomatik tətbiq olunurdu.
- **Uyğunluq səs-küyü:** 9 xəbərdarlıq dövrü və 3 dayandırma müşahidə edilib və
  ödənişlərin azaldılmasına çevrilir; heç bir rele 12 xəbərdarlıq həddi keçmədi.
- **Əməliyyat gigiyenası:** əskik olduğuna görə heç bir metrik snepşot buraxılmadı
  konfiqurasiya, istiqrazlar və ya dublikatlar; heç bir kalkulyator xətası buraxılmadı.

## Müşahidələr

- Asma relelərin texniki xidmət rejiminə daxil olduğu dövrlərə uyğundur. The
  ödəniş mühərriki qoruyarkən həmin dövrlər üçün sıfır ödənişlər buraxdı
  kölgədə idarə olunan JSON-da audit izi.
- Xəbərdarlıq cəzaları təsirə məruz qalan ödənişlərdən 2% azaldıldı; nəticəsində
  paylama hələ də iş vaxtı/bant genişliyi çəkiləri (650/350
  promille).
- Bant genişliyi fərqi anonim qoruyucu istilik xəritəsini izləyir. Ən aşağı göstərici
  (`6666...6666`) pəncərə boyunca, 0,6x mərtəbənin üstündə 620 XOR saxladı.
- Gecikməyə həssas xəbərdarlıqlar (`SoranetRelayLatencySpike`) xəbərdarlıqdan aşağıda qaldı
  pəncərə boyunca eşiklər; korrelyasiya panelləri altında tutulur
  `dashboards/grafana/soranet_incentives.json`.

## GA-dan əvvəl Tövsiyə Edilən Fəaliyyətlər

1. Aylıq kölgə təkrarlamalarını davam etdirin və artefakt dəstini yeniləyin və bu
   donanmanın tərkibi dəyişdikdə təhlil.
2. Yol xəritəsində istinad edilən Grafana xəbərdarlıq dəstində avtomatik ödənişləri keçin
   (`dashboards/alerts/soranet_incentives_rules.yml`); ekran görüntülərini kopyalayın
   yenilənmə axtararkən idarəetmə protokolları.
3. Baza mükafatı, iş vaxtı/bant genişliyi çəkiləri və ya əgər iqtisadi stress testini yenidən həyata keçirin
   uyğunluq cəzası >=10% dəyişir.