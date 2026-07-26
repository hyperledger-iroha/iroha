---
lang: az
direction: ltr
source: docs/source/fraud_monitoring_system.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c8262bacbb15b83bd70c824990e4948832418b59f184bca353eee899e44f4d4
source_last_modified: "2025-12-29T18:16:35.960562+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Fırıldaqçılıq Monitorinq Sistemi

Bu sənəd əsas mühasibat kitabçasını müşayiət edəcək paylaşılan fırıldaqçılıq monitorinqi qabiliyyəti üçün istinad dizaynını əks etdirir. Məqsəd hesablaşma mexanizmi xaricində təyin edilmiş operatorların nəzarəti altında saxlama, məxfilik və siyasət qərarlarını saxlamaqla ödəniş xidməti təminatçılarını (PSP) hər bir əməliyyat üçün yüksək keyfiyyətli risk siqnalları ilə təmin etməkdir.

## Məqsədlər və Uğur Meyarları
- Hesablaşma mühərrikinə toxunan hər bir ödəniş üçün real vaxt rejimində fırıldaqçılıq riskinin qiymətləndirilməsini (<120 ms 95p, <40 ms median) təqdim edin.
- Mərkəzi xidmətin heç vaxt şəxsiyyəti müəyyənləşdirən məlumatları (PII) emal etməməsini və yalnız təxəllüs identifikatorlarını və davranış telemetriyasını qəbul etməsini təmin etməklə istifadəçi məxfiliyini qoruyun.
- Hər bir provayderin əməliyyat muxtariyyətini saxladığı, lakin paylaşılan kəşfiyyatı sorğulaya biləcəyi multi-PSP mühitlərini dəstəkləyin.
- Qeyri-deterministik kitab davranışını təqdim etmədən nəzarət edilən və nəzarət olunmayan modellər vasitəsilə davamlı olaraq yeni hücum modellərinə uyğunlaşın.
- Həssas pul kisələri və ya qarşı tərəfləri ifşa etmədən tənzimləyicilər və müstəqil rəyçilər üçün yoxlanıla bilən qərar izlərini təmin edin.

## Əhatə dairəsi
- **Əhatə dairəsi:** Əməliyyat riskinin qiymətləndirilməsi, davranış analitikası, çarpaz PSP korrelyasiyası, anomaliya xəbərdarlığı, idarəetmə qarmaqları və PSP inteqrasiya API-ləri.
- **Əhatə dairəsi xaricində:** Birbaşa icra (PSP məsuliyyəti qalır), sanksiyaların yoxlanılması (mövcud uyğunluq boru kəmərləri tərəfindən idarə olunur) və şəxsiyyətin yoxlanılması (ləqəb rəhbərliyi bunu əhatə edir).

## Funksional Tələblər
1. **Transaction Scoring API**: PSP-lərin ödənişi hesablaşma mühərrikinə yönləndirməzdən, risk xalını, kateqoriyalı hökmü və əsaslandırma xüsusiyyətlərini qaytarmazdan əvvəl zəng etdiyi Sinxron API.
2. **Hadisə qəbulu**: Davamlı öyrənmə üçün hesablaşma nəticələrinin axını, pul kisəsinin həyat dövrü hadisələri, cihazın barmaq izləri və PSP səviyyəli saxtakarlıq rəyi.
3. **Model Həyat Dövrünün İdarə Edilməsi**: Oflayn təlim, kölgə yerləşdirmə, mərhələli yayım və geri çəkilmə dəstəyi ilə versiyalı modellər. Deterministik geri dönüş evristikası hər bir xüsusiyyət üçün mövcud olmalıdır.
4. **Əlaqə Döngüsü**: PSP-lər təsdiqlənmiş fırıldaqçılıq hallarını, yanlış pozitivləri və düzəliş qeydlərini təqdim edə bilməlidir. Sistem rəyi risk xüsusiyyətləri ilə uyğunlaşdırır və analitikanı yeniləyir.
5. **Məxfiliyə Nəzarətlər**: Saxlanan və ötürülən bütün məlumatlar ləqəb əsasında olmalıdır. Xam şəxsiyyət metadatasını ehtiva edən istənilən sorğu rədd edilir və qeyd olunur.
6. **İdarəetmə Hesabatı**: Ümumiləşdirilmiş ölçülərin (PSP üzrə aşkarlamalar, tipologiyalar, cavab gecikmələri) və səlahiyyətli auditorlar üçün xüsusi araşdırma API-lərinin planlaşdırılmış ixracı.
7. **Dayanıqlılıq**: Avtomatik növbənin boşaldılması və təkrar oynatma ilə ən azı iki obyekt üzrə aktiv-aktiv yerləşdirmə. Xidmət pisləşərsə, PSP-lər kitabı bloklamadan yerli qaydalara qayıdırlar.## Qeyri-Funksional Tələblər
- **Determinizm və Ardıcıllıq**: Risk xalları PSP qərarlarına rəhbərlik edir, lakin mühasibat kitabının icrasını dəyişdirmir. Ledger öhdəlikləri qovşaqlar arasında deterministik olaraq qalır.
- **Ölçəklənmə**: Yalançı pul kisəsi identifikatorları ilə əsaslanan üfüqi miqyaslama və mesaj bölmələri ilə saniyədə ≥10k risk qiymətləndirməsini davam etdirin.
- **Müşahidə oluna bilənlik**: Hər bir qiymətləndirmə zəngi üçün ölçüləri (`fraud.scoring_latency_ms`, `fraud.risk_score_distribution`, `fraud.api_error_rate`, `fraud.model_version_active`) və strukturlaşdırılmış qeydləri ifşa edin.
- **Təhlükəsizlik**: PSP-lər və mərkəzi xidmət arasında qarşılıqlı TLS, cavab zərflərinin imzalanması üçün aparat təhlükəsizlik modulları, saxtakarlıq aşkar edilən audit yolları.
- **Uyğunluq**: PL/TMM tələbləri ilə uyğunlaşın, konfiqurasiya edilə bilən saxlama müddətlərini təmin edin və sübutların qorunması iş axınları ilə inteqrasiya edin.

