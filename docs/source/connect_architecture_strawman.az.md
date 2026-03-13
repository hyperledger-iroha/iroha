---
lang: az
direction: ltr
source: docs/source/connect_architecture_strawman.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1a6bcc6bca3d7f70b82e35734b71d706ac46d8dc9c728351fabbd8a61dd3f31
source_last_modified: "2026-01-05T09:28:12.003461+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Strawman Session Arxitekturasına qoşulun (Swift / Android / JS)

Bu strawman təklifi Nexus Connect iş axınları üçün ortaq dizaynı təsvir edir
Swift, Android və JavaScript SDK-larında. dəstəkləmək üçün nəzərdə tutulub
2026-cı ilin fevralında SDK-lararası seminar və tətbiq etməzdən əvvəl açıq sualları əldə edin.

> Son yenilənmə: 29.01.2026  
> Müəlliflər: Swift SDK Lead, Android Networking TL, JS Lead  
> Status: Şuranın nəzərdən keçirilməsi üçün layihə (təhlükə modeli + məlumatların saxlanması uyğunluğu 2026-03-12 əlavə edildi)

## Məqsədlər

1. Bağlantı açılışı daxil olmaqla, pul kisəsi ↔ dApp sessiyasının həyat dövrünü uyğunlaşdırın,
   təsdiqlər, imzalama sorğuları və sökülmə.
2. Hamı tərəfindən paylaşılan Norito zərf sxemini (açmaq/təsdiqləmək/imzalamaq/nəzarət) müəyyən edin
   SDK-lar və `connect_norito_bridge` ilə pariteti təmin edin.
3. Nəqliyyat (WebSocket/WebRTC), şifrələmə arasında öhdəliklərin bölünməsi
   (Norito Qoşulma çərçivələri + açar mübadiləsi) və tətbiq təbəqələri (SDK fasadları).
4. Daxil olmaqla, masaüstü/mobil platformalarda deterministik davranışı təmin edin
   oflayn buferləmə və yenidən qoşulma.

## Sessiya Həyat Dövrü (Yüksək Səviyyə)

```
┌────────────┐      ┌─────────────┐      ┌────────────┐
│  dApp SDK  │←────→│  Connect WS │←────→│ Wallet SDK │
└────────────┘      └─────────────┘      └────────────┘
      │                    │                    │
      │ 1. open (app→wallet) frame (metadata, permissions, chain_id)
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 2. route frame     │
      │                    │────────────────────│
      │                    │                    │
      │                    │     3. approve frame (wallet pk, account,
      │                    │        permissions, proof/attest)
      │<────────────────────────────────────────│
      │                    │                    │
      │ 4. sign request    │                    │
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 5. sign result     │
      │                    │────────────────────│
      │                    │                    │
      │ 6. control frames for reject/close, error propagation, heartbeats.
```

## Zərf/Norito Sxem

Bütün SDK-lar `connect_norito_bridge`-də müəyyən edilmiş kanonik Norito sxemindən istifadə etməlidir:

- `EnvelopeV1` (açmaq / təsdiqləmək / imzalamaq / nəzarət etmək)
- `ConnectFrameV1` (AEAD yükü ilə şifrəli mətn çərçivələri)
- Nəzarət kodları:
  - `open_ext` (metadata, icazələr)
  - `approve_ext` (hesab, icazələr, sübutlar, imza)
  - `reject`, `close`, `ping/pong`, `error`

Swift əvvəllər göndərilmiş yertutan JSON kodlayıcıları (`ConnectCodec.swift`). 2026-cı ilin aprelindən etibarən SDK
həmişə Norito körpüsündən istifadə edir və XCFramework əskik olduqda bağlanmır, lakin bu saman adamı
körpü inteqrasiyasına səbəb olan mandatı hələ də ələ keçirir:

| Funksiya | Təsvir | Status |
|----------|-------------|--------|
| `connect_norito_encode_control_open_ext` | dApp açıq çərçivə | Körpüdə həyata keçirilir |
| `connect_norito_encode_control_approve_ext` | Pul kisəsinin təsdiqi | Həyata keçirilən |
| `connect_norito_encode_envelope_sign_request_tx/raw` | İmza sorğuları | Həyata keçirilən |
| `connect_norito_encode_envelope_sign_result_ok/err` | Nəticələri imzala | Həyata keçirilən |
| `connect_norito_decode_*` | Pul kisələri/dApps üçün təhlil | Həyata keçirilən |

### Tələb olunan iş

- Swift: `ConnectCodec` JSON köməkçilərini körpü zəngləri və səth ilə əvəz edin
  paylaşılan Norito növlərindən istifadə edərək çap edilmiş sarğılar (`ConnectFrame`, `ConnectEnvelope`). ✅ (Aprel 2026)
- Android/JS: Eyni sarğıların mövcud olduğundan əmin olun; səhv kodları və metadata açarlarını uyğunlaşdırın.
- Paylaşılan: Ardıcıl açar əldə etmə ilə sənəd şifrələməsi (X25519 açar mübadiləsi, AEAD)
  Norito spesifikasiyasına uyğun olaraq və Rust körpüsündən istifadə edərək nümunə inteqrasiya testlərini təmin edin.

