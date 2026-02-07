---
lang: az
direction: ltr
source: docs/source/mochi_architecture_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffca282b5a2bb2506f46ac2c7a8985ff2f7d10a46bc999a002277956c9f452b0
source_last_modified: "2026-01-05T09:28:12.023255+00:00"
translation_last_reviewed: 2026-02-07
title: MOCHI Architecture Plan
description: High-level design for the MOCHI local-network GUI supervisor.
translator: machine-google-reviewed
---

# MOCHI Memarlıq Planı


## Məqsədlər

- Bootstrap tək-peer və ya multi-peer (dörd node BFT) yerli şəbəkələri tez.
- `kagami`, `irohad` və dəstəkləyici ikili faylları dost GUI iş prosesində sarın.
- Torii HTTP/WebSocket son nöqtələri vasitəsilə canlı blok, hadisə və vəziyyət məlumatlarını səthə çıxarın.
- Yerli imzalama və təqdimat ilə əməliyyatlar və Iroha Xüsusi Təlimatlar (ISI) üçün strukturlaşdırılmış inşaatçılarla təmin edin.
- Faylları əl ilə redaktə etmədən snapshotları, yenidən yaranma axınlarını və konfiqurasiya parametrlərini idarə edin.
- Veb görünüşü və ya Docker asılılığı olmayan tək çarpaz platforma Rust binar kimi göndərin.

## Memarlığa Baxış

