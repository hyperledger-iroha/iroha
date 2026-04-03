<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
direction: ltr
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Təhlükəsizlik Auditi Hesabatı

Tarix: 26-03-2026

## İcra Xülasəsi

Bu audit cari ağacdakı ən yüksək riskli səthlərə yönəldilib: Torii HTTP/API/auth axınları, P2P nəqliyyatı, məxfi idarəolunma API-ləri, SDK nəqliyyat qoruyucuları və qoşma təmizləyicisi yolu.

6 həll edilə bilən problem tapdım:

- 2 yüksək ciddilik tapıntıları
- 4 Orta şiddət tapıntısı

Ən mühüm problemlər bunlardır:

1. Torii hazırda hər HTTP sorğusu üçün daxil olan sorğu başlıqlarını qeyd edir ki, bu da daşıyıcı tokenləri, API tokenlərini, operator seansını/bootstrap tokenlərini və loglara yönləndirilmiş mTLS markerlərini ifşa edə bilər.
2. Çoxsaylı ictimai Torii marşrutları və SDK-lar hələ də xam `private_key` dəyərlərinin serverə göndərilməsini dəstəkləyir, beləliklə Torii zəng edənin adından daxil ola bilər.
3. Bəzi SDK-larda məxfi toxum əldəetmə və kanonik sorğu auth daxil olmaqla, bir neçə "gizli" yol adi sorğu orqanları kimi qəbul edilir.

## Metod

- Torii, P2P, kripto/VM və SDK gizli idarə yollarının statik nəzərdən keçirilməsi
- Məqsədli doğrulama əmrləri:
  - `cargo check -p iroha_torii --lib --message-format short` -> keç
  - `cargo check -p iroha_p2p --message-format short` -> keç
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> keç
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> keçid, yalnız dublikat versiya xəbərdarlıqları
- Bu keçiddə tamamlanmayıb:
  - tam iş sahəsinin qurulması/test/klipi
  - Swift/Gradle test paketləri
  - CUDA/Metal iş vaxtının yoxlanılması

## Tapıntılar

### SA-001 Yüksək: Torii qlobal olaraq həssas sorğu başlıqlarını qeyd edirTəsir: Gəmilərin izləmə tələb etdiyi hər hansı yerləşdirmə daşıyıcı/API/operator tokenlərini və əlaqədar auth materialını tətbiq jurnallarına sızdıra bilər.

Sübut:

- `crates/iroha_torii/src/lib.rs:20752` `TraceLayer::new_for_http()` imkan verir
- `crates/iroha_torii/src/lib.rs:20753` `DefaultMakeSpan::default().include_headers(true)`-i aktivləşdirir
- Həssas başlıq adları eyni xidmətdə başqa yerlərdə aktiv şəkildə istifadə olunur:
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

Bu niyə vacibdir:

- `include_headers(true)` tam daxil olan başlıq dəyərlərini izləmə aralığına qeyd edir.
- Torii, `Authorization`, `x-api-token`, `x-iroha-operator-session`, `x-iroha-operator-token` və `x-forwarded-client-cert` kimi başlıqlarda autentifikasiya materialını qəbul edir.
- Günlük sink kompromisi, debug jurnalının toplanması və ya dəstək paketi buna görə də etimadnamənin açıqlanması hadisəsinə çevrilə bilər.

Tövsiyə olunan təmir:

- İstehsal aralığına tam sorğu başlıqlarını daxil etməyi dayandırın.
- Hataların aradan qaldırılması üçün başlıq qeydi hələ də lazımdırsa, təhlükəsizliyə həssas başlıqlar üçün açıq redaksiya əlavə edin.
- Məlumatların müsbət olaraq icazə verilən siyahıya salınmadığı halda sorğu/cavab qeydini defolt olaraq gizli hesab edin.

### SA-002 Yüksək: İctimai Torii API-ləri hələ də server tərəfində imzalanma üçün xam şəxsi açarları qəbul edir

Təsir: Müştərilərə şəbəkə üzərindən xam şəxsi açarları ötürmək tövsiyə olunur ki, server onların adından daxil ola bilsin, API, SDK, proxy və server yaddaşı təbəqələrində lazımsız məxfi ifşa kanalı yarada bilsin.

Sübut:- İdarəetmə marşrutu sənədləri açıq şəkildə server tərəfində imzalamağı reklam edir:
  - `crates/iroha_torii/src/gov.rs:495`
- Marşrutun tətbiqi təmin edilmiş şəxsi açarı təhlil edir və server tərəfini işarələyir:
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- SDK-lar `private_key`-i JSON orqanlarına aktiv şəkildə seriallaşdırır:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

Qeydlər:

