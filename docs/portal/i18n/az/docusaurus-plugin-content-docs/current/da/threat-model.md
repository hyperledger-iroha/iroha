---
lang: az
direction: ltr
source: docs/portal/docs/da/threat-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

başlıq: Məlumat Əlçatımlılığı Təhlükə Modeli
sidebar_label: Təhdid Modeli
təsvir: Sora Nexus məlumatların mövcudluğu üçün təhlükə təhlili, azaldılması və qalıq risklər.
---

:::Qeyd Kanonik Mənbə
:::

# Sora Nexus Data Availability Threat Model

_Son nəzərdən keçirilmə: 2026-01-19 — Növbəti planlaşdırılan baxış: 2026-04-19_

Baxım tempi: Məlumatların Əlçatanlığı üzrə İşçi Qrupu (<=90 gün). Hər revizyon olmalıdır
`status.md`-də aktiv azaltma biletləri və simulyasiya artefaktları ilə bağlantılar ilə görünür.

## Məqsəd və əhatə dairəsi

Data Availability (DA) proqramı Taikai yayımlarını, Nexus zolaqlı blobları və
Bizans, şəbəkə və operator xətaları altında əldə edilə bilən idarəetmə artefaktları.
Bu təhdid modeli DA-1 (arxitektura və təhlükə modeli) üçün mühəndislik işlərini birləşdirir.
və aşağı axın DA tapşırıqları üçün baza kimi xidmət edir (DA-2-dən DA-10).

Daxil olan komponentlər:
- Torii DA genişləndirilməsi və Norito metadata müəllifləri.
- SoraFS dəstəkli blob saxlama ağacları (isti/soyuq səviyyələr) və təkrarlama siyasətləri.
- Nexus blok öhdəlikləri (tel formatları, sübutlar, yüngül müştəri API-ləri).
- DA yüklərinə xas olan PDP/PoTR tətbiq qarmaqları.
- Operatorun iş axınları (sancaqlama, çıxarma, kəsmə) və müşahidə boru kəmərləri.
- DA operatorlarını və məzmununu qəbul edən və ya çıxaran idarəetmə təsdiqləri.

Bu sənədin əhatə dairəsi xaricində:
- Tam iqtisadi modelləşdirmə (DA-7 iş axınında çəkilmişdir).
- SoraFS təhlükə modeli ilə artıq əhatə olunmuş SoraFS baza protokolları.
- Təhdid səthi mülahizələrindən kənarda müştəri SDK erqonomikası.

## Memarlıq Baxışı

1. **Təqdimat:** Müştərilər blobları Torii DA ingest API vasitəsilə təqdim edirlər. Düyün
   blobları parçalayır, Norito manifestləri kodlayır (blob növü, zolaq, dövr, kodek bayraqları),
   və parçaları isti SoraFS səviyyəsində saxlayır.
2. **Reklam:** Pin niyyətləri və təkrarlama göstərişləri yaddaşa yayılır
   siyasət teqləri ilə reyestr (SoraFS bazarı) vasitəsilə provayderlər
   dövlət isti/soyuq saxlama hədəfləri.
3. **Öhdəlik:** Nexus sekvenserlərinə blob öhdəlikləri daxildir (CID + isteğe bağlı KZG
   kökləri) kanonik blokda. Light müştərilər öhdəlik hash və etibar
   mövcudluğu yoxlamaq üçün reklam edilmiş metadata.
4. **Replikasiya:** Saxlama qovşaqları təyin olunmuş payları/parçaları çəkir, PDP/PoTR-i təmin edir
   problemlər və siyasət üzrə isti və soyuq səviyyələr arasında məlumatları təşviq edin.
5. **Gətir:** İstehlakçılar məlumatları SoraFS və ya DA-dan xəbərdar şlüzlər vasitəsilə əldə edir, təsdiqləyir
   sübutlar və replikalar yox olduqda təmir tələblərinin artırılması.
6. **İdarəetmə:** Parlament və DA nəzarət komitəsi operatorları təsdiq edir,
   icarə cədvəlləri və icra eskalasiyaları. İdarəetmə artefaktları saxlanılır
   prosesin şəffaflığını təmin etmək üçün eyni DA yolu ilə.

## Aktivlər və Sahiblər

