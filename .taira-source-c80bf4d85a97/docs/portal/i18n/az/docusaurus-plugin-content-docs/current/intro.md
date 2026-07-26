---
lang: az
direction: ltr
source: docs/portal/docs/intro.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SORA Nexus Developer Portalına xoş gəlmisiniz

SORA Nexus tərtibatçı portalı interaktiv sənədləri, SDK paketlərini birləşdirir
Nexus operatorları və Hyperledger Iroha üçün dərsliklər və API istinadları
ianəçilər. Təcrübəli bələdçilərə müraciət etməklə əsas sənədlər saytını tamamlayır
və spesifikasiyaları birbaşa bu depodan yaratdı. Açılış səhifəsi indi daşıyır
temalı Norito/SoraFS giriş nöqtələri, imzalanmış OpenAPI anlıq görüntülər və xüsusi
Norito Yayım arayışı, beləliklə, ianəçilər axın nəzarət müstəvisini tapa bilsinlər
kök spesifikasiyasını qazmadan müqavilə bağlayın.

## Burada nə edə bilərsən

- **Norito öyrənin** – ümumi baxışla başlayın və başa düşmək üçün sürətli başlayın
  serializasiya modeli və bayt kodu aləti.
- **Bootstrap SDKs** – bu gün JavaScript və Rust üçün sürətli başlanğıcları izləyin; piton,
  Reseptlər köçürüldükcə Swift və Android bələdçiləri onlara qoşulacaq.
- **API arayışlarını nəzərdən keçirin** – Torii OpenAPI səhifəsi ən son REST-i təqdim edir
  spesifikasiya və konfiqurasiya cədvəlləri kanonik Markdown ilə əlaqələndirilir
  mənbələr.
- **Yerləşdirmələri hazırlayın** – əməliyyat kitabçaları (telemetri, məskunlaşma, Nexus
  örtüklər) `docs/source/`-dən daşınır və bu sayta aşağıdakı kimi enəcək
  miqrasiya irəliləyir.

## Cari status

- ✅ Yenilənmiş tipoqrafiya ilə temalı Docusaurus v3 enişi, gradient əsaslı
  qəhrəman/kartlar və Norito Yayım xülasəsini ehtiva edən resurs plitələri.
- ✅ Torii OpenAPI plaqini, imzalanmış şəkil ilə `npm run sync-openapi`-ə qoşulmuşdur
  `buildSecurityHeaders` tərəfindən tətbiq edilən çeklər və CSP mühafizəçiləri.
- ✅ Önizləmə və əhatə dairəsini CI-də yoxlayın (`docs-portal-preview.yml` +
  `scripts/portal-probe.mjs`), indi axın sənədini bağlayır, SoraFS sürətli başlanğıcları,
  və artefaktlar dərc edilməzdən əvvəl istinad siyahıları.
- ✅ Norito, SoraFS və SDK sürətli başlanğıcları və istinad bölmələri burada canlıdır
  yan panel; `docs/source/`-dən yeni idxallar (axın, orkestrasiya, runbooks)
  onlar müəllif kimi burada torpaq.

## İştirak etmək

- Yerli inkişaf əmrləri üçün baxın `docs/portal/README.md` (`npm install`,
  `npm run start`, `npm run build`).
- Məzmun köçürmə tapşırıqları `DOCS-*` yol xəritəsi elementləri ilə yanaşı izlənilir.
  Töhfələr qəbul olunur - `docs/source/`-dən port bölmələri və səhifəni əlavə edin
  yan panelə.
- Yaradılmış artefakt əlavə etsəniz (xüsusiyyətlər, konfiqurasiya cədvəlləri), quruluşu sənədləşdirin
  əmr edin ki, gələcək töhfəçilər onu asanlıqla yeniləyə bilsinlər.