---
id: nexus-spec
lang: az
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Nexus technical specification
description: Full mirror of `docs/source/nexus.md`, covering the architecture and design constraints for the Iroha 3 (Sora Nexus) ledger.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
Bu səhifə `docs/source/nexus.md`-i əks etdirir. Tərcümə gecikməsi portala düşənə qədər hər iki nüsxəni uyğunlaşdırın.
:::

#! Iroha 3 – Sora Nexus Ledger: Texniki Dizayn Spesifikasiyası

Bu sənəd Iroha 3 üçün Sora Nexus Ledger arxitekturasını təklif edir, Iroha 2-ni Data Spaces (DS) ətrafında təşkil edilmiş vahid qlobal, məntiqi birləşdirilmiş kitab kitabçasına doğru təkmilləşdirir. Data Spaces güclü məxfilik domenləri (“özəl məlumat məkanları”) və açıq iştirak (“ictimai məlumat məkanları”) təmin edir. Dizayn özəl DS məlumatları üçün ciddi təcrid və məxfiliyi təmin etməklə yanaşı, qlobal kitabda kompozisiyanı qoruyur və Kür (blok saxlama) və WSV (Dünya Dövlət Baxışı) üzrə silmə kodlaşdırması vasitəsilə məlumatların əlçatanlığının miqyasını təqdim edir.

Eyni repozitoriya həm Iroha 2 (öz-özünə yerləşdirilən şəbəkələr) həm də Iroha 3 (SORA Nexus) qurur. İcra ilə təchiz edilmişdir
paylaşılan Iroha Virtual Maşın (IVM) və Kotodama alətlər silsiləsi, buna görə də müqavilələr və bayt kodu artefaktları qalır
öz-özünə yerləşdirilən yerləşdirmələrdə və Nexus qlobal kitabçasında portativdir.

Məqsədlər
- Bir çox əməkdaşlıq edən təsdiqləyicilərdən və Məlumat Məkanlarından ibarət qlobal məntiqi kitab.
- İcazəli əməliyyat üçün Şəxsi Məlumat Məkanları (məsələn, CBDC-lər), heç vaxt şəxsi DS-dən çıxmayan məlumatlar.
- Açıq iştiraklı İctimai Məlumat Məkanları, Ethereum kimi icazəsiz giriş.
- Şəxsi DS aktivlərinə daxil olmaq üçün açıq icazələrə tabe olan Data Spaces üzrə tərtib edilə bilən ağıllı müqavilələr.
- İctimai fəaliyyətin şəxsi-DS daxili əməliyyatlarını aşağı sala bilməməsi üçün performans izolyasiyası.
- Geniş miqyasda məlumatların əlçatanlığı: məxfi DS məlumatlarını məxfi saxlayarkən effektiv şəkildə qeyri-məhdud məlumatları dəstəkləmək üçün silinmə ilə kodlanmış Kür və WSV.

Qeyri-məqsədlər (İlkin Faza)
- Token iqtisadiyyatının və ya validator stimullarının müəyyən edilməsi; planlaşdırma və staking siyasətləri birləşdirilə bilər.
- Yeni ABI versiyasının təqdim edilməsi; IVM siyasətinə uyğun olaraq açıq sistem zəngi və göstərici-ABI genişləndirmələri ilə hədəf ABI v1-i dəyişir.

Terminologiya
- Nexus Ledger: Data Space (DS) bloklarının vahid, sifarişli tarixə və dövlət öhdəliyinə daxil edilməsi ilə formalaşan qlobal məntiqi kitab.
- Məlumat Məkanı (DS): Öz təsdiqləyiciləri, idarəetmə, məxfilik sinfi, DA siyasəti, kvotalar və ödəniş siyasəti ilə məhdud icra və saxlama domeni. İki sinif mövcuddur: ictimai DS və özəl DS.
- Şəxsi Məlumat Məkanı: İcazəli təsdiqləyicilər və girişə nəzarət; əməliyyat məlumatları və dövlət heç vaxt DS-dən çıxmır. Yalnız öhdəliklər/metadata qlobal miqyasda bağlanır.
- İctimai Məlumat Məkanı: İcazəsiz iştirak; tam məlumat və dövlət ictimaiyyətə açıqdır.
- Data Space Manifest (DS Manifest): DS parametrlərini (təsdiqləyicilər/QC açarları, məxfilik sinfi, ISI siyasəti, DA parametrləri, saxlama, kvotalar, ZK siyasəti, ödənişlər) elan edən Norito kodlu manifest. Manifest hash nexus zəncirinə bərkidilir. Ləğv edilmədiyi halda, DS kvorum sertifikatları defolt post-kvant imza sxemi kimi ML‑DSA‑87 (Dilithium5‑sinif) istifadə edir.
- Space Directory: həll oluna bilmə və audit üçün DS manifestlərini, versiyalarını və idarəetmə/fırlanma hadisələrini izləyən qlobal zəncirli kataloq müqaviləsi.
- DSID: Məlumat Məkanı üçün qlobal unikal identifikator. Bütün obyektlərin və istinadların ad sahəsi üçün istifadə olunur.
- Çapa: DS tarixini qlobal kitab kitabçasına bağlamaq üçün nexus zəncirinə daxil edilmiş DS blokundan/başlığından kriptoqrafik öhdəlik.
- Kür: Iroha blok saxlama. Silinmə ilə kodlanmış blob yaddaşı və öhdəlikləri ilə burada genişləndirilib.
- WSV: Iroha Dünya Dövlət Görünüşü. Burada versiyalı, snapshot qabiliyyətinə malik, silmək üçün kodlaşdırılmış vəziyyət seqmentləri ilə genişləndirilmişdir.
- IVM: Ağıllı müqavilənin icrası üçün Iroha Virtual Maşın (Kotodama bayt kodu `.to`).
 - AIR: Cəbri Aralıq Nümayəndəlik. STARK üslublu sübutlar üçün hesablamanın cəbri görünüşü, icranı keçid və sərhəd məhdudiyyətləri ilə sahə əsaslı izlər kimi təsvir edir.

