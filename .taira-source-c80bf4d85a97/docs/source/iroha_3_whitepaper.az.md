---
lang: az
direction: ltr
source: docs/source/iroha_3_whitepaper.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 07e149429887b0dfc38cf0619552cbefcbae4dd1ec9fe9e9d47a05371ed08f29
source_last_modified: "2025-12-29T18:16:35.968351+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v3.0 (Nexus Ön Baxış)

Bu sənəd çox zolaqlı yola diqqət yetirərək, perspektivli Hyperledger Iroha v3 arxitekturasını əks etdirir.
boru kəməri, Nexus məlumat boşluqları və Aktiv Mübadilə Alətlər dəsti (AXT). O, Iroha v2 ağ kağızını tamamlayır
fəal şəkildə inkişaf etdirilən gələcək imkanları təsvir edir.

---

## 1. İcmal

Iroha v3 üfüqi genişlənmə və daha zəngin çarpaz domen ilə v2-nin deterministik əsasını genişləndirir
iş axınları. **Nexus** kod adlı buraxılış təqdim edir:

- **SORA Nexus** adlı vahid, qlobal şəkildə paylaşılan şəbəkə. Bütün Iroha v3 həmyaşıdları bu universalda iştirak edirlər
  təcrid olunmuş yerləşdirmələri idarə etməkdənsə, kitab kitabçası. Təşkilatlar öz məlumat boşluqlarını qeydiyyatdan keçirərək qoşulur,
  ümumi kitabçaya daxil olarkən siyasət və məxfilik üçün təcrid olunmuş qalır.
- Paylaşılan kod bazası: eyni repozitoriya həm Iroha v2 (öz-özünə yerləşdirilən şəbəkələr) həm də Iroha v3 (SORA Nexus) qurur.
  Konfiqurasiya hədəf rejimi seçir ki, operatorlar proqram təminatını dəyişmədən Nexus xüsusiyyətlərini qəbul edə bilsinlər.
  yığınlar. Iroha Virtual Maşın (IVM) hər iki buraxılışda eynidir, ona görə də Kotodama müqavilələri və bayt kodu
  artefaktlar öz-özünə yerləşdirilən şəbəkələrdə və qlobal Nexus kitabçasında problemsiz işləyir.
- Müstəqil iş yüklərini paralel olaraq emal etmək üçün çox zolaqlı blok istehsalı.
- Zəncir üzərindəki lövbərlər vasitəsilə birləşdirilə bilən icra mühitlərini təcrid edən məlumat boşluqları (DS).
- Atom, çarpaz kosmos dəyər köçürmələri və müqavilə ilə idarə olunan svoplar üçün Aktiv Mübadilə Alətləri (AXT).
- Reliable Broadcast Commit (RBC) zolaqları, müəyyən edilmiş son tarixlər və sübut vasitəsilə gücləndirilmiş etibarlılıq
  seçmə büdcələri.

Bu xüsusiyyətlər aktiv inkişaf mərhələsində qalır; API və tərtibatlar v3 generalından əvvəl inkişaf edə bilər
mövcudluq mərhələsi. `nexus.md`, `nexus_transition_notes.md` və `new_pipeline.md`-ə baxın
mühəndislik səviyyəli detal.

## 2. Çox zolaqlı arxitektura

- ** Planlayıcı:** Nexus planlaşdırıcı arakəsmələri məlumat məkanı identifikatorlarına və
  kompozisiya qrupları. Zolaqlar daxilində deterministik sifariş zəmanətlərini qoruyaraq paralel olaraq icra olunur
  hər zolaq.
- **Lane qrupları:** Əlaqədar məlumat məkanları `LaneGroupId` paylaşır ki, bu da iş axınları üçün əlaqələndirilmiş icraya imkan verir.
  çoxsaylı komponentləri əhatə edir (məsələn, CBDC DS və onun ödəniş dApp DS).