Təsir miqyası: **Kritik** kitabın təhlükəsizliyini/canlılığını pozur; **Yüksək** DA-nı bloklayır
doldurma və ya müştərilər; **Orta** keyfiyyəti aşağı salır, lakin bərpa edilə bilən olaraq qalır;
**Aşağı** məhdud təsir.

| Aktiv | Təsvir | Dürüstlük | Mövcudluq | Məxfilik | Sahibi |
| --- | --- | --- | --- | --- | --- |
| DA blobları (parçalar + manifestlər) | SoraFS-də saxlanılan Taikai, zolaq, idarəetmə blobları | Kritik | Kritik | Orta | DA WG / Saxlama Komandası |
| Norito DA təzahür edir | Blobları təsvir edən tipli metadata | Kritik | Yüksək | Orta | Əsas Protokol WG |
| Blok öhdəlikləri | Nexus bloklarında CID + KZG kökləri | Kritik | Yüksək | Aşağı | Əsas Protokol WG |
| PDP/PoTR cədvəlləri | DA replikaları üçün icra kadansı | Yüksək | Yüksək | Aşağı | Saxlama Komandası |
| Operator reyestri | Təsdiqlənmiş yaddaş təminatçıları və siyasətlər | Yüksək | Yüksək | Aşağı | İdarəetmə Şurası |
| Kirayə və həvəsləndirmə qeydləri | DA icarə və cərimələr üçün mühasibat kitabçası qeydləri | Yüksək | Orta | Aşağı | Xəzinədarlıq WG |
| Müşahidə panelləri | DA SLOs, təkrarlama dərinliyi, siqnallar | Orta | Yüksək | Aşağı | SRE / Müşahidə oluna bilmə |
| Təmir niyyətləri | Çatışmayan parçaları rehidratlaşdırmaq üçün sorğular | Orta | Orta | Aşağı | Saxlama Komandası |

## Rəqiblər və Bacarıqlar

| Aktyor | İmkanlar | Motivasiyalar | Qeydlər |
| --- | --- | --- | --- |
| Zərərli müştəri | Səhv formalaşmış blobları təqdim edin, köhnəlmiş manifestləri təkrarlayın, qəbul zamanı DoS cəhd edin. | Taikai yayımlarını pozun, etibarsız məlumatları daxil edin. | İmtiyazlı açarlar yoxdur. |
| Bizans saxlama qovşağı | Təyin edilmiş replikaları buraxın, PDP/PoTR sübutlarını düzəldin, başqaları ilə danışın. | DA-nın saxlanmasını kəsin, kirayədən qaçın, məlumatları girov saxlayın. | Etibarlı operator etimadnaməsinə malikdir. |
| Təhlükəli sıralayıcı | Öhdəlikləri buraxın, bloklar üzrə müdriklik edin, blob metadatasını yenidən sıralayın. | DA təqdimatlarını gizlədin, uyğunsuzluq yaradın. | Konsensus çoxluğu ilə məhdudlaşır. |
| İnsayder operator | İdarəetmə girişindən sui-istifadə edin, saxlama siyasətlərinə müdaxilə edin, etimadnamələri sızdırın. | İqtisadi qazanc, təxribat. | İsti/soyuq səviyyəli infrastruktura giriş. |
| Şəbəkə rəqibi | Bölmə qovşaqları, replikasiyanı gecikdirin, MITM trafikini daxil edin. | Əlçatanlığı azaldın, SLO-ları pisləşdirin. | TLS-ni poza bilməz, lakin bağlantıları azalda/yavaşlaşdıra bilər. |
| Müşahidə oluna bilən təcavüzkar | İdarə panellərini/xəbərdarlıqlarını dəyişdirin, insidentləri aradan qaldırın. | DA kəsintilərini gizlədin. | Telemetriya boru kəmərinə giriş tələb edir. |

## Güvən Sərhədləri

- **Giriş sərhədi:** Torii DA genişləndirilməsi üçün müştəri. Sorğu səviyyəsində doğrulama tələb edir,
  sürətin məhdudlaşdırılması və yükün doğrulanması.
- **Replikasiya sərhədi:** Parçalar və sübutlar mübadiləsi edən saxlama qovşaqları. Düyünlərdir
  qarşılıqlı təsdiqlənmiş, lakin Bizanslı davrana bilər.
