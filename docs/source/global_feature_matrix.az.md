---
lang: az
direction: ltr
source: docs/source/global_feature_matrix.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a406b7656a87bb1469444db1cc2d2d5922f16660b53cc7eaef5b838199127e8
source_last_modified: "2026-01-23T23:46:10.135119+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Qlobal Xüsusiyyət Matrisi

Əfsanə: `◉` tam icra edilib · `○` əsasən həyata keçirilib · `▲` qismən icra edilib · `△` tətbiqi yeni başlayıb · `✖︎` başlamadı

## Konsensus və Şəbəkə

| Xüsusiyyət | Status | Qeydlər | Sübut |
|---------|--------|-------|----------|
| Çox kollektor K/r dəstəyi və ilk icra sertifikatı qazanır | ◉ | Deterministik kollektor seçimi, lazımsız fan-out, zəncirdə K/r parametrləri və sınaqlarla göndərilən ilk etibarlı icra sertifikatı qəbulu. | status.md:255; status.md:314 |
| Kardiostimulyatorun geri çəkilməsi, RTT mərtəbəsi, deterministik titrəmə | ◉ | Konfiqurasiya, telemetriya və sənədlər vasitəsilə simli titrəmə zolağı ilə konfiqurasiya edilə bilən taymerlər. | status.md:251 |
| NEW_VIEW keçid və ən yüksək QC izləmə | ◉ | Nəzarət axını NEW_VIEW/Dəlil daşıyır, ən yüksək QC monoton şəkildə qəbul edilir, əl sıxma hesablanmış barmaq izini qoruyur. | status.md:210 |
| mövcudluğu sübut izleme (məsləhət) | ◉ | Mövcudluq sübutu buraxıldı və izlənildi; commit v1-də mövcudluğu bağlamır. | status.md:son |
| Etibarlı Yayım (DA faydalı yük daşımaları) | ◉ | RBC mesaj axını (Init/Chunk/Ready/Deliver) nəqliyyat/bərpa yolu kimi `da_enabled=true` olduqda aktivləşdirilir; Mövcudluq sübutu izlənilir (məsləhətdir) və öhdəliklər müstəqil şəkildə həyata keçirilir. | status.md:son |
| QC dövlət-kök bağlamasını həyata keçirin | ◉ | QC-lər `parent_state_root`/`post_state_root` daşıyacaq; ayrıca icra-QC qapısı yoxdur. | status.md:son |
| Sübutların yayılması və auditin son nöqtələri | ◉ | ControlFlow::Dəlil, Torii sübut son nöqtələri və mənfi testlər çıxdı. | status.md:176; status.md:760-761 |
| RBC telemetriyası, hazırlıq/çatdırılmış ölçülər | ◉ | `/v2/sumeragi/rbc*` son nöqtələri və telemetriya sayğacları/histoqramı operatorlar üçün əlçatandır. | status.md:283-284; status.md:772 |
| Konsensus parametr reklamı və topologiya yoxlanışı | ◉ | Qovşaqlar `(collectors_k, redundant_send_r)` yayımlayır və həmyaşıdları arasında bərabərliyi təsdiqləyir. | status.md:255 |
| İcazəli PRF əsaslı fırlanma | ◉ | İcazə verilən lider/kollektor seçimi kanonik siyahı üzərində PRF toxum + hündürlük/görünüşdən istifadə edir; əvvəlki hash fırlanması köhnə köməkçi olaraq qalır. | status.md:son |

## Boru Kəməri, Kür və Dövlət| Xüsusiyyət | Status | Qeydlər | Sübut |
|---------|--------|-------|----------|
| Karantin zolağı qapaqları və telemetriya | ◉ | Konfiqurasiya düymələri, deterministik daşqın idarəsi və telemetriya sayğacları həyata keçirilir. | status.md:263 |
| Boru kəməri işçisi hovuz düyməsi | ◉ | `[pipeline].workers` env təhlili testləri ilə dövlət başlanğıcından keçir. | status.md:264 |
| Snapshot sorğu zolağı (saxlanan/efemer kursorlar) | ◉ | Torii inteqrasiyası və işçi hovuzlarının bloklanması ilə saxlanılan kursor rejimi. | status.md:265; status.md:371; status.md:501 |
| Statik DAG barmaq izini bərpa edən yan arabalar | ◉ | Kürdə saxlanılan yan avtomobillər işə salındıqda təsdiqlənir, uyğunsuzluqlar barədə xəbərdarlıqlar verilir. | status.md:106; status.md:349 |
| Kura blok mağaza hash decoding hardening | ◉ | Hash oxunuşları Norito-dən asılı olmayan gediş-gəliş testləri ilə xam 32 bayt işləməyə keçdi. | status.md:608; status.md:668 |
| Norito kodeklər üçün adaptiv telemetriya | ◉ | AoS vs NCB seçim ölçüləri Norito-ə əlavə edildi. | status.md:156 |
| Torii | vasitəsilə Snapshot WSV sorğuları ◉ | Torii snapshot sorğu zolağı bloklayan işçi hovuzundan, deterministik semantikadan istifadə edir. | status.md:501 |
| Zənglə icra zəncirini işə salın | ◉ | Data deterministik qaydada çağırışla icra edildikdən dərhal sonra zənciri tetikler. | status.md:668 |