- **Son tarixlər:** Hər bir zolaq zəmanət vermək üçün müəyyən edilmiş son tarixləri (blok, sübut, məlumatların mövcudluğu) izləyir.
  tərəqqi və məhdud resurs istifadəsi.
- **Telemetriya:** Zolaq səviyyəli ölçülər ötürmə qabiliyyətini, növbənin dərinliyini, son tarix pozuntularını və bant genişliyindən istifadəni ifşa edir.
  CI skriptləri tablosunu planlaşdırıcı ilə uyğunlaşdırmaq üçün bu sayğacların mövcudluğunu təsdiqləyir.

## 3. Məlumat boşluqları (Nexus)- **İzolyasiya:** Hər bir məlumat məkanı öz konsensus zolağı, dünya vəziyyəti seqmenti və Kür yaddaşını saxlayır. Bu
  qlobal SORA Nexus kitabçasını lövbərlər vasitəsilə ardıcıl saxlayarkən məxfilik domenlərini dəstəkləyir.
- **Ankerlər:** Daimi tapşırıqlar DS vəziyyətini ümumiləşdirən anker artefaktları yaradır (Merkle kökləri, sübutlar,
  öhdəliklər) və onları yoxlanılabilirlik üçün qlobal zolağa dərc edin.
- ** Zolaq qrupları və birləşmə:** Məlumat məkanları atomik AXT-yə icazə verən birləşmə qruplarını elan edə bilər.
  təsdiq edilmiş iştirakçılar arasında əməliyyatlar. İdarəetmə üzvlük dəyişikliklərinə və aktivləşmə dövrlərinə nəzarət edir.
- **Silinmə kodlu yaddaş:** Kura və WSV snapshotları datanı miqyaslaşdırmaq üçün `(k, m)` silmə kodlaşdırma parametrlərini qəbul edir
  determinizmdən imtina etmədən mövcudluq. Bərpa prosedurları itkin fraqmentləri deterministik şəkildə bərpa edir.

## 4. Aktiv Mübadilə Alətlər dəsti (AXT)

- **Deskriptor və bağlama:** Müştərilər deterministik AXT deskriptorlarını qururlar. `axt_binding` hash ankerləri
  fərdi zərflərin təsviri, təkrarın qarşısının alınması və konsensus iştirakçılarının bayt-for-
  bayt Norito faydalı yüklər.
- **Sistemlər:** IVM `AXT_BEGIN`, `AXT_TOUCH` və `AXT_COMMIT` sistem zənglərini ifşa edir. Müqavilələr özlərinin bəyan edir
  hər bir məlumat sahəsi üçün oxu/yazma dəstləri, hosta zolaqlar arasında atomikliyi tətbiq etməyə imkan verir.
- ** Tutacaqlar və dövrlər:** Pul kisələri `(dataspace_id, epoch_id, sub_nonce)` ilə əlaqəli qabiliyyət tutacaqları əldə edir.
  Paralel konfliktdən determinist şəkildə istifadə edir, məhdudiyyətlər olduqda kanonik `AxtTrap` kodları qaytarır.
  pozulub.
- **Siyasətin icrası:** Əsas hostlar indi WSV-də Space Directory manifestlərindən AXT siyasət snapshotlarını əldə edir,
  manifest kökü, hədəf zolağı, aktivasiya dövrü, sub-nonce və bitmə yoxlamalarının tətbiqi (`current_slot >= expiry_slot`
  abortlar) hətta minimal sınaq hostlarında belə. Siyasətlər məlumat məkanı identifikatoru ilə əsaslanır və zolaqlı kataloqdan belə qurulur
  tutacaqlar buraxılış zolağından qaça bilməz və ya köhnəlmiş manifestlərdən istifadə edə bilməz.
  - Rədd edilmə səbəbləri müəyyəndir: naməlum məlumat məkanı, açıq kök uyğunsuzluğu, hədəf zolağı uyğunsuzluğu,
    manifest aktivasiyasının altında handle_era, siyasət mərtəbəsinin altında sub_nonce, vaxtı keçmiş tutacaq, üçün toxunma yoxdur
    tutacaq məlumat məkanı və ya lazım olduqda sübut yoxdur.