## Memarlığa Baxış
1. **API Gateway Layer**
  - Doğrulanmış HTTP/JSON API-ləri üzərindən qiymətləndirmə və rəy sorğularını qəbul edir.
   - Norito kodeklərindən istifadə edərək sxemin yoxlanılmasını həyata keçirir və hər PSP id-si üzrə tarif limitlərini tətbiq edir.

2. **Funksiya Toplama Xidməti**
   - Zaman seriyası xüsusiyyətləri mağazasında saxlanılan tarixi aqreqatlarla (sürət, coğrafi nümunələr, cihaz istifadəsi) daxil olan sorğuları birləşdirir.
   - Deterministik toplama funksiyalarından istifadə edərək konfiqurasiya edilə bilən xüsusiyyət pəncərələrini (dəqiqələr, saatlar, günlər) dəstəkləyir.

3. **Risk Mühərriki**
   - Aktiv model boru kəmərini (qradient gücləndirilmiş ağaclar ansamblı, anomaliya detektorları, qaydalar) icra edir.
   - Model xalları mövcud olmadıqda məhdud cavabları təmin etmək üçün deterministik geri qaytarma qaydası daxildir.
   - Hesab, qrup, töhfə verən xüsusiyyətlər və model versiyası ilə `FraudAssessment` zərfləri buraxır.## Qiymətləndirmə Modelləri və Evristika
- **Xal Şkalası və Qruplar**: Risk balları 0-1000 arasında normallaşdırılır. Qruplar aşağıdakı kimi müəyyən edilir: `0–249` (aşağı), `250–549` (orta), `550–749` (yüksək), `750+` (kritik). Qruplar PSP-lər üçün tövsiyə olunan hərəkətləri (avtomatik təsdiq, addım-addım, nəzərdən keçirmək üçün növbə, avtomatik imtina) ilə əlaqələndirir, lakin icra PSP-yə xas olaraq qalır.
- **Model Ansamblı**:
  - Qradientlə gücləndirilmiş qərar ağacları məbləğ, ləqəb/cihazın sürəti, tacir kateqoriyası, autentifikasiya gücü, PSP etibar səviyyəsi və çarpaz pul kisəsi qrafiki xüsusiyyətləri kimi strukturlaşdırılmış xüsusiyyətləri qəbul edir.
  - Avtokodlayıcı əsaslı anomaliya detektoru zaman pəncərəli davranış vektorlarında işləyir (ləqəb üzrə xərcləmə kadansı, cihazın dəyişdirilməsi, müvəqqəti entropiya). Xallar sürüşməni məhdudlaşdırmaq üçün son PSP fəaliyyətinə görə kalibrlənir.
  - Deterministik siyasət qaydaları əvvəlcə icra olunur; onların çıxışları statistik modelləri ikili/davamlı xüsusiyyətlər kimi qidalandırır ki, ansambl qarşılıqlı əlaqəni öyrənə bilsin.
- **Fallback Heuristics**: Modelin icrası uğursuz olduqda, deterministik təbəqə yenə də qayda cəzalarını birləşdirərək məhdud xal yaradır. Hər bir qayda konfiqurasiya edilə bilən çəkiyə töhfə verir, cəmləndikdən sonra 0-1000 miqyasında sıxışdırılır və ən pis vəziyyətdə gecikmə və izah edilə bilənliyə zəmanət verir.
- **Gecikmə Büdcəsi**: Qiymətləndirmə boru kəməri hədəfləri API şlüzü + doğrulama üçün <20 ms, funksiyaların birləşdirilməsi üçün <30 ms (davamlı mağazalara arxada yazılan yaddaşdaxili keşlərdən xidmət göstərir) və ansamblın qiymətləndirilməsi üçün <40 ms. ML nəticəsi büdcəsini keçərsə, deterministik geri dönüş <10 ms ərzində qaytarılır və ümumi P95-in 120 ms altında qalmasını təmin edir.
 - **Gecikmə Büdcəsi**: Qiymətləndirmə boru kəməri hədəfləri API şlüzü + doğrulama üçün <20 ms, funksiyaların birləşdirilməsi üçün <30 ms (davamlı mağazalara arxada yazılan yaddaşdaxili keşlərdən xidmət göstərir) və ansamblın qiymətləndirilməsi üçün <40 ms. ML nəticəsi büdcəsini keçərsə, deterministik geri dönüş <10 ms ərzində qaytarılır və ümumi P95-in 120 ms altında qalmasını təmin edir.## Yaddaşdaxili Xüsusiyyət Keş Dizaynı
