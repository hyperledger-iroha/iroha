<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
direction: ltr
source: security_best_practices_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6055090fbfab93113392610b3eb78ef0a0e97d6fef4a94292c5609ec5562e925
source_last_modified: "2026-03-26T06:12:11.487656+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Ən Yaxşı Təhlükəsizlik Təcrübələri Hesabatı

Tarix: 25-03-2026

## İcra Xülasəsi

Mən əvvəlki Torii/Soracloud hesabatını cari iş sahəsinə qarşı yenilədim
kodu və nəzərdən keçirilməsini ən yüksək riskli server, SDK və
kripto/seriyalaşdırma səthləri. Bu audit əvvəlcə üçü təsdiqlədi
giriş/autentifikasiya problemləri: iki yüksək şiddət və bir orta ciddilik.
Bu üç tapıntı indi remediasiya ilə cari ağacda bağlanıb
aşağıda təsvir edilmişdir. Nəqliyyatın təqibi və daxili göndərmə baxışı təsdiqləndi
doqquz əlavə orta ciddilik problemi: bir gedən P2P identifikasiyası
boşluq, bir gedən P2P TLS-in səviyyəsini aşağı salan defolt, iki Torii etibar sərhədi xətası
webhook çatdırılması və MCP daxili dispetçer, bir çarpaz SDK həssas nəqliyyat
Swift, Java/Android, Kotlin və JS müştərilərində boşluq, bir SoraFS
etibarlı-proksi/müştəri-IP siyasəti boşluğu, bir SoraFS yerli-proksi
bağlama/autentifikasiya boşluğu, bir həmyaşıd telemetriya geo-axtarış açıq mətn geri qaytarılması,
və bir operator-auth uzaqdan IP uğursuz-açıq lockout/rate-keying boşluğu. Bunlar
Sonrakı tapıntılar da cari ağaca bağlanır.Dörd əvvəllər Soracloud-un xam şəxsi açarlarla bağlı tapıntılarını bildirdilər
HTTP, daxili yalnız yerli oxunan proksi icrası, ölçülməmiş ictimai iş vaxtı
ehtiyat və uzaqdan IP qoşması icarəsi artıq cari kodda mövcud deyil.
Bunlar yenilənmiş kod istinadları ilə aşağıda qapalı/əvəzi ilə işarələnmişdir.Bu, hərtərəfli qırmızı komanda məşqi deyil, kod mərkəzli audit olaraq qaldı.
Xarici olaraq əldə edilə bilən Torii giriş və sorğu-auth yollarına üstünlük verdim, sonra
spot yoxlanılmış IVM, `iroha_crypto`, `norito`, Swift/Android/JS SDK
sorğu imzalayan köməkçilər, həmyaşıd telemetriya geo yolu və SoraFS
iş stansiyası proksi köməkçiləri və SoraFS pin/şluz müştəri-IP siyasəti
səthlər. Bu giriş/auth, çıxış siyasətindən canlı təsdiqlənmiş problem yoxdur,
peer-telemetry geo, nümunə götürülmüş P2P nəqliyyat defoltları, MCP-göndərmə, nümunə götürülmüş SDK
nəqliyyat, operator-auth lockout/rate-keying, SoraFS etibarlı-proxy/client-IP
siyasət və ya yerli proksi dilimi bu hesabatdakı düzəlişlərdən sonra qalır.
Sonrakı sərtləşmə, həmçinin uğursuz bağlı başlanğıc həqiqət dəstlərini genişləndirdi
nümunə götürülmüş IVM CUDA/Metal sürətləndirici yolları; ki, iş yeni təsdiq etmədi
uğursuz-açıq məsələ. Nümunələnmiş Metal Ed25519
imza yolu indi çoxsaylı ref10 sürüşməsi düzəldildikdən sonra bu hostda bərpa olunur
Metal/CUDA portlarında nöqtələr: yoxlamada müsbət baza nöqtəsi ilə işləmə,
`d2` sabiti, dəqiq `fe_sq2` azalma yolu, başıboş son
`fe_mul` daşıma addımı və əməliyyatdan sonrakı sahənin normallaşmasına imkan verən çatışmazlıq
sərhədlər skalyar nərdivan boyunca sürüşür. Fokuslanmış Metal reqressiya əhatə dairəsi indi
imza boru kəmərini aktiv saxlayır və `[true, false]`-i yoxlayırCPU istinad yoluna qarşı sürətləndirici. Nümunələnmiş başlanğıc həqiqəti indi müəyyən edilir
həmçinin canlı vektoru birbaşa yoxlayın (`vadd64`, `vand`, `vxor`, `vor`) və
bu backendlərdən əvvəl həm Metal, həm də CUDA-da birdəfəlik AES toplu nüvələri
aktiv qalın. Daha sonra asılılıq taraması yeddi canlı üçüncü tərəf tapıntısını əlavə etdi
geridə qaldı, lakin indiki ağac o vaxtdan hər iki aktiv `tar`-ni sildi
`xtask` Rust `tar` asılılığını ləğv edərək və
`iroha_crypto` `libsodium-sys-stable` OpenSSL dəstəyi ilə qarşılıqlı əlaqə testləri
ekvivalentlər. Cari ağac birbaşa PQ asılılıqlarını da əvəz etdi
həmin taramada işarələnmiş, `soranet_pq`, `iroha_crypto` və `ivm`-dən köçür
`pqcrypto-dilithium` / `pqcrypto-kyber`
Mövcud ML-DSA-nı qoruyarkən `pqcrypto-mldsa` / `pqcrypto-mlkem` /
ML-KEM API səthi. Daha sonra eyni gündə asılılıq keçidi daha sonra iş sahəsini bağladı
`reqwest` / `rustls` versiyaları yamaqlı yamaq buraxılışlarına qədər
Cari həlldə sabit `0.103.10` xəttində `rustls-webpki`. yeganə
qalan asılılıq siyasəti istisnaları qorunmayan iki keçiddir
indi açıq şəkildə qəbul edilən makro qutular (`derivative`, `paste`)
`deny.toml`, çünki təhlükəsiz təkmilləşdirmə yoxdur və onların çıxarılması tələb olunur
birdən çox yuxarı axının dəyişdirilməsi və ya satılması. TheQalan sürətləndirici işidir
güzgülənmiş CUDA düzəlişinin icra müddətinin yoxlanılması və genişləndirilmiş CUDA həqiqəti təyin edilmişdir
təsdiqlənmiş düzgünlük və ya uğursuzluq deyil, canlı CUDA sürücü dəstəyi ilə host
cari ağacda problem.

## Yüksək Ciddilik

### SEC-05: Tətbiq kanonik sorğunun yoxlanması multisig hədlərini keçdi (24-03-2026-cı il tarixdə bağlıdır)

Təsir:

- Multisig tərəfindən idarə olunan hesabın istənilən tək üzv açarı icazə verə bilər
  həddi və ya ölçülməsini tələb edən tətbiqlə bağlı sorğular
  kvorum.
- Bu, daxil olmaqla, `verify_canonical_request`-ə güvənən hər bir son nöqtəyə təsir göstərir
  Soracloud imzalı mutasiya girişi, məzmuna giriş və imzalanmış hesab ZK
  əlavə icarə.

Sübut:

- `verify_canonical_request` multisig nəzarətçisini tam üzvə genişləndirir
  açıq açar siyahısı və sorğunu təsdiqləyən ilk açarı qəbul edir
  həddi və ya yığılmış çəkisini qiymətləndirmədən imza:
  `crates/iroha_torii/src/app_auth.rs:198-210`.
- Həqiqi multisig siyasət modeli həm `threshold`, həm də ağırlıqlı
  üzvlərini qəbul edir və həddi ümumi çəkidən çox olan siyasətləri rədd edir:
  `crates/iroha_data_model/src/account/controller.rs:92-95`,
  `crates/iroha_data_model/src/account/controller.rs:163-178`,
  `crates/iroha_data_model/src/account/controller.rs:188-196`.
- Köməkçi Soracloud mutasiyasının daxil olması üçün icazə yolundadır
  `crates/iroha_torii/src/lib.rs:2141-2157`, daxil olan məzmuna daxil olan hesaba giriş
  `crates/iroha_torii/src/content.rs:359-360` və əlavə icarə
  `crates/iroha_torii/src/lib.rs:7962-7968`.

Bu niyə vacibdir:- Sorğu imzalayan şəxs HTTP qəbulu üçün hesab səlahiyyətlisi kimi qəbul edilir,
  lakin tətbiq səssizcə multisig hesablarını "hər hansı bir tək"ə endirir
  üzv təkbaşına fəaliyyət göstərə bilər”.
- Bu, dərin müdafiə HTTP imza qatını avtorizasiyaya çevirir
  multisig ilə qorunan hesablar üçün yan keçid.

Tövsiyə:

- Ya app-auth qatında multisig ilə idarə olunan hesabları a
  müvafiq şahid formatı mövcuddur və ya protokolu HTTP sorğusu üçün genişləndirin
  həddi təmin edən tam multisig şahid dəstini daşıyır və yoxlayır
  çəki.
- Soracloud mutasiya proqram təminatı, məzmun auth və ZK əhatə edən reqressiyalar əlavə edin
  eşikdən aşağı multisig imzaları üçün əlavələr.

Təmir statusu:

- Multisig ilə idarə olunan hesablarda uğursuz bağlanaraq cari kodda bağlandı
  `crates/iroha_torii/src/app_auth.rs`.
- Doğrulayıcı artıq "hər hansı bir üzv imzalaya bilər" semantikasını qəbul etmir
  multisig HTTP icazəsi; a qədər multisig sorğuları rədd edilir
  həddi qane edən şahid formatı mövcuddur.
- Reqressiyanın əhatə dairəsinə indi xüsusi multisig imtina halı daxildir
  `crates/iroha_torii/src/app_auth.rs`.