- **Ledger sərhədi:** Təhlükəli blok məlumatları və zəncirdənkənar saxlama. Konsensus qoruyucuları
  bütövlük, lakin mövcudluq zəncirdən kənar tətbiqetməni tələb edir.
- **İdarəetmə sərhədi:** Operatorları təsdiq edən Şura/Parlament qərarları,
  büdcələr və azalma. Buradakı fasilələr DA yerləşdirməyə birbaşa təsir göstərir.
- **Müşahidə olunma sərhədi:** Metriklər/log kolleksiyası tablosuna/xəbərdarlığa ixrac edilib
  alətlər. Tampering kəsilmələri və ya hücumları gizlədir.

## Təhdid Ssenariləri və Nəzarətlər

### Yol Hücumlarını qəbul edin

**Ssenari:** Zərərli müştəri səhv formalaşdırılmış Norito faydalı yükləri və ya böyük ölçülü yüklər təqdim edir
resursları tükəndirmək və ya etibarsız metadatanı qaçırmaq üçün bloblar.

**Nəzarətlər**
- Ciddi versiya danışıqları ilə Norito sxeminin yoxlanılması; naməlum bayraqları rədd edin.
- Torii qəbul son nöqtəsində dərəcənin məhdudlaşdırılması və autentifikasiyası.
- Parça ölçüsü sərhədləri və SoraFS chunker tərəfindən tətbiq edilən deterministik kodlaşdırma.
- Qəbul boru kəməri yalnız bütövlük yoxlama cəmi uyğunlaşdıqdan sonra özünü göstərir.
- Deterministik təkrar keş yaddaşı (`ReplayCache`) `(lane, epoch, sequence)` pəncərələrini izləyir, diskdə yüksək su izlərini saxlayır və dublikatları/köhnə təkrarları rədd edir; əmlak və qeyri-səlis qoşqular fərqli barmaq izlərini və sıradan çıxmış təqdimləri əhatə edir.【crates/iroha_core/src/da/replay_cache.rs:1】【fuzz/da_replay_cache.rs:1】【crates/iroha_torii/src/da:1/ingest.

**Qalıq boşluqlar**
- Torii qəbulu təkrar oynatma önbelleğini qəbula daxil etməli və yenidən başlatmalar arasında ardıcıl kursorları davam etdirməlidir.
- Norito DA sxemləri indi invariantların kodlaşdırılması/şifrinin açılması üçün xüsusi qeyri-səlis qoşquya (`fuzz/da_ingest_schema.rs`) malikdir; əhatə dairəsi panelləri hədəfin geriləməsi barədə xəbərdarlıq etməlidir.

### Replikasiyanın Tutulması

**Ssenari:** Bizans saxlama operatorları pin təyinatlarını qəbul edir, lakin parçaları atırlar,
saxta cavablar və ya sövdələşmə yolu ilə PDP/PoTR çağırışlarından keçmək.

**Nəzarətlər**
- PDP/PoTR çağırış cədvəli hər dövr əhatə edən DA yüklərinə qədər uzanır.
- Kvorum hədləri ilə çoxmənbəli replikasiya; gətirmə orkestratoru aşkar edir
  itkin qırıqlar və tetikleyicilərin təmiri.
- Uğursuz sübutlar və itkin replikalarla əlaqəli idarəetmənin kəsilməsi.
- Avtomatlaşdırılmış uzlaşma işi (`cargo xtask da-commitment-reconcile`) müqayisə edir
  DA öhdəlikləri ilə qəbzləri qəbul edin (SignedBlockWire, `.norito` və ya JSON),
  idarəetmə üçün JSON sübut paketi buraxır və çatışmayan/uyğunsuzluqda uğursuz olur
  Biletlər, beləliklə, Alertmanager buraxılma/taxtalanma barədə səhifəyə baxa bilər.

**Qalıq boşluqlar**
- `integration_tests/src/da/pdp_potr.rs`-də simulyasiya qoşqu (
  `integration_tests/tests/da/pdp_potr_simulation.rs`) indi sövdələşməni həyata keçirir
  və PDP/PoTR cədvəlinin aşkar etdiyini təsdiqləyən bölmə ssenariləri
  Bizans davranışı deterministik olaraq. Onu DA-5 ilə yanaşı uzatmağa davam edin
  yeni sübut səthləri əhatə edir.