## Nəqliyyat Müqaviləsi- Əsas nəqliyyat: WebSocket (`/v2/connect/ws?sid=<session_id>`).
- Könüllü gələcək: WebRTC (TBD) – ilkin strawman üçün əhatə dairəsi xaricindədir.
- Yenidən əlaqə strategiyası: tam titrəmə ilə eksponensial geri çəkilmə (baza 5s, maksimum 60s); Swift, Android və JS-də paylaşılan sabitlər, beləliklə təkrar cəhdlər proqnozlaşdırıla bilər.
- Ping/ponq kadansı: yenidən qoşulmazdan əvvəl buraxılmış üç tennis üçün tolerantlıqla 30s ürək döyüntüsü; JS brauzerin tənzimləmə qaydalarını təmin etmək üçün minimum intervalı 15 saniyəyə qədər sıxışdırır.
- Push qarmaqlar: Android pul kisəsi SDK oyanışlar üçün isteğe bağlı FCM inteqrasiyasını nümayiş etdirir, JS isə sorğu əsasında qalır (brauzer təkan icazələri üçün sənədləşdirilmiş məhdudiyyətlər).
- SDK öhdəlikləri:
  - Pinq/ponq ürək döyüntülərini qoruyun (mobil telefonda batareyaları boşaltmayın).
  - Oflayn olduqda gedən çərçivələri bufer edin (məhdud növbə, dApp üçün davamlı).
- Hadisə axını API təmin edin (Swift Combine `AsyncStream`, Android Flow, JS async iter).
- Qarmaqları səthi yenidən birləşdirin və əl ilə yenidən abunə olmağa icazə verin.
- Telemetriya redaksiyası: yalnız sessiya səviyyəli sayğacları buraxır (`sid` hash, istiqamət,
  ardıcıllıq pəncərəsi, növbənin dərinliyi) qoşulma telemetriyasında sənədləşdirilmiş duzlarla
  bələdçi; başlıqlar/açarlar heç vaxt qeydlərdə və ya sazlama sətirlərində görünməməlidir.

## Şifrələmə və Açar İdarəetmə

### Sessiya identifikatorları və duzları

- `sid` `BLAKE2b-256("iroha-connect|sid|" || chain_id || app_ephemeral_pk || nonce16)`-dən əldə edilən 32 baytlıq identifikatordur.  
  DApps onu `/v2/connect/session`-ə zəng etməzdən əvvəl hesablayır; pul kisələri onu `approve` çərçivələrində əks etdirir ki, hər iki tərəf jurnalları və telemetriyanı ardıcıl olaraq əsaslandıra bilsin.
- Eyni duz hər bir açar törəmə addımını qidalandırır ki, SDK heç vaxt ana platformadan toplanan entropiyaya etibar etmir.

### Efemer açarla işləmə

- Hər sessiya təzə X25519 əsas materialından istifadə edir.  
  Swift onu `ConnectCrypto` vasitəsilə Keychain/Secure Enclave-də saxlayır, Android cüzdanları defolt olaraq StrongBox-a (TEE dəstəkli açar anbarlarına qayıdır) və JS təhlükəsiz kontekstli WebCrypto nümunəsi və ya yerli `iroha_js_host` plaginini tələb edir.
- Açıq çərçivələrə dApp efemer açıq açarı və əlavə sertifikat paketi daxildir. Pulqabı təsdiqləri pul kisəsinin açıq açarını və uyğunluq axını üçün lazım olan hər hansı aparat sertifikatını qaytarır.
- Attestasiya yükləri qəbul edilmiş sxemə uyğundur:  
  `attestation { platform, evidence_b64, statement_hash }`.  
  Brauzerlər bloku buraxa bilər; Doğma pul kisələri, hardware tərəfindən dəstəklənən açarlar istifadə edildikdə onu ehtiva edir.

### İstiqamət açarları və AEAD

- Paylaşılan sirrlər HKDF-SHA256 (Rust körpü köməkçiləri vasitəsilə) və domenlə ayrılmış məlumat sətirləri ilə genişləndirilir:
  - `iroha-connect|k_app` → proqram→pul kisəsi trafiki.
  - `iroha-connect|k_wallet` → pul kisəsi → proqram trafiki.
- AEAD v1 zərfi üçün ChaCha20-Poly1305-dir (`connect_norito_bridge` hər platformada köməkçiləri ifşa edir).  
  Əlaqədar məlumatlar `("connect:v1", sid, dir, seq_le, kind=ciphertext)`-ə bərabərdir, beləliklə başlıqlarda müdaxilə aşkar edilir.
- Nonces 64-bit ardıcıllıq sayğacından əldə edilir (`nonce[0..4]=0`, `nonce[4..12]=seq_le`). Paylaşılan köməkçi testlər BigInt/UInt dönüşümlərinin SDK-lar arasında eyni davranmasını təmin edir.