## Norito Seriyalaşdırma və Alətlər

| Xüsusiyyət | Status | Qeydlər | Sübut |
|---------|--------|-------|----------|
| Norito JSON miqrasiyası (iş sahəsi) | ◉ | Serde istehsaldan çıxarıldı; inventar + qoruyucu barmaqlıqlar yalnız Norito iş yerini saxlayır. | status.md:112; status.md:124 |
| Serde deny-list & CI guardrails | ◉ | Qoruyucu iş axınları/skriptləri iş məkanında yeni birbaşa Serde istifadəsinin qarşısını alır. | status.md:218 |
| Norito kodek qızılları və AoS/NCB testləri | ◉ | AoS/NCB qızılları, kəsilmə testləri və sənəd sinxronizasiyası əlavə edildi. | status.md:140-147; status.md:149-150; status.md:332; status.md:666 |
| Norito xüsusiyyət matris alətləri | ◉ | `scripts/run_norito_feature_matrix.sh` aşağı axın tüstü testlərini dəstəkləyir; CI paketlənmiş seq/struct kombinlərini əhatə edir. | status.md:146; status.md:152 |
| Norito dil bağlamaları (Python/Java) | ◉ | Python və Java Norito kodekləri sinxronizasiya skriptləri ilə təmin edilir. | status.md:74; status.md:81 |
| Norito Mərhələ-1 SIMD struktur təsnifatçıları | ◉ | Çarpaz qövs qızılları və təsadüfi korpus testləri ilə NEON/AVX2 mərhələ-1 təsnifatçıları. | status.md:241 |

## İdarəetmə və İcra Zamanı Təkmilləşdirmələri| Xüsusiyyət | Status | Qeydlər | Sübut |
|---------|--------|-------|----------|
| Runtime təkmilləşdirmə qəbulu (ABI gating) | ◉ | Aktiv ABI dəsti strukturlaşdırılmış səhvlər və testlərlə qəbul zamanı tətbiq edilir. | status.md:196 |
| Qorunan ad məkanının yerləşdirilməsi qapısı | ▲ | Metadata tələblərini yerləşdirin və simli keçid; siyasət/UX hələ də inkişaf edir. | status.md:171 |
| Torii idarəetmə son nöqtələrini oxuyun | ◉ | `/v2/gov/*` marşrutlaşdırıcı testləri ilə yönləndirilmiş API-ləri oxuyur. | status.md:212 |
| Doğrulama açarı reyestrinin həyat dövrü və hadisələri | ◉ | VK qeydiyyatı/yeniləmə/köhnəlmə, hadisələr, CLI filtrləri və saxlama semantikası həyata keçirilir. | status.md:236-239; status.md:595; status.md:603 |

## Sıfır Bilik İnfrastruktur

| Xüsusiyyət | Status | Qeydlər | Sübut |
|---------|--------|-------|----------|
| Qoşma saxlama API-ləri | ◉ | `POST/GET/LIST/DELETE` deterministik identifikatorları və testləri olan qoşma son nöqtələri. | status.md:231 |
| Fon prover işçi & hesabat TTL | ▲ | Xüsusiyyət bayrağının arxasındakı Prover stub; TTL GC və konfiqurasiya düymələri simli; tam boru kəməri gözlənilir. | status.md:212; status.md:233 |
| CoreHost |-da zərf hash bağlaması ◉ | CoreHost vasitəsilə bağlanmış və audit impulsları vasitəsilə ifşa olunmuş zərflərin heşlərini yoxlayın. | status.md:250 |
| Ekranlı kök tarixçəsi | ◉ | Kök snapşotları məhdud tarixçə və boş kök konfiqurasiyası ilə CoreHost-a ötürülür. | status.md:303 |
| ZK səsvermə icrası və idarəetmə kilidləri | ○ | Nullifier törəmə, kilid yeniləmələri, yoxlama keçidləri həyata keçirilir; tam sübut həyat dövrü hələ yetişməkdədir. | status.md:126-128; status.md:194-195 |
| Sübut əlavəsi əvvəlcədən doğrulayın və silin | ◉ | Backend-teg ağlı başında olma, təkmilləşdirmə və sübut qeydləri icradan əvvəl davam etdi. | status.md:348; status.md:602 |
| ZK Torii sübut gətirmə son nöqtəsi | ◉ | `/v2/zk/proof/{backend}/{hash}` sübut qeydlərini ifşa edir (status, boy, vk_ref/commitment). | status.md:94 |

