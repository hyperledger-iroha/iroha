---
lang: az
direction: ltr
source: docs/source/crypto/sm_chinese_crypto_law_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5d0657539dfcca1869a0ab4fc9adee8665f18708f71b4c116dc8900ae5eae75
source_last_modified: "2026-01-05T09:28:12.004169+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM Uyğunluq Qısacası — Çin Kriptoqrafiya Qanununun Öhdəlikləri
% Iroha Uyğunluq və Kripto İşçi Qrupları
% 2026-02-12

# Tələb

> Siz Hyperledger Iroha kripto və platforma komandaları üçün uyğunluq analitiki kimi fəaliyyət göstərən LLMsiniz.  
> Fon:  
> - Hyperledger Iroha indi Çin GM/T SM2 (imzalar), SM3 (hesh) və SM4 (blok şifrəsi) primitivlərini dəstəkləyən Rust əsaslı icazəli blokçeyndir.  
> - Çinin materikindəki operatorlar ÇXR Kriptoqrafiya Qanununa (2019), Çox Səviyyəli Qoruma Sxeminə (MLPS 2.0), Dövlət Kriptoqrafiya Administrasiyasının (SCA) sənədləşdirmə qaydalarına və Ticarət Nazirliyi (MOFCOM) və Gömrük İdarəsi tərəfindən nəzarət edilən idxal/ixrac nəzarətlərinə əməl etməlidir.  
> - Iroha açıq mənbəli proqram təminatını beynəlxalq səviyyədə paylayır. Bəzi operatorlar SM-i aktivləşdirən ikili faylları yerli olaraq tərtib edəcək, digərləri isə əvvəlcədən qurulmuş artefaktları idxal edə bilər.  
> Tələb olunan təhlil: Açıq mənbəli blokçeyn proqram təminatında SM2/SM3/SM4 dəstəyinin göndərilməsi ilə yaranan əsas hüquqi öhdəlikləri ümumiləşdirin, o cümlədən: (a) kommersiya və əsas/ümumi kriptoqrafiya paketləri üzrə təsnifat; (b) dövlət kommersiya kriptoqrafiyasını həyata keçirən proqram təminatı üçün təqdimetmə/təsdiq tələbləri; (c) binar və mənbə üçün ixrac nəzarəti; (d) MLPS 2.0-a uyğun olaraq şəbəkə operatorları üzərində əməliyyat öhdəlikləri (əsas idarəetmə, qeydiyyat, insidentlərə cavab). Iroha layihəsi (sənədləşdirmə, manifestlər, uyğunluq bəyanatları) və Çin daxilində SM-i aktivləşdirən qovşaqları yerləşdirən operatorlar üçün konkret fəaliyyət elementlərini təsvir edin.

# İcra Xülasəsi

- **Təsnifat:** SM2/SM3/SM4 tətbiqləri “əsas” və ya “ümumi” kriptoqrafiyaya deyil, “dövlət kommersiya kriptoqrafiyası”na (商业密码) aiddir, çünki onlar mülki/kommersiya məqsədləri üçün icazə verilmiş ictimai alqoritmlərdir. Açıq mənbəli paylamaya icazə verilir, lakin Çində təklif olunan kommersiya məhsullarında və ya xidmətlərdə istifadə edildikdə sənədləşməyə tabedir.
- **Layihə öhdəlikləri:** Alqoritmin mənşəyini, deterministik qurma təlimatlarını və binarların dövlət kommersiya kriptoqrafiyasını həyata keçirdiyini qeyd edən uyğunluq bəyanatını təmin edin. Aşağı axın inteqratorlarının sənədləri tamamlaya bilməsi üçün SM qabiliyyətini işarələdiyini göstərən Norito-i qoruyun.
- **Operator öhdəlikləri:** Çinli operatorlar SM alqoritmlərindən istifadə edərək məhsul/xidmətləri əyalət SCA bürosuna təqdim etməli, MLPS 2.0 qeydiyyatını tamamlamalı (maliyyə şəbəkələri üçün çox güman ki, Səviyyə 3), təsdiqlənmiş açarların idarə edilməsi və giriş nəzarətlərini yerləşdirməli və ixrac/idxal bəyannamələrinin MOFCOM kataloqu ilə uyğunluğunu təmin etməlidirlər.

