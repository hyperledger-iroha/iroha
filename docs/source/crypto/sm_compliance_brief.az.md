---
lang: az
direction: ltr
source: docs/source/crypto/sm_compliance_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 73f5ca7a7484a26e901102dd6950b7110a18e7fa215a46540c7189c919e0958f
source_last_modified: "2025-12-29T18:16:35.942266+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SM2/SM3/SM4 Uyğunluq və İxrac Qısacası

Bu qısaca `docs/source/crypto/sm_program.md`-dəki memarlıq qeydlərini tamamlayır
və mühəndislik, əməliyyatlar və hüquq qrupları üçün təsirli təlimatlar təqdim edir
GM/T alqoritmi ailəsi yalnız yoxlama üçün önizləmədən daha geniş imkana keçir.

## Xülasə
- **Tənzimləyici əsas:** Çinin *Kriptoqrafiya Qanunu* (2019), *Kibertəhlükəsizlik Qanunu* və
  *Məlumat Təhlükəsizliyi Qanunu* tətbiq edildikdə SM2/SM3/SM4-ü “kommersiya kriptoqrafiyası” kimi təsnif edir
  quruda. Operatorlar istifadə hesabatlarını təqdim etməlidirlər və müəyyən sektorlar akkreditasiya tələb edir
  istehsaldan əvvəl sınaqdan keçirilir.
- **Beynəlxalq nəzarət:** Çindən kənarda alqoritmlər ABŞ EAR Kateqoriyasına aiddir.
  5 Hissə 2, Aİ 2021/821 Əlavə 1 (5D002) və oxşar milli rejimlər. Açıq mənbə
  nəşr adətən lisenziya istisnaları (ENC/TSU) üçün uyğundur, lakin binaries
  embarqoya məruz qalan bölgələrə sövq edilən ixracat nəzarətdə qalır.
- **Layihə siyasəti:** SM funksiyaları defolt olaraq qeyri-aktiv olaraq qalır. İmza funksiyası
  yalnız xarici audit bağlandıqdan sonra aktivləşdiriləcək, deterministik perf/telemetriya
  qapı və operator sənədləri (bu qısa) torpaq.

## Funksiyaya görə tələb olunan hərəkətlər
| Komanda | Məsuliyyətlər | Artefaktlar | Sahiblər |
|------|------------------|-----------|--------|
| Kripto WG | GM/T spesifikasiyalarının yeniləmələrini izləyin, üçüncü tərəf auditlərini koordinasiya edin, deterministik siyasəti davam etdirin (bir dəfə törəmə, kanonik r∥s). | `sm_program.md`, audit hesabatları, qurğu paketləri. | Kripto WG aparıcı |
| Release Engineering | Gate SM xüsusiyyətləri açıq konfiqurasiyanın arxasındadır, yalnız defolt yoxlamaq üçün qorunur, funksiyaların buraxılış yoxlama siyahısını idarə edir. | `release_dual_track_runbook.md`, buraxılış manifestləri, buraxılış bileti. | Buraxılış TL |
| Əməliyyatlar / SRE | SM aktivləşdirmə yoxlama siyahısını, telemetriya tablosunu (istifadə, səhv dərəcələri), insidentlərə cavab planını təmin edin. | Runbooks, Grafana idarə panelləri, uçuş biletləri. | Ops/SRE |
| Hüquqi Əlaqə | Qovşaqlar materik Çində işləyərkən ÇXR inkişafı/istifadəsi hesabatlarını fayl; hər paket üçün ixrac vəziyyətini nəzərdən keçirin. | Şablonların verilməsi, ixrac hesabatları. | Hüquqi əlaqə |
| SDK Proqramı | Surface SM alqoritmini ardıcıl olaraq dəstəkləyir, deterministik davranışı tətbiq edir, uyğunluq qeydlərini SDK sənədlərinə yayır. | SDK buraxılış qeydləri, sənədlər, CI qapısı. | SDK liderləri |## Sənədləşdirmə və Təqdimat Tələbləri (Çin)
1. **Məhsul sənədləri (开发备案):** Quruda inkişaf üçün məhsul təsvirini təqdim edin,
   mənbənin mövcudluğu bəyanatı, asılılıq siyahısı və deterministik qurma addımları
   azad edilməzdən əvvəl əyalət kriptoqrafiya idarəsi.
2. **Satış/İstifadə sənədləri (销售/使用备案):** SM-i aktivləşdirən qovşaqları idarə edən operatorlar
   istifadə sahəsini, açar idarəetməni və telemetriya kolleksiyasını eyni ilə qeyd edin
   səlahiyyət. Əlaqə məlumatı və hadisəyə cavab SLA-larını təmin edin.