- Bu nümunə bir marşrut ailəsi üçün təcrid olunmur. Cari ağac idarəetmə, oflayn nağd pul, abunəliklər və digər tətbiqlərlə bağlı DTO-lar üzrə eyni rahatlıq modelini ehtiva edir.
- Yalnız HTTPS nəqliyyat yoxlamaları təsadüfi açıq mətn daşınmasını azaldır, lakin onlar server tərəfində məxfi işləmə və ya giriş/yaddaş məruz qalma riskini həll etmir.

Tövsiyə olunan təmir:

- Xam `private_key` məlumatını daşıyan bütün sorğu DTO-larını ləğv edin.
- Müştərilərdən yerli olaraq imza atmağı və imzaları və ya tam imzalanmış əməliyyatları/zərfləri təqdim etmələrini tələb edin.
- Uyğunluq pəncərəsindən sonra `private_key` nümunələrini OpenAPI/SDK-lardan silin.

### SA-003 Orta: Məxfi açarın əldə edilməsi gizli toxum materialını Torii-ə göndərir və onu geri qaytarır

Təsir: Məxfi açar törəmə API-si toxum materialını normal sorğu/cavab yükü datasına çevirir, proksilər, ara proqram, qeydlər, izlər, qəza hesabatları və ya müştərinin sui-istifadəsi vasitəsilə toxumun açıqlanması şansını artırır.

Sübut:- Sorğu birbaşa toxum materialını qəbul edir:
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  - `crates/iroha_torii/src/routing.rs:2740`
- Cavab sxemi toxumu həm hex, həm də base64-də əks etdirir:
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- İşləyici toxumu açıq şəkildə yenidən kodlayır və qaytarır:
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- Swift SDK bunu adi şəbəkə metodu kimi ifşa edir və cavab modelində əks-sədalanan toxumu saxlayır:
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

Tövsiyə olunan təmir:

- CLI/SDK kodunda yerli açar əldə etməyə üstünlük verin və uzaqdan törəmə marşrutunu tamamilə silin.
- Marşrut qalmalıdırsa, cavabda heç vaxt toxumu qaytarmayın və bütün nəqliyyat qoruyucularında və telemetriya/karotaj yollarında toxum daşıyan cisimləri həssas olaraq qeyd edin.

### SA-004 Orta: SDK nəqliyyat həssaslığının aşkarlanması `private_key` olmayan gizli material üçün kor nöqtələrə malikdir

Təsir: Bəzi SDK-lar xam `private_key` sorğuları üçün HTTPS-i tətbiq edəcək, lakin yenə də digər təhlükəsizliyə həssas sorğu materiallarının etibarsız HTTP üzərindən və ya uyğun gəlməyən hostlara getməsinə icazə verəcək.

Sübut:- Swift kanonik sorğu auth başlıqlarını həssas hesab edir:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- Ancaq Swift hələ də yalnız `"private_key"`-də bədənə uyğun gəlir:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- Kotlin yalnız `authorization` və `x-api-token` başlıqlarını tanıyır, sonra eyni `"private_key"` gövdə evristikinə qayıdır:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android eyni məhdudiyyətə malikdir:
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- Kotlin/Java kanonik sorğu imzalayanlar öz nəqliyyat mühafizəçiləri tərəfindən həssas kimi təsnif edilməyən əlavə auth başlıqları yaradırlar:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

Tövsiyə olunan təmir:

- Evristik bədən skanını açıq sorğu təsnifatı ilə əvəz edin.
- Kanonik auth başlıqlarını, toxum/parol sahələrini, imzalanmış mutasiya başlıqlarını və hər hansı gələcək məxfi sahələri alt sətir uyğunluğu ilə deyil, müqavilə ilə həssas hesab edin.
- Həssaslıq qaydalarını Swift, Kotlin və Java arasında uyğunlaşdırın.

### SA-005 Orta: "Qum qutusu" qoşması yalnız bir alt prosesdir plus `setrlimit`Təsir: Qoşmanın dezinfeksiyaedicisi "qum qutusu" kimi təsvir edilir və bildirilir, lakin tətbiq resurs məhdudiyyətləri olan cari binarın yalnız fork/execsidir. Parser və ya arxiv istismarı hələ də Torii ilə eyni istifadəçi, fayl sistemi görünüşü və ətraf şəbəkə/proses imtiyazları ilə icra olunacaq.

Sübut:

- Xarici yol, uşağın kürü verdikdən sonra nəticəni sandboxed kimi qeyd edir:
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- Uşaq cari icra olunana defolt olaraq təyin edir:
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- Alt proses açıq şəkildə yenidən `AttachmentSanitizerMode::InProcess`-ə keçir:
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
- Tətbiq olunan yeganə sərtləşdirmə CPU/ünvan sahəsi `setrlimit`-dir:
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  - `crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

Tövsiyə olunan təmir:

- Ya real OS sandboxunu tətbiq edin (məsələn, ad boşluqları/seccomp/landlock/həbsxana tipli izolyasiya, imtiyazların düşməsi, şəbəkəsiz, məhdudlaşdırılmış fayl sistemi) və ya nəticənin `sandboxed` kimi etiketlənməsini dayandırın.
- Həqiqi izolyasiya mövcud olana qədər cari dizaynı API, telemetriya və sənədlərdə "qum qutusu" deyil, "alt proses izolyasiyası" kimi qəbul edin.

### SA-006 Orta: İsteğe bağlı P2P TLS/QUIC daşımaları sertifikatın yoxlanılmasını söndürürTəsir: `quic` və ya `p2p_tls` aktiv olduqda, kanal şifrələməni təmin edir, lakin uzaq son nöqtəni təsdiq etmir. Aktiv yolda olan təcavüzkar operatorların TLS/QUIC ilə əlaqələndirdiyi normal təhlükəsizlik gözləntilərini məğlub edərək kanalı ötürə və ya dayandıra bilər.

Sübut:

- QUIC icazə verilən sertifikat yoxlanışını açıq şəkildə sənədləşdirir:
  - `crates/iroha_p2p/src/transport.rs:12`
  - `crates/iroha_p2p/src/transport.rs:13`
  - `crates/iroha_p2p/src/transport.rs:14`
  - `crates/iroha_p2p/src/transport.rs:15`
- QUIC doğrulayıcısı qeyd-şərtsiz server sertifikatını qəbul edir:
  - `crates/iroha_p2p/src/transport.rs:33`
  - `crates/iroha_p2p/src/transport.rs:35`
  - `crates/iroha_p2p/src/transport.rs:44`
  - `crates/iroha_p2p/src/transport.rs:112`
  - `crates/iroha_p2p/src/transport.rs:114`
  - `crates/iroha_p2p/src/transport.rs:115`
- TCP üzərindən TLS nəqliyyatı eyni şeyi edir:
  - `crates/iroha_p2p/src/transport.rs:229`
  - `crates/iroha_p2p/src/transport.rs:232`
  - `crates/iroha_p2p/src/transport.rs:241`
  - `crates/iroha_p2p/src/transport.rs:279`
  - `crates/iroha_p2p/src/transport.rs:281`
  - `crates/iroha_p2p/src/transport.rs:282`

Tövsiyə olunan təmir:

- Ya həmyaşıd sertifikatlarını yoxlayın və ya daha yüksək səviyyəli imzalanmış əl sıxma və nəqliyyat sessiyası arasında açıq kanal bağlaması əlavə edin.
- Əgər cari davranış qəsdəndirsə, operatorlar onu tam TLS həmyaşıdlarının autentifikasiyası ilə səhv salmasınlar deyə, funksiyanı təsdiqlənməmiş şifrəli nəqliyyat kimi dəyişdirin/sənədləşdirin.

## Tövsiyə olunan Təmir Sifarişi1. Başlıq qeydini redaktə edərək və ya söndürməklə SA-001-i dərhal düzəldin.
2. SA-002 üçün miqrasiya planını hazırlayın və göndərin ki, xam şəxsi açarlar API sərhədini keçməsin.
3. Uzaqdan məxfi açar əldə etmə marşrutunu çıxarın və ya daraldın və toxum daşıyan cisimləri həssas olaraq təsnif edin.
4. Swift/Kotlin/Java üzrə SDK daşıma həssaslığı qaydalarını uyğunlaşdırın.
5. Qoşulma sanitarizasiyasının həqiqi qum qutusuna, yoxsa dürüst adlandırılmasına/yenidən əhatə olunmasına ehtiyac olub-olmadığına qərar verin.
6. Operatorlar təsdiqlənmiş TLS gözləyən nəqliyyatları işə salmazdan əvvəl P2P TLS/QUIC təhlükə modelini aydınlaşdırın və sərtləşdirin.

## Doğrulama Qeydləri

- `cargo check -p iroha_torii --lib --message-format short` keçdi.
- `cargo check -p iroha_p2p --message-format short` keçdi.
- `cargo deny check advisories bans sources --hide-inclusion-graph` qum qutusundan kənarda qaçdıqdan sonra keçdi; o, dublikat versiya xəbərdarlıqları yaydı, lakin `advisories ok, bans ok, sources ok` məlumat verdi.
- Bu audit zamanı məxfi törəmə-açar dəstləri marşrutu üçün fokuslanmış Torii testi başladıldı, lakin hesabat yazılmamışdan əvvəl tamamlanmadı; tapıntı birbaşa mənbə yoxlaması ilə təsdiqlənir.