### Fırlanma və bərpa əl sıxma- Fırlanma isteğe bağlı olaraq qalır, lakin protokol müəyyən edilib: ardıcıllıq sayğacları sarğı qoruyucuya yaxınlaşdıqda dApps `Control::RotateKeys` çərçivəsi buraxır, pul kisələri yeni açıq açar və imzalanmış təsdiqlə cavab verir və hər iki tərəf sessiyanı bağlamadan dərhal yeni istiqamət açarları əldə edir.
- Pul kisəsi tərəfində açar itkisi eyni əl sıxışmasını və ardınca `resume` nəzarətini tetikler, beləliklə dApp-lar köhnəlmiş açarı hədəf alan keşlənmiş şifrə mətnini silməyi bilirlər.

Tarixi CryptoKit ehtiyatları üçün baxın `docs/connect_swift_ios.md`; Kotlin və JS `docs/connect_kotlin_ws*.md` altında uyğun istinadlara malikdir.

## İcazələr və Sübutlar

- İcazə manifestləri körpü tərəfindən ixrac edilən paylaşılan Norito strukturu vasitəsilə gediş-gəliş etməlidir.  
  Sahələr:
  - `methods` — fellər (`sign_transaction`, `sign_raw`, `submit_proof`, …).  
  - `events` — dApp-ın qoşulmasına icazə verilən abunəliklər.  
  - `resources` — pul kisələrinin girişi əhatə edə bilməsi üçün əlavə hesab/aktiv filtrləri.  
  - `constraints` — pul kisəsinin imzalamadan əvvəl tətbiq etdiyi zəncir ID, TTL və ya fərdi siyasət düymələri.
- İcazələrlə yanaşı uyğunluq metadata gəzintiləri:
  - Könüllü `attachments[]`-də Norito qoşma arayışları var (KYC paketləri, tənzimləyici qəbzlər).  
  - `compliance_manifest_id` sorğunu əvvəllər təsdiq edilmiş manifestlə əlaqələndirir ki, operatorlar mənşəyi yoxlaya bilsinlər.
- Pul kisəsi cavabları razılaşdırılmış kodlardan istifadə edir:
  - `user_declined`, `permissions_mismatch`, `compliance_failed`, `internal_error`.  
  Hər biri UI göstərişləri üçün `localized_message` və maşınla oxuna bilən `reason_code` daşıya bilər.
- Təsdiq çərçivələrinə seçilmiş hesab/nəzarətçi, icazə əks-sədası, sübut paketi (ZK sübutu və ya attestasiya) və hər hansı siyasət keçidləri (məsələn, `offline_queue_enabled`) daxildir.  
  Rəddlər eyni sxemi boş `proof` ilə əks etdirir, lakin yenə də audit üçün `sid` qeyd edir.

## SDK Fasadları

| SDK | Təklif olunan API | Qeydlər |
|-----|--------------|-------|
| Swift | `ConnectClient`, `ConnectSession`, `ConnectRequest`, `ConnectApproval` | Yer tutucuları yazılmış sarğılar + asinxron axınlarla əvəz edin. |
| Android | Kotlin coroutines + çərçivələr üçün möhürlənmiş siniflər | Portativlik üçün Swift strukturu ilə uyğunlaşdırın. |
| JS | Async iterators + çərçivə növləri üçün TypeScript nömrələri | Bundler üçün uyğun SDK (brauzer/qovşaq) təmin edin. |

### Ümumi davranışlar- `ConnectSession` həyat dövrünü təşkil edir:
  1. WebSocket qurun, əl sıxma həyata keçirin.
  2. Açıq/təsdiq çərçivələrini mübadilə edin.
  3. İmza sorğularını/cavablarını idarə edin.
  4. Tətbiq qatına hadisələri buraxın.
- Yüksək səviyyəli köməkçiləri təmin edin:
  - `requestSignature(tx, metadata)`
  - `approveSession(account, permissions)`
  - `reject(reason)`
  - `cancelRequest(hash)` – pul kisəsi tərəfindən təsdiqlənmiş idarəetmə çərçivəsi yayır.