- **Shard Layout**: Feature mağazalar `N = 256` parçalarına 64-bit ləqəb hash ilə bölünür. Hər bir parça sahibidir:
  - Keş xəttinin yerini maksimum dərəcədə artırmaq üçün massivlərin strukturu kimi saxlanılan son əməliyyat deltaları (5 dəq + 1 saat pəncərələr) üçün kilidsiz ring buferi.
  - Tam təkrar hesablama olmadan 24 saat/7 günlük aqreqatları saxlamaq üçün sıxılmış Fenwick ağacı (bit-dolu 16-bit vedrələr).
  - Qarşı tərəfləri xəritələyən hop-skoç hash xəritəsi → hər ləqəb üçün 1024 girişlə məhdudlaşan statistik göstəricilər (say, cəmi, variasiya, son vaxt damgası).
- **Yaddaş Rezidentliyi**: İsti qırıntılar RAM-da qalır. Son bir saatda 1% aktiv olan 50 M ləqəbli kainat üçün keş rezidentliyi ~500k ləqəbdir. Aktiv metadata ləqəbi üçün ~320 B-də iş dəsti ~160 MB-dır - müasir serverlərdə L3 keşi üçün kifayət qədər kiçikdir.
- **Concurrency**: Oxucular dövrə əsaslanan meliorasiya vasitəsilə dəyişməz istinadlar götürürlər; yazıçılar müqayisə və dəyişdirmə üsulundan istifadə edərək deltaları əlavə edir və aqreqatları yeniləyir. Bu, mutex mübahisəsinin qarşısını alır və iki atom əməliyyatı + məhdud göstərici təqibinə isti yollar saxlayır.
- **Öncədən gətirilmə**: Sorğunun yoxlanılması başa çatdıqdan sonra qiymətləndirmə işçisi əsas yaddaş gecikməsini (~80 ns) funksiyaların birləşdirilməsinin arxasında gizlədərək növbəti ləqəb parçası üçün `prefetch_read` təlimatı verir.
- **Arxasında yazılan Log**: Hər 50 ms-dən bir (və ya 4 KB) bir WAL paketləri deltalar verir və davamlı mağazaya axır. Bərpa sərhədlərini möhkəm saxlamaq üçün yoxlama məntəqələri hər 5 dəqiqədən bir işləyir.

### Nəzəri Gecikmə Bölməsi (Intel Ice Lake səviyyəli server, 3.1 GHz)
- **Kəskin axtarış + əvvəlcədən gətirmə**: 1 keş buraxılması (~80 ns) üstəgəl hash hesablanması (<10 ns).
- **Ring bufer iterasiyası (32 giriş)**: 32 × 2 yük = 64 yük; 32 B keş xətti və ardıcıl giriş ilə bu L1 → ~20 ns-də qalır.
- **Fenwick yeniləmələri (log₂ 2048 ≈ 11 addım)**: 11 göstərici hops; L1-in yarısını, L2-nin yarısının vurduğunu fərz etsək → ~30 ns.
- **Hop-skoç xəritə zondu (yük əmsalı 0,75, 2 zond)**: 2 keş xətti, ~2 × 15 ns.
- **Model xüsusiyyətlərinin yığılması**: 150 skalyar əməliyyat (hər biri <0,1 ns) → ~15 ns.Bunların cəmlənməsi hər sorğu üçün ~160 ns hesablama və ~120 ns yaddaş bloku verir (~0,28 µs). Hər nüvədə dörd eyni vaxtda birləşdirici işçi ilə səhnə hətta partlayış yükü altında 30 ms büdcəni asanlıqla qarşılayır; faktiki yerləşdirmə təsdiq etmək üçün histoqramları qeyd etməlidir (`fraud.feature_cache_lookup_ms` vasitəsilə).
- **Funksiya Windows və Toplama**:
  - Qısamüddətli (5 dəqiqə, 1 saat) və uzunmüddətli (24 saat, 7 gün) pəncərələr xərcləmə sürətini, cihazın təkrar istifadəsini və ləqəb qrafik dərəcələrini izləyir.
  - Qrafik xüsusiyyətləri (məsələn, ləqəblər arasında paylaşılan cihazlar, qəfil fan-out, yüksək riskli klasterlərdə yeni qarşı tərəflər) sorğuların millisaniyədən aşağı qalması üçün müntəzəm sıxılmış xülasələrə əsaslanır.
  - Məkan evristikası hədsiz Haversine əsaslı risk artımından istifadə edərək, qeyri-mümkün sıçrayışları (məsələn, bir neçə dəqiqə ərzində çoxlu uzaq yerləri) qeyd edərək, qaba geobuketləri tarixi davranışla müqayisə edir.
  - Axın formalı detektorlar daxil olan/çıxan məbləğlərin və qarşı tərəflərin yuvarlanan histoqramlarını qarışdırma/tumbling imzalarını aşkar etmək üçün saxlayır (sürətli giriş, ardınca oxşar fan-çıxış, tsiklik hop ardıcıllıqları, qısa müddətli vasitəçilər).