## Yüksək Ciddilik

### SEC-06: Tətbiq kanonik sorğu imzaları qeyri-müəyyən müddətə təkrar oxuna bilərdi (2026-03-24 bağlanıb)

Təsir:- Əldə edilmiş etibarlı sorğu təkrar oxuna bilər, çünki imzalanmış mesajda yoxdur
  vaxt damgası, bir dəfə yox, son istifadə müddəti və ya təkrar keş.
- Bu, vəziyyəti dəyişən Soracloud mutasiya sorğularını təkrarlaya və yenidən buraxa bilər
  orijinal müştəridən çox sonra hesaba bağlı məzmun/qoşma əməliyyatları
  onları nəzərdə tuturdu.

Sübut:

- Torii tətbiqin kanonik sorğusunu yalnız olaraq təyin edir
  `METHOD + path + sorted query + body hash` in
  `crates/iroha_torii/src/app_auth.rs:1-17` və
  `crates/iroha_torii/src/app_auth.rs:74-89`.
- Doğrulayıcı yalnız `X-Iroha-Account` və `X-Iroha-Signature`-i qəbul edir və edir
  təzəliyi tətbiq etməyin və ya təkrar oynatma keşini saxlamayın:
  `crates/iroha_torii/src/app_auth.rs:137-218`.
- JS, Swift və Android SDK köməkçiləri eyni təkrar oxumağa meylli başlığı yaradır
  birdəfəlik/zaman damğası sahələri olmayan cütləşdirin:
  `javascript/iroha_js/src/canonicalRequest.js:50-82`,
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift:41-68`, və
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:67-106`.
- Torii operator-imza yolu artıq daha güclü modeldən istifadə edir.
  tətbiqə baxan yol yoxdur: vaxt damğası, bir dəfə yox və keşi təkrar oynatmaq
  `crates/iroha_torii/src/operator_signatures.rs:1-21` və
  `crates/iroha_torii/src/operator_signatures.rs:266-294`.

Bu niyə vacibdir:

- Yalnız HTTPS əks proksi, debug logger tərəfindən təkrarın qarşısını almır,
  güzəşt edilmiş müştəri hostu və ya etibarlı sorğuları qeyd edə bilən hər hansı vasitəçi.
- Çünki eyni sxem bütün əsas müştəri SDK-larında həyata keçirilir, təkrar oynatma
  zəiflik yalnız server deyil, sistemlidir.

Tövsiyə:- Tətbiq auth sorğularına imzalanmış təzəlik materialı əlavə edin, minimum vaxt damğası
  və qeyri-neft və məhdud təkrar keş yaddaşı ilə köhnəlmiş və ya təkrar istifadə edilmiş dəstləri rədd edin.
- Torii və SDK-lar açıq şəkildə tətbiqin kanonik sorğu formatını versiyası
  köhnə iki başlıqlı sxemi etibarlı şəkildə ləğv edin.
- Soracloud mutasiyaları, məzmunu üçün təkrar təkrar rədd edilməsini sübut edən reqressiyalar əlavə edin
  giriş və CRUD əlavəsi.

Təmir statusu:

- Cari kodda bağlanıb. Torii indi dörd başlıq sxemini tələb edir
  (`X-Iroha-Account`, `X-Iroha-Signature`, `X-Iroha-Timestamp-Ms`,
  `X-Iroha-Nonce`) və imzalayır/yoxlayır
  `METHOD + path + sorted query + body hash + timestamp + nonce` in
  `crates/iroha_torii/src/app_auth.rs`.
- Təravətin yoxlanılması indi məhdud saat əyri pəncərəsini tətbiq edir, təsdiqləyir
  nonce forma verir və yaddaşdaxili təkrar oynatma keşi ilə təkrar istifadə edilmiş nonceləri rədd edir
  düymələr `crates/iroha_config/src/parameters/{defaults,actual,user}.rs` vasitəsilə üzə çıxır.
- JS, Swift və Android köməkçiləri indi eyni dörd başlıq formatını yayırlar
  `javascript/iroha_js/src/canonicalRequest.js`,
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift`, və
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java`.
- Reqressiya əhatə dairəsinə indi müsbət imza yoxlanışı və təkrar oynatma daxildir,
  köhnəlmiş vaxt damğası və itkin təzəlikdən imtina halları
  `crates/iroha_torii/src/app_auth.rs`.

## Orta Ciddilik

### SEC-07: mTLS tətbiqi saxtalaşdırıla bilən yönləndirilmiş başlığa etibar etdi (2026-03-24 bağlandı)

Təsir:- `require_mtls`-ə əsaslanan yerləşdirmələr Torii birbaşa olduqda yan keçə bilər
  əlçatandır və ya ön proksi müştəri tərəfindən təmin edilənləri silmir
  `x-forwarded-client-cert`.
- Problem konfiqurasiyadan asılıdır, lakin işə salındıqda iddia edilənə çevrilir
  müştəri sertifikatı tələbini düz başlıq yoxlamasına çevirin.

Sübut:

- Norito-RPC qapısı zəng edərək `require_mtls`-i tətbiq edir
  `norito_rpc_mtls_present`, yalnız olub olmadığını yoxlayır
  `x-forwarded-client-cert` mövcuddur və boş deyil:
  `crates/iroha_torii/src/lib.rs:1897-1926`.
- Operator-auth bootstrap/giriş axınları yalnız rədd edən `check_common` çağırır
  `mtls_present(headers)` yanlış olduqda:
  `crates/iroha_torii/src/operator_auth.rs:562-570`.
- `mtls_present` həm də yalnız boş olmayan `x-forwarded-client-cert` girişidir
  `crates/iroha_torii/src/operator_auth.rs:1212-1216`.
- Həmin operator-auth işləyiciləri hələ də marşrutlar kimi məruz qalır
  `crates/iroha_torii/src/lib.rs:16658-16672`.

Bu niyə vacibdir:

- Yönləndirilmiş başlıq konvensiyası yalnız Torii arxada oturduqda etibarlıdır.
  başlığı ayıran və yenidən yazan sərtləşdirilmiş proxy. Kod təsdiqlənmir
  yerləşdirmə fərziyyəsinin özü.
- Səssiz şəkildə əks-proksi gigiyenasından asılı olan təhlükəsizlik nəzarətləri asandır
  səhnələşdirmə, kanarya və ya insident-cavab marşrutu dəyişiklikləri zamanı yanlış konfiqurasiya.

Tövsiyə:- Mümkün olduqda birbaşa nəqliyyat-dövlət icrasına üstünlük verin. Əgər proxy olmalıdır
  istifadə edildikdə, təsdiqlənmiş proksi-Torii kanalına etibar edin və icazə siyahısı tələb edin
  və ya xam başlığın mövcudluğu əvəzinə həmin proxydən imzalanmış attestasiya.
- `require_mtls`-in birbaşa məruz qalmış Torii dinləyiciləri üçün təhlükəli olduğuna dair sənəd.
- Norito-RPC-də saxta `x-forwarded-client-cert` girişi üçün mənfi testlər əlavə edin
  və operator-auth bootstrap marşrutları.

Təmir statusu:

- Konfiqurasiya edilmiş proksiyə yönləndirilmiş başlıq etibarını bağlamaqla cari kodda bağlanıb
  Yalnız xam başlıq mövcudluğu əvəzinə CIDR-lər.
- `crates/iroha_torii/src/limits.rs` indi paylaşılanı təmin edir
  `has_trusted_forwarded_header(...)` qapısı və hər ikisi Norito-RPC
  (`crates/iroha_torii/src/lib.rs`) və operator-auth
  (`crates/iroha_torii/src/operator_auth.rs`) onu zəng edən TCP peer ilə istifadə edin
  ünvanı.
- `iroha_config` indi hər ikisi üçün `mtls_trusted_proxy_cidrs`-i ifşa edir
  operator-auth və Norito-RPC; defoltlar yalnız geri dönmədir.
- Reqressiya əhatə dairəsi indi saxta `x-forwarded-client-cert` girişini rədd edir
  həm operator-auth, həm də paylaşılan limitlər köməkçisində etibarsız pult.

## Orta Ciddilik

### SEC-08: Giden P2P yığımları təsdiqlənmiş açarı nəzərdə tutulan həmyaşıd identifikatoruna bağlamadı (2026-03-25 bağlandı)

Təsir:- `X` peer-ə gedən zəng, açarı olan hər hansı digər `Y` peer kimi tamamlana bilər
  app-layer handshake uğurla imzalandı, çünki handshake
  "bu əlaqənin açarı" təsdiqləndi, lakin açarın olub-olmadığını heç vaxt yoxlamadı
  şəbəkə aktyorunun çatmaq niyyətində olduğu peer id.
- İcazə verilən üst-üstə düşmələrdə sonrakı topologiya/icazə siyahısı yoxlanışı hələ də azalır
  səhv açar, buna görə də bu, ilk növbədə əvəzetmə / əlçatanlıq səhvi idi
  birbaşa konsensus təqlid səhvindən daha çox. Ictimai örtüklər bu imkan verə bilər
  pozulmuş ünvan, DNS cavabı və ya relay son nöqtəsi başqasını əvəz edir
  gedən zəngdə müşahidəçinin şəxsiyyəti.

Sübut:

- Giden həmyaşıd dövlət nəzərdə tutulan `peer_id`-i saxlayır
  `crates/iroha_p2p/src/peer.rs:5153-5179`, lakin köhnə əl sıxma axını
  imzanın yoxlanılmasından əvvəl bu dəyəri atdı.