# Tənzimləyici mənzərə| Tənzimləmə | Əhatə dairəsi | Iroha SM dəstəyinə təsir |
|------------|-------|----------------------------|
| **ÇXR-nin Kriptoqrafiya Qanunu (2019)** | Əsas/ümumi/kommersiya kriptoqrafiyasını, mandat idarəetmə sistemini, sənədləşdirməni və sertifikatlaşdırmanı müəyyən edir. | SM2/SM3/SM4 “kommersiya kriptoqrafiyası”dır və Çində məhsul/xidmət kimi təqdim edildikdə sənədləşdirmə/sertifikasiya qaydalarına əməl etməlidir. |
| **Kommersiya Kriptoqrafiya Məhsulları üçün SCA İnzibati Tədbirləri** | İstehsal, satış və xidmət təminatlarını idarə edir; məhsulun təqdim edilməsi və ya sertifikatlaşdırılması tələb olunur. | SM alqoritmlərini həyata keçirən açıq mənbə proqram təminatı kommersiya təkliflərində istifadə edildikdə operator sənədlərinə ehtiyac duyur; tərtibatçılar sənədləri təqdim etməyə kömək etmək üçün sənədlər təqdim etməlidirlər. |
| **MLPS 2.0 (Kibertəhlükəsizlik Qanunu + MLPS qaydaları)** | Operatorlardan informasiya sistemlərinin təsnifləşdirilməsini və təhlükəsizlik nəzarətinin həyata keçirilməsini tələb edir; Səviyyə 3 və ya yuxarıda kriptoqrafiya uyğunluğu sübutu tələb olunur. | Maliyyə/identifikasiya məlumatlarını idarə edən blokçeyn qovşaqları adətən MLPS 3-cü Səviyyədə qeydiyyatdan keçin; operatorlar SM-dən istifadəni, açarların idarə edilməsini, qeydiyyatı və insidentlərin idarə edilməsini sənədləşdirməlidirlər. |
| **MOFCOM İxrac Nəzarəti Kataloqu və Gömrük İdxal Qaydaları** | Kriptoqrafik məhsulların ixracına nəzarət edir, müəyyən alqoritmlər/avadanlıqlar üçün icazə tələb edir. | Mənbə kodunun dərci ümumiyyətlə “ictimai domen” müddəalarına əsasən azaddır, lakin SM qabiliyyəti ilə tərtib edilmiş ikili faylların ixracı təsdiq edilmiş alıcılara göndərilmədiyi halda kataloqu işə sala bilər; idxalçılar dövlət kommersiya kriptoqrafiyasını bəyan etməlidirlər. |

# Əsas Öhdəliklər

## 1. Məhsul və Xidmətlərin Təqdimatı (Dövlət Kriptoqrafiya İdarəsi)

- **Kim fayl verir:** Çində məhsul/xidmət təqdim edən qurum (məsələn, operator, SaaS provayderi). Açıq mənbə baxıcılarından fayl vermək tələb olunmur, lakin qablaşdırma təlimatı aşağı axın sənədlərini təmin etməlidir.
- **Çatdırılanlar:** Alqoritmin təsviri, təhlükəsizlik dizayn sənədləri, sınaq sübutları, təchizat zəncirinin mənşəyi və əlaqə məlumatları.
- **Iroha fəaliyyət:** Alqoritmin əhatə dairəsi, deterministik qurma addımları, asılılıq heşləri və təhlükəsizlik sorğuları üçün əlaqə daxil olmaqla “SM kriptoqrafiya bəyanatı” dərc edin.

## 2. Sertifikatlaşdırma və Sınaq

