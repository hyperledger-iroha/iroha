---
lang: az
direction: ltr
source: docs/source/governance_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9f765fbe3170f654a9c44c3cd1afc5d82a72ff49137f32b98cf9d310faf114e
source_last_modified: "2025-12-29T18:16:35.963528+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% İdarəetmə Boru Kəməri (Iroha 2 və SORA Parlament)

# Cari vəziyyət (v1)
- İdarəetmə təklifləri aşağıdakı kimi həyata keçirilir: təklifçi → referendum → tally → qüvvəyə minmə. Referendum pəncərələri və iştirak/təsdiq hədləri `gov.md`-də təsvir olunduğu kimi tətbiq edilir; kilidlər yalnız uzadılır və müddəti bitdikdən sonra kilidini açın.
- Parlament seçimində deterministik sıralama və müddət sərhədləri ilə VRF əsaslı tirajlardan istifadə edilir; heç bir davamlı siyahı mövcud olmadıqda, Torii `gov.parliament_*` konfiqurasiyasından istifadə edərək ehtiyat əldə edir. Şura girişi və kvorum yoxlamaları `gov_parliament_bodies` / `gov_pipeline_sla` testlərində həyata keçirilir.
- Səsvermə rejimləri: ZK (standart, daxili baytlarla `Active` VK tələb olunur) və Düz (kvadrat çəki). Rejim uyğunsuzluğu rədd edilir; kilidin yaradılması/uzatılması ZK üçün reqressiya testləri və sadə təkrar səslər ilə hər iki rejimdə monotondur.
- Təsdiqləyicinin səhv davranışı sübut boru kəməri (`/v2/sumeragi/evidence*`, CLI köməkçiləri) vasitəsilə `NextMode` + `ModeActivationHeight` tərəfindən tətbiq edilən birgə konsensus təhvil-təslimləri ilə fəaliyyət göstərir.
- Qorunan ad məkanları, iş vaxtını təkmilləşdirmə qarmaqları və idarəetmə manifestinin qəbulu `governance_api.md`-də sənədləşdirilib və telemetriya ilə əhatə olunub (`governance_manifest_*`, `governance_protected_namespace_total`).

# Uçuş zamanı / geriləmə
- VRF çəkiliş artefaktlarını (toxum, sübut, sifariş siyahısı, alternativlər) dərc edin və nümayişlərin olmaması üçün əvəzetmə qaydalarını kodlaşdırın; heç-heçə və dəyişdirmə üçün qızıl qurğular əlavə edin.
- Parlament orqanları üçün mərhələ-SLA tətbiqi (qaydalar → gündəm → araşdırma → nəzərdən keçirmə → münsiflər heyəti → qüvvəyə minmə) açıq taymerlərə, eskalasiya yollarına və telemetriya sayğaclarına ehtiyac duyur.
- Siyasət-münsiflər heyətinin məxfi/təhvil vermə-açıqlama səsverməsi və əlaqəli rüşvətxorluğa qarşı müqavimət yoxlamaları hələ də həyata keçirilməlidir.
- Rol bağlarının çarpanları, yüksək riskli orqanlar üçün səhv davranış və xidmət yuvaları arasında soyuducu fasilələr konfiqurasiya santexnika və testlər tələb edir.
- İdarəetmə zolağının möhürlənməsi və referendum pəncərəsi/iştirakçı qapıları `gov.md`/`status.md`-də izlənilir; Qalan qəbul testləri yerləşdikcə yol xəritəsi qeydlərini yeniləyin.