- Səhvlərin idarə edilməsi: Norito xəta kodlarını SDK-ya xas xətalarla əlaqələndirin; daxildir
  paylaşılan taksonomiyadan istifadə edən UI üçün domenə məxsus kodlar (`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, I180NI08000). Swift-in əsas tətbiqi + telemetriya bələdçisi [`connect_error_taxonomy.md`](connect_error_taxonomy.md) daxilində yaşayır və Android/JS pariteti üçün istinaddır.
- Növbənin dərinliyi üçün telemetriya qarmaqlarını buraxın, sayları yenidən birləşdirin və gecikməni tələb edin (`connect.queue_depth`, `connect.reconnects_total`, `connect.latency_ms`).
 
## Ardıcıl Nömrələr və Axına Nəzarət

- Hər bir istiqamət seans açıldığında sıfırdan başlayan xüsusi 64-bit `sequence` sayğacını saxlayır. Paylaşılan köməkçi tiplər artımları sıxışdırır və sayğacın sarılmasından xeyli əvvəl `ConnectError.sequenceOverflow` + düymə fırlanması ilə əl sıxışdırır.
- Nonces və əlaqəli məlumatlar ardıcıllıq nömrəsinə istinad edir, beləliklə dublikatlar faydalı yükləri təhlil etmədən rədd edilə bilər. SDK-lar təkrar qoşulmalar arasında təkmilləşdirməni deterministik etmək üçün `{sid, dir, seq, payload_hash}`-i jurnallarında saxlamalıdırlar.
- Pul kisələri məntiqi pəncərə (`FlowControl` idarəetmə çərçivələri) vasitəsilə əks təzyiqi reklam edir. DApps yalnız pəncərə nişanı mövcud olduqda sıradan çıxır; cüzdanlar boru kəmərlərini məhdud saxlamaq üçün şifrəli mətni emal etdikdən sonra yeni tokenlər buraxır.
- Danışıqların davam etdirilməsi açıqdır: hər iki tərəf yenidən qoşulduqdan sonra `Control::Resume { seq_app_max, seq_wallet_max, queue_depths }` yayır ki, müşahidəçilər nə qədər məlumatın yenidən göndərildiyini və jurnallarda boşluqların olub-olmadığını yoxlaya bilsinlər.
- Münaqişələr (məsələn, eyni `(sid, dir, seq)`, lakin fərqli heşlərə malik iki faydalı yük) `ConnectError.Internal`-ə qədər yüksəlir və səssiz fərqin qarşısını almaq üçün yeni `sid`-ni məcbur edir.

## Təhdid modeli və məlumatların saxlanması uyğunlaşdırılması- ** Nəzərə alınan səthlər:** WebSocket nəqliyyatı, Norito körpü kodlaşdırma/şifrələmə,
  jurnal əzmkarlığı, telemetriya ixracatçıları və proqramla bağlı geri zənglər.
- **Əsas məqsədlər:** sessiya sirlərini qorumaq (X25519 açarları, əldə edilmiş AEAD açarları,
  qeyri-ardıcıllıq sayğacları) jurnallarda/telemetriyada sızmalardan, təkrar oynatmanın qarşısını almaq və
  aşağı səviyyəli hücumlar və jurnalların və anomaliya hesabatlarının məcburi saxlanması.
- **Mütəxəssislər:**
  - Jurnallar yalnız şifrəli mətn daşıyır; Saxlanılan metadata həşlərlə, uzunluqla məhdudlaşır
    sahələr, vaxt ştampları və sıra nömrələri.
  - Telemetriya yükləri hər hansı başlıq/faydalı məzmunu redaktə edir və yalnız daxildir
    `sid` və aqreqat sayğacların duzlu hashləri; redaktə yoxlama siyahısı paylaşıldı
    audit pariteti üçün SDK-lar arasında.
  - Sessiya qeydləri fırlanır və standart olaraq 7 gündən sonra köhnəlir. Pul kisələri ifşa olunur
    `connectLogRetentionDays` düyməsi (SDK standart 7) və davranışı sənədləşdirin
    beləliklə, tənzimlənən yerləşdirmələr daha sərt pəncərələri bağlaya bilər.
  - Bridge API-dən sui-istifadə (çatışmayan bağlamalar, zədələnmiş şifrə mətni, etibarsız ardıcıllıq)
    xam faydalı yükləri və ya açarları əks etdirmədən yazılmış səhvləri qaytarır.

Nəzərdən keçirilməsi gözlənilən suallar `docs/source/sdk/swift/connect_workshop.md`-də izlənilir
və şuranın protokolunda həll edilir; bağlandıqdan sonra samançı olacaq
layihədən qəbul edilənə qədər yüksəldi.

## Oflayn Buferləşdirmə və Yenidən Bağlantılar

### Jurnal müqaviləsi

Hər bir SDK seans başına yalnız əlavə jurnal saxlayır ki, dApp və pul kisəsi olsun
oflayn olarkən çərçivələri növbəyə qoya, məlumat itkisi olmadan davam etdirə və sübut təqdim edə bilər
telemetriya üçün. Müqavilə eyni bayt olan Norito körpü növlərini əks etdirir
təmsilçilik mobil/JS yığınlarında sağ qalır.- Jurnallar hashed sessiya identifikatoru (`sha256(sid)`) altında yaşayır və iki ədəd yaradır
  seans başına fayllar: `app_to_wallet.queue` və `wallet_to_app.queue`. Swift istifadə edir
  sandboxed fayl paketi, Android faylları `Room`/`FileChannel` vasitəsilə saxlayır,
  və JS IndexedDB-yə yazır; bütün formatlar ikili və endian-sabitdir.
- Hər bir qeyd `ConnectJournalRecordV1` kimi seriyalaşdırılır:
  - `direction: u8` (`0 = app→wallet`, `1 = wallet→app`)
  - `sequence: u64`
  - `payload_hash: [u8; 32]` (şifrəli mətn + başlıqların Blake3)
  - `ciphertext_len: u32`
  - `received_at_ms: u64`
  - `expires_at_ms: u64`
  - `ciphertext: [u8; ciphertext_len]` (dəqiq Norito çərçivəsi artıq AEAD ilə bükülmüşdür)
- Jurnallar şifrəli mətni hərfi saxlayır. Biz heç vaxt faydalı yükü yenidən şifrələyirik; AEAD
  başlıqlar artıq istiqamət düymələrini təsdiqləyir, buna görə də əzmkarlıq azalır
  əlavə edilmiş qeydin sinxronizasiyası.
- Yaddaşdakı `ConnectQueueState` strukturu fayl metadatasını əks etdirir (dərinlik,
  istifadə olunan baytlar, ən köhnə/yeni sıra). O, telemetriya ixracatçılarını və
  `FlowControl` köməkçisi.
- Defolt olaraq 32 kadr / 1MiB-də jurnalların həcmi; qapağı vuraraq çıxarır
  ən köhnə qeydlər (`reason=overflow`). `ConnectFeatureConfig.max_queue_len`
  hər yerləşdirmə üçün bu defoltları ləğv edir.
- Jurnallar məlumatları 24 saat ərzində saxlayır (`expires_at_ms`). Fon GC köhnəlmiş funksiyanı aradan qaldırır
  seqmentləri həvəslə ayırır ki, diskdəki iz məhdud qalsın.
- Qəza təhlükəsizliyi: bildiriş vermədən əvvəl yaddaş güzgüsünü əlavə edin, fsync edin və yeniləyin
  zəng edən. Başlanğıcda SDK-lar kataloqu skan edir, yoxlama məbləğlərini yoxlayır,
  və `ConnectQueueState`-i yenidən qurun. Korrupsiya qanun pozuntularının qeydə alınmasına səbəb olur
  atlanır, telemetriya vasitəsilə işarələnir və isteğe bağlı olaraq dəstək zibillikləri üçün karantinə alınır.
- Şifrə mətni artıq Norito məxfilik zərfinə cavab verdiyi üçün, yeganə
  qeydə alınan əlavə metadata hashed sessiya id-dir. Əlavə istəyən proqramlar
  məxfilik jurnalları saxlayan `telemetry_opt_in = false`-ə daxil ola bilər, lakin
  növbə dərinliyi ixracını redaktə edir və qeydlərdə hashed `sid` paylaşılmasını deaktiv edir.
- SDK-lar `ConnectQueueObserver`-i ifşa edir ki, pul kisələri/dApps növbə dərinliyini yoxlaya bilsin,
  drenajlar və GC nəticələri; bu çəngəl logları təhlil etmədən status UI-lərini qidalandırır.

### Semantikanı təkrarlayın və davam etdirin

1. Yenidən qoşulduqda SDK-lar `{seq_app_max, `Control::Resume` yayır
   seq_wallet_max, queued_app, queued_wallet, journal_hash}`. hash edir
   Yalnız əlavə edilən jurnalın Blake3 həzmi beləliklə, uyğun olmayan həmyaşıdlar sürüşməni aşkar edə bilsin.
2. Qəbul edən həmyaşıd rezyume yükünü onun vəziyyəti, sorğuları ilə müqayisə edir
   boşluqlar mövcud olduqda təkrar ötürmə və vasitəsilə təkrar oynatılmış kadrları qəbul edir
   `Control::ResumeAck`.
3. Təkrarlanan kadrlar həmişə daxiletmə sırasına riayət edir (`sequence` sonra yazma vaxtı).
   Pulqabı SDK-ları `FlowControl` tokenlərini (həmçinin) verməklə geri təzyiq tətbiq etməlidirlər
   jurnallı) beləliklə, dApps oflayn olarkən növbəni doldura bilməz.
4. Jurnallar şifrəli mətni hərfi saxlayır, beləliklə təkrar oxutma sadəcə qeydə alınmış baytları pompalayır
   nəqliyyat və dekoder vasitəsilə geri qayıt. Hər SDK üçün yenidən kodlaşdırmaya icazə verilmir.

### Yenidən qoşulma axını1. Nəqliyyat WebSocket-i yenidən qurur və yeni ping intervalı üzrə danışıqlar aparır.
2. dApp cüzdandan geri təzyiqə riayət edərək növbəyə qoyulmuş kadrları ardıcıllıqla təkrarlayır
   (`ConnectSession.nextControlFrame()` `FlowControl` tokenlərini verir).
3. Pul kisəsi buferləşdirilmiş nəticələrin şifrəsini açır, ardıcıllığın monotonluğunu yoxlayır və
   gözlənilən təsdiqlər/nəticələr təkrarlanır.
4. Hər iki tərəf `resume` `seq_app_max`, `seq_wallet_max`,
   və telemetriya üçün növbə dərinlikləri.
5. Dublikat çərçivələr (uyğun olan `sequence` + `payload_hash`) qəbul edilir və silinir; münaqişələr `ConnectError.Internal` artırır və məcburi sessiyanın yenidən başlamasına səbəb olur.

### Uğursuzluq rejimləri

- Sessiya köhnəlmiş hesab edilərsə (`offline_timeout_ms`, standart 5 dəqiqə),
  tamponlanmış çərçivələr təmizlənir və SDK `ConnectError.sessionExpired`-i qaldırır.
- Jurnalın pozulması halında, SDK-lar tək Norito deşifrə təmirinə cəhd edir; haqqında
  uğursuzluq onlar jurnalı buraxır və `connect.queue_repair_failed` telemetriya yayırlar.
- Ardıcıl uyğunsuzluq `ConnectError.replayDetected`-i işə salır və yeni
  əl sıxma (yeni `sid` ilə sessiya yenidən başladın).

### Offline tamponlama planı və operator nəzarəti

Seminarın çatdırılması sənədləşdirilmiş plan tələb edir, buna görə də hər bir SDK eyni şəkildə göndərilir
oflayn davranış, remediasiya axını və sübut səthləri. Aşağıdakı plan
Swift (`ConnectSessionDiagnostics`), Android-də ümumi
(`ConnectDiagnosticsSnapshot`) və JS (`ConnectQueueInspector`).

| Dövlət | Tətik | Avtomatik cavab | Əl ilə ləğv | Telemetriya bayrağı |
|-------|---------|-------------------|-----------------|----------------|
| `Healthy` | Növbədən istifadə  5/dəq | Yeni işarə sorğularını dayandırın, axın-nəzarət nişanlarını yarı nisbətdə buraxın | Proqramlar `clearOfflineQueue(.app|.wallet)`-ə zəng edə bilər; SDK həmyaşıdlarından bir dəfə onlayn olaraq vəziyyəti yenidən nəmləndirir | `connect.queue_state=\"throttled\"`, `connect.queue_watermark` ölçü cihazı |
| `Quarantined` | İstifadə ≥ `disk_watermark_drop` (defolt 85%), iki dəfə korrupsiya aşkar edildi və ya `offline_timeout_ms` keçdi | Buferləşdirməni dayandırın, `ConnectError.QueueQuarantined`-i qaldırın, operatorun təsdiqini tələb edin | `ConnectSessionDiagnostics.forceReset()` | paketini ixrac etdikdən sonra jurnalları silir `connect.queue_state=\"quarantined\"`, `connect.queue_quarantine_total` sayğac |- Eşiklər `ConnectFeatureConfig` (`disk_watermark_warn`,
  `disk_watermark_drop`, `max_disk_bytes`, `offline_timeout_ms`). Ev sahibi olduqda
  dəyəri buraxırsa, SDK-lar defoltlarına qayıdır və konfiqurasiya üçün xəbərdarlıq daxil olur
  telemetriyadan yoxlanıla bilər.
- SDK-lar `ConnectQueueObserver` plus diaqnostika köməkçilərini ifşa edir:
  - Swift: `ConnectSessionDiagnostics.snapshot()` `{vəziyyət, dərinlik, bayt,
    səbəb}` and `exportJournalBundle(url:)` dəstək üçün hər iki növbəni davam etdirir.
  - Android: `ConnectDiagnostics.snapshot()` + `exportJournalBundle(path)`.
  - JS: `ConnectQueueInspector.read()` eyni strukturu və blob sapını qaytarır
    həmin UI kodu Torii dəstək alətlərinə yükləyə bilər.
- Proqram `offline_queue_enabled=false`-i dəyişdikdə, SDK-lar dərhal boşalır və
  hər iki jurnalı təmizləyin, vəziyyəti `Disabled` olaraq qeyd edin və terminal buraxın
  telemetriya hadisəsi. İstifadəçiyə baxan üstünlük Norito-də əks olunur
  təsdiq çərçivəsi belədir ki, həmyaşıdlar tamponlanmış çərçivələri davam etdirə bilsinlər.
- Operatorlar `connect queue inspect --sid <sid>` (SDK ətrafında CLI sarğı) işlədirlər
  diaqnostika) xaos testləri zamanı; bu əmr dövlət keçidlərini çap edir,
  su nişanı tarixçəsi və sübutları davam etdirin, belə ki, idarəetmə rəylərindən asılı deyil
  platforma xüsusi alətlər.