- `GetKey::read_their_public_key` imzalanmış əl sıxma yükünü təsdiqlədi və
  sonra dərhal elan edilmiş uzaqdan açıq açardan `Peer` qurdu
  `crates/iroha_p2p/src/peer.rs:6266-6355` ilə müqayisə edilmədən
  `peer_id` əvvəlcə `connecting(...)` ilə təchiz edilmişdir.
- Eyni nəqliyyat yığını TLS / QUIC sertifikatını açıq şəkildə deaktiv edir
  `crates/iroha_p2p/src/transport.rs`-də P2P üçün doğrulama, buna görə də məcburi
  Tətbiq səviyyəsinin təsdiqlənmiş açarı nəzərdə tutulan peer identifikatoru üçün kritikdir
  gedən əlaqələrdə şəxsiyyət yoxlaması.

Bu niyə vacibdir:- Dizayn qəsdən həmyaşıdların autentifikasiyasını nəqliyyatdan yuxarıya itələyir
  qat, bu, əl sıxma açarını yeganə davamlı şəxsiyyət bağlamasını yoxlayır
  gedən zənglərdə.
- Bu yoxlama olmadan şəbəkə təbəqəsi səssizcə "uğurla" müalicə edə bilər
  authenticated some peer" kimi "zəng etdiyimiz həmyaşıdı çatdı"
  bu daha zəif zəmanətdir və topologiya/reputasiya vəziyyətini təhrif edə bilər.

Tövsiyə:

- Nəzərdə tutulan `peer_id`-i imzalanmış əl sıxma mərhələləri vasitəsilə həyata keçirin və
  doğrulanmış uzaqdan idarəetmə açarı ona uyğun gəlmirsə, uğursuz bağlanır.
- Etibarlı şəkildə imzalanmış əl sıxışmasını sübut edən fokuslanmış reqressiyanı saxlayın
  normal imzalanmış əl sıxma hələ də uğurlu olarkən səhv açar rədd edilir.

Təmir statusu:

- Cari kodda bağlanıb. `ConnectedTo` və aşağı axın əl sıxma indi bildirir
  gözlənilən gedən `PeerId`-i daşıyır və
  `GetKey::read_their_public_key` uyğun olmayan autentifikasiya edilmiş açarı rədd edir
  `HandshakePeerMismatch`, `crates/iroha_p2p/src/peer.rs`.
- Fokuslanmış reqressiya əhatəsi indi daxildir
  `outgoing_handshake_rejects_unexpected_peer_identity` və mövcud
  müsbət `handshake_v1_defaults_to_trust_gossip` yolu
  `crates/iroha_p2p/src/peer.rs`.

### SEC-09: HTTPS/WSS webhook çatdırılması əlaqə zamanı yoxlanılmış host adlarını yenidən həll etdi (2026-03-25 bağlandı)

Təsir:- Təhlükəsiz webhook çatdırılması təsdiqlənmiş təyinat DNS cavablarını veb-qancaya qarşı
  egress siyasəti, lakin sonra bu yoxlanılan ünvanları atılır və müştəri icazə
  stack faktiki HTTPS və ya WSS bağlantısı zamanı host adını yenidən həll edir.
- Doğrulama və əlaqə vaxtı arasında DNS-ə təsir edə bilən təcavüzkar
  əvvəllər icazə verilən host adını bloklanmış şəxsi və ya
  yalnız operator üçün təyinat yeri seçin və CIDR-əsaslı veb-qanca mühafizəsini keçin.

Sübut:

- Çıxış mühafizəçisi namizədin təyinat ünvanlarını həll edir və filtrləyir
  `crates/iroha_torii/src/webhook.rs:1746-1829` və təhlükəsiz çatdırılma yolları
  bu yoxlanılmış ünvan siyahılarını HTTPS / WSS köməkçilərinə ötürün.
- Köhnə HTTPS köməkçisi daha sonra orijinal URL-ə qarşı ümumi müştəri qurdu
  `crates/iroha_torii/src/webhook.rs`-də host və əlaqəni bağlamadı
  yoxlanılmış ünvan dəstinə, yəni DNS həlli yenidən içəridə baş verdi
  HTTP müştəri.
- Köhnə WSS köməkçisi də `tokio_tungstenite::connect_async(url)` adlanır
  orijinal host adına qarşı, bu da yerinə hostu yenidən həll etdi
  artıq təsdiqlənmiş ünvanı təkrar istifadə etmək.

Bu niyə vacibdir:

- Təyinat icazə siyahıları yalnız yoxlanılan ünvan eyni olduqda işləyir
  müştəri əslində bağlanır.
- Siyasət təsdiq edildikdən sonra yenidən həll edilməsi, a-da DNS yenidən bağlanma / TOCTOU boşluğu yaradır
  operatorların SSRF tipli saxlama üçün etibar edəcəyi yol.

Tövsiyə:- Yoxlanmış DNS cavablarını qoruyarkən faktiki HTTPS əlaqə yoluna daxil edin
  SNI / sertifikatın təsdiqi üçün orijinal host adı.
- WSS üçün TCP yuvasını birbaşa yoxlanılmış ünvana qoşun və TLS-ni işə salın
  host adına əsaslanan zəng əvəzinə həmin axın üzərində websocket əl sıxma
  rahatlıq bağlayıcısı.

Təmir statusu:

- Cari kodda bağlanıb. `crates/iroha_torii/src/webhook.rs` indi əldə edilir
  `https_delivery_dns_override(...)` və
  Yoxlanmış ünvan dəstindən `websocket_pinned_connect_addr(...)`.
- HTTPS çatdırılması indi `reqwest::Client::builder().resolve_to_addrs(...)` istifadə edir
  beləliklə, TCP əlaqəsi olduqda orijinal host adı TLS üçün görünən qalır
  artıq təsdiq edilmiş ünvanlara bərkidilir.
- WSS çatdırılması indi yoxlanılmış ünvana xam `TcpStream` açır və yerinə yetirir
  Həmin axın üzərində `tokio_tungstenite::client_async_tls_with_config(...)`,
  siyasətin doğrulanmasından sonra ikinci DNS axtarışından yayınır.
- İndi reqressiya əhatəsinə daxildir
  `https_delivery_dns_override_pins_vetted_domain_addresses`,
  `https_delivery_dns_override_skips_ip_literals`, və
  `websocket_pinned_connect_addr_pins_secure_delivery_when_guarded` in
  `crates/iroha_torii/src/webhook.rs`.

### SEC-10: MCP daxili marşrut göndərmə möhürlü geri dönmə və miras alınmış icazə siyahısı imtiyazı (25-03-2026-cı il tarixdə bağlıdır)

Təsir:- Torii MCP aktivləşdirildikdə, daxili alət göndərilməsi hər sorğunu aşağıdakı kimi yenidən yazdı
  faktiki zəng edəndən asılı olmayaraq geri dönmə. Zəng edən CIDR-ə etibar edən marşrutlar
  imtiyaz və ya məhdudlaşdırıcı bypass buna görə də MCP trafikini görə bilər
  `127.0.0.1`.
- Problem konfiqurasiya ilə bağlı idi, çünki MCP standart olaraq qeyri-aktivdir və
  təsirə məruz qalan marşrutlar hələ də icazə siyahısından və ya oxşar geri dönmə etibarından asılıdır
  siyasət, lakin bir dəfə MCP-ni imtiyazların yüksəldilməsi körpüsünə çevirdi
  düymələr birlikdə işə salındı.

Sübut:

- `dispatch_route(...)`, `crates/iroha_torii/src/mcp.rs` əvvəllər daxil edilib
  üçün `x-iroha-remote-addr: 127.0.0.1` və sintetik geri dönmə `ConnectInfo`
  hər bir daxili göndərilən sorğu.
- `iroha.parameters.get` yalnız oxumaq üçün rejimdə MCP səthində ifşa olunur və
  `/v1/parameters`, zəng edən IP-yə aid olduqda normal audentifikasiyadan yan keçir.
  `crates/iroha_torii/src/lib.rs:5879-5888`-də konfiqurasiya edilmiş icazə siyahısı.
- `apply_extra_headers(...)` həmçinin ixtiyari `headers` girişlərini qəbul etdi
  kimi MCP zəng edən, belə qorunur daxili etibar başlıqları
  `x-iroha-remote-addr` və `x-forwarded-client-cert` açıq şəkildə deyildi
  qorunur.

Bu niyə vacibdir:- Daxili körpü təbəqələri orijinal etibar sərhədini qorumalıdır. Əvəz olunur
  geri dönmə ilə real zəng edən hər MCP zəng edənə təsirli şəkildə yanaşır
  sorğu körpünü keçdikdən sonra daxili müştəri.
- Səhv incədir, çünki xarici görünən MCP profili hələ də görünə bilər
  daxili HTTP marşrutu daha imtiyazlı mənşəyi görərkən yalnız oxunur.

Tövsiyə:

- Xarici `/v1/mcp` sorğusunun artıq qəbul etdiyi zəng edənin IP-ni qoruyun
  Torii-in uzaq ünvanlı orta proqramından və `ConnectInfo`-dən sintez edin
  loopback əvəzinə həmin dəyər.
- `x-iroha-remote-addr` və kimi yalnız giriş üçün etibarlı başlıqları müalicə edin
  `x-forwarded-client-cert` qorunan daxili başlıqlar kimi MCP zəng edənlər edə bilməz
  `headers` arqumenti vasitəsilə onları qaçaqmalçılıq yolu ilə keçirin və ya ləğv edin.

Təmir statusu:

- Cari kodda bağlanıb. `crates/iroha_torii/src/mcp.rs` indi əldə edir
  xarici sorğunun yeridilmiş daxildən göndərilən uzaq IP
  `x-iroha-remote-addr` başlığı və həmin realdan `ConnectInfo` sintez edir
  geri dönmə əvəzinə zəng edən IP.