- Müəyyən sektorlar (maliyyə, telekommunikasiya, kritik infrastruktur) akkreditə olunmuş laboratoriya testi və ya sertifikatlaşdırma tələb edə bilər (məsələn, CC-Grade/OSCCA sertifikatı).
- GM/T spesifikasiyalarına uyğunluğu nümayiş etdirən reqressiya testi artefaktlarını daxil edin.

## 3. MLPS 2.0 Əməliyyat İdarəetmələri

Operatorlar:1. **Blokçeyn sistemini** İctimai Təhlükəsizlik Bürosunda qeydiyyatdan keçirin, o cümlədən kriptoqrafiyadan istifadə xülasələri.
2. **Əsas idarəetmə siyasətlərini həyata keçirin**: SM2/SM4 tələblərinə uyğun olaraq açarın yaradılması, paylanması, fırlanması, məhv edilməsi; əsas həyat dövrü hadisələrini qeyd edin.
3. **Təhlükəsizlik auditini aktivləşdirin**: SM-nin aktivləşdirdiyi əməliyyat qeydlərini, kriptoqrafik əməliyyat hadisələrini və anomaliya aşkarlanmasını əldə edin; logları ≥6 ay saxlamaq.
4. **Hadisə cavabı:** kriptoqrafik kompromis prosedurlarını və hesabat qrafiklərini daxil edən sənədləşdirilmiş cavab planlarını qoruyun.
5. **Vendorun idarə edilməsi:** yuxarı proqram təminatçılarının (Iroha layihəsi) zəiflik bildirişləri və yamaqları təmin edə bilməsini təmin edin.

## 4. İdxal/İxrac Mülahizələri

- **Açıq mənbə kodu:** Bir qayda olaraq, ictimai domen istisnası ilə azad edilir, lakin baxıcılar giriş qeydlərini izləyən və dövlət kommersiya kriptoqrafiyasına istinad edən lisenziya/imtina bildirişini ehtiva edən serverlərdə yükləmələri təşkil etməlidir.
- **Öncədən qurulmuş binalar:** SM-i təmin edən ikili faylları Çinə/çökə göndərən ixracatçılar elementin “Kommersiya Kriptoqrafiya İxrac Nəzarəti Kataloqu” ilə əhatə olunub-olunmadığını təsdiq etməlidir. Xüsusi avadanlıq olmadan ümumi təyinatlı proqram təminatı üçün sadə ikili istifadə bəyannaməsi kifayət edə bilər; yerli müşavir təsdiq etmədikcə, baxıcılar daha sərt nəzarəti olan yurisdiksiyalardan ikili faylları yaymamalıdırlar.
- **Operator idxalı:** Çinə ikili faylları gətirən qurumlar kriptoqrafiyadan istifadəni bəyan etməlidirlər. Gömrük yoxlamasını sadələşdirmək üçün hash manifestləri və SBOM təqdim edin.

# Tövsiyə olunan Layihə Fəaliyyətləri

1. **Sənədləşdirmə**
   - `docs/source/crypto/sm_program.md`-ə dövlət kommersiya kriptoqrafiyası statusunu, sənədləşdirmə gözləntilərini və əlaqə nöqtələrini qeyd edən uyğunluq əlavəsi əlavə edin.
   - Operatorların sənədləri hazırlayarkən istifadə edə biləcəyi Norito manifest sahəsini (`crypto.sm.enabled=true`, `crypto.sm.approval=l0|l1`) dərc edin.
   - Torii `/v1/node/capabilities` reklamının (və `iroha runtime capabilities` CLI ləqəbinin) hər buraxılışla göndərilməsinə əmin olun ki, operatorlar MLPS/密 üçün `crypto.sm` manifest şəklini çəkə bilsinlər.
   - Öhdəlikləri ümumiləşdirən ikidilli (EN/ZH) uyğunluğun sürətli başlanğıcını təmin edin.
2. **Artefaktları buraxın**
   - SM-in effektiv qurulması üçün SBOM/CycloneDX fayllarını göndərin.
   - Deterministik quruluş skriptləri və təkrarlana bilən Dockerfiles daxil edin.