3. **Sertifikatlaşdırma (检测/认证):** Kritik infrastruktur operatorları tələb edə bilər
   akkreditə olunmuş sınaq. Təkrarlana bilən tikinti skriptləri, SBOM və test hesabatlarını təmin edin
   beləliklə, aşağı axın inteqratorları kodu dəyişdirmədən sertifikatlaşdırmanı tamamlaya bilərlər.
4. **Uçotun aparılması:** Arxiv sənədləri və uyğunluq izləyicisində təsdiqlər.
   Yeni bölgələr və ya operatorlar prosesi tamamlayanda `status.md`-i yeniləyin.

## Uyğunluq Yoxlama Siyahısı

### SM funksiyalarını işə salmazdan əvvəl
- [ ] Hüquq məsləhətçisinin hədəf yerləşdirmə bölgələrini nəzərdən keçirdiyini təsdiqləyin.
- [ ] Deterministik qurma təlimatlarını, asılılıq manifestlərini və SBOM-u çəkin
      sənədlərə daxil edilmək üçün ixrac.
- [ ] `crypto.allowed_signing`, `crypto.default_hash` və qəbulu uyğunlaşdırın
      siyasət buraxılış bileti ilə özünü göstərir.
- [ ] SM funksiyasının əhatə dairəsini təsvir edən operator rabitəsi yaratmaq,
      aktivləşdirmə ilkin şərtləri və aradan qaldırılması üçün ehtiyat planları.
- [ ] SM yoxlama/imza sayğaclarını əhatə edən telemetriya tablosunu ixrac edin,
      səhv dərəcələri və mükəmməl ölçülər (`sm3`, `sm4`, sistem zəngi vaxtı).
- [ ] Sahil üçün insidentlərə cavab verən kontaktları və eskalasiya yollarını hazırlayın
      operatorları və Crypto WG.

### Sənədləşdirmə və audit hazırlığı
- [ ] Müvafiq sənədləşdirmə şablonunu seçin (məhsul və satış/istifadə) və doldurun
      təqdim etməzdən əvvəl buraxılış metadatasında.
- [ ] SBOM arxivlərini, deterministik test transkriptlərini və manifest hashlarını əlavə edin.
- [ ] İxrac-nəzarət bəyanatının mövcud olan dəqiq artefaktları əks etdirdiyinə əmin olun
      çatdırılır və istinad edilən lisenziya istisnalarına istinad edir (ENC/TSU).
- [ ] Yoxlayın ki, audit hesabatları, remediasiya izləmə və operator runbooks
      fayl paketindən əlaqələndirilir.
- [ ] İmzalanmış sənədləri, təsdiqləri və yazışmaları uyğunluqda saxlayın
      versiyalı istinadları olan izləyici.

### Təsdiqdən sonrakı əməliyyatlar
- [ ] Sənədləşmə qəbul edildikdən sonra `status.md` və buraxılış biletini yeniləyin.
- [ ] Müşahidə oluna bilən əhatə dairəsinin uyğunluqlarını təsdiqləmək üçün telemetriya yoxlamasını yenidən işə salın
      fayl girişləri.
- [ ] Müraciətlərin, audit hesabatlarının dövri olaraq nəzərdən keçirilməsini (ən azı ildə bir dəfə) planlaşdırın,
      və spesifikasiyalar/tənzimləyici yeniləmələri əldə etmək üçün bəyannamələri ixrac edin.
- [ ] Konfiqurasiya, funksiyanın əhatə dairəsi və ya hostinq zamanı fayl əlavələrini işə salın
      izi maddi cəhətdən dəyişir.## İxrac və Paylama Rəhbərliyi
- Etibarlılığa istinad edən buraxılış qeydlərinə/manifestlərinə qısa ixrac bəyanatını daxil edin
  ENC/TSU-da. Misal:
  > "Bu buraxılış SM2/SM3/SM4 tətbiqlərini ehtiva edir. Paylanma ENC-dən sonra
  > (15 CFR Hissə 742) / Aİ 2021/821 Əlavə 1 5D002. Operatorlar uyğunluğu təmin etməlidirlər
  > yerli ixrac/idxal qanunları ilə.”
- Çin daxilində yerləşdirilən tikintilər üçün, artefaktları dərc etmək üçün Ops ilə koordinasiya edin
  quruda infrastruktur; hallar istisna olmaqla, SM-in effektiv ikili fayllarının transsərhəd ötürülməsindən çəkinin
  müvafiq lisenziyalar mövcuddur.
