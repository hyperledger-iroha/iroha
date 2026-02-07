---
lang: az
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/intro.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e797879d1f77c8cfd62fcc67874d584f6bdeee9395faafe52fc33f26ce2e6a21
source_last_modified: "2025-12-29T18:16:35.904811+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SORA Nexus Developer Portalına xoş gəlmisiniz

SORA Nexus developer portalı interaktiv sənədləri, SDK paketlərini birləşdirir
Nexus operatorları və Hyperledger Iroha üçün dərsliklər və API istinadları
ianəçilər. Təcrübəli bələdçilərə müraciət etməklə əsas sənədlər saytını tamamlayır
və spesifikasiyaları birbaşa bu depodan yaratdı.

## Burada nə edə bilərsən

- **Norito öyrənin** – ümumi baxışla başlayın və başa düşmək üçün sürətli başlayın
  serializasiya modeli və bayt kodu aləti.
- **Bootstrap SDKs** – bu gün JavaScript və Rust üçün sürətli başlanğıcları izləyin; piton,
  Reseptlər köçürüldükcə Swift və Android bələdçiləri onlara qoşulacaq.
- **API arayışlarını nəzərdən keçirin** – Torii OpenAPI səhifəsi ən son REST-i təqdim edir
  spesifikasiya və konfiqurasiya cədvəlləri kanonik Markdown ilə əlaqələndirilir
  mənbələr.
- **Yerləşdirmələri hazırlayın** – əməliyyat kitabçaları (telemetri, məskunlaşma, Nexus
  örtüklər) `docs/source/`-dən köçürülür və bu sayta aşağıdakı kimi enəcək
  miqrasiya irəliləyir.

## Cari status

- ✅ Norito və SDK sürətli başlanğıcları üçün canlı səhifələri olan Docusaurus v3 iskele.
- ✅ Torii OpenAPI plagini `npm run sync-openapi`-ə qoşulub.
- ⏳ Qalan bələdçilərin `docs/source/`-dən köçürülməsi.
- ⏳ CI sənədlərinə ilkin baxış quruluşlarının və lintinglərin əlavə edilməsi.

## İştirak etmək

- Yerli inkişaf əmrləri üçün baxın `docs/portal/README.md` (`npm install`,
  `npm run start`, `npm run build`).
- Məzmun köçürmə tapşırıqları `DOCS-*` yol xəritəsi elementləri ilə yanaşı izlənilir.
  Töhfələr qəbul olunur - `docs/source/`-dən port bölmələri və səhifəni əlavə edin
  yan panelə.
- Yaradılmış artefakt əlavə etsəniz (spesifikasiyalar, konfiqurasiya cədvəlləri), quruluşu sənədləşdirin
  əmr edin ki, gələcək töhfəçilər onu asanlıqla yeniləyə bilsinlər.