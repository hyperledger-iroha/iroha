---
lang: az
direction: ltr
source: docs/source/connect_architecture_feedback.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 097ea58d49f48d059cda762cd719bc62f0b2d6f6ddecedef3f9bac030ae46aec
source_last_modified: "2025-12-29T18:16:35.934098+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! Memarlıq Əlaqəsi Yoxlama Siyahısını birləşdirin

Bu yoxlama siyahısı Connect Session Architecture-dan açıq sualları özündə cəmləşdirir
Android və JavaScript-dən giriş tələb edən strawman
Fevral 2026 SDK arası seminar. Şərhləri asinxron şəkildə toplamaq, izləmək üçün istifadə edin
sahiblik edin və seminar gündəliyini blokdan çıxarın.

> Status / Qeydlər sütununda Android və JS liderlərindən alınan yekun cavablar
> 2026-cı ilin fevral ayında seminar öncəsi sinxronizasiya; qərarlar daxilində yeni təqib məsələlərini əlaqələndirin
> inkişaf.

## Sessiyanın Həyat Dövrü və Nəqliyyat

| Mövzu | Android sahibi | JS sahibi | Status / Qeydlər |
|-------|---------------|----------|----------------|
| WebSocket reconnect back-off strategiyası (eksponensial və qapalı xətti) | Android Şəbəkə TL | JS Aparıcı | ✅ 60-larla məhdudlaşdırılmış titrəmə ilə eksponensial geri çəkilmə ilə bağlı razılaşdırılmışdır; JS brauzer/qovşaq pariteti üçün eyni sabitləri əks etdirir. |
| Offline bufer tutumunun defoltları (cari strawman: 32 kadr) | Android Şəbəkə TL | JS Aparıcı | ✅ Konfiqurasiyanın ləğvi ilə təsdiqlənmiş 32 çərçivəli standart; Android `ConnectQueueConfig` vasitəsilə davam edir, JS `window.connectQueueMax`-ə hörmət edir. |
| Push-stil yenidən qoşulma bildirişləri (FCM/APNS və səsvermə) | Android Şəbəkə TL | JS Aparıcı | ✅ Android cüzdan proqramları üçün əlavə FCM qarmaqlarını ifşa edəcək; JS, brauzerin təkan məhdudiyyətlərini nəzərə alaraq, eksponensial geriləmə ilə sorğuya əsaslanan olaraq qalır. |
| Mobil müştərilər üçün ping/pong kadens qoruyucuları | Android Şəbəkə TL | JS Aparıcı | ✅ 3× miss tolerantlığı ilə standartlaşdırılmış 30s ping; Android Doze təsirini balanslaşdırır, JS brauzerin tıxanmasının qarşısını almaq üçün ≥15 saniyəyə qədər sıxışdırır. |

## Şifrələmə və Açar İdarəetmə

| Mövzu | Android sahibi | JS sahibi | Status / Qeydlər |
|-------|---------------|----------|----------------|
| X25519 əsas saxlama gözləntiləri (StrongBox, WebCrypto təhlükəsiz kontekstlər) | Android Kripto TL | JS Aparıcı | ✅ Android mövcud olduqda X25519-u StrongBox-da saxlayır (TEE-yə qayıdır); JS, Node-da yerli `iroha_js_host` körpüsünə qayıdaraq, dApps üçün təhlükəsiz kontekstli WebCrypto-ya mandat verir. |
| ChaCha20-Poly1305 SDK-lar arasında birdəfəlik idarəetmə mübadiləsi | Android Kripto TL | JS Aparıcı | ✅ 64 bitlik qoruyucu qoruyucu və paylaşılan testlərlə paylaşılan `sequence` sayğac API-ni qəbul edin; JS, Rust davranışına uyğunlaşdırmaq üçün BigInt sayğaclarından istifadə edir. |
| Aparat tərəfindən dəstəklənən attestasiya yükü sxemi | Android Kripto TL | JS Aparıcı | ✅ Sxem tamamlandı: `attestation { platform, evidence_b64, statement_hash }`; JS isteğe bağlıdır (brauzer), Node HSM plug-in çəngəlindən istifadə edir. |
| İtirilmiş pul kisələri üçün bərpa axını (açar fırlanma əl sıxma) | Android Kripto TL | JS Aparıcı | ✅ Pul kisəsinin fırlanması ilə əl sıxışması qəbul edildi: dApp `rotate` nəzarəti, pul kisəsi cavabları yeni pubkey + imzalanmış təsdiq; JS WebCrypto materialını dərhal yenidən açar. |

