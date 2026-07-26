---
lang: az
direction: ltr
source: docs/automation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c56bacde8ee42c2427d06038a3a6ca65035d4055c42f6e5ded7e54b33c1fe921
source_last_modified: "2025-12-29T18:16:35.060432+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Sənədləşmənin Avtomatlaşdırılması Bazaları

Bu kataloq kimi elementləri yol xəritəsini tərtib edən avtomatlaşdırma səthlərini tutur
AND5/AND6 (Android Developer Təcrübəsi + Buraxılış Hazırlığı) və DA-1
(Data-Availability təhdid modelinin avtomatlaşdırılması) çağırdıqları zaman istinad edin
yoxlanıla bilən sənəd sübutları. Əmr istinadlarının və gözlənilənlərin hazırlanması
ağacdakı artefaktlar hətta uyğunluq nəzərdən keçirmək üçün ilkin şərtləri saxlayır
CI boru kəmərləri və ya idarə panelləri oflayn olduqda.

## Directory Layout

| Yol | Məqsəd |
|------|---------|
| `docs/automation/android/` | Android sənədləri və lokalizasiya avtomatlaşdırmasının ilkin göstəriciləri (AND5), o cümlədən i18n stub sinxronizasiya jurnalları, paritet xülasələri və AND6 imzalanmadan əvvəl tələb olunan SDK nəşr sübutları. |
| `docs/automation/da/` | `cargo xtask da-threat-model-report` tərəfindən istinad edilən Data-Mövcudluq təhlükəsi modeli avtomatlaşdırma nəticələri və gecə sənədləri yenilənir. |

Hər bir alt kataloq dəlilləri yaradan əmrləri sənədləşdirir
yoxlamağı gözlədiyimiz fayl düzeni (adətən JSON xülasələri, işləmə qeydləri və ya
təzahür edir). Komandalar hər dəfə yeni artefaktları müvafiq qovluğun altına atırlar
avtomatlaşdırma işi dərc edilmiş sənədləri əhəmiyyətli dərəcədə dəyişir, sonra öhdəliyə keçid verir
müvafiq status/yol xəritəsi girişindən.

## İstifadəsi

1. **Avtomatlaşdırmanı** alt kataloqda təsvir edilən əmrlərdən istifadə edərək işə salın
   README (məsələn, `ci/check_android_fixtures.sh` və ya
   `cargo xtask da-threat-model-report`).
2. **Nəticədə JSON/log artefaktlarını** `artifacts/…`-dən kopyalayın
   ISO-8601 vaxt damğası ilə uyğun gələn `docs/automation/<program>/…` qovluğu
   auditorların sübutları idarəetmə protokolları ilə əlaqələndirə bilməsi üçün fayl adı.
3. Yol xəritəsini bağlayarkən **`status.md`/`roadmap.md`-də ** öhdəliyə istinad edin**
   gate beləliklə, rəyçilər həmin qərar üçün istifadə olunan avtomatlaşdırmanın əsasını təsdiq edə bilsinlər.
4. **Faylları yüngül saxlayın**. Gözlənti strukturlaşdırılmış metadatadır,
   manifestlər və ya xülasələr - toplu ikili bloblar deyil. Daha böyük zibilxanalar qalmalıdır
   burada qeydə alınmış imzalanmış arayışla obyekt saxlama.

Bu avtomatlaşdırma qeydlərini mərkəzləşdirməklə biz “sənədlər/avtomatlaşdırma əsaslarını” blokdan çıxarırıq
audit üçün əlçatandır” ilkin şərti AND6-nın çağırdığı və DA təhlükəsi verdiyidir
model axını gecə hesabatları və əl ilə yoxlanış üçün deterministik bir evdir.