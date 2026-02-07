---
lang: az
direction: ltr
source: docs/source/connect_architecture_followups.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 476331772efe169a7a073b561fa9935e314ff89d6bfff440f7246606c1c02669
source_last_modified: "2025-12-29T18:16:35.934525+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Memarlıq təqib hərəkətlərini birləşdirin

Bu qeyd çarpaz SDK-dan yaranan mühəndislik təqiblərini əks etdirir
Memarlıq baxışını birləşdirin. Hər bir sıra bir məsələ ilə əlaqələndirilməlidir (Jira bileti və ya PR)
iş təyin edildikdən sonra. Sahiblər izləmə biletləri yaratdıqca cədvəli yeniləyin.| Maddə | Təsvir | Sahib(lər) | İzləmə | Status |
|------|-------------|----------|----------|--------|
| Paylaşılan geri çəkilmə sabitləri | Eksponensial geri çəkilmə + titrəmə köməkçilərini (`connect_retry::policy`) həyata keçirin və onları Swift/Android/JS SDK-larına təqdim edin. | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-001](project_tracker/connect_architecture_followups_ios.md#ios-connect-001) | Tamamlandı — `connect_retry::policy` deterministik splitmix64 seçmə ilə eniş etdi; Swift (`ConnectRetryPolicy`), Android və JS SDK-lar güzgü köməkçiləri və qızıl testlər göndərir. |
| Ping/pong tətbiqi | Razılaşdırılmış 30-cu kadans və brauzerin minimum sıxıcısı ilə konfiqurasiya edilə bilən ürək döyüntüsünün tətbiqini əlavə edin; səth ölçüləri (`connect.ping_miss_total`). | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-002](project_tracker/connect_architecture_followups_ios.md#ios-connect-002) | Tamamlandı — Torii indi konfiqurasiya edilə bilən ürək döyüntüsü intervallarını tətbiq edir (`ping_interval_ms`, `ping_miss_tolerance`, `ping_min_interval_ms`), `connect.ping_miss_total` metrikanı ifşa edir və əlaqənin yoxlanılmasını əhatə edir. SDK xüsusiyyətinin anlıq görüntüləri müştərilər üçün yeni düymələri təqdim edir. |
| Oflayn növbənin davamlılığı | Paylaşılan sxemdən istifadə edərək Connect növbələri üçün Norito `.to` jurnal yazıçılarını/oxuyucularını (Swift `FileManager`, Android şifrəli yaddaşı, JS IndexedDB) həyata keçirin. | Swift SDK, Android Data Model TL, JS Lead | [IOS-CONNECT-003](project_tracker/connect_architecture_followups_ios.md#ios-connect-003) | Tamamlandı — Swift, Android və JS indi paylaşılan `ConnectQueueJournal` + diaqnostika köməkçilərini saxlama/daşma testləri ilə göndərir ki, sübut paketləri hər yerdə determinist olsun. SDK.【IrohaSwift/Mənbələr/IrohaSwift/ConnectQueueJournal.swift:1】【java/iroha_android/src/main/java/org/hype rledger/iroha/android/connect/ConnectQueueJournal.java:1】【javascript/iroha_js/src/connectQueueJournal.js:1】 |
| StrongBox attestasiya yükü | Pulqabı təsdiqləri vasitəsilə `{platform,evidence_b64,statement_hash}` yazın və dApp SDK-larına doğrulama əlavə edin. | Android Crypto TL, JS Aparıcı | [IOS-CONNECT-004](project_tracker/connect_architecture_followups_ios.md#ios-connect-004) | Gözləyən |
| Fırlanma nəzarət çərçivəsi | `Control::RotateKeys` + `RotateKeysAck` tətbiq edin və bütün SDK-larda `cancelRequest(hash)` / rotasiya API-lərini ifşa edin. | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-005](project_tracker/connect_architecture_followups_ios.md#ios-connect-005) | Gözləyən |
| Telemetriya ixracatçıları | `connect.queue_depth`, `connect.reconnects_total`, `connect.latency_ms` buraxın və sayğacları mövcud telemetriya boru kəmərlərinə (OpenTelemetry) təkrarlayın. | Telemetriya WG, SDK sahibləri | [IOS-CONNECT-006](project_tracker/connect_architecture_followups_ios.md#ios-connect-006) | Gözləyən |
| Swift CI qapısı | Bağlantı ilə əlaqəli boru kəmərlərinin `make swift-ci`-i işə saldığından əmin olun ki, armatur pariteti, idarə paneli lentləri və Buildkite `ci/xcframework-smoke:<lane>:device_tag` metadatası SDK-lar arasında uyğunlaşdırılsın. | Swift SDK Rəhbəri, İnfra qurmaq | [IOS-CONNECT-007](project_tracker/connect_architecture_followups_ios.md#ios-connect-007) | Gözləyən |
| Fallback insident hesabatı | Paylaşılan görünürlük üçün XCFramework tüstü kəməri insidentlərini (`xcframework_smoke_fallback`, `xcframework_smoke_strongbox_unavailable`) Qoşulun idarə panellərinə bağlayın. | Swift QA Rəhbəri, İnfra qurmaq | [IOS-CONNECT-008](project_tracker/connect_architecture_followups_ios.md#ios-connect-008) | Gözləyən || Uyğunluq qoşmaları keçidi | SDK-ların isteğe bağlı `attachments[]` + `compliance_manifest_id` sahələrini təsdiqləmə yüklərində itkisiz qəbul edib yönləndirməsini təmin edin. | Swift SDK, Android Data Model TL, JS Lead | [IOS-CONNECT-009](project_tracker/connect_architecture_followups_ios.md#ios-connect-009) | Gözlənir |
| Səhv taksonomiya uyğunlaşdırılması | Paylaşılan nömrəni (`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`) platforma-spesifik xəta ilə əlaqələndirin. | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-010](project_tracker/connect_architecture_followups_ios.md#ios-connect-010) | Tamamlandı — Swift, Android və JS SDK-ları README/TypeScript/Java sənədləri və TLS/timeout/HTTP/kodek/növbəni əhatə edən reqressiya testləri ilə paylaşılan `ConnectError` sarğı + telemetriya köməkçilərini göndərir. hallarda.【docs/source/connect_error_taxonomy.md:1】【IrohaSwift/Sources/IrohaSwift/ConnectError.swift:1】【java/iroha_android/src /test/java/org/hyperledger/iroha/android/connect/ConnectErrorTests.java:1】【javascript/iroha_js/test/connectError.test.js:1】 |
| Seminar qərar jurnalı | Qəbul edilmiş qərarları ümumiləşdirən şərhli göyərtəni/qeydləri şura arxivinə dərc edin. | SDK Proqram Rəhbəri | [IOS-CONNECT-011](project_tracker/connect_architecture_followups_ios.md#ios-connect-011) | Gözlənir |

> İzləmə identifikatorları sahibləri açıq biletlər kimi doldurulacaq; problemin inkişafı ilə yanaşı `Status` sütununu yeniləyin.