### Sübut paketinin iş axını

Dəstək və uyğunluq qrupları audit zamanı deterministik sübutlara əsaslanır
oflayn davranış. Buna görə də hər bir SDK eyni üç addımlı ixracı həyata keçirir:

1. `exportJournalBundle(..)` `{app_to_wallet,wallet_to_app}.queue` üstəgəl a yazır
   qurulma hashını, xüsusiyyət bayraqlarını və disk su nişanlarını təsvir edən manifest.
2. `exportQueueMetrics(..)` son 1000 telemetriya nümunəsini yayır, beləliklə idarə panelləri
   oflayn olaraq yenidən qurula bilər. Nümunələr hashed sessiya id daxildir
   istifadəçi daxil oldu.
3. CLI köməkçisi həm ixracı sıxışdırır, həm də imzalanmış Norito metadata faylını əlavə edir
   (`ConnectQueueEvidenceV1`) beləliklə, Torii qəbulu paketi SoraFS-də arxivləşdirə bilər.

Doğrulama uğursuz olan paketlər `connect.evidence_invalid` ilə rədd edilir
telemetriya beləliklə SDK komandası ixracatçını çoxalda və yamaq edə bilsin.

## Telemetriya və Diaqnostika- Paylaşılan OpenTelemetry ixracatçıları vasitəsilə Norito JSON hadisələrini yayımlayın. Məcburi ölçülər:
  - `ConnectQueueState` tərəfindən qidalanan `connect.queue_depth{direction}` (ölçü).
  - Diskdə dəstəklənən iz üçün `connect.queue_bytes{direction}` (ölçü).
  - `overflow|ttl|repair` üçün `connect.queue_dropped_total{reason}` (sayğac).
  - `connect.offline_flush_total{direction}` (sayğac) növbələr olduqda artır
    nəqliyyat olmadan drenaj; uğursuzluqlar artımı `connect.offline_flush_failed`.
  - `connect.replay_success_total`/`connect.replay_error_total`.
  - `connect.resume_latency_ms` histoqramı (yenidən qoşulma və sabitlik arasındakı vaxt
    dövlət) üstəgəl `connect.resume_attempts_total`.
  - `connect.session_duration_ms` histoqramı (başa çatmış sessiyaya görə).
  - `connect.error` `code`, `fatal`, `telemetry_profile` ilə strukturlaşdırılmış hadisələr.