- Soyuq səviyyəli evakuasiya siyasəti gizli düşmələrin qarşısını almaq üçün imzalanmış audit izi tələb edir.

### Öhdəliyin dəyişdirilməsi

**Ssenari:** Təhlükəli sekvenser DA-nı buraxan və ya dəyişdirən blokları dərc edir
gətirmə uğursuzluqlarına və ya yüngül müştəri uyğunsuzluğuna səbəb olan öhdəliklər.

**Nəzarətlər**
- Konsensus çarpaz yoxlamalar DA təqdim növbələri ilə təklifləri bloklayır; həmyaşıdları rədd edir
  tələb olunan öhdəlikləri olmayan təkliflər.
- Yüngül müştərilər, götürmə tutacaqlarını örtməzdən əvvəl öhdəliklərin daxil edilməsi sübutlarını yoxlayır.
- Təqdimat qəbzlərini blok öhdəlikləri ilə müqayisə edən audit izi.
- Avtomatlaşdırılmış uzlaşma işi (`cargo xtask da-commitment-reconcile`) müqayisə edir
  DA öhdəlikləri ilə qəbzləri qəbul edin (SignedBlockWire, `.norito` və ya JSON),
  idarəetmə üçün JSON sübut paketi buraxır və çatışmayan və ya uğursuz olur
  Uyğun olmayan biletlər, beləliklə Alertmanager buraxılma/taxtalanma barədə səhifəyə keçə bilər.

**Qalıq boşluqlar**
- Barışıq işi + Alertmanager çəngəl ilə əhatə olunur; indi idarəetmə paketləri
  defolt olaraq JSON sübut paketini qəbul edin.

### Şəbəkə Bölməsi və Senzura**Ssenari:** Düşmən arakəsmələrinin təkrarlanması şəbəkəsi, qovşaqların qarşısını alır
təyin edilmiş parçaları əldə etmək və ya PDP/PoTR çağırışlarına cavab vermək.

**Nəzarətlər**
- Çox regionlu provayder tələbləri müxtəlif şəbəkə yollarını təmin edir.
- Çağırış pəncərələrinə titrəmə və diapazondan kənar təmir kanallarına geri dönmə daxildir.
- Müşahidə panelləri replikasiya dərinliyinə nəzarət edir, müvəffəqiyyətlə mübarizə aparır və
  xəbərdarlıq hədləri ilə gecikməni əldə edin.

**Qalıq boşluqlar**
- Taikai canlı hadisələri üçün bölmə simulyasiyaları hələ də yoxdur; islatma testləri lazımdır.
- Təmir bant genişliyi rezervasiya siyasəti hələ kodlaşdırılmayıb.

### Daxildən sui-istifadə

**Ssenari:** Reyestrə girişi olan operator saxlama siyasətlərini manipulyasiya edir,
zərərli provayderləri ağ siyahıya salır və ya xəbərdarlıqları dayandırır.

**Nəzarətlər**
- İdarəetmə tədbirləri üçün çoxtərəfli imzalar və Norito notarial qaydada təsdiq edilmiş qeydlər tələb olunur.
- Siyasət dəyişiklikləri hadisələri monitorinq və arxiv qeydlərinə göndərir.
- Müşahidə edilə bilən boru kəməri yalnız əlavə olunan Norito qeydlərini hash zəncirləmə ilə tətbiq edir.
- Rüblük girişin nəzərdən keçirilməsinin avtomatlaşdırılması (`cargo xtask da-privilege-audit`) gəzintiləri
  DA manifest/replay qovluqları (üstəlik operator tərəfindən təmin edilən yollar), bayraqlar
  əskik/kataloq olmayan/dünyada yazıla bilən girişlər və imzalanmış JSON paketini yayır
  idarəetmə panelləri üçün.

**Qalıq boşluqlar**
- İdarə panelinin dəyişdirilməsinə dair sübutlar imzalanmış snapshotlar tələb edir.

## Qalıq Risk Qeydiyyatı