- **Qayda Kataloqu (ətraflı deyil)**:
  - **Sürətin pozulması**: Hər bir ləqəb və ya cihaz üçün hədləri aşan yüksək dəyərli ötürmələrin sürətli seriyası.
  - **Ləqəb qrafik anomaliyası**: Alias ​​təsdiqlənmiş fırıldaqçılıq halları və ya məlum qatır nümunələri ilə əlaqəli klasterlə qarşılıqlı əlaqədə olur.
  - **Cihazın təkrar istifadəsi**: Əvvəlcədən əlaqə olmadan müxtəlif PSP istifadəçi kohortlarına aid ləqəblər üzrə ortaq cihaz barmaq izi.
  - **İlk dəfə yüksək dəyər**: Yeni ləqəb PSP-nin tipik giriş dəhlizindən yuxarı məbləğləri sınamağa çalışır.
  - **Autentifikasiya səviyyəsinin aşağı salınması**: Tranzaksiya, PSP tərəfindən elan edilmiş əsaslandırma olmadan hesabın ilkin səviyyəsindən (məsələn, biometrikdən PİN-ə qayıtma) daha zəif amillərdən istifadə edir.
  - **Qarışdırma/tumbling nümunəsi**: Ləqəb qısa pəncərələrdə çoxlu ləqəblər arasında sıx əlaqəli vaxt, təkrar gediş-gəliş məbləğləri və ya dairəvi axınlarla yüksək fan-in/fan-out zəncirlərində iştirak edir. Qayda qrafik mərkəzlik sıçrayışları və axın forması detektorlarından istifadə edərək balı artırır; ağır hallarda hətta ML çıxışından əvvəl də `high` bandına sıxışdırılır.
  - **Əməliyyat qara siyahısına vuruldu**: Ləqəb və ya qarşı tərəf zəncirvari idarəetmə səsverməsi və ya `sudo` nəzarətləri (məs., tənzimləyici sifarişlər, təsdiqlənmiş fırıldaqçılıq) ilə həvalə edilmiş səlahiyyət vasitəsilə seçilən paylaşılan qara siyahı lentində görünür. `critical` bandına sıxaclar vurur və `BLACKLIST_MATCH` səbəb kodunu yayır; PSP-lər audit üçün ləğvetmələri daxil etməlidir.
  - **Sandbox imza uyğunsuzluğu**: PSP köhnəlmiş model imzası ilə yaradılan qiymətləndirməni təqdim edir; xal `critical` səviyyəsinə yüksəlir və audit çəngəl tetikler.
- **Səbəb Kodları**: Hər bir qiymətləndirməyə töhfə çəkisinə görə sıralanan maşın tərəfindən oxuna bilən səbəb kodları daxildir (məsələn, `VELOCITY_BREACH`, `NEW_DEVICE`, `GRAPH_HIGH_RISK`, `AUTH_DOWNGRADE`). PSP-lər istifadəçi mesajlaşması üçün bunları operatorlara və ya pul kisələrinə çatdıra bilər.- **Model İdarəetmə**: Kalibrləmə və hədd parametrləri sənədləşdirilmiş oyun kitablarına əməl edir - ROC/PR əyriləri rüblük nəzərdən keçirilir, etiketlənmiş fırıldaqçılığa qarşı sınaqdan keçirilir və rəqib modellər stabil olana qədər kölgədə işləyir. İstənilən hədd yeniləməsi ikili təsdiq tələb edir (fırıldaqçılıq əməliyyatları + müstəqil risk).

## İdarəetmə Mənbəli Qara Siyahı axını
- **Zəncirvari müəlliflik**: Qara siyahı qeydləri idarəetmə alt sistemi (`iroha_core::smartcontracts::isi::governance`) vasitəsilə bloklanacaq ləqəbləri, PSP identifikatorlarını və ya cihazın barmaq izlərini sadalayan `BlacklistProposal` ISI kimi təqdim edilir. Maraqlı tərəflər standart seçki bülletenindən istifadə edərək səs verirlər; kvorum yerinə yetirildikdən sonra, zəncir təsdiqlənmiş əlavələr/çıxarılmalar və monoton şəkildə artan `blacklist_epoch` daşıyan `GovernanceEvent::BlacklistUpdated` rekordu yayır.
- **Təqdim edilmiş sudo yolu**: Fövqəladə hallar eyni `BlacklistUpdated` hadisəsini yayan, lakin dəyişikliyi `origin = Sudo` kimi qeyd edən `sudo::Execute` təlimatı vasitəsilə icra edilə bilər. Bu, açıq mənbə ilə zəncirvari tarixi əks etdirir ki, auditorlar konsensus səslərini həvalə edilmiş müdaxilələrdən ayıra bilsinlər.
- **Paylanma kanalı**: FMS körpü xidməti `LedgerEvent` axınına (Norito kodlu) abunə olur və `BlacklistUpdated` hadisələrini izləyir. Hər bir hadisə idarəçilik Merkle sübutu ilə təsdiqlənir və tətbiq edilməzdən əvvəl blok imzası ilə təsdiqlənir. Hadisələr idempotentdir; FMS təkrarların qarşısını almaq üçün ən son `blacklist_epoch`-i saxlayır.
- **FMS daxilində proqram**: Yeniləmə qəbul edildikdən sonra qeydlər deterministik qaydalar anbarına yazılır (yalnız audit qeydləri ilə əlavə yaddaşla dəstəklənir). Hesablama mühərriki 30 saniyə ərzində qara siyahını yenidən yükləyir, sonrakı qiymətləndirmələrin `BLACKLIST_MATCH` qaydasını işə salmasını və `critical`-ə bərkidilməsini təmin edir.
- **Audit və geri qaytarma**: İdarəetmə eyni boru kəməri vasitəsilə qeydləri silmək üçün səs verə bilər. FMS, `blacklist_epoch` tərəfindən işarələnmiş tarixi anlıq görüntüləri saxlayır ki, operatorlar məhkəmə-tibbi suallara cavab verə və ya araşdırmalar zamanı keçmiş qərarları təkrarlaya bilsinlər.