- İxracatçılar `{platform, sdk_version, feature_hash}` etiketlərini əlavə etməlidirlər
  tablosuna SDK quruluşu ilə bölünə bilər. Hashed `sid` isteğe bağlıdır və yalnız
  telemetriyaya qoşulma doğru olduqda yayılır.
- SDK səviyyəli qarmaqlar eyni hadisələri üzə çıxarır ki, proqramlar daha çox təfərrüatı ixrac edə bilsin:
  - Sürətli: `ConnectSession.addObserver(_:) -> ConnectEvent`.
  - Android: `Flow<ConnectEvent>`.
  - JS: async iterator və ya geri çağırış.
- CI qapısı: Sürətli işlər `make swift-ci` ilə işləyir, Android `./gradlew sdkConnectCi` istifadə edir,
  və JS `npm run test:connect` ilə işləyir, beləliklə telemetriya/iş panelləri əvvəl yaşıl qalır
  birləşdirən Connect dəyişiklikləri.
- Strukturlaşdırılmış jurnallara heşlənmiş `sid`, `seq`, `queue_depth` və `sid_epoch` daxildir.
  operatorların müştəri məsələlərini əlaqələndirə bilməsi üçün dəyərlər. Təmiri uğursuz olan jurnallar yayılır
  `connect.queue_repair_failed{reason}` hadisələri və əlavə qəza atma yolu.