## IVM & Kotodama İnteqrasiya| Xüsusiyyət | Status | Qeydlər | Sübut |
|---------|--------|-------|----------|
| CoreHost syscall→ISI körpüsü | ○ | Göstərici TLV-nin dekodlanması və sistem zəngi növbəsi işləyir; əhatə boşluqları/paritet testləri planlaşdırılır. | status.md:299-307; status.md:477-486 |
| Göstərici konstruktorları və domen quruluşları | ◉ | Kotodama qurğuları tipli Norito TLV və SCALL-ları IR/e2e testləri və sənədləri ilə yayır. | status.md:299-301 |
| Pointer-ABI ciddi yoxlama və sənəd sinxronizasiyası | ◉ | TLV siyasəti qızıl testlər və yaradılan sənədlərlə host/IVM üzərində tətbiq edilir. | status.md:227; status.md:317; status.md:344; status.md:366; status.md:527 |
| CoreHost | vasitəsilə ZK syscall qapısı ◉ | Hər əməliyyat növbəsi təsdiqlənmiş zərfləri bağlayır və ISI icrasından əvvəl hash uyğunluğunu təmin edir. | crates/iroha_core/src/smartcontracts/ivm/host.rs:213; crates/iroha_core/src/smartcontracts/ivm/host.rs:279 |
| Kotodama göstərici-ABI sənədləri və qrammatikası | ◉ | Qrammatika/sənədlər canlı konstruktorlar və SCALL xəritələri ilə sinxronlaşdırılır. | status.md:299-301 |
| ISO 20022 sxemlə idarə olunan mühərrik və Torii körpüsü | ◉ | Kanonik ISO 20022 sxemləri daxil edilmiş, deterministik XML təhlili və `/v2/iso20022/status/{MsgId}` API ifşa edilmişdir. | status.md:65-70 |

## Avadanlıq Sürətləndirilməsi

| Xüsusiyyət | Status | Qeydlər | Sübut |
|---------|--------|-------|----------|
| SIMD quyruq/səhv paritet testləri | ◉ | Təsadüfi paritet testləri SIMD vektor əməliyyatlarının ixtiyari uyğunlaşma üçün skalyar semantikaya uyğun olmasını təmin edir. | status.md:243 |
| Metal/CUDA geri qaytarılması və özünü testlər | ◉ | GPU arxa ucları qızılı özünü test edir və uyğunsuzluqda skalyar/SIMD-ə qayıdır; paritet dəstləri SHA-256/Keccak/AES-i əhatə edir. | status.md:244-246 |

## Şəbəkə Vaxtı və Konsensus Rejimləri

| Xüsusiyyət | Status | Qeydlər | Sübut |
|---------|--------|-------|----------|
| Şəbəkə Vaxt Xidməti (NTS) | ✖︎ | Dizayn `new_pipeline.md`-də mövcuddur; tətbiqi status yeniləmələrində hələ izlənilmir. | new_pipeline.md |
| Nominasiya edilmiş PoS konsensus rejimi | ✖︎ | Nexus dizayn sənədləri qapalı dəst və NPoS rejimləri; əsas icrası gözlənilir. | new_pipeline.md; nexus.md |

## Nexus Ledger Yol Xəritəsi| Xüsusiyyət | Status | Qeydlər | Sübut |
|---------|--------|-------|----------|
| Space Directory müqavilə iskele | ✖︎ | DS manifestləri/idarəetmə üçün qlobal reyestr müqaviləsi hələ həyata keçirilməyib. | nexus.md |
| Data Space manifest formatı və həyat dövrü | ✖︎ | Norito manifest sxemi, versiyalaşdırma və idarəetmə axını yol xəritəsində qalır. | nexus.md |
| DS idarəçiliyi və validator rotasiyası | ✖︎ | DS üzvlüyü/fırlanması üçün zəncirvari prosedurlar hələ dizayn mərhələsindədir. | nexus.md |
| Cross-DS anker & Nexus blok tərkibi | ✖︎ | Kompozisiya təbəqəsi və lövbər öhdəlikləri təsvir edilmiş, lakin yerinə yetirilməmişdir. | nexus.md |
| Kür/WSV silmə kodlu saxlama | ✖︎ | İctimai/özəl DS üçün silinmə kodlu blob/snapshot yaddaşı hələ qurulmayıb. | nexus.md |
| DS üçün ZK/optimist sübut siyasəti | ✖︎ | Per-DS sübut tələbləri və icrası kodda izlənilmir. | nexus.md |
| Məlumat Məkanı üçün ödəniş/kvota izolyasiyası | ✖︎ | DS üçün xüsusi kvotalar və ödəniş siyasəti mexanizmləri gələcək iş olaraq qalır. | nexus.md |

## Xaos və Arızanın İnyeksiyası

| Xüsusiyyət | Status | Qeydlər | Sübut |
|---------|--------|-------|----------|
| Izanami xaosnet orkestri | ○ | Izanami iş yükü indi yeni yollar üçün vahid əhatə dairəsi ilə aktiv tərifi, metadata, NFT və tətik-təkrar reseptlərini idarə edir. | crates/izanami/src/instructions.rs; crates/izanami/src/instructions.rs#tests |