- Paket anbarlarına əks etdirərkən, hansı artefaktların SM funksiyalarını ehtiva etdiyini qeyd edin
  uyğunluq hesabatını sadələşdirmək.

## Operator Yoxlama Siyahısı
- [ ] Buraxılış profilini təsdiqləyin (`scripts/select_release_profile.py`) + SM xüsusiyyət bayrağı.
- [ ] `sm_program.md` və bu qısa icmalı; qanuni sənədlərin qeydə alınmasını təmin edin.
- [ ] `sm` ilə tərtib etməklə, `crypto.allowed_signing`-i `sm2`-i daxil etmək üçün güncəlləməklə və `crypto.default_hash`-i `sm3-256`-ə keçirməklə SM xüsusiyyətlərini aktivləşdirin.
- [ ] SM sayğaclarını (doğrulama uğursuzluqları,
      sorğuların imzalanması, mükəmməl ölçülər).
- [ ] Manifestləri, hash/imza sübutlarını və sənədləşdirmə təsdiqlərini əlavə edin
      buraxılış bileti.

## Nümunə Sənədləşdirmə Şablonları

Şablonlar asanlıqla daxil olmaq üçün `docs/source/crypto/attachments/` altında yaşayır
fayl paketləri. Müvafiq Markdown şablonunu operator dəyişikliyinə kopyalayın
daxil edin və ya yerli hakimiyyət orqanlarının tələb etdiyi kimi PDF-yə ixrac edin.

- [`sm_product_filing_template.md`](attachments/sm_product_filing_template.md) —
  əyalət məhsul faylı (开发备案) buraxılış metadatasını, alqoritmləri,
  SBOM istinadları və dəstək kontaktları.
- [`sm_sales_usage_filing_template.md`](attachments/sm_sales_usage_filing_template.md) —
  operator satışları/istifadə sənədləri (销售/使用备案) yerləşdirmə izlərini göstərən,
  əsas idarəetmə, telemetriya və insidentlərə cavab prosedurları.
- [`sm_export_statement_template.md`](attachments/sm_export_statement_template.md) —
  buraxılış qeydləri, manifestlər və ya qanuni sənədlər üçün uyğun olan ixrac-nəzarət bəyannaməsi
  ENC/TSU lisenziya istisnalarına əsaslanan yazışmalar.## Standartlar və Sitatlar
- **GM/T 0002-2012 / GB/T 32907-2016** — SM4 blok şifrəsi və AEAD parametrləri (ECB/GCM/CCM). `docs/source/crypto/sm_vectors.md`-də çəkilmiş vektorlara uyğun gəlir.
- **GM/T 0003-2012 / GB/T 32918.x-2016** — SM2 açıq açar kriptoqrafiyası, əyri parametrlər, imza/doğrulama prosesi və Əlavə D məlum cavab testləri.
- **GM/T 0004-2012 / GB/T 32905-2016** — SM3 hash funksiyasının spesifikasiyası və uyğunluq vektorları.
- **RFC 8998** — TLS-də SM2 açar mübadiləsi və imza istifadəsi; OpenSSL/Tongsuo ilə qarşılıqlı əlaqəni sənədləşdirərkən istinad edin.
- **Çin Xalq Respublikasının Kriptoqrafiya Qanunu (2019)**, **Kibertəhlükəsizlik Qanunu (2017)**, **Məlumatların Təhlükəsizliyi Qanunu (2021)** — Yuxarıda qeyd olunan sənədləşmə iş prosesi üçün hüquqi əsas.
- **US EAR Kateqoriya 5 Hissə 2** və **Aİ Qaydaları 2021/821 Əlavə 1 (5D002)** — SM-i aktivləşdirən ikili faylları tənzimləyən ixrac-nəzarət rejimləri.
- **Iroha artefaktları:** `scripts/sm_interop_matrix.sh` və `scripts/sm_openssl_smoke.sh` uyğunluq hesabatlarını imzalamadan əvvəl auditorların təkrar oxuya biləcəyi deterministik qarşılıqlı əlaqə stenoqramlarını təmin edir.

## İstinadlar
- `docs/source/crypto/sm_program.md` — texniki memarlıq və siyasət.
- `docs/source/release_dual_track_runbook.md` — buraxılış qapısı və buraxılış prosesi.
- `docs/source/sora_nexus_operator_onboarding.md` — nümunə operatorun işə qəbul axını.
- GM/T 0002-2012, GM/T 0003-2012, GM/T 0004-2012, GB/T 32918 seriyası, RFC 8998.

Suallar? SM yayım izləyicisi vasitəsilə Crypto WG və ya Hüquqi əlaqə ilə əlaqə saxlayın.