4. **Öyrənmə və Analitika Platforması**
   - Yalnız əlavə dəftər (məsələn, Kafka + obyekt saxlama) vasitəsilə təsdiqlənmiş fırıldaqçılıq hadisələrini, hesablaşma nəticələrini və PSP rəyini alır.
   - Modelləri yenidən hazırlamaq üçün məlumat alimləri üçün oflayn noutbuklar/iş yerləri təqdim edir. Model artefaktlar təqdimatdan əvvəl versiyalanır və imzalanır.

5. **İdarəetmə Portalı**
   - Auditorlar üçün tendensiyaları nəzərdən keçirmək, tarixi qiymətləndirmələri axtarmaq və insident hesabatlarını ixrac etmək üçün məhdud interfeys.
   - Siyasət yoxlamalarını həyata keçirir ki, müstəntiqlər PSP əməkdaşlığı olmadan PII-yə keçə bilməsinlər.

6. **İnteqrasiya Adapterləri**
   - Norito sorğuları/cavablarını və yerli keşləməni həyata keçirən PSP-lər (Rust, Kotlin, Swift, TypeScript) üçün yüngül SDK-lar.
   - Hesablaşma mühərriki qarmaq (`iroha_core` daxilində) PSP-lər yoxlamadan sonra əməliyyatları yönləndirən zaman risk qiymətləndirmə istinadlarını qeyd edir.## Məlumat axını
1. PSP API şluzuna autentifikasiya edir və aşağıdakıları ehtiva edən `RiskQuery` təqdim edir:
   - Ödəyici/ödəyici üçün ləqəb identifikatorları, heşlənmiş cihaz identifikatoru, əməliyyat məbləği, kateqoriya, coğrafi yerləşdirmə qaba kovası, PSP güvən bayraqları və son sessiya metadatası.
2. Şlüz faydalı yükü təsdiq edir, PSP metadatası (lisenziya səviyyəsi, SLA) və funksiyaların birləşdirilməsi üçün növbələrlə zənginləşdirir.
3. Xüsusiyyət xidməti ən son aqreqatları çəkir, model vektorunu qurur və onu risk mühərrikinə göndərir.
4. Risk mühərriki sorğunu qiymətləndirir, deterministik səbəb kodları əlavə edir, `FraudAssessment` imzalayır və onu PSP-yə qaytarır.
5. PSP əməliyyatı təsdiqləmək, rədd etmək və ya təsdiqləmək üçün qiymətləndirməni yerli siyasətləri ilə birləşdirir.
6. Nəticə (təsdiqlənmiş/rədd edilmiş, saxtakarlıq təsdiqlənmiş/yalan müsbət) davamlı təkmilləşdirmə üçün asinxron şəkildə öyrənmə platformasına ötürülür.
7. Gündəlik toplu proseslər idarəetmə hesabatları üçün ölçüləri toplayır və siyasət xəbərdarlıqlarını (məsələn, artan sosial mühəndislik işləri) PSP tablosuna gətirir.