- `apply_extra_headers(...)` indi həm `x-iroha-remote-addr`, həm də düşür
  Qorunmuş daxili başlıqlar kimi `x-forwarded-client-cert`, beləliklə MCP zəng edənlər
  alət arqumentləri vasitəsilə geri dönmə / giriş-proksi etibarını saxtalaşdıra bilməz.
- İndi reqressiya əhatəsinə daxildir
  `dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks`
  `dispatch_route_blocks_remote_addr_spoofing_from_extra_headers`, və
  `apply_extra_headers_blocks_reserved_internal_headers` in
  `crates/iroha_torii/src/mcp.rs`.### SEC-11: SDK müştəriləri etibarlı olmayan və ya hostlar arası daşımalar üzərində həssas sorğu materialına icazə verdi (2026-03-25 bağlandı)

Təsir:

- Nümunələnmiş Swift, Java/Android, Kotlin və JS müştəriləri bunu etmədi
  ardıcıl olaraq bütün həssas sorğu formalarını nəqliyyata həssas kimi qəbul edin.
  Köməkçidən asılı olaraq, zəng edənlər daşıyıcı/API-token başlıqlarını xam, göndərə bilər
  `private_key*` JSON sahələri və ya tətbiqin kanonik auth imzalama materialı üzərində
  düz `http` / `ws` və ya hostlar arası mütləq URL ləğvetmələri vasitəsilə.
- Xüsusilə JS müştərisində `canonicalAuth` başlıqları sonra əlavə edildi
  `_request(...)` nəqliyyat yoxlamalarını tamamladı və yalnız bədən üçün `private_key`
  JSON ümumiyyətlə həssas nəqliyyat kimi sayılmadı.

Sübut:- Swift indi mühafizəçini mərkəzləşdirir
  `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift` və onu tətbiq edir
  `IrohaSwift/Sources/IrohaSwift/NoritoRpcClient.swift`,
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`, və
  `IrohaSwift/Sources/IrohaSwift/ConnectClient.swift`; bu keçiddən əvvəl
  o köməkçilər bir nəqliyyat-siyasət qapısını bölüşmürdülər.
- Java/Android indi eyni siyasəti mərkəzləşdirir
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java`
  və onu `NoritoRpcClient.java`, `ToriiRequestBuilder.java`,
  `OfflineToriiClient.java`, `SubscriptionToriiClient.java`,
  `stream/ToriiEventStreamClient.java`, və
  `websocket/ToriiWebSocketClient.java`.
- Kotlin indi bu siyasəti əks etdirir
  `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt`
  və onu uyğun JVM müştəri/istək qurucusu/hadisə axınından tətbiq edir.
  websocket səthləri.
- JS `ToriiClient._request(...)` indi `canonicalAuth` plus JSON gövdələrini müalicə edir
  içərisində həssas nəqliyyat materialı kimi `private_key*` sahələrini ehtiva edir
  `javascript/iroha_js/src/toriiClient.js` və telemetriya hadisəsi şəklində
  `javascript/iroha_js/index.d.ts` indi `hasSensitiveBody` qeyd edir /
  `allowInsecure` istifadə edildikdə `hasCanonicalAuth`.

Bu niyə vacibdir:

- Mobil, brauzer və yerli inkişaf köməkçiləri tez-tez dəyişən inkişaf/səhnəyə işarə edir
  əsas URL-lər. Müştəri həssas sorğuları konfiqurasiyaya bağlamazsa
  sxemi/host, bir rahatlıq mütləq URL override və ya düz-HTTP baza bilər
  SDK-nı gizli-exfiltrasiya və ya imzalanmış sorğu-downgrade yoluna çevirin.
- Risk daşıyıcı tokenlərdən daha genişdir. Raw `private_key` JSON və təzə
  canonical-auth imzaları da teldə təhlükəsizliyə həssasdır və
  nəqliyyat siyasətindən səssizcə yan keçməməlidir.

Tövsiyə:- Hər bir SDK-da nəqliyyatın təsdiqini mərkəzləşdirin və şəbəkə I/O-dan əvvəl tətbiq edin
  bütün həssas sorğu formaları üçün: auth başlıqları, tətbiqin kanonik-auth imzalanması,
  və xam `private_key*` JSON.
- `allowInsecure`-i yalnız açıq yerli/dev escape lyuk kimi saxlayın və emissiya edin
  zəng edənlər ona qoşulduqda telemetriya.
- Təkcə yox, ortaq sorğu qurucuları üzərində fokuslanmış reqressiyalar əlavə edin
  daha yüksək səviyyəli rahatlıq üsulları, beləliklə gələcək köməkçilər eyni mühafizəçini miras alırlar.

Təmir statusu:- Cari kodda bağlanıb. Nümunələnmiş Swift, Java/Android, Kotlin və JS
  müştərilər indi həssas üçün etibarsız və ya çarpaz host daşımaları rədd edir
  Zəng edən şəxs yalnız sənədləşdirilmiş inkişaf etdiriciyə daxil olmadıqda yuxarıdakı formaları tələb edin
  etibarsız rejim.
- Swift fokuslanmış reqressiyalar indi etibarsız Norito-RPC auth başlıqlarını əhatə edir,
  etibarsız Connect websocket nəqliyyatı və xam-`private_key` Torii sorğusu
  orqanlar.
- Kotlin fokuslanmış reqressiyalar indi etibarsız Norito-RPC auth başlıqlarını əhatə edir,
  oflayn/abunə `private_key` gövdələri, SSE auth başlıqları və veb-soket
  auth başlıqları.
- Java/Android fokuslanmış reqressiyalar indi etibarsız Norito-RPC authunu əhatə edir
  başlıqlar, oflayn/abunə `private_key` gövdələri, SSE auth başlıqları və
  paylaşılan Gradle qoşqu vasitəsilə websocket auth başlıqları.
- JS fokuslanmış reqressiyalar indi etibarsız və çarpaz hostları əhatə edir
  `private_key`-bədən sorğuları və etibarsız `canonicalAuth` sorğuları
  `javascript/iroha_js/test/transportSecurity.test.js`, isə
  `javascript/iroha_js/test/toriiCanonicalAuth.test.js` indi müsbət işləyir
  təhlükəsiz əsas URL-də kanonik doğrulama yolu.

### SEC-12: SoraFS yerli QUIC proksi müştəri autentifikasiyası olmadan geri dönməyən bağlamaları qəbul etdi (25-03-2026-cı il tarixdə bağlıdır)

Təsir:- `LocalQuicProxyConfig.bind_addr` əvvəllər `0.0.0.0` olaraq təyin edilə bilər, a
  LAN IP və ya "yerli" ni ifşa edən hər hansı digər geri dönməz ünvan
  uzaqdan əldə edilə bilən QUIC dinləyicisi kimi iş stansiyası proksi.
- Həmin dinləyici müştərilərin autentifikasiya etmədi. Mümkün olan istənilən əlçatan həmyaşıd
  QUIC/TLS sessiyasını tamamlayın və versiyaya uyğun əl sıxma göndərin
  sonra hansından asılı olaraq `tcp`, `norito`, `car` və ya `kaigi` axınlarını açın.
  körpü rejimləri konfiqurasiya edilmişdir.
- `bridge` rejimində operator səhv konfiqurasiyasını uzaq TCP-yə çevirdi
  operator iş stansiyasında relay və yerli fayl axını səthi.

Sübut:

- `LocalQuicProxyConfig::parsed_bind_addr(...)` in
  `crates/sorafs_orchestrator/src/proxy.rs` əvvəllər yalnız yuvanı təhlil edirdi
  ünvana müraciət etdi və geri dönməyən interfeysləri rədd etmədi.
- Eyni faylda `spawn_local_quic_proxy(...)` a ilə QUIC serverini işə salır
  özünü imzalayan sertifikat və `.with_no_client_auth()`.
- `handle_connection(...)`, `ProxyHandshakeV1` olan istənilən müştərini qəbul edir
  versiya tək dəstəklənən protokol versiyasına uyğun gəldi və sonra daxil oldu
  proqram axını döngəsi.
- `handle_tcp_stream(...)` vasitəsilə ixtiyari `authority` dəyərlərini yığır
  `TcpStream::connect(...)`, `handle_norito_stream(...)` isə,
  `handle_car_stream(...)` və `handle_kaigi_stream(...)` yerli faylları yayımlayır
  konfiqurasiya edilmiş spool/cache qovluqlarından.

Bu niyə vacibdir:- Öz-özünə imzalanan sertifikat yalnız müştəri olduqda server şəxsiyyətini qoruyur
  yoxlamağı seçir. Müştərinin kimliyini təsdiqləmir. Bir dəfə proxy
  geri dönmədən əlçatan idi, əl sıxma yolu yalnız versiyaya bərabər idi
  qəbul.
- API və sənədlər bu köməkçini yerli iş stansiyasının proksisi kimi təsvir edir
  browser/SDK inteqrasiyaları, beləliklə uzaqdan əldə edilə bilən bağlama ünvanlarına icazə verildi
  nəzərdə tutulan uzaqdan xidmət rejimi deyil, inam-sərhəd uyğunsuzluğu.

Tövsiyə:

- Hər hansı bir geri dönmə olmayan `bind_addr`-də uğursuzluq bağlandı, buna görə də cari köməkçi ola bilməz
  yerli iş stansiyasından kənarda ifşa olunur.
- Uzaqdan proksi məruz qalma məhsul tələbinə çevrilərsə, təqdim edin
  əvəzinə ilk növbədə açıq müştəri autentifikasiyası/bacarığı qəbulu
  bağlayıcı qoruyucuyu rahatlaşdırır.

Təmir statusu:

- Cari kodda bağlanıb. `crates/sorafs_orchestrator/src/proxy.rs` indi
  `ProxyError::BindAddressNotLoopback` ilə geri dönməyən əlaqə ünvanlarını rədd edir
  QUIC dinləyicisi başlamazdan əvvəl.