### Telemetriya qarmaqları və idarəetmə sübutları

- `connect.queue_state` yol xəritəsi risk göstəricisi kimi ikiqat artır. İdarə panelləri qrupu
  `{platform, sdk_version}` tərəfindən və ştatda vaxt göstərin ki, idarəetmə nümunə götürə bilsin
  mərhələli buraxılışları təsdiq etməzdən əvvəl aylıq qazma sübutu.
- `connect.queue_watermark` və `connect.queue_bytes` Connect risk xalını verir
  (`risk.connect.offline_buffer`), SRE-dən çox olduqda avtomatik olaraq səhifələnir
  Seansların 5%-i `Throttled`-də >10 dəqiqə sərf edir.
- İxracatçılar hər hadisəyə `feature_hash` əlavə edirlər ki, auditor alətləri təsdiq edə bilsin
  Norito kodek + oflayn planın nəzərdən keçirilən quruluşa uyğun olduğunu. SDK CI uğursuz oldu
  telemetriya naməlum hash bildirdikdə sürətli.
- Samançı hələ də təhdid modeli əlavə tələb edir; göstəriciləri aşdıqda
  siyasət hədləri, SDK-lar ümumiləşdirən `connect.policy_violation` hadisələri yayır.
  pozan tərəf (hashed), vəziyyət və həll edilmiş fəaliyyət (`drain|purge|quarantine`).
- `exportQueueMetrics` vasitəsilə əldə edilən sübutlar eyni SoraFS ad məkanına düşür
  Connect runbook artefaktları kimi şura rəyçiləri hər bir məşqi izləyə bilsinlər
  daxili jurnalları tələb etmədən xüsusi telemetriya nümunələrinə qayıdın.