## Iroha Komponentləri ilə İnteqrasiya
- ** Əsas Host Qarmaqları**: Tranzaksiya qəbulu indi `fraud_assessment_band` metadatasını `fraud_monitoring.enabled` və `required_minimum_band` təyin edildikdə tətbiq edir. Ev sahibi sahəni itirən və ya konfiqurasiya edilmiş minimumdan aşağı diapazon daşıyan əməliyyatları rədd edir və `missing_assessment_grace_secs` sıfırdan fərqli olduqda deterministik xəbərdarlıq verir (uzaqdan doğrulayıcı sim bağlandıqdan sonra FM-204 mərhələsində silinməsi planlaşdırılır). Qiymətləndirmələrə həmçinin `fraud_assessment_score_bps` daxil edilməlidir; ev sahibi elan edilmiş diapazonla xalı çarpaz yoxlayır (0–249 ➜ aşağı, 250–549 ➜ orta, 550–749 ➜ yüksək, 750+ ➜ kritik, 10000-ə qədər dəstəklənən əsas bal dəyərləri ilə). `fraud_monitoring.attesters` konfiqurasiya edildikdə, əməliyyatlar Norito kodlu `fraud_assessment_envelope` (base64) və uyğun `fraud_assessment_digest` (hex) əlavə etməlidir. Demon deterministik şəkildə zərfin şifrəsini açır, Ed25519 imzasını attestator reyestrinə qarşı yoxlayır, imzalanmamış faydalı yük üzrə həzmi yenidən hesablayır və uyğunsuzluqları rədd edir ki, yalnız təsdiq edilmiş qiymətləndirmələr konsensusa nail olsun.
- **Konfiqurasiya**: Risk xidmətinin son nöqtələri, fasilələr və tələb olunan qiymətləndirmə diapazonları üçün `iroha_config::fraud_monitoring` altında konfiqurasiya qeydləri əlavə edin. Defoltlar yerli inkişaf üçün tətbiqetməni söndürür.| Açar | Növ | Defolt | Qeydlər |
  | --- | --- | --- | --- |
  | `enabled` | bool | `false` | Qəbul yoxlamaları üçün əsas keçid; `required_minimum_band` olmadan ev sahibi xəbərdarlığı qeyd edir və icranı atlayır. |
  | `service_endpoints` | massiv | `[]` | Fırıldaqçılıq xidmətinin əsas URL-lərinin sifarişli siyahısı. Dublikatlar deterministik şəkildə silinir; qarşıdan gələn doğrulayıcı üçün qorunur. |
  | `connect_timeout_ms` | müddəti | `500` | Bağlantının dayandırılması cəhdlərindən əvvəl millisaniyələr; sıfır dəyərləri standarta qaytarır. |
  | `request_timeout_ms` | müddəti | `1500` | Risk xidmətindən cavab gözləmək üçün millisaniyələr. |
  | `missing_assessment_grace_secs` | müddəti | `0` | Çatışmayan qiymətləndirmələrə imkan verən lütf pəncərəsi; sıfırdan fərqli dəyərlər əməliyyatı qeyd edən və icazə verən deterministik geri dönüşü tetikler. |
  | `required_minimum_band` | enum (`low`, `medium`, `high`, `critical`) | `null` | Təyin edildikdə, əməliyyatlar bu ciddilik diapazonunda və ya yuxarıda qiymətləndirmə əlavə etməlidir; aşağı qiymətlər rədd edilir. `enabled` doğru olsa belə, qapını deaktiv etmək üçün `null` olaraq təyin edin. |
  | `attesters` | massiv | `[]` | Attestasiya mühərriklərinin isteğe bağlı reyestri. Doldurulduqda, zərflər sadalanan açarlardan biri ilə imzalanmalı və uyğun həzm daxil edilməlidir. |

- **Validasiya**: `crates/iroha_core/tests/fraud_monitoring.rs`-də vahid testləri əlil, çatışmayan və qeyri-kafi diapazonlu yolları əhatə edir; `integration_tests::fraud_monitoring_requires_assessment_bands` istehzalı qiymətləndirmə axınını başdan sona həyata keçirir.

- **Telemetriya**: `iroha_telemetry` qiymətləndirmə saymalarını (`fraud_psp_assessments_total{tenant,band,lane,subnet}`), çatışmayan metadata (`fraud_psp_missing_assessment_total{tenant,lane,subnet,cause}`), gecikmə histoqramlarını (`fraud_psp_latency_ms{tenant,lane,subnet}`), paylanma 0400-da tutan PSP-yə baxan kollektorları ixrac edir. faydalı yüklər (`fraud_psp_invalid_metadata_total{tenant,field,lane,subnet}`), attestasiya nəticələri (`fraud_psp_attestation_total{tenant,engine,lane,subnet,status}`) və nəticə uyğunsuzluqları (`fraud_psp_outcome_mismatch_total{tenant,direction,lane,subnet}`). Hər bir tranzaksiyada gözlənilən metadata açarları `fraud_assessment_band`, `fraud_assessment_tenant`, `fraud_assessment_score_bps`, `fraud_assessment_latency_ms`, attestator zərf/həzm cütü (`fraud_assessment_envelope`, I003NI0X, I003NI0X və I003NI0X), `fraud_assessment_disposition` bayrağı (dəyərlər: `approved`, `declined`, `manual_review`, `confirmed_fraud`, `false_positive`, `false_positive`, Prometheus, Prometheus).
- **Norito Sxemi**: `RiskQuery`, `FraudAssessment` və idarəetmə hesabatları üçün Norito növlərini müəyyənləşdirin. Kodek sabitliyinə zəmanət vermək üçün gediş-gəliş testləri təmin edin.