3. **Dəstək Operator Sənədləri**
   - Alqoritmə uyğunluğu təsdiq edən şablon məktubları təklif edin (məsələn, GM/T arayışları, test əhatəsi).
   - Təchizatçı-bildiriş tələblərini təmin etmək üçün təhlükəsizlik məsləhətlərinin poçt siyahısını saxlamaq.
4. **Daxili İdarəetmə**
   - Buraxılış yoxlama siyahısında SM uyğunluq yoxlama nöqtələrini izləyin (audit tamamlandı, sənədlər yeniləndi, manifest sahələri yerində).

# Operator Fəaliyyət Elementləri (Çin)1. Yerləşdirmənin “kommersiya kriptoqrafiyası məhsulu/xidməti” olub-olmadığını müəyyən edin (əksər müəssisə şəbəkələri bunu edir).
2. Əyalət SCA bürosunda məhsul/xidmət faylı; Iroha uyğunluq bəyanatını, SBOM, test hesabatlarını əlavə edin.
3. MLPS 2.0, hədəf Səviyyə 3 nəzarəti altında blokçeyn sistemini qeydiyyatdan keçirin; Iroha loglarını təhlükəsizlik monitorinqinə inteqrasiya edin.
4. SM açarının həyat dövrü prosedurlarını qurun (lazım olduqda təsdiq edilmiş KMS/HSM-dən istifadə edin).
5. Hadisələrə cavab təlimlərinə kriptoqrafiyanın kompromis ssenarilərini daxil edin; Iroha baxıcıları ilə eskalasiya əlaqələri qurun.
6. Transsərhəd məlumat axını üçün, əgər şəxsi məlumatlar ixrac edilirsə, əlavə CAC (Kiberməkan Administrasiyası) sənədlərini təsdiqləyin.

# Müstəqil Sorğu (Kopyala/Yapışdır)

> Siz Hyperledger Iroha kripto və platforma komandaları üçün uyğunluq analitiki kimi fəaliyyət göstərən LLMsiniz.  
> Ümumi məlumat: Hyperledger Iroha indi Çin GM/T SM2 (imzalar), SM3 (hesh) və SM4 (blok şifrəsi) primitivlərini dəstəkləyən Rust əsaslı icazəli blokçeyndir. Çinin materikindəki operatorlar ÇXR Kriptoqrafiya Qanununa (2019), Çox Səviyyəli Mühafizə Sxeminə (MLPS 2.0), Dövlət Kriptoqrafiya Administrasiyasının (SCA) sənədləşdirmə qaydalarına və MOFCOM və Gömrük İdarəsi tərəfindən nəzarət edilən idxal/ixrac nəzarətlərinə əməl etməlidir. Iroha layihəsi SM-nin aktivləşdirdiyi açıq mənbəli proqram təminatını beynəlxalq səviyyədə paylayır; bəzi operatorlar ikili faylları ölkə daxilində tərtib edir, digərləri isə əvvəlcədən qurulmuş artefaktları idxal edir.  
> Tapşırıq: Açıq mənbəli blokçeyn proqramında SM2/SM3/SM4 dəstəyinin göndərilməsi ilə yaranan hüquqi öhdəlikləri ümumiləşdirin. Bu alqoritmlərin təsnifatını (kommersiya və əsas/ümumi kriptoqrafiya), proqram məhsulları üçün tələb olunan sənədləri və ya sertifikatları, mənbə və ikili sənədlərə aid olan ixrac/idxal nəzarətlərini və MLPS 2.0 (açar idarəetmə, qeyd, insident reaksiyası) üzrə şəbəkə operatorları üçün əməliyyat vəzifələrini əhatə edin. Iroha layihəsi (sənədləşdirmə, manifestlər, uyğunluq bəyanatları) və Çin daxilində SM-i aktivləşdirən qovşaqları yerləşdirən operatorlar üçün konkret fəaliyyət maddələri təmin edin.