## Çərçivə Sahibliyi və Məsuliyyətləri| Çərçivə / Nəzarət | Sahibi | Sequence domain | Jurnal davam etdi? | Telemetriya etiketləri | Qeydlər |
|----------------|-------|-----------------|--------------------|------------------|-------|
| `Control::Open` | dApp | `seq_app` | ✅ (`app_to_wallet`) | `event=open` | Metadata + icazə bitmapını daşıyır; pul kisələri təkliflərdən əvvəl ən son açılışı təkrarlayır. |
| `Control::Approve` | Pul kisəsi | `seq_wallet` | ✅ (`wallet_to_app`) | `event=approve` | Hesab, sübutlar, imzalar daxildir. Burada qeyd edilən metadata versiyası artımları. |
| `Control::Reject` | Pul kisəsi | `seq_wallet` | ✅ | `event=reject`, `reason` | Könüllü lokallaşdırılmış mesaj; dApp gözləyən işarə sorğularını buraxır. |
| `Control::Close` (init) | dApp | `seq_app` | ✅ | `event=close`, `initiator=app` | Pul kisəsi öz `Close` ilə təsdiqlənir. |
| `Control::Close` (ack) | Pul kisəsi | `seq_wallet` | ✅ | `event=close`, `initiator=wallet` | Sökülməni təsdiqləyir; Hər iki tərəf çərçivəni saxladıqdan sonra GC jurnalları silir. |
| `SignRequest` | dApp | `seq_app` | ✅ | `event=sign_request`, `payload_hash` | Təkrar oxutma konfliktinin aşkarlanması üçün faydalı yük heşi qeydə alınıb. |
| `SignResult` | Pul kisəsi | `seq_wallet` | ✅ | `event=sign_result`, `status=ok|err` | İmzalanmış baytların BLAKE3 hashını ehtiva edir; uğursuzluqlar `ConnectError.Signing` artırır. |
| `Control::Error` | Pul kisəsi (ən çox) / dApp (nəqliyyat) | uyğun sahib domeni | ✅ | `event=error`, `code` | Fatal səhvlər seansı yenidən başlatmağa məcbur edir; telemetriya işarələri `fatal=true`. |
| `Control::RotateKeys` | Pul kisəsi | `seq_wallet` | ✅ | `event=rotate_keys`, `reason` | Yeni istiqamət düymələrini elan edir; dApp `RotateKeysAck` ilə cavab verir (tətbiq tərəfində jurnal). |
| `Control::Resume` / `ResumeAck` | Hər ikisi | yalnız yerli domen | ✅ | `event=resume`, `direction=app|wallet` | Növbənin dərinliyini + ardıcıl vəziyyətini ümumiləşdirir; Hashed jurnal həzm yardımları diaqnoz. |

- İstiqamətli şifrə açarları hər rol üçün simmetrik olaraq qalır (`app→wallet`, `wallet→app`).
  Pul kisəsinin fırlanma təklifləri `Control::RotateKeys` və dApps vasitəsilə elan edilir
  `Control::RotateKeysAck` emissiyası ilə təsdiq edin; hər iki çərçivə diskə dəyməlidir
  təkrar oynatma boşluqlarının qarşısını almaq üçün düymələri dəyişdirməzdən əvvəl.
- Metadata əlavəsi (ikonalar, lokallaşdırılmış adlar, uyğunluq sübutları) tərəfindən imzalanır
  cüzdan və dApp tərəfindən keşlənmiş; yeniləmələr ilə yeni təsdiq çərçivəsi tələb olunur
  artan `metadata_version`.
- Yuxarıdakı sahiblik matrisi SDK sənədlərindən istinad edilir, buna görə CLI/web/automation
  müştərilər eyni müqavilə və alətlərin defoltlarına əməl edirlər.

## Açıq Suallar

1. **Sessiyanın kəşfi**: Bizə WalletConnect kimi QR kodları / diapazondan kənar əl sıxma lazımdır? (Gələcək iş.)
2. **Multisig**: Çox işarəli təsdiqlər necə təmsil olunur? (Birdən çox imzanı dəstəkləmək üçün işarə nəticəsini genişləndirin.)
3. **Uyğunluq**: Hansı sahələr tənzimlənən axınlar üçün məcburidir (yol xəritəsi üzrə)? (Uyğunluq qrupunun rəhbərliyini gözləyin.)
4. **SDK qablaşdırma**: Paylaşılan kodu (məsələn, Norito Connect kodekləri) çarpaz platforma qutusuna daxil etməliyikmi? (TBD.)

## Növbəti Addımlar- Bu saman adamını SDK şurasına göndərin (fevral 2026-cı il iclası).
- Açıq suallar üzrə rəy toplayın və müvafiq olaraq sənədi yeniləyin.
- SDK (Swift IOS7, Android AND7, JS Connect mərhələləri) üzrə icra bölgüsünü planlaşdırın.
- Yol xəritəsi qaynar siyahısı vasitəsilə tərəqqi izləyin; strawman ratifikasiya edildikdən sonra `status.md` yeniləyin.