- **Sübutlar və son tarixlər:** Aktiv pəncərə Δ zamanı validatorlar sübutlar, məlumatların mövcudluğu nümunələri,
  və təzahür edir. Son tarixlərə əməl edilməməsi müştərinin təkrar cəhdləri üçün təlimatla AXT-ni qəti şəkildə dayandırır.
- **İdarəetmə inteqrasiyası:** Siyasət modulları hansı məlumat boşluqlarının AXT-də iştirak edə biləcəyini, tarif limitini müəyyənləşdirir
  öhdəlikləri, ləğvediciləri və hadisə qeydlərini tutan auditor üçün uyğun manifestləri idarə edir və dərc edir.

## 5. Reliable Broadcast Commit (RBC) zolaqları- **Xüsusi zolaqlı DA:** RBC zolaqları zolaq qruplarını əks etdirir, hər bir çox zolaqlı boru kəmərinin xüsusi dataya malik olmasını təmin edir.
  mövcudluğuna zəmanət verir.
- **Nümunə alma büdcələri:** Qiymətləndiricilər sübutları təsdiqləmək üçün deterministik seçmə qaydalarına (`q_in_slot_per_ds`) əməl edirlər
  və mərkəzi koordinasiya olmadan şahid materialı.
- **Əks təzyiq anlayışları:** Sumeragi kardiostimulyator hadisələri dayanmış zolaqları diaqnoz etmək üçün RBC statistikası ilə əlaqələndirilir
  (bax `scripts/sumeragi_backpressure_log_scraper.py`).

## 6. Əməliyyatlar və miqrasiya

- **Keçid planı:** `nexus_transition_notes.md` tək zolaqlıdan (Iroha v2) mərhələli miqrasiyanı təsvir edir
  çox zolaqlı (Iroha v3), o cümlədən telemetriya quruluşu, konfiqurasiya keçidi və genezis yeniləmələri.
- **Universal şəbəkə:** SORA Nexus həmyaşıdları ümumi genezis və idarəetmə yığınını idarə edir. Bortda yeni operatorlar
  məlumat məkanı (DS) yaratmaq və müstəqil şəbəkələri işə salmaq əvəzinə Nexus qəbul siyasətlərini təmin etmək.
- **Konfiqurasiya:** Yeni konfiqurasiya düymələri zolaqlı büdcələri, sübut son tarixləri, AXT kvotalarını və məlumat məkanı metadatasını əhatə edir.
  Operatorlar Nexus rejiminə daxil olana qədər defoltlar mühafizəkar olaraq qalır.
- **Sınaq:** Qızıl testlər AXT deskriptorlarını, zolaqlı manifestləri və sistem zəngi siyahılarını çəkir. İnteqrasiya testləri
  (`integration_tests/tests/repo.rs`, `crates/ivm/tests/axt_host_flow.rs`) başdan sona axınları həyata keçirin.
- ** Alətlər:** `kagami` Nexus-dən xəbərdar olan genezis generasiyası əldə edir və tablosunun skriptləri zolaq ötürmə qabiliyyətini təsdiqləyir,
  sübut büdcələri və RBC sağlamlığı.

## 7. Yol xəritəsi

- **Mərhələ 1:** Yerli AXT dəstəyi və auditi ilə tək domenli çox zolaqlı icranı aktivləşdirin.
- **Mərhələ 2:** İcazəli domenlərarası AXT üçün birləşmə qruplarını aktivləşdirin və telemetriya əhatəsini genişləndirin.
- **Mərhələ 3:** Tam Nexus məlumat məkanı federasiyası, silinmə kodlu yaddaşı və təkmil sübut mübadiləsini təqdim edin.

Status yeniləmələri `roadmap.md` və `status.md`-də yaşayır. Nexus dizaynına uyğun gələn töhfələr izlənməlidir
v3 üçün müəyyən edilmiş deterministik icra və idarəetmə siyasətləri.