Məlumat Məkanları Modeli
- İdentifikasiya: `DataSpaceId (DSID)` DS-i müəyyən edir və hər şeyin ad boşluğunu verir. DS iki dənəvərlikdə yaradıla bilər:
  - Domain‑DS: `ds::domain::<domain_name>` — icra və vəziyyət domen üçün əhatə olunub.
  - Aktiv-DS: `ds::asset::<domain_name>::<asset_name>` — icra və vəziyyət vahid aktiv tərifi ilə əhatə olunub.
  Hər iki forma birlikdə mövcuddur; əməliyyatlar atomik olaraq çoxlu DSID-lərə toxuna bilər.
- Manifest həyat dövrü: DS yaradılması, yeniləmələr (açarların fırlanması, siyasət dəyişiklikləri) və işdən çıxma Kosmik kataloqda qeyd olunur. Hər bir yuva üçün DS artefaktı ən son manifest hashına istinad edir.
- Dərslər: İctimai DS (açıq iştirak, ictimai DA) və Şəxsi DS (icazəli, məxfi DA). Hibrid siyasətlər manifest bayraqları vasitəsilə mümkündür.
- DS üzrə siyasətlər: ISI icazələri, DA parametrləri `(k,m)`, şifrələmə, saxlama, kvotalar (blok üzrə min/maks. tx payı), ZK/optimist sübut siyasəti, ödənişlər.
- İdarəetmə: manifestin idarəetmə bölməsi ilə müəyyən edilən DS üzvlüyü və validator rotasiyası (zəncirvari təkliflər, multisig və ya nexus əməliyyatları və sertifikatlarla bağlanmış xarici idarəetmə).