## Məxfilik və Məlumatların Minimallaşdırılması
- Ləqəblər, hashed cihaz identifikatorları və kobud geolokasiya kovaları mərkəzi xidmətlə paylaşılan bütün məlumat müstəvisini təşkil edir.
- PSP-lər ləqəblərdən real identifikasiyalara qədər xəritələşdirməni saxlayır; heç bir belə xəritələmə onların perimetrini tərk etmir.
- Risk modelləri yalnız təxəllüslü davranış siqnalları və PSP tərəfindən təqdim edilmiş kontekstdə (tacir kateqoriyası, kanal, autentifikasiya gücü) işləyir.
- Audit ixracı ümumiləşdirilir (məsələn, gündə bir PSP üzrə saylar). İstənilən qazma ikili nəzarət və PSP tərəfində anonimləşdirmə tələb edir.## Əməliyyatlar və Yerləşdirmə
- Qiymətləndirmə platformasını mərkəzi bank qovşağı operatorlarından fərqli olaraq təyin edilmiş operator tərəfindən idarə olunan xüsusi alt sistem kimi yerləşdirin.
- Mavi/yaşıl mühitləri təmin edin: `fraud-scoring-prod`, `fraud-scoring-shadow`, `fraud-lab`.
- Avtomatlaşdırılmış sağlamlıq yoxlamalarını həyata keçirin (API gecikməsi, mesajların yığılması, model yüklənməsinin müvəffəqiyyəti). Sağlamlıq yoxlamaları uğursuz olarsa, PSP SDK avtomatik olaraq yalnız yerli rejimə keçir və operatorları xəbərdar edir.
- Saxlama qablarını qoruyun: isti anbar (30 gün xüsusiyyət mağazasında), isti anbar (obyekt saxlamasında 1 il), soyuq arxiv (5 il sıxılmış).

## Telemetriya Kollektorları və İdarə Panelləri

### Kollektorlar tələb olunur

- **Prometheus scrape**: `fraud_psp_*` seriyasının ixrac edilməsi üçün PSP inteqrasiya profili ilə işləyən hər doğrulayıcıda `/metrics`-i aktiv edin. Defolt etiketlərə yertutan `subnet="global"` və `lane` identifikatorları daxildir ki, tablosuna bir dəfə çox alt şəbəkə marşrutlaşdırma gəmilərində dönə bilsin.
- **Qiymətləndirmənin yekunları**: `fraud_psp_assessments_total{tenant,band}` hər ciddilik diapazonu üzrə qəbul edilmiş qiymətləndirmələri hesablayır; icarəçi 5 dəqiqə ərzində hesabat verməyi dayandırarsa, yanğın barədə xəbərdarlıq edir.
- **Çatışmayan metadata**: `fraud_psp_missing_assessment_total{tenant,cause}` sərt imtinaları (`cause="missing"`) güzəştli pəncərə güzəştlərindən (`cause="grace"`) fərqləndirir. Dəfələrlə lütf kovasına düşən Gate əməliyyatları.
- **Gecikmə histoqramı**: `fraud_psp_latency_ms_bucket` PSP tərəfindən bildirilmiş xal gecikməsini izləyir. Hədəf 20% kənara çıxarsa, artırın.
- **Etibarsız metadata**: `fraud_psp_invalid_metadata_total{field}` PSP faydalı yük reqressiyalarını (məsələn, çatışmayan kirayəçi identifikatorları, pozulmuş dispozisiyalar) qeyd edir, beləliklə SDK yeniləmələrini tez bir zamanda yaymaq olar.
- **Atestasiya statusu**: `fraud_psp_attestation_total{tenant,engine,status}` zərflərin imzalandığını və həzmlərin uyğun olduğunu təsdiqləyir. `status!="verified"` hər hansı kirayəçi və ya mühərrik üçün sıçrayış olarsa xəbərdar edin.

### İdarə panelinin əhatə dairəsi

- **İdarəçinin icmalı**: hər icarəçiyə görə qrup üzrə `fraud_psp_assessments_total` yığılmış sahə diaqramı, P95 gecikmə və uyğunsuzluq saylarını ümumiləşdirən cədvəllə birlikdə.
- **Əməliyyatlar**: həftə ərzində müqayisəli `fraud_psp_latency_ms` və `fraud_psp_score_bps` üçün histoqram panelləri, üstəgəl `fraud_psp_missing_assessment_total` üçün tək statistik sayğaclar `cause` ilə bölünür.
- **Risk monitorinqi**: hər kirayəçiyə görə `fraud_psp_outcome_mismatch_total` bar diaqramı, `band`-in `low` və ya `medium` olduğu son `fraud_assessment_disposition=confirmed_fraud` hallarının siyahısını aşağıya doğru açan cədvəl.
- **Xəbərdarlıq qaydaları**:
  - `rate(fraud_psp_missing_assessment_total{cause="missing"}[5m]) > 0` → səhifələmə xəbərdarlığı (qəbul PSP trafikini rədd edir).
  - `histogram_quantile(0.95, sum(rate(fraud_psp_latency_ms_bucket[10m])) by (le,tenant)) > 150` → gecikmə SLO pozuntusu.
  - `sum by (tenant) (rate(fraud_psp_outcome_mismatch_total{direction="missed_fraud"}[1h])) > 0.01` → model sürüşməsi / siyasət boşluğu.

### Failover gözləntiləri- PSP SDK-ları iki aktiv hesablama son nöqtəsini saxlamalı və nəqliyyat xətalarını və ya gecikmə sıçrayışlarını >200ms aşkar etdikdən sonra 15 saniyə ərzində sıradan çıxmalıdır. Kitab ən çox `fraud_monitoring.missing_assessment_grace_secs` üçün lütf trafikinə dözür; operatorlar istehsalda düyməni 5 dəqiqə ərzində lütfdə qalırsa, PSP əl ilə yoxlamaya keçməli və paylaşılan fırıldaq əməliyyatları komandası ilə Sev2 hadisəsi açmalıdır.
- Fəal-aktiv yerləşdirmələr fəlakətin bərpası təlimləri zamanı növbənin boşaldılmasını/təkrarını nümayiş etdirməlidir. Təkrar oxutma göstəriciləri təkrar oynatma pəncərəsi üçün `fraud_psp_latency_ms` P99-u 400ms-dən aşağı saxlamalıdır.