## İcazələr və Sübut Paketləri| Mövzu | Android sahibi | JS sahibi | Status / Qeydlər |
|-------|---------------|----------|----------------|
| GA | üçün minimum icazə sxemi (metodlar/hadisələr/resurslar). Android Data Model TL | JS Aparıcı | ✅ GA baza: `methods`, `events`, `resources`, `constraints`; JS TypeScript növlərini Rust manifestinə uyğunlaşdırır. |
| Pul kisəsindən imtina yükü (`reason_code`, lokallaşdırılmış mesajlar) | Android Şəbəkə TL | JS Aparıcı | ✅ Kodlar yekunlaşdırılıb (`user_declined`, `permissions_mismatch`, `compliance_failed`, `internal_error`) üstəgəl isteğe bağlı `localized_message`. |
| Sübut paketinin isteğe bağlı sahələri (uyğunluq/KYC qoşmaları) | Android Data Model TL | JS Aparıcı | ✅ Bütün SDK-lar isteğe bağlı `attachments[]` (Norito `AttachmentRef`) və `compliance_manifest_id` qəbul edir; davranış dəyişikliyinə ehtiyac yoxdur. |
| Norito JSON sxemində uyğunlaşdırma körpü ilə yaradılan strukturlara qarşı | Android Data Model TL | JS Aparıcı | ✅ Qərar: körpü ilə yaradılan strukturlara üstünlük verin; JSON yolu yalnız sazlama üçün qalır, JS `Value` adapterini saxlayır. |

## SDK Fasadları və API Forması

| Mövzu | Android sahibi | JS sahibi | Status / Qeydlər |
|-------|---------------|----------|----------------|
| Yüksək səviyyəli asinxron interfeyslər (`Flow`, async iterators) pariteti | Android Şəbəkə TL | JS Aparıcı | ✅ Android `Flow<ConnectEvent>`-i ifşa edir; JS `AsyncIterable<ConnectEvent>` istifadə edir; paylaşılan `ConnectEventKind` üçün hər iki xəritə. |
| Səhv taksonomiyasının xəritələşdirilməsi (`ConnectError`, tipli alt siniflər) | Android Şəbəkə TL | JS Aparıcı | ✅ Platforma xüsusi yüklənmə təfərrüatları ilə paylaşılan nömrəni {`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`} qəbul edin. |
| Uçuş zamanı işarə sorğuları üçün ləğv semantikası | Android Şəbəkə TL | JS Aparıcı | ✅ `cancelRequest(hash)` nəzarətini təqdim etdi; hər iki SDK pul kisəsinin təsdiqi ilə bağlı ləğv edilə bilən koroutinlər/vədlər təqdim edir. |
| Paylaşılan telemetriya qarmaqları (hadisələr, ölçülərin adlandırılması) | Android Şəbəkə TL | JS Aparıcı | ✅ Metrik adlar düzülüb: `connect.queue_depth`, `connect.latency_ms`, `connect.reconnects_total`; nümunə ixracatçılar sənədləşdirilir. |

## Oflayn Davamlılıq və Jurnalistlik

