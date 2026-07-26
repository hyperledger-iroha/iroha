---
lang: az
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e1f2fc637a1fc283e3079ffc22ddff70c6eb1e568a21b951b61491529052c234
source_last_modified: "2025-12-29T18:16:35.906895+00:00"
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
  Redok. `npm run sync-openapi` ilə spesifikasiyanı bərpa edin.
- **Konfiqurasiya cədvəlləri** – Tam parametrlər kataloqu saxlanılır
  `docs/source/references/configuration.md`. Portal avtomatik idxal göndərənə qədər buna istinad edin
  Dəqiq defoltlar və mühitin ləğvi üçün Markdown faylı.

## Tezliklə

- **Torii REST arayışı** – OpenAPI tərifləri vasitəsilə bu bölməyə sinxronizasiya ediləcək
  Boru kəməri işə salındıqdan sonra `docs/portal/scripts/sync-openapi.mjs`.
- **CLI komanda indeksi** – Yaradılmış komanda matrisi (`crates/iroha_cli/src/commands`-i əks etdirən)
  burada kanonik nümunələrlə yanaşı düşəcək.
- **IVM ABI cədvəlləri** – Göstərici tipli və sistem zəngi matrisləri (`crates/ivm/docs` altında saxlanılır)
  sənəd yaratma işi bağlandıqdan sonra portala təqdim ediləcək.

## Bu indeksi cari saxlamaq

Yeni istinad materialı əlavə edildikdə - yaradılan API sənədləri, kodek xüsusiyyətləri, konfiqurasiya matrisləri - yerləşdirin
səhifəni `docs/portal/docs/reference/` altında açın və yuxarıda əlaqələndirin. Səhifə avtomatik yaradılıbsa, qeyd edin
scripti sinxronlaşdırın ki, töhfə verənlər onu necə yeniləməyi bilsinlər. Bu, istinad ağacına qədər faydalı saxlayır
tam avtomatik yaradılan naviqasiya torpaqları.