MOCHI yeni `/mochi` kataloqunda yerləşdirilmiş iki əsas qutuya bölünür (bax:
Quraşdırma və istifadə təlimatları üçün [MOCHI Quickstart](mochi/quickstart.md):

1. `mochi-core`: konfiqurasiya şablonu, açar və genesis materialının yaradılması, uşaq proseslərə nəzarət, Torii müştərilərini idarə etmək və fayl sisteminin vəziyyətini idarə etmək üçün cavabdeh olan başsız kitabxana.
2. `mochi-ui-egui`: `egui`/`eframe` üzərində qurulmuş iş masası proqramı, istifadəçi interfeysini təqdim edir və `mochi-core` API vasitəsilə bütün orkestrasiyanı həvalə edir.

Əlavə ön uclar (məsələn, Tauri qabığı) daha sonra nəzarətçi məntiqini yenidən işləmədən `mochi-core`-ə qoşula bilər.

## Proses Modeli

- Peer qovşaqları ayrıca `irohad` uşaq prosesləri kimi işləyir. MOCHI heç vaxt qeyri-sabit daxili API-lərdən və uyğun istehsal yerləşdirmə topologiyalarından qaçaraq peer-i kitabxana kimi əlaqələndirmir.
- Yaradılış və əsas material `kagami` çağırışları vasitəsilə istifadəçi tərəfindən təqdim edilən daxiletmələrlə (zəncir identifikatoru, ilkin hesablar, aktivlər) yaradılır.
- Konfiqurasiya faylları Torii və P2P portlarını, saxlama yollarını, anlıq görüntü parametrlərini və etibarlı həmyaşıd siyahılarını dolduraraq TOML şablonlarından yaradılır. Yaradılmış konfiqurasiyalar hər bir şəbəkə iş sahəsi kataloqunun altında saxlanılır.
- Nəzarətçi proseslərin həyat dövrlərini izləyir, log səthləri üçün stdout/stderr axınları aparır və sağlamlıq üçün `/status`, `/metrics` və `/configuration` son nöqtələrini sorğulayır.
- Nazik Torii müştəri təbəqəsi HTTP və WebSocket zənglərini əhatə edir, mümkün olduğu yerlərdə SCALE kodlaşdırma/deşifrəni təkrar tətbiq etməmək üçün Iroha Rust müştəri qutularına söykənir.

## `mochi-core` tərəfindən dəstəklənən istifadəçi axınları- **Şəbəkə Yaratma Sihirbazı**: tək və ya dörd həmyaşıdlı profili seçin, kataloqları seçin və şəxsiyyətlər və genezis yaratmaq üçün `kagami`-ə zəng edin.
- **Lifecycle Controls**: start, stop, restart həmyaşıdları; səth canlı ölçüləri; log quyruqlarını ifşa etmək; iş vaxtı konfiqurasiyasının son nöqtələrini dəyişdirin (məsələn, log səviyyələri).
- **Blok və Hadisə Axınları**: UI panelləri üçün yaddaşda yuvarlanan bufer saxlayaraq `/block/stream` və `/events`-ə abunə olun.
- **State Explorer**: domenləri, hesabları, aktivləri və aktiv təriflərini sadalamaq üçün Norito tərəfindən dəstəklənən `/query` zənglərini səhifələşdirmə köməkçiləri və metadata xülasələri ilə işə salın.
- **Transaction Composer**: nanə/köçürmə təlimatı layihələrini səhnələşdirin, onları imzalanmış əməliyyatlara yığın, Norito faydalı yükünü nəzərdən keçirin, `/transaction` vasitəsilə təqdim edin və nəticədə baş verən hadisələrə nəzarət edin; tonoz imzalama qarmaqları gələcək iterasiya olaraq qalır.
- **Snapshots və Re-Genesis**: Kura snapshot ixrac/idxalını təşkil edin, dükanları silin və tez sıfırlamalar üçün genezis materialını bərpa edin.

## UI Layeri (`mochi-ui-egui`)

- Xarici iş vaxtları olmadan tək yerli icra olunan faylı göndərmək üçün `egui`/`eframe` istifadə edir.
- Layout daxildir:
  - Həmyaşıd kartları, sağlamlıq göstəriciləri və sürətli hərəkətləri olan **Şəbəkə İdarəetmə Paneli**.
  - **Bloklar** paneli son törəmələri yayımlayır və hündürlükdə axtarışa icazə verir.
  - **Hadisələr** paneli əməliyyat statuslarını hash və ya hesaba görə süzür.
  - Səhifələnmiş Norito nəticələri və yoxlama üçün xam tullantılar ilə domenlər, hesablar, aktivlər və aktiv tərifləri üçün **State Explorer** tabları.
- **Bəstəkar** forması yığıla bilən nanə/köçürmə palitrası, növbənin idarə edilməsi (əlavə/sil/təmizləmə), xam Norito önizləməsi və imzalayan seyf tərəfindən dəstəklənən təqdimetmə rəyi, beləliklə operatorlar inkişaf etdirici və real hakimiyyət orqanları arasında keçid edə bilsinlər.
- **Genesis & Snapshots** idarəetmə görünüşü.
- İş vaxtı keçidləri və məlumat kataloqu qısa yolları üçün **Parametrlər**.
- UI kanallar vasitəsilə `mochi-core`-dən asinxron yeniləmələrə abunə olur; əsas strukturlaşdırılmış hadisələri (peer statusu, blok başlıqları, əməliyyat yeniləmələri) axın edən `SupervisorHandle`-i ifşa edir.

## Yerli İnkişaf Qeydləri

- İş sahəsinin konfiqurasiyası satıcı arxivləri almaq əvəzinə `zstd-sys` host `libzstd`-ə qarşı `zstd-sys` bağlantılarını təyin edir. Bu, pqcrypto-dan asılı quruluşların (və MOCHI testlərinin) oflayn və ya sandboxed mühitlərdə işləməsini təmin edir.

## Qablaşdırma və Paylama

- MOCHI paketləri (və ya `PATH`-də aşkarlanır) `irohad`, `iroha_cli` və `kagami` ikili faylları.
- OpenSSL asılılığından qaçmaq üçün gedən HTTPS üçün `rustls` istifadə edir.
- Bütün yaradılan artefaktları hər bir şəbəkə alt kataloqları ilə xüsusi proqram məlumat kökündə (məsələn, `~/.local/share/mochi` və ya platforma ekvivalenti) saxlayır. GUI “Finder/Explorer-də aşkar etmək” köməkçilərini təmin edir.
- Münaqişələrin qarşısını almaq üçün həmyaşıdları işə salmazdan əvvəl Torii (8080+) və P2P (1337+) portlarını avtomatik aşkarlayır və ehtiyatda saxlayır.

## Gələcək Genişləndirmələr (MVP üçün əhatə dairəsi xaricində)- Alternativ ön uçlar (Tauri, CLI başsız rejim) `mochi-core` paylaşır.
- Paylanmış test qrupları üçün çoxlu host orkestrasiyası.
- Konsensus daxili elementlər üçün vizualizatorlar (Sumeragi dəyirmi dövlətlər, dedi-qodu vaxtları).
- Avtomatlaşdırılmış efemer şəbəkə anlıq görüntüləri üçün CI boru kəmərləri ilə inteqrasiya.
- Fərdi tablolar və ya domen-xüsusi müfəttişlər üçün plug-in sistemi.

## İstinadlar

- [Torii Son nöqtələr](https://docs.iroha.tech/reference/torii-endpoints.html)
- [Peer Konfiqurasiya Parametrləri](https://docs.iroha.tech/reference/peer-config/params.html)
- [`kagami` repozitor sənədləri](https://github.com/hyperledger-iroha/iroha)
- [Iroha Xüsusi Təlimatlar](https://iroha-test.readthedocs.io/en/iroha2-dev/references/isi/)