| Mövzu | Android sahibi | JS sahibi | Status / Qeydlər |
|-------|---------------|----------|----------------|
| Növbəyə qoyulmuş çərçivələr üçün yaddaş formatı (ikili Norito və JSON) | Android Data Model TL | JS Aparıcı | ✅ Binar Norito (`.to`) hər yerdə saxlayın; JS IndexedDB `ArrayBuffer` istifadə edir. |
| Jurnal saxlama siyasəti və ölçü hədləri | Android Şəbəkə TL | JS Aparıcı | ✅ Defolt saxlama 24 saat və hər seans üçün 1MiB; `ConnectQueueConfig` vasitəsilə konfiqurasiya edilə bilər. |
| Hər iki tərəf kadrları təkrar oynatdıqda münaqişənin həlli | Android Şəbəkə TL | JS Aparıcı | ✅ `sequence` + `payload_hash` istifadə edin; dublikatlar nəzərə alınmır, konfliktlər telemetriya hadisəsi ilə `ConnectError.Internal`-i işə salır. |
| Növbənin dərinliyi və təkrar oynatma müvəffəqiyyəti üçün telemetriya | Android Şəbəkə TL | JS Aparıcı | ✅ Emit `connect.queue_depth` gauge və `connect.replay_success_total` sayğacı; hər iki SDK paylaşılan Norito telemetriya sxeminə qoşulur. |

## Implementation Spikes & References- **Rust körpü qurğuları:** `crates/connect_norito_bridge/src/lib.rs` və əlaqəli testlər hər bir SDK tərəfindən istifadə edilən kanonik kodlaşdırma/şifrləmə yollarını əhatə edir.
- **Swift demo qoşqu:** `examples/ios/NoritoDemoXcode/NoritoDemoXcodeTests/ConnectViewModelTests.swift` məşqləri Sessiya axınlarını istehza edilmiş nəqliyyatlarla birləşdirin.
- **Swift CI qapısı:** digər SDK-larla paylaşmadan əvvəl armatur paritetini, idarə paneli lentlərini və Buildkite `ci/xcframework-smoke:<lane>:device_tag` metadatasını doğrulamaq üçün Connect artefaktlarını yeniləyərkən `make swift-ci`-i işə salın.
- **JavaScript SDK inteqrasiya testləri:** `javascript/iroha_js/test/integrationTorii.test.js` Torii-ə qarşı qoşulma statusu/sessiya köməkçilərini təsdiq edir.
- **Android müştəri davamlılığı qeydləri:** `java/iroha_android/README.md:150` növbə/geri-off defoltlarını ilhamlandıran cari əlaqə təcrübələrini sənədləşdirir.

## Seminar üçün Hazırlıq Maddələri

- [x] Android: yuxarıdakı hər bir cədvəl sırası üçün nöqtə adamı təyin edin.
- [x] JS: yuxarıdakı hər bir cədvəl sırası üçün nöqtə adamı təyin edin.
- [x] Mövcud tətbiq sıçrayışlarına və ya təcrübələrə keçidlər toplayın.
- [x] Fevral 2026-cı il şurasından əvvəl işdən əvvəl nəzərdən keçirin (Android TL, JS Lead, Swift Lead ilə 29-01-2026 15:00 UTC üçün sifariş edilib).
- [x] Qəbul edilmiş cavablarla `docs/source/connect_architecture_strawman.md`-i yeniləyin.

## Əvvəlcədən oxuyun Paketi

- ✅ Paket `artifacts/connect/pre-read/20260129/` altında qeydə alınıb (samançı, SDK təlimatları və bu yoxlama siyahısını təzələdikdən sonra `make docs-html` vasitəsilə yaradılıb).
- 📄 Xülasə + paylama addımları `docs/source/project_tracker/connect_architecture_pre_read.md`-də canlıdır; linki fevral 2026 seminar dəvətinə və `#sdk-council` xatırladıcısına daxil edin.
- 🔁 Paketi təzələyərkən, əvvəlcədən oxunmuş qeydin içindəki yolu və hashı yeniləyin və elanı IOS7/AND7 hazırlıq jurnalları altında `status.md`-də arxivləşdirin.