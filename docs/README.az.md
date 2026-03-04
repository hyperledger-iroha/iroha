---
lang: az
direction: ltr
source: docs/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26e6f90205e98b5db87d442eb7e4e7691cce47e1c33ef3d11c9bfba25269294e
source_last_modified: "2026-01-14T17:53:24.552406+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha Sənədləşdirmə

日本版の概要は [`README.ja.md`](./README.ja.md) を参照してください。

İş sahəsi eyni kod bazasından iki buraxılış xəttini göndərir: **Iroha 2** (öz-özünə yerləşdirilən yerləşdirmələr) və
**Iroha 3 / SORA Nexus** (vahid qlobal Nexus kitabçası). Hər ikisi eyni Iroha Virtual Maşını (IVM) təkrar istifadə edir və
Kotodama alətlər silsiləsi, buna görə də müqavilələr və bayt kodu yerləşdirmə hədəfləri arasında portativ olaraq qalır. Sənədləşmə tətbiq edilir
başqa cür qeyd edilmədiyi təqdirdə hər ikisinə.

[Əsas Iroha sənədlərində](https://docs.iroha.tech/) siz tapa bilərsiniz:

- [Başlanğıc Bələdçisi](https://docs.iroha.tech/get-started/)
- Rust, Python, Javascript və Java/Kotlin üçün [SDK Dərslikləri](https://docs.iroha.tech/guide/tutorials/)
- [API Referansı](https://docs.iroha.tech/reference/torii-endpoints.html)

Buraxılış üçün xüsusi sənədlər və xüsusiyyətlər:

- [Iroha 2 Whitepaper](./source/iroha_2_whitepaper.md) — özünə məxsus şəbəkə spesifikasiyası.
- [Iroha 3 (SORA Nexus) Whitepaper](./source/iroha_3_whitepaper.md) — Nexus çox zolaqlı və məlumat məkanı dizaynı.
- [Məlumat Modeli və ISI Spesifikasiyası (tətbiqdən əldə edilən)](./source/data_model_and_isi_spec.md) — əks-mühəndisli davranış arayışı.
- [ZK Zərfləri (Norito)](./source/zk_envelopes.md) — yerli IPA/STARK Norito zərfləri və yoxlayıcı gözləntiləri.

## Lokallaşdırma

Yapon (`*.ja.*`), İvrit (`*.he.*`), İspan (`*.es.*`), Portuqal
(`*.pt.*`), Fransız (`*.fr.*`), Rus (`*.ru.*`), Ərəb (`*.ar.*`) və Urdu
(`*.ur.*`) sənəd stubları hər bir ingilis mənbə faylının yanında yerləşir. Bax
[`docs/i18n/README.md`](./i18n/README.md) yaratmaq və yaratmaq haqqında ətraflı məlumat üçün
tərcümələrin saxlanması, həmçinin yeni dillərin əlavə edilməsi üçün təlimat
gələcək.

## Alətlər

Bu depoda siz Iroha 2 alətləri üçün sənədləri tapa bilərsiniz:

- [Kagami](../crates/iroha_kagami/README.md)
- [`iroha_derive`](../crates/iroha_derive/) konfiqurasiya strukturları üçün makrolar (`config_base` xüsusiyyətinə baxın)
- [Profilin qurulması addımları](./profile_build.md) yavaş `iroha_data_model` tərtib tapşırıqlarını müəyyən etmək üçün

## Swift / iOS SDK Referansları

- [Swift SDK icmalı](./source/sdk/swift/index.md) — boru kəməri köməkçiləri, sürətləndirmə keçidləri və Connect/WebSocket API-ləri.
- [Sürətli başlanğıcı birləşdirin](./connect_swift_ios.md) — SDK-da ilk addım üstəgəl CryptoKit arayışı.
- [Xcode inteqrasiya təlimatı](./connect_swift_integration.md) — ChaChaPoly və çərçivə köməkçiləri ilə NoritoBridgeKit/Connect-i proqrama naqil etmək.
- [SwiftUI demo iştirakçı bələdçisi](./norito_demo_contributor.md) — iOS demosunu yerli Torii qovşağına qarşı işlədir, üstəlik sürətləndirmə qeydləri.
- Swift artefaktlarını və ya Connect dəyişikliklərini dərc etməzdən əvvəl `make swift-ci`-i işə salın; o, armatur paritetini, idarə paneli lentlərini və Buildkite `ci/xcframework-smoke:<lane>:device_tag` metadatasını yoxlayır.

## Norito (Seriallaşdırma kodek)

Norito iş sahəsinin serializasiya kodekidir. Biz `parity-scale-codec` istifadə etmirik
(ÖLÇƏ). Sənədlər və ya müqayisələr SCALE ilə müqayisə edildikdə, bu, yalnız üçündür
kontekst; bütün istehsal yolları Norito istifadə edir. `norito::codec::{Encode, Decode}`
API-lər hashing və tel üçün başlıqsız (“çılpaq”) Norito faydalı yükü təmin edir.
səmərəlilik — Norito, SCALE deyil.

Ən son vəziyyət:

- Sabit başlıq (sehrli, versiya, 16 bayt sxem, sıxılma, uzunluq, CRC64, bayraqlar) ilə deterministik kodlaşdırma/şifrləmə.
- İş vaxtı ilə seçilmiş sürətləndirmə ilə CRC64-XZ yoxlama məbləği:
  - x86_64 PCLMULQDQ (daşıyıcıdan az çoxalma) + Barrett reduksiya, 32 baytlıq parçalar üzərində qatlanmış.
  - uyğun qatlama ilə aarch64 PMULL.
  - Daşınma üçün 8-ə qədər dilimləmə və bit üzrə ehtiyatlar.
- Ayrılmaları azaltmaq üçün törəmələr və əsas növlər tərəfindən həyata keçirilən kodlaşdırılmış uzunluq göstərişləri.
- Daha böyük axın buferləri (64 KiB) və deşifrə zamanı artan CRC yeniləməsi.
- Könüllü zstd sıxılma; GPU sürətləndirilməsi xüsusiyyətlərə bağlıdır və deterministikdir.
- Adaptiv yol seçimi: `norito::to_bytes_auto(&T)` nömrələr arasından seçir
  sıxılma, CPU zstd və ya GPU-boşaldılmış zstd (tərtib edildikdə və mövcud olduqda)
  faydalı yükün ölçüsünə və keşlənmiş aparat imkanlarına əsaslanır. Seçim yalnız təsir edir
  performans və başlığın `compression` baytı; faydalı yük semantikası dəyişməzdir.

Paritet testləri, müqayisələr və istifadə nümunələri üçün `crates/norito/README.md`-ə baxın.

Qeyd: Bəzi alt sistem sənədləri (məsələn, IVM sürətləndirilməsi və ZK sxemləri) inkişaf edir. Funksionallıq natamam olduqda, fayllar qalan işi və səyahət istiqamətini çağırır.

Statusun son nöqtəsi kodlaşdırma qeydləri
- Torii `/status` gövdəsi kompaktlıq üçün başlıqsız (“çılpaq”) faydalı yüklə standart olaraq Norito-dən istifadə edir. Müştərilər əvvəlcə Norito kodunu deşifrə etməyə cəhd etməlidirlər.
- İstənilən halda serverlər JSON-u qaytara bilər; `content-type` `application/json` olarsa, müştərilər JSON-a qayıdırlar.
- Tel formatı SCALE deyil, Norito-dir. `norito::codec::{Encode,Decode}` API-ləri çılpaq variant üçün istifadə olunur.