Qabiliyyət təzahürləri və UAID
- Universal hesablar: Hər bir iştirakçı bütün məlumat məkanlarını əhatə edən deterministik UAID (`UniversalAccountId` `crates/iroha_data_model/src/nexus/manifest.rs`) alır. Qabiliyyət manifestləri (`AssetPermissionManifest`) UAID-i xüsusi məlumat məkanına, aktivləşdirmə/söhbət dövrlərinə və `dataspace`, `dataspace`, I18NI0000070X, I18NI00000, I18NI0000, I18NI00X00, I18NI0000060, `ManifestEntry` icazə/inkar etmə qaydalarının sıralanmış siyahısına bağlayır. `asset` və isteğe bağlı AMX rolları. İnkar qaydaları həmişə qalib gəlir; qiymətləndirici audit səbəbi ilə ya `ManifestVerdict::Denied`, ya da uyğun müavinət metadatası ilə `Allowed` qrant verir.
- Müavinətlər: Hər icazə girişi deterministik `AllowanceWindow` vedrələrini (`PerSlot`, `PerMinute`, `PerDay`) və əlavə olaraq `max_amount` daşıyır. Hostlar və SDK-lar eyni Norito faydalı yükü istehlak edir, ona görə də tətbiqetmə aparat və SDK tətbiqləri arasında eyni qalır.
- Audit telemetriyası: Kosmik kataloq hər dəfə manifest vəziyyəti dəyişdikdə `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) yayımlayır. Yeni `SpaceDirectoryEventFilter` səthi Torii/data-hadisə abunəçilərinə xüsusi santexnika olmadan UAID manifest yeniləmələrini, ləğvetmələrini izləməyə və inkar-qalib qərarlarına imkan verir.

Başdan sona operator sübutu, SDK miqrasiya qeydləri və manifest nəşri yoxlama siyahıları üçün bu bölməni Universal Hesab Bələdçisi (`docs/source/universal_accounts_guide.md`) ilə əks etdirin. UAID siyasəti və ya alətlər dəyişdikdə hər iki sənədi uyğunlaşdırın.

Yüksək Səviyyəli Memarlıq
1) Qlobal Kompozisiya Layeri (Nexus Zənciri)
- Bir və ya daha çox Məlumat Məkanını (DS) əhatə edən atom əməliyyatlarını yekunlaşdıran 1 saniyəlik Nexus Blokların vahid, kanonik sıralamasını saxlayır. Hər bir həyata keçirilən əməliyyat vahid qlobal dünya vəziyyətini (per‑DS köklərinin vektoru) yeniləyir.
- Kompozisiyaya uyğunluğu, yekunluğu və fırıldaqçılığın aşkarlanmasını təmin etmək üçün minimal metadata və ümumiləşdirilmiş sübutlar/QC-lərdən ibarətdir (toxunulan DSID-lər, DS-dən əvvəl/sonra vəziyyət kökləri, DA öhdəlikləri, per-DS üçün etibarlılıq sübutları və ML‑DSA‑87 istifadə edən DS kvorum sertifikatı). Heç bir şəxsi məlumat daxil edilmir.
- Konsensus: 22 ölçüdə (f=7 ilə 3f+1) vahid qlobal, boru kəməri ilə təchiz edilmiş BFT komitəsi, epoxal VRF/stake mexanizmi ilə ~200k-a qədər potensial təsdiqləyicilərdən ibarət hovuzdan seçilmişdir. Nexus komitəsi əməliyyatları ardıcıllıqla həyata keçirir və bloku 1 saniyə ərzində yekunlaşdırır.

2) Məlumat Məkanı Layeri (İctimai/Özəl)
- Qlobal tranzaksiyaların hər DS fraqmentlərini yerinə yetirir, DS-yerli WSV-ni yeniləyir və 1 saniyəlik Nexus Blokuna yığılan hər blok üzrə etibarlılıq artefaktları (bir DS sübutları və DA öhdəlikləri) yaradır.
- Şəxsi DS səlahiyyətli validatorlar arasında istirahətdə olan məlumatları və uçuş zamanı məlumatları şifrələyir; yalnız öhdəliklər və PQ etibarlılıq sübutları DS-dən ayrılır.
- İctimai DS tam məlumat orqanlarını (DA vasitəsilə) və PQ etibarlılıq sübutlarını ixrac edir.

3) Atom Çapraz Məlumat-Kosmik Əməliyyatlar (AMX)
- Model: Hər bir istifadəçi əməliyyatı birdən çox DS-yə toxuna bilər (məsələn, domen DS və bir və ya daha çox aktiv DS). O, tək Nexus Blokunda atomik şəkildə törədir və ya ləğv edir; qismən təsiri yoxdur.
- 1 saniyə ərzində hazırlayın: Hər bir namizəd əməliyyatı üçün toxunulmuş DS eyni snapshot (yuvanın başlanğıcı DS kökləri) ilə paralel olaraq yerinə yetirilir və hər DS PQ etibarlılıq sübutları (FASTPQ‑ISI) və DA öhdəlikləri hazırlanır. Nexus komitəsi əməliyyatı yalnız bütün tələb olunan DS sübutları təsdiq edildikdə və DA sertifikatları gəldikdə (≤300 ms hədəfi) həyata keçirir; əks halda tranzaksiya növbəti slot üçün yenidən planlaşdırılır.
- Ardıcıllıq: Oxu-yazma dəstləri elan edilir; münaqişənin aşkarlanması yuvanın başlanğıc köklərinə qarşı törədilmiş zaman baş verir. DS üçün kilidsiz optimist icra qlobal stendlərdən qaçır; atomiklik nexus commit qaydası ilə həyata keçirilir (DS üzrə hər şey və ya heç nə).
- Məxfilik: Şəxsi DS yalnız DS-dən əvvəlki/sonrası köklərə bağlı sübutlar/öhdəliklər ixrac edir. Heç bir xam şəxsi məlumat DS-dən çıxmır.4) Silinmə Kodlaşdırması ilə Məlumatların Əlçatanlığı (DA).
- Kür blok gövdələrini və WSV snapshotlarını silmə kodlu bloblar kimi saxlayır. İctimai bloblar geniş şəkildə parçalanır; şəxsi bloblar yalnız özəl DS validatorlarında şifrələnmiş parçalarla saxlanılır.
- DA Öhdəlikləri həm DS artefaktlarında, həm də Nexus Bloklarında qeydə alınır, şəxsi məzmunu aşkar etmədən nümunə götürmə və bərpa zəmanətlərini təmin edir.

Blok və Təhlükə Strukturu
- Data Space Proof Artefact (1s slot, hər DS)
  - Sahələr: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML‑DSA‑87), ds_validity_proof (FASTIQ).
  - Məlumat orqanı olmayan özəl-DS ixrac artefaktları; ictimai DS DA vasitəsilə cəsədin axtarışına imkan verir.

- Nexus Blok (1s kadans)
  - Sahələr: blok_nömrə, valideyn_hash, slot_time, tx_list (toxunulan DSID-lərlə atom arası DS əməliyyatları), ds_artifacts[], nexus_qc.
  - Funksiya: tələb olunan DS artefaktları yoxlanılan bütün atom əməliyyatlarını yekunlaşdırır; DS köklərinin qlobal dünya dövlət vektorunu bir addımda yeniləyir.

Konsensus və Planlaşdırma
- Nexus Zəncir Konsensusu: 1s blokları və 1-lərin yekunluğunu hədəfləyən 22 qovşaqlı komitə (f=7 ilə 3f+1) ilə tək qlobal, boru kəməri ilə təchiz edilmiş BFT (Sumeragi sinif). Komitə üzvləri epoxal olaraq VRF/stake vasitəsilə ~200k namizəd arasından seçilir; fırlanma mərkəzsizləşdirmə və senzura müqavimətini saxlayır.
- Məlumat Məkanı Konsensusu: Hər bir DS hər bir yuva üçün artefakt (sübutlar, DA öhdəlikləri, DS QC) yaratmaq üçün təsdiqləyiciləri arasında öz BFT-ni işlədir. Zolaqlı relay komitələri `3f+1` məlumat məkanı `fault_tolerance` parametrindən istifadə edərək ölçülür və `(dataspace_id, lane_id)` ilə bağlanmış VRF epox toxumundan istifadə edərək məlumat məkanı təsdiqləyici hovuzundan müəyyən dövrə görə seçilir. Şəxsi DS-yə icazə verilir; ictimai DS anti-Sybil siyasətlərinə tabe olaraq açıq canlılığa icazə verir. Qlobal Nexus Komitəsi dəyişməz olaraq qalır.
- Tranzaksiya Planlaması: İstifadəçilər toxunulmuş DSID və oxu-yazma dəstlərini elan edərək atom əməliyyatları təqdim edirlər. DS slot daxilində paralel olaraq icra edir; bütün DS artefaktları yoxlanarsa və DA sertifikatları vaxtında olarsa (≤300 ms) nexus komitəsi əməliyyatı 1s blokuna daxil edir.
- Performans İzolyasiyası: Hər DS-nin müstəqil mempoolları və icrası var. DS-ə görə kvotalar, DS-ə toxunan hər blokda nə qədər əməliyyatın həyata keçirilə biləcəyini bağlayır ki, əsas bloklanmanın qarşısını almaq və şəxsi DS gecikməsini qorumaq.

Məlumat modeli və ad boşluğu
- DS‑Qualified ID-lər: Bütün qurumlar (domenlər, hesablar, aktivlər, rollar) `dsid` tərəfindən uyğunlaşdırılıb. Misal: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Qlobal İstinadlar: Qlobal arayış `(dsid, object_id, version_hint)` dəstidir və çarpaz DS istifadəsi üçün nexus qatında və ya AMX deskriptorlarında zəncirdə yerləşdirilə bilər.
- Norito Seriyalaşdırma: Bütün çarpaz DS mesajları (AMX deskriptorları, sübutlar) Norito kodeklərindən istifadə edir. İstehsal yollarında serde istifadə edilmir.

Ağıllı Müqavilələr və IVM Genişləndirilməsi
- İcra konteksti: `dsid`-i IVM icra kontekstinə əlavə edin. Kotodama müqavilələri həmişə xüsusi Məlumat Məkanı daxilində icra olunur.
- Atomic Cross-DS Primitives:
  - `amx_begin()` / `amx_commit()` IVM hostunda atomik multi-DS əməliyyatını ayırın.
  - `amx_touch(dsid, key)` slot snapshot köklərinə qarşı ziddiyyətin aşkarlanması üçün oxumaq/yazmaq niyyətini bəyan edir.
  - `verify_space_proof(dsid, proof, statement)` → bool
  - `use_asset_handle(handle, op, amount)` → nəticə (yalnız siyasət icazə verdikdə və idarə etibarlı olduqda əməliyyata icazə verilir)
- Aktiv Dəstəkləri və Rüsumlar:
  - Aktiv əməliyyatları DS-nin ISI/rol siyasətləri ilə icazə verilir; ödənişlər DS-nin qaz nişanında ödənilir. Könüllü qabiliyyət tokenləri və daha zəngin siyasət (çoxlu təsdiqləyici, tarif limitləri, geofencing) atom modelini dəyişdirmədən sonra əlavə edilə bilər.
- Determinizm: Bütün yeni sistemlər təmiz və deterministik verilmiş girişlər və elan edilmiş AMX oxu/yazma dəstləridir. Heç bir gizli vaxt və ətraf mühit təsiri yoxdur.

Post-kvant etibarlılıq sübutları (ümumiləşdirilmiş ISI)
- FASTPQ‑ISI (PQ, etibarlı quraşdırma yoxdur): GPU-sinif aparatında 20k-miqyaslı partiyalar üçün saniyəaltı sübutu hədəfləyərkən bütün ISI ailələri üçün köçürmə dizaynını ümumiləşdirən nüvələşdirilmiş, hash-əsaslı arqument.
  - Əməliyyat profili:
    - İstehsal qovşaqları proveri `fastpq_prover::Prover::canonical` vasitəsilə qurur ki, bu da indi həmişə istehsal backendini işə salır; deterministik istehza silindi.【crates/fastpq_prover/src/proof.rs:126】
    - `zk.fastpq.execution_mode` (konfiqurasiya) və `irohad --fastpq-execution-mode` operatorlara CPU/GPU-nun icrasını deterministik şəkildə bağlamaq imkanı verir, eyni zamanda müşahidəçi çəngəl donanma üçün tələb olunan/həll edilən/backend üçqatını qeyd edir. auditlər.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:2192】【crates/iroha_config/src/parameters/8】crates/iroha_telemeics.8.
- Arifmetizasiya:
  - KV‑Update AIR: WSV-ni Poseidon2‑SMT vasitəsilə yazılmış açar-dəyər xəritəsi kimi qəbul edin. Hər bir ISI açarlar (hesablar, aktivlər, rollar, domenlər, metadata, təchizat) üzərində oxu-yoxlama-yazma sıralarının kiçik dəstinə genişlənir.
  - Opcode-qapılı məhdudiyyətlər: Selektor sütunları olan tək AIR cədvəli hər ISI qaydalarını (mühafizə, monoton sayğaclar, icazələr, diapazon yoxlamaları, məhdud metadata yeniləmələri) tətbiq edir.
  - Axtarış arqumentləri: İcazələr/rollar, aktiv dəqiqlikləri və siyasət parametrləri üçün şəffaf, heş-təsdiqlənmiş cədvəllər bitwise ilə bağlı ağır məhdudiyyətlərdən qaçır.
- Dövlət öhdəlikləri və yeniliklər:
  - Toplanmış SMT Proof: Bütün toxunulan düymələr (əvvəlcədən/sonradan) deupasiya edilmiş qardaşlarla sıxılmış sərhəddən istifadə etməklə `old_root`/`new_root`-ə qarşı sübut edilmişdir.
  - İnvariantlar: Qlobal invariantlar (məsələn, hər aktiv üzrə ümumi tədarük) effekt cərgələri və izlənilən sayğaclar arasında multiset bərabərliyi vasitəsilə tətbiq edilir.
- Sübut sistemi:
  - Yüksək aritmə (8/16) və 8-16 partlayışa malik FRI tipli polinom öhdəlikləri (DEEP-FRI); Poseidon2 hashları; SHA‑2/3 ilə Fiat-Şamir transkripti.
  - İsteğe bağlı rekursiya: Lazım gələrsə, mikro topluları hər slot üçün bir sübuta qədər sıxışdırmaq üçün DS-yerli rekursiv aqreqasiya.
- əhatə dairəsi və nümunələr:
  - Aktivlər: köçürmə, köçürmə, yandırma, aktiv təriflərini qeydiyyatdan keçirmə/qeydiyyatdan çıxarma, dəqiqliyi təyin etmək (məhdudlaşdırılmış), metadata qurmaq.
  - Hesablar/Domenlər: yaratmaq/silmək, açar/ərəfəni təyin etmək, imza edənləri əlavə etmək/çıxarmaq (yalnız dövlət; imza yoxlamaları DS validatorları tərəfindən təsdiqlənir, AIR daxilində sübut olunmur).
  - Rollar/İcazələr (ISI): rolları və icazələri vermək/ləğv etmək; axtarış cədvəlləri və monoton siyasət yoxlamaları ilə həyata keçirilir.
  - Müqavilələr/AMX: AMX start/commit markerləri, aktivləşdirildiyi təqdirdə imkanları ləğv etmək/ləğv etmək; dövlət keçidləri və siyasət sayğacları kimi sübut edilmişdir.
- Gecikməni qorumaq üçün havadan kənar yoxlamalar:
  - İmzalar və ağır kriptoqrafiya (məsələn, ML‑DSA istifadəçi imzaları) DS validatorları tərəfindən yoxlanılır və DS QC-də təsdiqlənir; etibarlılıq sübutu yalnız dövlət ardıcıllığını və siyasətə uyğunluğu əhatə edir. Bu, sübutları PQ və sürətli saxlayır.
- Performans hədəfləri (illüstrativ, 32 nüvəli CPU + tək müasir GPU):
  - Kiçik düymə toxunuşu ilə 20k qarışıq ISI (≤8 düymələr/ISI): ~0,4–0,9 s sübut, ~150–450 KB sübut, ~5–15 ms doğrulama.
  - Daha ağır ISI-lər (daha çox açar/zəngin məhdudiyyətlər): mikro-toplu (məs., 10×2k) + slot başına <1 s saxlamaq üçün rekursiya.
- DS Manifest konfiqurasiyası:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (DS QC tərəfindən təsdiqlənmiş imzalar)
  - `attestation.qc_signature = "ml_dsa_87"` (defolt; alternativlər açıq şəkildə elan edilməlidir)
- Geri dönmələr:
  - Mürəkkəb/xüsusi İSİ-lər QC attestasiyası + etibarsız sübutlar üzərində kəsişmə yolu ilə təxirə salınmış sübut və 1 s yekun ilə ümumi STARK (`zk.policy = "stark_fri_general"`) istifadə edə bilər.
  - Qeyri-PQ seçimləri (məsələn, KZG ilə Plonk) etibarlı quraşdırma tələb edir və artıq defolt quruluşda dəstəklənmir.

AIR Primer (Nexus üçün)
- İcra izi: Eni (registr sütunları) və uzunluğu (addımları) olan matris. Hər bir sıra ISI emalının məntiqi addımıdır; sütunlar əvvəlki/post dəyərlərini, seçiciləri və bayraqları saxlayır.
- Məhdudiyyətlər:
  - Keçid məhdudiyyətləri: cərgə-sətir münasibətlərini tətbiq edin (məsələn, post_balance = pre_balance − `sel_transfer = 1` olduqda debet sırası üçün məbləğ).
  - Sərhəd məhdudiyyətləri: ictimai I/O-nu (old_root/new_root, counters) birinci/son cərgələrə bağlayın.
  - Axtarışlar/permütasyonlar: bit-ağır sxemlər olmadan qəbul edilmiş cədvəllərə (icazələr, aktiv parametrləri) qarşı üzvlük və çoxset bərabərliklərini təmin edin.
- Öhdəlik və yoxlama:
  - Prover hash-əsaslı kodlaşdırmalar vasitəsilə izləri öhdəsinə götürür və məhdudiyyətlər olduqda etibarlı olan aşağı dərəcəli polinomlar qurur.
  - Doğrulayıcı bir neçə Merkle açılışı ilə FRI (hesh-əsaslı, post-kvant) vasitəsilə aşağı dərəcəni yoxlayır; dəyəri addımlarla loqarifmikdir.
- Nümunə (Transfer): registrlərə balans əvvəli, məbləğ, balansdan sonrakı, qeyri-müəyyənlik və seçicilər daxildir. Məhdudiyyətlər mənfi olmayan/aralıq, qorunma və qeyri monotonluğu tətbiq edir, birləşdirilmiş SMT isə yarpaqlardan əvvəl/sonradan köhnə/yeni köklərə bağlantılar yaradır.ABI və Syscall Evolution (ABI v1)
- Əlavə ediləcək sistemlər (illüstrativ adlar):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Əlavə ediləcək göstərici-ABI növləri:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Tələb olunan yeniləmələr:
  - `ivm::syscalls::abi_syscall_list()`-ə əlavə edin (sifariş verməyə davam edin), siyasətə uyğun olaraq keçin.
  - Hostlarda naməlum nömrələri `VMError::UnknownSyscall` ilə əlaqələndirin.
  - Yeniləmə testləri: syscall siyahısı qızıl, ABI hash, pointer type ID goldens və siyasət testləri.
  - Sənədlər: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Məxfilik Modeli
- Şəxsi məlumatların saxlanması: Transaksiya orqanları, dövlət fərqləri və özəl DS üçün WSV snapşotları heç vaxt şəxsi təsdiqləyici alt dəstini tərk etmir.
- İctimai Təsir: Yalnız başlıqlar, DA öhdəlikləri və PQ etibarlılıq sübutları ixrac edilir.
- Əlavə ZK Sübutları: Şəxsi DS daxili vəziyyəti aşkar etmədən çarpaz DS əməliyyatlarına imkan verən ZK sübutları (məsələn, kifayət qədər balans, siyasət təmin) yarada bilər.
- Girişə Nəzarət: Avtorizasiya DS daxilində ISI/rol siyasətləri ilə həyata keçirilir. Bacarıq tokenləri isteğe bağlıdır və lazım olduqda sonradan təqdim edilə bilər.

Performans izolyasiyası və QoS
- DS üçün ayrı-ayrı konsensus, mempoollar və saxlama.
- Nexus DS üzrə planlaşdırma kvotaları lövbər daxiletmə vaxtını bağlamaq və xəttin bloklanmasının qarşısını almaq üçün.
- IVM host tərəfindən tətbiq edilən DS (hesablama/yaddaş/IO) üzrə müqavilə resurs büdcələri. İctimai-DS mübahisəsi şəxsi-DS büdcələrini istehlak edə bilməz.
- Asinxron çarpaz DS zəngləri şəxsi-DS icrası daxilində uzun sinxron gözləmələrin qarşısını alır.

Məlumatların Əlçatanlığı və Saxlama Dizaynı
1) Kodlaşdırmanın silinməsi
- Kür bloklarının və WSV snapşotlarının blob səviyyəli silmə kodlaması üçün sistematik Reed‑Solomon (məsələn, GF(2^16)) istifadə edin: `(k, m)` parametrləri `n = k + m` parçaları ilə.
- Defolt parametrlər (təklif edilən, ictimai DS): `k=32, m=16` (n=48), ~1,5× genişlənmə ilə 16-ya qədər parça itkisindən bərpa etməyə imkan verir. Şəxsi DS üçün: icazə verilən dəst daxilində `k=16, m=8` (n=24). Hər ikisi DS Manifestinə görə konfiqurasiya edilə bilər.
- İctimai Bloblar: Nümunələrə əsaslanan mövcudluq yoxlamaları ilə bir çox DA qovşağı/təsdiqləyicisi arasında paylanmış parçalar. Başlıqlardakı DA öhdəlikləri yüngül müştərilərə yoxlamağa imkan verir.
- Şəxsi Bloblar: Şifrələnmiş və yalnız şəxsi DS validatorları (və ya təyin edilmiş qəyyumlar) daxilində paylanmış parçalar. Qlobal zəncir yalnız DA öhdəliklərini daşıyır (parça yerləri və ya açarları yoxdur).

2) Öhdəliklər və Nümunə götürmə
- Hər blob üçün: qırıqlar üzərində Merkle kökünü hesablayın və onu `*_da_commitment`-ə daxil edin. Elliptik əyri öhdəliklərindən qaçaraq PQ-da qalın.
- DA Attesters: VRF-nümunə götürülmüş regional attestatorlar (məsələn, hər bölgə üçün 64) uğurlu parça seçməni təsdiq edən ML-DSA‑87 sertifikatı verir. Hədəf DA attestasiya gecikməsi ≤300 ms. Nexus komitəsi qırıqları çəkmək əvəzinə sertifikatları təsdiqləyir.

3) Kür İnteqrasiyası
- Bloklar Merkle öhdəlikləri ilə silinmə kodlu bloblar kimi əməliyyat orqanlarını saxlayır.
- Başlıqlar blob öhdəlikləri daşıyır; orqanlar ictimai DS üçün DA şəbəkəsi və özəl DS üçün özəl kanallar vasitəsilə əldə edilə bilər.

4) WSV inteqrasiyası
- WSV Snapshotting: Dövri olaraq yoxlama nöqtəsi DS vəziyyəti başlıqlarda qeyd edilmiş öhdəliklərlə parçalanmış, silinmə kodlu snapshotlara çevrilir. Anlık görüntülər arasında dəyişiklik qeydlərini saxlayın. İctimai snapshotlar geniş şəkildə parçalanır; şəxsi snapshotlar şəxsi təsdiqləyicilərdə qalır.
- Proof-Daşıma Girişi: Müqavilələr snapshot öhdəlikləri ilə bağlanmış dövlət sübutlarını (Merkle/Verkle) təmin edə (və ya tələb edə bilər). Şəxsi DS xam sübutlar əvəzinə sıfır bilik sertifikatları təqdim edə bilər.

5) Saxlama və Budama
- İctimai DS üçün budama yoxdur: DA (üfüqi miqyaslama) vasitəsilə bütün Kür gövdələrini və WSV snapshotlarını saxlayın. Şəxsi DS daxili saxlanmanı müəyyən edə bilər, lakin ixrac edilmiş öhdəliklər dəyişməz olaraq qalır. Nexus təbəqəsi bütün Nexus Bloklarını və DS artefakt öhdəliklərini saxlayır.

Şəbəkə və qovşaq rolları
- Qlobal Validatorlar: Nexus konsensusunda iştirak edin, Nexus Bloklarını və DS artefaktlarını təsdiqləyin, ictimai DS üçün DA yoxlamalarını həyata keçirin.
- Data Space Validators: DS konsensusunu işə salın, müqavilələri icra edin, yerli Kür/WSV-ni idarə edin, DS üçün DA-nı idarə edin.
- DA qovşaqları (istəyə görə): İctimai blobları saxlayın/yayımlayın, nümunə götürməyi asanlaşdırın. Şəxsi DS üçün DA qovşaqları validatorlar və ya etibarlı qəyyumlarla birgə yerləşdirilir.

Sistem Səviyyəsində Təkmilləşdirmələr və Mülahizələr
- Ardıcıllıq/mempulun ayrılması: Məntiqi modeli dəyişmədən gecikməni azaltmaq və ötürmə qabiliyyətini yaxşılaşdırmaq üçün nexus qatında boru kəməri ilə təchiz edilmiş BFT-ni qidalandıran DAG mempoolunu (məsələn, Narval üslubunda) qəbul edin.
- DS kvotaları və ədalətlilik: DS-ə görə blok kvotaları və çəki hədləri xəttin bloklanmasının qarşısını almaq və şəxsi DS üçün proqnozlaşdırıla bilən gecikmə müddətini təmin etmək.
- DS attestasiyası (PQ): Defolt DS kvorum sertifikatları ML‑DSA‑87 (Dilithium5-class) istifadə edir. Bu post-kvantdır və AK imzalarından böyükdür, lakin hər yuva üçün bir QC-də məqbuldur. DS, DS Manifestində bəyan edildiyi halda, açıq şəkildə ML-DSA‑65/44 (daha kiçik) və ya AK imzalarını seçə bilər; ictimai DS ML‑DSA‑87-ni saxlamağa güclü şəkildə təşviq edilir.
- DA attesters: İctimai DS üçün, DA sertifikatları verən VRF-nümunələnmiş regional attesterlərdən istifadə edin. Nexus komitəsi xam parça nümunəsi əvəzinə sertifikatları təsdiqləyir; özəl DS DA sertifikatlarını daxili saxlayır.
- Rekursiya və dövr sübutları: Sübut ölçülərini saxlamaq və yüksək yük altında sabit vaxtı yoxlamaq üçün isteğe bağlı olaraq, DS daxilində çoxlu mikro-partları hər yuva/epox üçün bir rekursiv sübuta birləşdirin.
- Zolaqların miqyası (lazım olduqda): Tək qlobal komitə darboğaza çevrilərsə, deterministik birləşmə ilə K paralel ardıcıllıq zolaqlarını təqdim edin. Bu, üfüqi olaraq miqyaslanarkən vahid qlobal nizamı qoruyur.
- Deterministik sürətləndirmə: Çarpaz hardware determinizmini qorumaq üçün bir az dəqiq CPU geri dönüşü ilə hashing/FFT üçün SIMD/CUDA funksiyalı ləpələri təmin edin.
- Zolaq aktivləşdirmə hədləri (təklif): Əgər (a) p95 sonluğu ardıcıl >3 dəqiqə ərzində 1,2 s-dən çox olarsa və ya (b) hər blokda doluluq >5 dəqiqə ərzində 85%-i keçərsə və ya (c) daxil olan tx dərəcəsi >1,2× blok tutumu tələb edərsə, 2-4 zolağı aktivləşdirin. Zolaqlar DSID hash ilə əməliyyatları deterministik şəkildə yığır və nexus blokunda birləşir.

Rüsumlar və İqtisadiyyat (İlkin Defoltlar)
- Qaz vahidi: ölçülmüş hesablama/IO ilə hər DS qaz nişanı; ödənişlər DS-nin təbii qaz aktivində ödənilir. DS üzrə konversiya tətbiqi narahat edir.
- Daxil olma prioriteti: ədalətliliyi və 1-ci SLO-ları qorumaq üçün hər DS kvota ilə DS üzrə round-robin; DS daxilində ödənişli təklif əlaqələri poza bilər.
- Gələcək: isteğe bağlı qlobal ödəniş bazarı və ya MEV-i minimuma endirən siyasətlər atomikliyi və ya PQ sübut dizaynını dəyişdirmədən araşdırıla bilər.

Cross-Data-Space Workflow (Nümunə)
1) İstifadəçi ictimai DS P və özəl DS S-ə toxunan AMX əməliyyatı təqdim edir: X aktivini S-dən hesabı P-də olan benefisiar B-yə köçürün.
2) Yuva daxilində P və S hər biri öz fraqmentini slot snapshotına qarşı icra edir. S icazəni və mövcudluğu yoxlayır, daxili vəziyyətini yeniləyir və PQ etibarlılıq sübutu və DA öhdəliyi yaradır (heç bir şəxsi məlumat sızdırılmayıb). P müvafiq vəziyyət yeniləməsini hazırlayır (məsələn, siyasətə uyğun olaraq P-də nanə/yandırma/kilidləmə) və onun sübutu.
3) Nexus komitəsi həm DS sübutlarını, həm də DA sertifikatlarını yoxlayır; hər ikisi slot daxilində təsdiq edərsə, əməliyyat qlobal dünya dövlət vektorunda hər iki DS kökünü yeniləyərək 1s Nexus Blokunda atomik şəkildə həyata keçirilir.
4) Əgər hər hansı sübut və ya DA sertifikatı yoxdursa/etibarsızdırsa, əməliyyat dayandırılır (heç bir effekt yoxdur) və müştəri növbəti slot üçün yenidən təqdim edə bilər. Heç bir şəxsi məlumat heç bir addımda S-ni tərk etmir.

- Təhlükəsizlik Mülahizələri
- Deterministik İcra: IVM sistem zəngləri deterministik olaraq qalır; Çapraz DS nəticələri divar saatı və ya şəbəkə vaxtı ilə deyil, AMX öhdəliyi və yekunluğu ilə idarə olunur.
- Girişə Nəzarət: Şəxsi DS-də ISI icazələri əməliyyatları kimin təqdim edə biləcəyini və hansı əməliyyatlara icazə verildiyini məhdudlaşdırır. Bacarıq tokenləri çarpaz DS istifadəsi üçün incə dənəli hüquqları kodlayır.
- Məxfilik: Şəxsi DS məlumatları üçün uç-to-end şifrələmə, yalnız səlahiyyətli üzvlər arasında saxlanılan silmə kodlu qırıntılar, xarici attestasiyalar üçün isteğe bağlı ZK sübutları.
- DoS Müqaviməti: Mempool/consensus/saxlama təbəqələrində izolyasiya ictimai sıxlığın özəl-DS tərəqqisinə təsir etməsinin qarşısını alır.Iroha Komponentlərinə Dəyişikliklər
- iroha_data_model: `DataSpaceId`, DS uyğun identifikatorları, AMX deskriptorları (oxu/yazma dəstləri), sübut/DA öhdəlik növlərini təqdim edin. Norito‑yalnız seriallaşdırma.
- ivm: AMX (`amx_begin`, `amx_commit`, `amx_touch`) və DA sübutları üçün sistem zəngləri və göstərici-ABI növləri əlavə edin; v1 siyasətinə uyğun olaraq ABI testlərini/sənədlərini yeniləyin.
- iroha_core: Nexus planlayıcısını, Kosmik Kataloqu, AMX marşrutlaşdırmasını/təsdiqini, DS artefaktının yoxlanmasını və DA seçmə və kvotalar üçün siyasətin icrasını həyata keçirin.
- Kosmik Kataloq və manifest yükləyiciləri: DS manifest təhlili vasitəsilə FMS son nöqtə metadatasını (və digər ümumi xidmət deskriptorlarını) ötürün ki, qovşaqlar Data Məkanına qoşulduqda yerli xidmət son nöqtələrini avtomatik kəşf etsin.
- kura: Şəxsi/ictimai siyasətlərə uyğun olaraq silmə kodlaşdırması, öhdəliklər, axtarış API-ləri ilə Blob mağazası.
- WSV: Snapshot, parçalama, öhdəliklər; sübut API; AMX münaqişənin aşkarlanması və yoxlanılması ilə inteqrasiya.
- irohad: Node rolları, DA üçün şəbəkə, şəxsi-DS üzvlük/identifikasiyası, `iroha_config` vasitəsilə konfiqurasiya (istehsal yollarında env keçidləri yoxdur).

Konfiqurasiya və Determinizm
- `iroha_config` vasitəsilə konfiqurasiya edilmiş və konstruktorlar/hostlar vasitəsilə ötürülən bütün iş zamanı davranışı. İstehsal env keçidi yoxdur.
- Avadanlıq sürətləndirilməsi (SIMD/NEON/METAL/CUDA) isteğe bağlıdır və xüsusiyyətlərə bağlıdır; deterministik geri dönmələr aparatda eyni nəticələr verməlidir.
 - Post-Kvant defolt: Bütün DS defolt olaraq DS QC-ləri üçün PQ etibarlılıq sübutlarından (STARK/FRI) və ML‑DSA‑87 istifadə etməlidir. Alternativlər açıq DS Manifest bəyannaməsi və siyasətin təsdiqini tələb edir.

Miqrasiya Yolu (Iroha 2 → Iroha 3)
1) Məlumat modelində məlumat məkanına uyğun identifikatorlar və nexus bloku/qlobal vəziyyət tərkibini təqdim etmək; keçid zamanı Iroha 2 köhnə rejimi saxlamaq üçün xüsusiyyət bayraqları əlavə edin.
2) Erkən mərhələlərdə cari backendləri defolt olaraq saxlayaraq, xüsusiyyət bayraqlarının arxasında Kura/WSV silmə-kodlaşdırma arxa uclarını tətbiq edin.
3) AMX (atomic multi-DS) əməliyyatları üçün IVM sistem zənglərini və göstərici növlərini əlavə edin; testləri və sənədləri genişləndirmək; ABI v1-i saxlayın.
4) Tək ictimai DS və 1s blokları ilə minimal nexus zəncirini çatdırın; sonra yalnız ilk özəl DS pilot ixrac sübutlarını/öhdəliklərini əlavə edin.
5) DS-yerli FASTPQ-ISI sübutları və DA təsdiqləyiciləri ilə tam atomik çarpaz DS əməliyyatlarına (AMX) genişləndirin; DS-də ML‑DSA‑87 QC-ləri aktiv edin.

Test Strategiyası
- Məlumat modeli növləri, Norito gediş-gəlişi, AMX sistem zəngi davranışları və sübut kodlaşdırma/şifrləmə üçün vahid testləri.
- Yeni sistemlər və ABI qızılları üçün IVM testləri.
- Atom cross-DS əməliyyatları (müsbət/mənfi), DA attester gecikmə hədəfləri (≤300 ms) və yük altında performans izolyasiyası üçün inteqrasiya testləri.
- DS QC yoxlanışı (ML‑DSA‑87), konfliktin aşkarlanması/abort semantikası və məxfi parça sızmasının qarşısının alınması üçün təhlükəsizlik testləri.

### NX-18 Telemetriya və Runbook Aktivləri

- **Grafana lövhəsi:** `dashboards/grafana/nexus_lanes.json` indi NX‑18 tərəfindən tələb olunan “Nexus Lane Finality & Oracles” idarə panelini ixrac edir. Panellər `histogram_quantile()`-i `iroha_slot_duration_ms`, `iroha_da_quorum_ratio`, DA mövcudluğu xəbərdarlığı (`sumeragi_da_gate_block_total{reason="missing_local_data"}`), oracle qiymət/köhnəlmə/TWAP/saç kəsimi ölçüləri və canlı I010X operatoru I01012 panellərini əhatə edir. 1s yuvası, DA və xəzinədarlıq SLOları sifarişli sorğular olmadan.
- **Runbook:** `docs/source/runbooks/nexus_lane_finality.md`, NX‑18-dən “operator tablosunu/runbooks dərc et” bəndini yerinə yetirərək, tablosunu müşayiət edən çağırış üzrə iş prosesini (ərəfələr, insident addımları, sübutların toplanması, xaos təlimləri) sənədləşdirir.
- **Telemetriya köməkçiləri:** mövcud `scripts/telemetry/compare_dashboards.py`-dən ixrac edilmiş idarə panellərini fərqləndirmək üçün yenidən istifadə edin (səhnələşdirmə/məhsul sürüşməsinin qarşısını almaq), DA/quorum/oracle/buffer/zəngini açmaq üçün `ci/check_nexus_lane_smoke.sh` daxilində `ci/check_nexus_lane_smoke.sh`-i işə salın, `scripts/telemetry/check_nexus_audit_outcome.py` marşrutlu izləmə və ya xaos məşqləri zamanı hər NX‑18 matkap uyğun `nexus.audit.outcome` faydalı yükünü arxivləşdirir.

Açıq suallar (aydınlaşdırma tələb olunur)
1) Əməliyyat imzaları: Qərar — son istifadəçilər hədəf DS-nin reklam etdiyi istənilən imzalama alqoritmini seçməkdə sərbəstdirlər (Ed25519, secp256k1, ML‑DSA və s.). Hostlar manifestlərdə multisig/əyri qabiliyyəti bayraqlarını tətbiq etməli, deterministik geri dönüşləri təmin etməli və alqoritmləri qarışdırarkən gecikmə nəticələrini sənədləşdirməlidir. Görkəmli: Torii/SDK-lar üzrə bacarıq danışıqlarını yekunlaşdırın və qəbul testlərini yeniləyin.
2) Qaz iqtisadiyyatı: Qlobal hesablaşma haqları SORA XOR-da ödənildiyi halda, hər bir DS qazı yerli işarədə göstərə bilər. Görkəmli: standart çevrilmə yolunu (ictimai DEX və digər likvidlik mənbələrinə qarşı), mühasibat uçotu qarmaqlarını və subsidiya verən və ya sıfır qiymətli əməliyyatlar üçün DS üçün qoruyucu vasitələri müəyyən edin.
3) DA təsdiqləyiciləri: Davamlılığı qoruyarkən ≤300 ms-ə cavab vermək üçün region və hədd üzrə hədəf sayı (məsələn, 64 nümunə götürülmüş, 64 ML‑DSA‑87 imzadan 43-ü). İlk gündən hər hansı bölgələri daxil etməliyik?
4) Defolt DA parametrləri: Biz ictimai DS `k=32, m=16` və özəl DS `k=16, m=8` təklif etdik. Müəyyən DS sinifləri üçün daha yüksək ehtiyat profili (məsələn, `k=30, m=20`) istəyirsiniz?
5) DS qranularlığı: Domenlər və aktivlərin hər ikisi DS ola bilər. Siyasətlərin isteğe bağlı miras qalması ilə iyerarxik DS-ni (aktiv DS-nin valideyni kimi domen DS) dəstəkləməliyik, yoxsa onları v1 üçün düz saxlamalıyıq?
6) Ağır İSİ-lər: Saniyədən aşağı sübutlar yarada bilməyən mürəkkəb ISI-lər üçün (a) onları rədd etməli, (b) bloklar arasında daha kiçik atom pillələrinə bölünməli və ya (c) açıq bayraqlarla gecikdirilmiş daxil edilməsinə icazə verməliyik?
7) Çapraz DS konfliktləri: Müştərinin elan etdiyi oxu/yazma dəsti kifayətdirmi, yoxsa host təhlükəsizlik üçün (daha çox münaqişələr hesabına) avtomatik olaraq nəticə çıxarıb genişləndirməlidir?

Əlavə: Repozitoriya Siyasətlərinə Uyğunluq
- Norito bütün tel formatları və Norito köməkçiləri vasitəsilə JSON serializasiyası üçün istifadə olunur.
- Yalnız ABI v1; ABI siyasətləri üçün işləmə vaxtı keçidi yoxdur. Syscall və göstərici tipli əlavələr qızıl testlərlə sənədləşdirilmiş təkamül prosesini izləyir.
- Aparatda qorunan determinizm; sürətləndirmə isteğe bağlıdır və qapalıdır.
- İstehsalat yollarında serde yoxdur; istehsalda ətraf mühitə əsaslanan konfiqurasiya yoxdur.