- Konfiqurasiya sahəsinin sənədləri
  `docs/source/sorafs/developer/orchestrator.md` və
  `docs/portal/docs/sorafs/orchestrator-config.md` indi sənədlər
  `bind_addr` yalnız geri dönmə kimi.
- İndi reqressiya əhatəsinə daxildir
  `spawn_local_quic_proxy_rejects_non_loopback_bind_addr` və mövcud
  müsbət yerli körpü testi
  `proxy::tests::tcp_stream_bridge_transfers_payload` in
  `crates/sorafs_orchestrator/src/proxy.rs`.

### SEC-13: TCP üzərindən çıxan P2P TLS səssizcə defolt olaraq açıq mətnə endirilmişdir (25-03-2026-cı il tarixdə bağlıdır)

Təsir:- `network.tls_enabled=true`-i aktivləşdirmək əslində yalnız TLS-ni tətbiq etmədi
  operatorlar aşkar edib təyin etmədikcə, gedən nəqliyyat
  `tls_fallback_to_plain=false`.
- Buna görə də çıxış yolunda hər hansı TLS əl sıxma uğursuzluğu və ya fasilə
  siferblatını defolt olaraq açıq mətn TCP-yə endirdi, bu da nəqliyyatı aradan qaldırdı
  yoldakı hücumçulara və ya səhv davranışlara qarşı məxfilik və bütövlük
  orta qutular.
- İmzalanmış ərizə əl sıxması hələ də həmyaşıd şəxsiyyətini təsdiqlədi, buna görə də
  bu, həmyaşıdları aldatmaqdan daha çox nəqliyyat siyasətinin aşağı salınması idi.

Sübut:

- `tls_fallback_to_plain` defolt olaraq `true`-də
  `crates/iroha_config/src/parameters/user.rs`, beləliklə, ehtiyat aktiv idi
  operatorlar onu konfiqurasiyada açıq şəkildə ləğv etmədikcə.
- `Connecting::connect_tcp(...)`-də `crates/iroha_p2p/src/peer.rs` cəhdləri
  `tls_enabled` qurulduqda TLS yığın, lakin TLS xətaları və ya vaxt aşımı zamanı
  xəbərdarlığı qeyd edir və istənilən vaxt açıq mətn TCP-yə qayıdır
  `tls_fallback_to_plain` aktivləşdirilib.
- `crates/iroha_kagami/src/wizard.rs`-də operatorla üzləşən nümunə konfiqurasiyası və
  `docs/source/p2p*.md`-də ictimai P2P nəqliyyat sənədləri də reklam olunur
  standart davranış kimi düz mətn geri qaytarılması.

Bu niyə vacibdir:- Operatorlar TLS-ni işə saldıqdan sonra, daha təhlükəsiz gözlənti uğursuzluqla bağlanır: əgər TLS
  seans qurmaq mümkün deyil, yığım səssiz deyil, uğursuz olmalıdır
  daşınma mühafizəsini itirir.
- Defolt olaraq endirmənin açıq qalması yerləşdirməni həssas edir
  şəbəkə yolu qəribəlikləri, proksi müdaxiləsi və aktiv əl sıxma pozulması
  yayılma zamanı əldən buraxmaq asandır.

Tövsiyə:

- Açıq uyğunluq düyməsi kimi açıq mətn ehtiyatını saxlayın, lakin standart olaraq bunu edin
  `false` beləliklə, `network.tls_enabled=true` operator seçim etmədiyi halda yalnız TLS deməkdir
  aşağı səviyyəli davranışa.

Təmir statusu:

- Cari kodda bağlanıb. `crates/iroha_config/src/parameters/user.rs` indi
  defolt olaraq `tls_fallback_to_plain` - `false`.
- Defolt konfiqurasiya qurğusu şəkli, Kagami nümunə konfiqurasiyası və defolt kimi
  P2P/Torii test köməkçiləri indi bu sərtləşdirilmiş iş vaxtının defoltunu əks etdirir.
- Təkrarlanan `docs/source/p2p*.md` sənədləri indi açıq mətnin geri qaytarılmasını belə təsvir edir
  göndərilmiş defolt əvəzinə açıq bir seçim.

### SEC-14: Peer telemetriya geo axtarışı səssizcə açıq mətnli üçüncü tərəf HTTP-yə qayıtdı (25-03-2026-cı il tarixdə bağlıdır)

Təsir:- Açıq son nöqtə olmadan `torii.peer_geo.enabled=true`-in aktivləşdirilməsi səbəb oldu
  Daxili açıq mətnə həmyaşıd host adlarını göndərmək üçün Torii
  `http://ip-api.com/json/...` xidməti.
- Təsdiqlənməmiş üçüncü tərəf HTTP-yə sızan həmyaşıd telemetriya hədəfləri
  asılılıq və hər hansı yolda olan təcavüzkarın və ya güzəşt edilmiş son nöqtənin saxtalaşdırılmasına icazə verin
  yer metadatasını Torii-ə qaytarın.
- Xüsusiyyət daxil idi, lakin ictimai telemetriya sənədləri və nümunə konfiqurasiyası
  təhlükəsiz yerləşdirmə modelini yaradan daxili standartı reklam etdi
  çox güman ki, operatorlar peer geo axtarışlarını aktivləşdirdikdən sonra.

Sübut:

- `crates/iroha_torii/src/telemetry/peers/monitor.rs` əvvəllər müəyyən edilmişdir
  `DEFAULT_GEO_ENDPOINT = "http://ip-api.com/json"` və
  `construct_geo_query(...)` istənilən vaxt bu defoltdan istifadə edirdi
  `GeoLookupConfig.endpoint` `None` idi.
- Həmyaşıd telemetriya monitoru istənilən vaxt `collect_geo(...)` yaradır
  `geo_config.enabled`-də doğrudur
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`, belə ki, açıq mətn
  geri qaytarma yalnız test kodundan daha çox göndərilmiş iş vaxtı kodunda əldə edilə bilərdi.
- Konfiqurasiya defoltları `crates/iroha_config/src/parameters/defaults.rs` və
  `crates/iroha_config/src/parameters/user.rs` `endpoint`-i quraşdırmadan buraxın və
  `docs/source/telemetry*.md` plus-da təkrarlanan telemetriya sənədləri
  `docs/source/references/peer.template.toml` açıq şəkildə sənədləşdirdi
  operatorlar funksiyanı aktivləşdirdikdə daxili geri qaytarma.

Bu niyə vacibdir:- Peer telemetriyası açıq mətn HTTP üzərindən həmyaşıd host adlarını səssizcə tərk etməməlidir
  operatorlar rahatlıq bayrağını çevirdikdən sonra üçüncü tərəf xidmətinə.
- Gizli etibarsız defolt dəyişikliklərin nəzərdən keçirilməsini də pozur: operatorlar edə bilər
  xarici təqdim etdiklərini fərq etmədən coğrafi axtarışı aktivləşdirin
  üçüncü tərəf metadatasının açıqlanması və təsdiqlənməmiş cavabların idarə edilməsi.

Tövsiyə:

- Daxili coğrafi son nöqtəni defolt silin.
- Peer geo axtarışları aktiv edildikdə və açıq HTTPS son nöqtəsi tələb olunsun
  əks halda axtarışları atlayın.
- Çatışmayan və ya HTTPS olmayan son nöqtələrin bağlanmadığını sübut edən fokuslanmış reqressiyaları saxlayın.

Təmir statusu:

- Cari kodda bağlanıb. `crates/iroha_torii/src/telemetry/peers/monitor.rs`
  indi `MissingEndpoint` ilə çatışmayan son nöqtələri rədd edir, HTTPS olmayanları rədd edir
  `InsecureEndpoint` ilə son nöqtələr və əvəzində həmyaşıdların geo axtarışını atlayır
  səssizcə açıq mətn daxili xidmətə qayıdır.
- `crates/iroha_config/src/parameters/user.rs` artıq gizli inyeksiya etmir
  təhlil zamanı son nöqtə, ona görə də təyin olunmamış konfiqurasiya vəziyyəti açıq olaraq qalır
  iş vaxtının doğrulanmasının yolu.
- Dublikat telemetriya sənədləri və kanonik nümunə konfiqurasiyası
  `docs/source/references/peer.template.toml` indi bunu bildirir
  `torii.peer_geo.endpoint` açıq şəkildə HTTPS ilə konfiqurasiya edilməlidir
  xüsusiyyət aktivləşdirilib.
- İndi reqressiya əhatəsinə daxildir
  `construct_geo_query_requires_explicit_endpoint`,
  `construct_geo_query_rejects_non_https_endpoint`,
  `collect_geo_requires_explicit_endpoint_when_enabled`, və
  `collect_geo_rejects_non_https_endpoint` in
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`.### SEC-15: SoraFS pin və şlüz siyasətində etibarlı-proksidən xəbərdar olan müştəri-IP həlli yoxdur (25-03-2026-cı il tarixdə bağlıdır)

Təsir:

- Ters proksiləşdirilmiş Torii yerləşdirmələri əvvəllər fərqli SoraFS-ni sıradan çıxara bilərdi.
  yaddaş pin CIDR-ni qiymətləndirərkən zəng edənlər proksi soket ünvanına daxil olur
  icazə siyahıları, hər müştəri üçün saxlama pin tənzimləyiciləri və şlüz müştərisi
  barmaq izləri.
- Bu, `/v1/sorafs/storage/pin` və SoraFS-də sui-istifadə nəzarətlərini zəiflətdi
  Gateway endirmə səthlərində ümumi tərs-proxy topologiyaları edərək
  birdən çox müştəri bir vedrə və ya bir icazə siyahısı şəxsiyyətini paylaşır.