| Risk | Ehtimal | Təsir | Sahibi | Təsirlərin Azaldılması Planı |
| --- | --- | --- | --- | --- |
| DA-nın təkrarı DA-2 ardıcıllığının keş yerlərindən əvvəl özünü göstərir | Mümkün | Orta | Əsas Protokol WG | DA-2-də ardıcıllıq önbelleğini həyata keçirin + birdəfəlik yoxlanış; reqressiya testləri əlavə edin. |
| >f qovşaqları güzəştə getdikdə PDP/PoTR sövdələşməsi | Çətin | Yüksək | Saxlama Komandası | Provayderlər arası seçmə ilə yeni çağırış cədvəli əldə edin; simulyasiya qoşqu vasitəsilə doğrulayın. |
| Soyuq səviyyəli boşalma audit boşluğu | Mümkün | Yüksək | SRE / Saxlama Komandası | İmzalanmış audit jurnallarını və çıxarılma üçün zəncirli qəbzləri əlavə edin; panelləri vasitəsilə monitorinq. |
| Sequencer buraxılışının aşkarlanması gecikməsi | Mümkün | Yüksək | Əsas Protokol WG | Gecəlik `cargo xtask da-commitment-reconcile` qəbzləri öhdəliklərlə (SignedBlockWire/`.norito`/JSON) və çatışmayan və ya uyğun olmayan biletlər üzrə səhifələrin idarə edilməsini müqayisə edir. |
| Taikai canlı yayımları üçün bölməyə davamlılıq | Mümkün | Kritik | Şəbəkə TL | arakəsmə matkaplarını yerinə yetirmək; ehtiyat təmir bant genişliyi; sənədin dəyişdirilməsi SOP. |
| İdarəetmə imtiyazının sürüşməsi | Çətin | Yüksək | İdarəetmə Şurası | İmzalanmış JSON + idarə paneli qapısı ilə rüblük `cargo xtask da-privilege-audit` qaçışı (manifest/təkrar dirsəkləri + əlavə yollar); zəncirvari lövbər audit artefaktları. |

## Tələb olunan təqiblər

1. DA qəbulu Norito sxemlərini və nümunə vektorlarını dərc edin (DA-2-də aparılır).
2. Təkrar keş yaddaşını Torii DA vasitəsilə keçirin və qovşağın yenidən başlamaları arasında ardıcıl kursorları davam etdirin.
3. **Tamamlandı (2026-02-05):** PDP/PoTR simulyasiya qoşqu indi QoS geriləmə modelləşdirməsi ilə sövdələşmə + bölmə ssenarilərini həyata keçirir; Aşağıda çəkilmiş icra və deterministik xülasələr üçün `integration_tests/src/da/pdp_potr.rs` (`integration_tests/tests/da/pdp_potr_simulation.rs` altında testlərlə) baxın.
4. **Tamamlandı (29-05-2026):** `cargo xtask da-commitment-reconcile` daxilolma qəbzlərini DA öhdəlikləri (SignedBlockWire/`.norito`/JSON) ilə müqayisə edir, `artifacts/da/commitment_reconciliation.json` yayır və ATV-yə bağlanır. buraxılma/dəyişmə siqnalları (`xtask/src/da.rs`).
5. **Tamamlandı (29-05-2026):** `cargo xtask da-privilege-audit` manifest/replay spool (üstəlik operator tərəfindən təmin edilmiş yollar) üzərində gəzir, çatışmayan/kataloq olmayan/dünyada yazıla bilən girişləri qeyd edir və nəzarət panelləri üçün imzalanmış JSON paketi yaradır. (`artifacts/da/privilege_audit.json`), giriş-nəzərdən avtomatlaşdırma boşluğunu bağlayır.

**Sonra hara baxmaq lazımdır:**

- DA replay keşi və kursorun davamlılığı DA-2-yə düşdü. baxın
  `crates/iroha_core/src/da/replay_cache.rs`-də həyata keçirilməsi (keş məntiqi) və
  `crates/iroha_torii/src/da/ingest.rs`-də Torii inteqrasiyası,
  barmaq izi `/v2/da/ingest` vasitəsilə yoxlanılır.
- PDP/PoTR axın simulyasiyaları proof-stream qoşqu vasitəsilə həyata keçirilir
  `crates/sorafs_car/tests/sorafs_cli.rs`, PoR/PDP/PoTR sorğu axınlarını əhatə edir
  və təhlükə modelində canlandırılan uğursuzluq ssenariləri.
