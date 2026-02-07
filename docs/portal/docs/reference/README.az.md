---
lang: az
direction: ltr
source: docs/portal/docs/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b3b2becfdbab1446f8f230ace905de306e1e89147f5a5e578d784be97445d74d
source_last_modified: "2025-12-29T18:16:35.156432+00:00"
translation_last_reviewed: 2026-02-07
title: Reference Index
slug: /reference
translator: machine-google-reviewed
---

Bu bölmə Iroha üçün "xüsusiyyət kimi oxuyun" materialını birləşdirir. Bu səhifələr belə sabit qalır
təlimatlar və dərsliklər inkişaf edir.

## Bu gün mövcuddur

- **Norito kodek icmalı** – `reference/norito-codec.md` birbaşa səlahiyyətli ilə əlaqələndirir
  Portal cədvəli doldurularkən `norito.md` spesifikasiyası.
- **Torii OpenAPI** – `/reference/torii-openapi` istifadə edərək ən son Torii REST spesifikasiyasını təqdim edir
  Redok. `npm run sync-openapi -- --version=current --latest` ilə spesifikasiyanı bərpa edin (əlavə edin
  Snapşotu əlavə tarixi versiyalara köçürmək üçün `--mirror=<label>`).
- **Konfiqurasiya cədvəlləri** – Tam parametrlər kataloqu saxlanılır
  `docs/source/references/configuration.md`. Portal avtomatik idxal göndərənə qədər buna istinad edin
  Dəqiq defoltlar və mühitin ləğvi üçün Markdown faylı.
- **Sənədlərin versiyasının yaradılması** – Navbar versiyasının açılan siyahısı ilə yaradılmış dondurulmuş snapşotları ifşa edir.
  `npm run docs:version -- <label>`, relizlər üzrə təlimatları müqayisə etməyi asanlaşdırır.

## Tezliklə

- **Torii REST arayışı** – OpenAPI tərifləri vasitəsilə bu bölməyə sinxronizasiya ediləcək
  Boru kəməri işə salındıqdan sonra `docs/portal/scripts/sync-openapi.mjs`.
- **CLI komanda indeksi** – Yaradılmış komanda matrisi (`crates/iroha_cli/src/commands`-i əks etdirən)
  burada kanonik nümunələrlə yanaşı düşəcək.
- **IVM ABI cədvəlləri** – Göstərici tipli və sistem matrisləri (`crates/ivm/docs` altında saxlanılır)
  sənəd yaratma işi bağlandıqdan sonra portala təqdim ediləcək.

## Bu indeksi cari saxlamaq

Yeni istinad materialı əlavə edildikdə - yaradılan API sənədləri, kodek xüsusiyyətləri, konfiqurasiya matrisləri - yerləşdirin
`docs/portal/docs/reference/` altındakı səhifəni açın və yuxarıda əlaqələndirin. Səhifə avtomatik yaradılıbsa, qeyd edin
scripti sinxronlaşdırın ki, töhfə verənlər onu necə yeniləməyi bilsinlər. Bu, istinad ağacına qədər faydalı saxlayır
tam avtomatik yaradılan naviqasiya torpaqları.