- Defolt marşrutlaşdırıcı hələ də daxili uzaqdan metadata yeridir, ona görə də bu deyildi
  yeni, təsdiqlənməmiş giriş bypass, lakin bu, əsl etibar-sərhəd boşluğu idi
  proksi-xəbərdar yerləşdirmələr və işləyici-sərhəddən artıq etibar üçün
  daxili uzaqdan IP başlığı.

Sübut:- `crates/iroha_config/src/parameters/user.rs` və
  `crates/iroha_config/src/parameters/actual.rs` əvvəllər ümumi yox idi
  `torii.transport.trusted_proxy_cidrs` düyməsi, proksidən xəbərdar olan kanonik müştəri
  IP həlli ümumi Torii giriş sərhədində konfiqurasiya olunmadı.
- `inject_remote_addr_header(...)`, `crates/iroha_torii/src/lib.rs`
  əvvəllər daxili `x-iroha-remote-addr` başlığının üzərinə yazıb
  Təkcə `ConnectInfo`, bu, etibarlı yönləndirilmiş müştəri-IP metadatasından imtina etdi.
  real əks proksilər.
- `PinSubmissionPolicy::enforce(...)` in
  `crates/iroha_torii/src/sorafs/pin.rs` və
  `gateway_client_fingerprint(...)`, `crates/iroha_torii/src/sorafs/api.rs`
  da etibarlı-proksi-xəbərdar kanonik-IP həlli addımı paylaşmadı
  işləyici sərhədi.
- `crates/iroha_torii/src/sorafs/pin.rs`-də saxlama pin tənzimləmə də açarlıdır
  mö'cüzə mövcud olduqda, yalnız daşıyıcı mö'cüzə üzərində, bu çoxlu demək idi
  bir etibarlı pin nişanını paylaşan etibarlı müştərilər eyni tarifə məcbur edildi
  bucket hətta onların müştəri IP-ləri fərqləndikdən sonra.

Bu niyə vacibdir:

- Əks proksilər Torii üçün normal yerləşdirmə nümunəsidir. Əgər iş vaxtı
  etibarlı proksi ilə etibarsız zəng edən IP-dən ardıcıl olaraq fərqləndirə bilmir
  icazə siyahıları və müştəri başına tənzimləmələr operatorların düşündüklərini ifadə edir
  demək.
- SoraFS pin və şlüz yolları açıq şəkildə sui-istifadəyə həssas səthlərdir, buna görə də
  Zəng edənlərin proksi IP-yə yıxılması və ya həddən artıq güvənən köhnəlməsi
  metadata, hətta baza marşrutu hələ də olsa da, əməliyyat baxımından əhəmiyyətlidir
  digər qəbulu tələb edir.

Tövsiyə:- Ümumi Torii `trusted_proxy_cidrs` konfiqurasiya səthini əlavə edin və problemi həll edin.
  `ConnectInfo`-dən bir dəfə kanonik müştəri IP-si üstəgəl hər hansı əvvəlcədən göndərilmiş
  başlıq yalnız socket peer həmin icazə siyahısında olduqda.
- SoraFS idarəedici yolları əvəzinə həmin kanonik-IP rezolyusiyasını yenidən istifadə edin
  daxili başlığa kor-koranə etibar etmək.
- Token və kanonik müştəri IP-si ilə paylaşılan mö'cüzə saxlama pin tənzimləmə sahəsi
  hər ikisi mövcud olduqda.

Təmir statusu:

- Cari kodda bağlanıb. `crates/iroha_config/src/parameters/defaults.rs`,
  `crates/iroha_config/src/parameters/user.rs`, və
  `crates/iroha_config/src/parameters/actual.rs` indi ifşa
  `torii.transport.trusted_proxy_cidrs`, onu boş siyahıya defolt edir.
- `crates/iroha_torii/src/lib.rs` indi kanonik müştəri IP-ni həll edir
  `limits::ingress_remote_ip(...)` giriş ara proqramı daxilində və yenidən yazır
  daxili `x-iroha-remote-addr` başlığını yalnız etibarlı proksilərdən.
- `crates/iroha_torii/src/sorafs/pin.rs` və
  `crates/iroha_torii/src/sorafs/api.rs` indi kanonik müştəri IP-lərini həll edir
  saxlama pin üçün işləyici sərhədində `state.trusted_proxy_nets`-ə qarşı
  siyasət və gateway müştəri barmaq izi, belə ki, birbaşa işləyici yolları bilməz
  həddindən artıq etibarsız köhnə yönləndirilmiş IP metadata.
- Yaddaş sancağının azaldılması indi `token + kanonik tərəfindən paylaşılan daşıyıcı tokenləri açarlar
  müştəri IP` hər ikisi mövcud olduqda, paylaşılan üçün hər bir müştəri üçün vedrələri qoruyur
  pin nişanları.
- İndi reqressiya əhatəsinə daxildir
  `limits::tests::ingress_remote_ip_preserves_trusted_forwarded_header`,
  `limits::tests::ingress_remote_ip_ignores_forwarded_header_from_untrusted_peer`,
  `sorafs::pin::tests::rate_key_scopes_shared_tokens_by_ip`,
  `storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`,
  `car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`, və
  konfiqurasiya qurğusu
  `torii_transport_trusted_proxy_cidrs_default_to_empty`.### SEC-16: Enjekte edilmiş uzaqdan IP başlığı çatışmayanda operatorun auth blokirovkası və sürət limitinin açarı yenidən paylaşılan anonim kovaya düşdü (25-03-2026-cı il tarixdə bağlıdır)

Təsir:

- Operator auth qəbulu artıq qəbul edilmiş soket IP-ni aldı, lakin
  lockout/rate-limit açarı buna məhəl qoymadı və sorğuları paylaşılana yığışdırdı
  Daxili `x-iroha-remote-addr` başlığı olduqda `"anon"` kovası
  yox.
- Bu, standart marşrutlaşdırıcıda yeni ictimai giriş keçidi deyildi, çünki
  giriş ara proqramı bu işləyicilər işə düşməzdən əvvəl daxili başlığı yenidən yazır.
  Bu, hələ də daha dar daxili üçün real uğursuz açıq etimad-sərhəd boşluğu idi
  idarəedici yolları, birbaşa testlər və çatan hər hansı gələcək marşrut
  `OperatorAuth` inyeksiya ara proqramından əvvəl.
- Belə hallarda bir zəng edən şəxs tarif limiti büdcəsini istehlak edə və ya digərini bloklaya bilər
  mənbə IP tərəfindən təcrid edilməli olan zəng edənlər.

Sübut:- `OperatorAuth::check_common(...)` in
  `crates/iroha_torii/src/operator_auth.rs` artıq alınıb
  `remote_ip: Option<IpAddr>`, lakin əvvəllər `auth_key(headers)` adlanırdı
  və nəqliyyat IP-ni tamamilə ləğv etdi.
- əvvəllər `crates/iroha_torii/src/operator_auth.rs`-də `auth_key(...)`
  yalnız `limits::REMOTE_ADDR_HEADER` təhlil edildi və əks halda `"anon"` qaytarıldı.
- `crates/iroha_torii/src/limits.rs`-də ümumi Torii köməkçisi artıq var idi
  `effective_remote_ip(başlıqlar,
  uzaqdan)`, yeridilmiş kanonik başlığa üstünlük verir, lakin geri qayıdır
  birbaşa işləyici çağırışları ara proqramdan yan keçdikdə qəbul edilən soket IP.

Bu niyə vacibdir:

- Lokaut və tarif limiti vəziyyəti eyni effektiv zəng edənin şəxsiyyətinə əsas verməlidir
  Torii-in qalan hissəsi siyasət qərarları üçün istifadə edir. Bir paylaşılana qayıdırıq
  anonim vedrə çatışmayan daxili metadata hopunu çarpaz müştəriyə çevirir
  effekti həqiqi zəng edənə lokallaşdırmaq əvəzinə müdaxilə.
- Operator auth sui-istifadəyə həssas sərhəddir, belə ki, hətta orta ciddilikdir
  vedrə-toqquşma məsələsi açıq şəkildə bağlanmağa dəyər.

Tövsiyə:

- Operator-auth açarını `limits::effective_remote_ip(başlıqlar,
  remote_ip)` beləliklə, yeridilmiş başlıq mövcud olduqda yenə də qalib gəlir, lakin birbaşa
  işləyici çağırışları `"anon"` əvəzinə nəqliyyat ünvanına düşür.
- `"anon"`-i yalnız son ehtiyat olaraq saxlayın, həm daxili başlıq, həm də
  nəqliyyat IP mövcud deyil.

Təmir statusu:- Cari kodda bağlanıb. `crates/iroha_torii/src/operator_auth.rs` indi zəng edir
  `check_common(...)`-dən `auth_key(headers, remote_ip)` və `auth_key(...)`
  indi lockout/rate-limit açarını ondan alır
  `limits::effective_remote_ip(headers, remote_ip)`.
- İndi reqressiya əhatəsinə daxildir
  `operator_auth_key_uses_remote_ip_when_internal_header_missing` və
  `operator_auth_key_prefers_injected_header_over_transport_remote_ip` in
  `crates/iroha_torii/src/operator_auth.rs`.

## Əvvəlki Hesabatdan Qapalı Və ya Əvəz Edilmiş Tapıntılar

- Əvvəllər xam-özəl açar Soracloud tapması: bağlanıb. Cari mutasiya girişi
  inline `authority` / `private_key` sahələrini rədd edir
  `crates/iroha_torii/src/soracloud.rs:5305-5308`, HTTP imzalayanı birləşdirir
  `crates/iroha_torii/src/soracloud.rs:5310-5315`-də mutasiya mənşəyi və
  imzalanmış server-təqdim etmək əvəzinə qaralama əməliyyat təlimatlarını qaytarır
  `crates/iroha_torii/src/soracloud.rs:5556-5565`-də əməliyyat.