## PSP Məlumat Paylaşımı Yoxlama Siyahısı

1. **Telemetriya santexnika**: kitabçaya verilən hər bir əməliyyat üçün yuxarıda sadalanan metadata açarlarını ifşa edin; icarəçi identifikatorları təxəllüslü olmalı və PSP müqaviləsinə uyğun olmalıdır.
2. **Anonimləşdirmə**: PSP perimetrini tərk etməzdən əvvəl cihazın heşlərinin, ləqəb identifikatorlarının və dispozisiyalarının təxəllüslə adlandırıldığını təsdiqləyin; heç bir PII Norito metadatasına daxil edilə bilməz.
3. **Gecikmə hesabatı**: `fraud_assessment_latency_ms`-i uç-to-end zamanlama (PSP-yə keçid) ilə doldurun ki, SLA reqressiyaları dərhal üzə çıxsın.
4. **Nəticələrin uzlaşdırılması**: uyğunsuzluq göstəricilərini dəqiq saxlamaq üçün fırıldaqçılıq halları təsdiqləndikdən sonra (məsələn, geri qaytarılma göndərilib) `fraud_assessment_disposition`-i yeniləyin.
5. **Failover drills**: paylaşılan yoxlama siyahısından istifadə edərək rüblük məşq edin—avtomatik son nöqtənin dəyişdirilməsini yoxlayın, lütf pəncərəsi qeydini təmin edin və `scripts/ci/schedule_fraud_scoring.sh` tərəfindən təqdim edilmiş izləmə tapşırığına təlim qeydləri əlavə edin.
6. **İdarə panelinin yoxlanılması**: PSP əməliyyat qrupları təyyarəyə daxil olduqdan sonra və hər qırmızı komanda məşqindən sonra ölçülərin gözlənilən icarəçi etiketləri ilə axdığını təsdiqləmək üçün Prometheus idarə panellərini nəzərdən keçirməlidir.

## Təhlükəsizlik Mülahizələri
- Bütün cavablar hardware tərəfindən dəstəklənən açarlarla imzalanır; PSP-lər ballara etibar etməzdən əvvəl imzaları təsdiqləyir.
- Model sərhədlərini öyrənmək məqsədi daşıyan araşdırma hücumlarını azaltmaq üçün ləqəb/cihaz üzrə dərəcə limiti.
- PSP şəxsiyyətini ictimaiyyətə açıqlamadan sızan cavabları izləmək üçün qiymətləndirmələrin içərisinə su nişanını daxil edin.
- Təhlükəsizlik İş Qrupu (Milestone 0) ilə koordinasiyalı olaraq rüblük qırmızı komanda məşqlərini həyata keçirin və tapıntıları yol xəritəsi yeniləmələrinə daxil edin.## İcra Mərhələləri
1. **Mərhələ 0 – Təməllər**
   - Norito sxemlərini, PSP SDK iskelesini, konfiqurasiya naqillərini və mühasibat kitabçası tərəfində yoxlama stubunu yekunlaşdırın.
   - Məcburi risk yoxlamalarını əhatə edən deterministik qayda mühərriki yaradın (sürət, ləqəb cütü üçün sürət, cihazın təkrar istifadəsi).
2. **Mərhələ 1 – Mərkəzi Qiymətləndirmə MVP**
   - Xüsusiyyətlər mağazası, qiymətləndirmə xidməti və telemetriya tablosunu yerləşdirin.
   - Məhdud PSP kohortu ilə real vaxt hesablamalarını birləşdirin; gecikmə və keyfiyyət göstəricilərini əldə edin.
3. **Mərhələ 2 – Qabaqcıl Analitika**
   - Anomaliyaların aşkarlanması, qrafikə əsaslanan keçid təhlili və adaptiv hədləri tətbiq edin.
   - İdarəetmə portalını və toplu hesabat boru kəmərlərini işə salın.
4. **Mərhələ 3 – Davamlı Öyrənmə və Avtomatlaşdırma**
   - Model təlimi/təsdiqləmə boru kəmərlərini avtomatlaşdırın, kanareyka yerləşdirmələri əlavə edin və SDK əhatə dairəsini genişləndirin.
   - Yurisdiksiyalararası məlumat mübadiləsi müqavilələri ilə uyğunlaşın və gələcək multi-alt şəbəkə körpülərinə qoşulun.

## Açıq Suallar
- Fırıldaqçılıq xidmətinin operatorunu hansı tənzimləyici orqan nizamlayacaq və nəzarət vəzifələri necə bölüşdürülür?
- PSP-lər provayderlər arasında ardıcıl UX saxlayarkən son istifadəçi problem axınını necə ifşa edir?
- Əsas xidmət sabit olduqdan sonra hansı məxfiliyi artıran texnologiyalara (məsələn, təhlükəsiz anklavlar, homomorf birləşmə) üstünlük verilməlidir?