- Tutum və təmir altında yaşamaq nəticələri islatmaq
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, daha geniş olsa da
  Sumeragi islatma matrisi `docs/source/sumeragi_soak_matrix.md`-də izlənilir
  (lokallaşdırılmış variantlar daxildir). Bu artefaktlar uzun müddət davam edən məşqləri ələ keçirir
  qalıq risk reyestrində istinad edilir.
- Uzlaşma + imtiyaz-audit avtomatlaşdırması yaşayır
  `docs/automation/da/README.md` və yeni `cargo xtask da-commitment-reconcile`
  / `cargo xtask da-privilege-audit` əmrləri; altındakı standart çıxışlardan istifadə edin
  İdarəetmə paketlərinə sübut əlavə edərkən `artifacts/da/`.

## Simulyasiya sübutu və QoS Modelləşdirmə (2026-02)

DA-1 təqibini №3 bağlamaq üçün biz deterministik PDP/PoTR simulyasiyasını kodlaşdırdıq
`integration_tests/src/da/pdp_potr.rs` altında qoşqu (örtülür
`integration_tests/tests/da/pdp_potr_simulation.rs`). Qoşqu
üç bölgə üzrə qovşaqlar ayırır, uyğun olaraq bölmələr/sövdələşmələr yeridir
yol xəritəsi ehtimalları, PoTR gecikmələrini izləyir və təmirin geri qalmasını təmin edir
isti səviyyəli təmir büdcəsini əks etdirən model. Defolt ssenarinin icrası
(12 dövr, 18 PDP problemi + hər dövr üçün 2 PoTR pəncərəsi)
aşağıdakı göstəricilər:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Metrik | Dəyər | Qeydlər |
| --- | --- | --- |
| PDP uğursuzluqları aşkar edildi | 48 / 49 (98,0%) | Arakəsmələr hələ də aşkarlanmağa başlayır; aşkarlanmayan tək bir uğursuzluq dürüst titrəmədən irəli gəlir. |
| PDP orta aşkarlama gecikməsi | 0,0 dövrlər | Uğursuzluqlar yaranma dövrü ərzində üzə çıxır. |
| PoTR xətaları aşkar edildi | 28 / 77 (36,4%) | Bir qovşaq ≥2 PoTR pəncərəsini qaçırdıqda aşkarlama işə salınır və hadisələrin çoxu qalıq risk reyestrində qalır. |
| PoTR orta aşkarlama gecikməsi | 2.0 dövrlər | Arxiv eskalasiyasına çevrilmiş iki dövr gecikmə həddinə uyğundur. |
| Təmir növbəsi pik | 38 manifest | Arakəsmələr hər dövr üçün mövcud olan dörd təmirdən daha sürətli yığıldıqda, geriləmə sürətlənir. |
| Cavab gecikməsi p95 | 30,068 ms | QoS nümunəsi üçün tətbiq olunan ±75 ms titrəmə ilə 30 s çağırış pəncərəsini əks etdirir. |
<!-- END_DA_SIM_TABLE -->

Bu çıxışlar indi DA tablosunun prototiplərini idarə edir və “simulyasiyanı təmin edir
yol xəritəsində istinad edilən qoşqu + QoS modelləşdirmə” qəbul meyarları.

Avtomatlaşdırma indi ortaq qoşqu adlandıran `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`-in arxasında yaşayır və
Defolt olaraq Norito JSON-u `artifacts/da/threat_model_report.json`-ə yayır. Gecə
işlər bu sənəddəki matrisləri yeniləmək və xəbərdar etmək üçün bu faylı istehlak edir
aşkarlama dərəcələrində, təmir növbələrində və ya QoS nümunələrində sürüşmə.

Sənədlər üçün yuxarıdakı cədvəli yeniləmək üçün `make docs-da-threat-model`-i işə salın.
`cargo xtask da-threat-model-report` çağırır, regenerasiya edir
`docs/source/da/_generated/threat_model_report.json` və bu bölməni yenidən yazır
`scripts/docs/render_da_threat_model_tables.py` vasitəsilə. `docs/portal` güzgü
(`docs/portal/docs/da/threat-model.md`) hər ikisi eyni keçiddə yenilənir
nüsxələr sinxron qalır.