- Əvvəlki daxili yalnız yerli oxunan proksi icra tapıntısı: bağlanıb. İctimai
  marşrut həlli indi ictimai olmayan və yeniləmə/özəl yeniləmə işləyicilərini atlayır
  `crates/iroha_torii/src/soracloud.rs:8445-8463` və icra müddəti rədd edir
  qeyri-ictimai yerli-oxumaq marşrutları
  `crates/irohad/src/soracloud_runtime.rs:5906-5923`.
- Əvvəllər ictimai iş vaxtı ölçülməmiş ehtiyat tapıntısı: yazılı olduğu kimi bağlandı. İctimai
  iş vaxtı girişi indi tarif məhdudiyyətlərini və uçuş məhdudiyyətlərini tətbiq edir
  İctimai marşrutu həll etməzdən əvvəl `crates/iroha_torii/src/lib.rs:8837-8852`
  `crates/iroha_torii/src/lib.rs:8858-8860`.
- Əvvəllər uzaqdan IP əlavəsi-icarə tapması: bağlanıb. İndi əlavə kirayə verilir
  təsdiqlənmiş hesaba giriş tələb edir
  `crates/iroha_torii/src/lib.rs:7962-7968`.
  Əlavə icarəsi əvvəllər SEC-05 və SEC-06-dan miras qalmışdır; o miras
  yuxarıdakı cari tətbiq auth remediasiyaları ilə bağlanıb.## Asılılıq tapıntıları- `cargo deny check advisories bans sources --hide-inclusion-graph` indi işləyir
  birbaşa izlənilən `deny.toml`-ə qarşı və indi üç canlı məlumat verir
  yaradılan iş sahəsi kilid faylından asılılıq tapıntıları.
- `tar` məsləhətləri artıq aktiv asılılıq qrafikində mövcud deyil:
  `xtask/src/mochi.rs` indi sabit arqumentlə `Command::new("tar")` istifadə edir
  vektor və `iroha_crypto` artıq `libsodium-sys-stable`-i çəkmir
  Ed25519, bu çekləri OpenSSL ilə dəyişdirdikdən sonra qarşılıqlı əlaqəni test edir.
- Cari tapıntılar:
  - `RUSTSEC-2024-0388`: `derivative` baxımsızdır.
  - `RUSTSEC-2024-0436`: `paste` baxımsızdır.
- Təsir triajı:
  - Daha əvvəl bildirilmiş `tar` məsləhətləri aktiv üçün bağlıdır
    asılılıq qrafiki. `cargo tree -p xtask -e normal -i tar`,
    `cargo tree -p iroha_crypto -e all -i tar`, və
    `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` indi hamısı uğursuz olur
    "paket ID spesifikasiyası ... heç bir paketə uyğun gəlmədi" və
    `cargo deny` artıq `RUSTSEC-2026-0067` və ya hesabat vermir
    `RUSTSEC-2026-0068`.
  - Daha əvvəl bildirilmiş birbaşa PQ dəyişdirmə məsləhətləri artıq bağlanıb
    indiki ağac. `crates/soranet_pq/Cargo.toml`,
    `crates/iroha_crypto/Cargo.toml` və `crates/ivm/Cargo.toml` indi asılıdır
    `pqcrypto-mldsa` / `pqcrypto-mlkem` və toxunan ML-DSA / ML-KEM-də
    iş vaxtı testləri köçdən sonra da keçir.
  - Daha əvvəl bildirilmiş `rustls-webpki` məsləhəti artıq aktiv deyil
    cari həll. İş sahəsi indi `reqwest` / `rustls`-ni`rustls-webpki`-i `0.103.10`-də saxlayan yamaqlı yamaq buraxılışları, yəni
    məsləhət çərçivəsindən kənarda.
  - `derivative` və `paste` birbaşa iş sahəsi mənbəyində istifadə edilmir.
    Onlar keçid altındakı BLS / arkworks yığını vasitəsilə daxil olurlar
    `w3f-bls` və bir neçə digər yuxarı qutular, buna görə də onların çıxarılması tələb olunur
    yerli makro təmizləmədən daha çox yuxarı axın və ya asılılıq yığını dəyişiklikləri.
    İndiki ağac indi bu iki məsləhəti açıq şəkildə qəbul edir
    Qeydə alınmış səbəblərlə `deny.toml`.

## Əhatə Qeydləri- Server/iş vaxtı/konfiqurasiya/şəbəkə: SEC-05, SEC-06, SEC-07, SEC-08, SEC-09,
  SEC-10, SEC-12, SEC-13, SEC-14, SEC-15 və SEC-16 müddətində təsdiq edilib.
  audit və indi cari ağac bağlıdır. Əlavə bərkitmə
  cari ağac indi də Connect websocket/sessiya qəbulunu uğursuz edir
  əvəzinə daxili enjekte edilmiş uzaqdan IP başlığı əskik olduqda bağlanır
  bu şərti geriyə döndərmək.
- IVM/kripto/seriyalaşdırma: bu auditdən əlavə təsdiqlənmiş tapıntı yoxdur
  dilim. Müsbət sübuta məxfi əsas materialın sıfırlanması daxildir
  `crates/iroha_crypto/src/confidential.rs:53-60` və təkrardan xəbərdar Soranet PoW
  `crates/iroha_crypto/src/soranet/pow.rs:823-879`-də imzalanmış biletin doğrulanması.
  Sonrakı sərtləşdirmə indi də ikiyə bölünmüş sürətləndirici çıxışını rədd edir
  nümunə götürülmüş Norito yolları: `crates/norito/src/lib.rs` sürətləndirilmiş JSON-u təsdiqləyir
  `TapeWalker`-dən əvvəl Mərhələ-1 lentləri ofsetləri ləğv edir və indi də tələb edir
  ilə pariteti sübut etmək üçün dinamik olaraq yüklənmiş Metal/CUDA Mərhələ-1 köməkçiləri
  aktivləşdirmədən əvvəl skalyar struktur indeks qurucusu və
  `crates/norito/src/core/gpu_zstd.rs` GPU tərəfindən bildirilmiş çıxış uzunluqlarını təsdiqləyir
  encode/decode buferlərini kəsmədən əvvəl. `crates/norito/src/core/simd_crc64.rs`
  indi də dinamik olaraq yüklənmiş GPU CRC64 köməkçilərinə qarşı özünü test edir
  `hardware_crc64`-dən əvvəl kanonik geri dönüş onlara etibar edəcək, buna görə də səhv formalaşmışdır
  Norito yoxlama məbləğini səssizcə dəyişmək əvəzinə köməkçi kitabxanalar bağlanmadıdavranış. Etibarsız köməkçi nəticələri indi çaxnaşma buraxmaq əvəzinə geri düşür
  yoxlama cəmi paritetini qurur və ya sürüklənir. IVM tərəfində, nümunə sürətləndirici
  startap qapıları indi də CUDA Ed25519 `signature_kernel`, CUDA BN254-ü əhatə edir
  əlavə/alt/mul ləpələri, CUDA `sha256_leaves` / `sha256_pairs_reduce`, canlı
  CUDA vektor/AES toplu nüvələri (`vadd64`, `vand`, `vxor`, `vor`,
  `aesenc_batch`, `aesdec_batch`) və uyğun Metal
  Bu yollara etibar edilməmişdən əvvəl `sha256_leaves`/vector/AES toplu nüvələri. The
  Nümunələnmiş Metal Ed25519 imza yolu indi də geri döndü
  Bu hostda quraşdırılmış canlı sürətləndiricinin içərisində: əvvəlki paritet uğursuzluğu idi
  skalyar nərdivan boyunca ref10 ekstremitələrə bağlı normallaşmanın bərpası ilə müəyyən edilir,
  və fokuslanmış Metal reqressiyası indi `[s]B`, `[h](-A)`,
  iki baza pilləsinin gücü və tam `[true, false]` toplu yoxlaması
  CPU istinad yoluna qarşı Metal üzərində. Güzgülənmiş CUDA mənbəyi dəyişir
  `--features cuda --tests` altında tərtib edin və CUDA başlanğıc həqiqətini indi təyin edin
  canlı Merkle yarpağı/cüt ləpələri CPU-dan sürüşərsə bağlanmır
  istinad yolu. Runtime CUDA doğrulaması bu vəziyyətdə host-məhdud olaraq qalır
  mühit.
- SDKs/nümunələr: SEC-11 nəqliyyata yönəlmiş nümunə zamanı təsdiq edilmişdir
  Swift, Java/Android, Kotlin və JS müştəriləri arasında keçir və butapma indi cari ağacda bağlıdır. JS, Swift və Android
  kanonik sorğu köməkçiləri də yeni ilə yeniləndi
  təravətdən xəbərdar olan dörd başlıq sxemi.
  Nümunə götürülmüş QUIC axın nəqliyyat icmalı da canlı yayım yaratmadı
  cari ağacda iş vaxtı tapma: `StreamingClient::connect(...)`,
  `StreamingServer::bind(...)` və qabiliyyət-danışıq köməkçiləridir
  hazırda yalnız `crates/iroha_p2p` və test kodundan istifadə olunur
  `crates/iroha_core`, beləliklə, həmin köməkçidə icazə verən özünü imzalayan doğrulayıcı
  yol hazırda göndərilən giriş səthindən daha çox sınaq/yardımçı üçündür.
- Nümunələr və mobil nümunə proqramlar yalnız spot yoxlama səviyyəsində nəzərdən keçirilmişdir
  hərtərəfli yoxlanılmış kimi qəbul edilməməlidir.

## Qiymətləndirmə və Əhatə Boşluqları- `cargo deny check advisories bans sources --hide-inclusion-graph` indi işləyir
  birbaşa izlənilən `deny.toml` ilə. Bu cari sxem altında,
  `bans` və `sources` təmizdir, `advisories` isə beş ilə uğursuzdur
  yuxarıda sadalanan asılılıq tapıntıları.
- Qapalı `tar` tapıntıları üçün asılılıq qrafikinin təmizlənməsi təsdiqi keçdi:
  `cargo tree -p xtask -e normal -i tar`,
  `cargo tree -p iroha_crypto -e all -i tar`, və
  `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` indi bütün hesabat
  “paket ID spesifikasiyası ... heç bir paketə uyğun gəlmədi” isə
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p xtask create_archive_packages_bundle_directory -- --nocapture`,
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo check -p xtask`,
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_verify -- --nocapture`,
  və
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_sign -- --nocapture`
  hamısı keçdi.
- `bash scripts/fuzz_smoke.sh` indi vasitəsilə real libFuzzer seanslarını həyata keçirir
  `cargo +nightly fuzz`, lakin IVM skriptin yarısı ərzində bitmədi
  bu keçid ona görə ki, `tlv_validate` üçün ilk gecə quruluşu hələ də var idi
  təhvil verilməsində irəliləyiş. Bu tikinti o vaxtdan bəri icra etmək üçün kifayət qədər tamamlandı
  birbaşa libFuzzer binar yaradıldı:
  `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
  indi libFuzzer run loopuna çatır və 200 qaçışdan sonra təmiz şəkildə çıxır
  boş korpus. Norito yarısı düzəldildikdən sonra uğurla tamamlandı
  qoşqu/manifest sürüşməsi və `json_from_json_equiv` qeyri-səlis hədəf kompilyasiyası
  fasilə.
- Torii remediation validasiyası indi daxildir:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo check -p iroha_torii --lib`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib --no-run`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_accepts_valid_signature -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_rejects_replayed_nonce -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_key_ -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib trusted_forwarded_header_requires_proxy_membership -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo check -p iroha_torii --lib --features app_api_https`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo test -p iroha_torii --lib https_delivery_dns_override_ --features app_api_https -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook2 cargo test -p iroha_torii --lib websocket_pinned_connect_addr_pins_secure_delivery_when_guarded -- --nocapture`- `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_blocks_remote_addr_spoofing_from_extra_headers -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib apply_extra_headers_blocks_reserved_internal_headers -- --nocapture`
  - daha dar `--no-default-features --features app_api,app_api_https` Torii
    test matrisi hələ də DA-da əlaqəli olmayan mövcud kompilyasiya xətalarına malikdir /
    Soracloud-qapılı lib-test kodu, buna görə də bu keçid göndəriləni təsdiq etdi
    defolt xüsusiyyətli MCP yolu və `app_api_https` webhook yolu əvəzinə
    tam minimal xüsusiyyət əhatə iddiası.
- Trusted-proxy/SoraFS remediation validasiyası indi daxildir:
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib limits::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib sorafs::pin::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_requires_token_and_respects_allowlist_and_rate_limit -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limits_repeated_clients -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_config --test fixtures torii_transport_trusted_proxy_cidrs_default_to_empty -- --nocapture`
- P2P remediation validasiyası indi daxildir:
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib --no-run`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib outgoing_handshake_rejects_unexpected_peer_identity -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib handshake_v1_defaults_to_trust_gossip -- --nocapture`
- SoraFS yerli-proksi remediasiyasının təsdiqi indi daxildir:
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-proxy cargo test -p sorafs_orchestrator spawn_local_quic_proxy_rejects_non_loopback_bind_addr -- --nocapture`
  - `/tmp/iroha-codex-target-proxy/debug/deps/sorafs_orchestrator-b3be10a343598c7b --exact proxy::tests::tcp_stream_bridge_transfers_payload --nocapture`
- P2P TLS-defolt remediation validasiyası indi daxildir:
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-config-tls2 cargo test -p iroha_config tls_fallback_defaults_to_tls_only -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib start_rejects_tls_without_feature_when_tls_only_outbound -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib tls_only_dial_requires_p2p_tls_feature_when_no_fallback -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-connect-gating cargo test -p iroha_torii --test connect_gating --no-run`
- SDK tərəfində remediasiya yoxlaması indi daxildir:
  - `node --test javascript/iroha_js/test/canonicalRequest.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  - `cd IrohaSwift && swift test --filter CanonicalRequestTests`
  - `cd IrohaSwift && swift test --filter 'NoritoRpcClientTests/testCallRejectsInsecureAuthorizationHeader'`
  - `cd IrohaSwift && swift test --filter 'ConnectClientTests/testBuildsConnectWebSocketRequestRejectsInsecureTransport'`
  - `cd IrohaSwift && swift test --filter 'ToriiClientTests/testCreateSubscriptionPlanRejectsInsecureTransportForPrivateKeyBody'`
  - `cd kotlin && ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.client.TransportSecurityClientTest --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew android:compileDebugUnitTestJavaWithJavac --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.NoritoRpcClientTests,org.hyperledger.iroha.android.client.OfflineToriiClientTests,org.hyperledger.iroha.android.client.SubscriptionToriiClientTests,org.hyperledger.iroha.android.client.stream.ToriiEventStreamClientTests,org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClientTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain`
  - `node --test javascript/iroha_js/test/transportSecurity.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  - `node --test javascript/iroha_js/test/toriiSubscriptions.test.js`
- Norito təqib yoxlaması indi daxildir:
  - `python3 scripts/check_norito_bindings_sync.py`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_out_of_bounds_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_non_structural_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_encode_rejects_invalid_success_length -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_decode_rejects_invalid_success_length -- --nocapture`
  - `bash scripts/fuzz_smoke.sh` (Norito hədəfləri `json_parse_string`,
    `json_parse_string_ref`, `json_skip_value` və `json_from_json_equiv`qoşqu/hədəf düzəlişlərindən sonra keçdi)
- IVM qeyri-səlis təqib yoxlaması indi daxildir:
  - `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
- IVM sürətləndirici təqib yoxlaması indi daxildir:
  - `xcrun -sdk macosx metal -c crates/ivm/src/metal_ed25519.metal -o /tmp/metal_ed25519.air`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo check -p ivm --features cuda --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda-check cargo check -p ivm --features cuda --tests`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_bitwise_single_vector_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_aes_batch_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_ed25519_batch_matches_cpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_sha256_leaves_matches_cpu -- --nocapture`
- Fokuslanmış CUDA lib-test icrası bu hostda mühitlə məhdud olaraq qalır:
  `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo test -p ivm --features cuda --lib selftest_covers_ -- --nocapture`
  CUDA sürücü simvolları (`cu*`) mövcud olmadığı üçün hələ də əlaqə yarada bilmir.
- Focused Metal iş vaxtının yoxlanılması indi tam sürətləndiricidə işləyir
  host: nümunə götürülmüş Ed25519 imza boru kəməri başlanğıc vasitəsilə aktiv olaraq qalır
  özünü test edir və `metal_ed25519_batch_matches_cpu` `[true, false]`-i yoxlayır
  birbaşa Metal üzərində CPU istinad yoluna qarşı.
- Mən tam iş sahəsinin Pas testini, tam `npm test` və ya
  bu remediasiya keçidi zamanı tam Swift/Android paketləri.

## Prioritetləşdirilmiş Bərpa İşi

### Növbəti Tranş- Açıq şəkildə qəbul edilmiş keçid üçün yuxarı axının dəyişdirilməsinə nəzarət edin
  `derivative` / `paste` makro borc və `deny.toml` istisnalarını aradan qaldırın
  təhlükəsiz yeniləmələr BLS / Halo2 / PQ / sabitliyini pozmadan əlçatan olur.
  UI asılılıq yığınları.
- Tam gecə IVM tünd tüstü skriptini isti bir keşdə yenidən işə salın
  `tlv_validate` / `kotodama_lower` yanında sabit qeydə alınmış nəticələr var
  indi yaşıl Norito hədəfləri. Birbaşa `tlv_validate` ikili işləmə indi tamamlanır,
  lakin tam scripted gecə tüstü hələ də görkəmli.
- Fokuslanmış CUDA lib-test özünü test dilimini CUDA sürücüsü ilə bir hostda təkrar işə salın
  kitabxanalar quraşdırılıb, beləliklə, genişləndirilmiş CUDA başlanğıc həqiqət dəsti təsdiqlənir
  `cargo check` və əks olunmuş Ed25519 normallaşdırma düzəlişindən başqa
  yeni vektor/AES başlanğıc zondları iş vaxtında həyata keçirilir.
- Daha geniş JS/Swift/Android/Kotlin suitlərini bir-biri ilə əlaqəsi olmayan dəst səviyyəsində yenidən işə salın
  Bu filialda blokerlər təmizlənir, buna görə də yeni kanonik-istək və
  nəqliyyat-təhlükəsizlik mühafizəçiləri yuxarıdakı diqqət mərkəzində olan köməkçi testlərdən kənarda əhatə olunur.
- Uzunmüddətli app-auth multisig hekayəsinin qalıb-qalmayacağına qərar verin
  uğursuz bağlandı və ya birinci dərəcəli HTTP multisig şahid formatını inkişaf etdirin.

### Monitor- `ivm` hardware-sürətləndirici / təhlükəli yolların diqqət mərkəzində saxlanılmasına davam edin
  və qalan `norito` axın/kripto sərhədləri. JSON Mərhələ-1
  və GPU zstd köməkçi təhvil-təslimləri indi bağlana bilməmək üçün sərtləşdirilib
  buraxılış qurur və nümunə götürülmüş IVM sürətləndirici başlanğıc həqiqət dəstləri indi
  daha geniş, lakin daha geniş təhlükəli/determinizm baxışı hələ də açıqdır.