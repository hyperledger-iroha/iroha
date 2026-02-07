---
lang: az
direction: ltr
source: docs/references/configuration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cff283a14bf65f185f81539f8fbcd78ddcc6447c5e9045e1b46493051febaf6a
source_last_modified: "2025-12-29T18:16:35.913045+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sürətlənmə

`[accel]` bölməsi IVM və köməkçilər üçün əlavə aparat sürətləndirilməsinə nəzarət edir. Hamısı
sürətləndirilmiş yollar deterministik CPU ehtiyatlarına malikdir; backend qızıl uğursuz olarsa
iş vaxtında özünü sınamaq avtomatik olaraq söndürülür və icra CPU-da davam edir.

- `enable_cuda` (defolt: doğru) – Tərtib edildikdə və mövcud olduqda CUDA-dan istifadə edin.
- `enable_metal` (defolt: doğru) – Mövcud olduqda macOS-da Metaldan istifadə edin.
- `max_gpus` (defolt: 0) – Başlamaq üçün maksimum GPU; `0` avtomatik/qapaqsız deməkdir.
- `merkle_min_leaves_gpu` (defolt: 8192) – Merkle-ni boşaltmaq üçün minimum yarpaqlar
  GPU-ya yarpaq hashing. Yalnız qeyri-adi sürətli GPU-lar üçün aşağı salın.
- Qabaqcıl (isteğe bağlı; adətən həssas defoltları miras alır):
  - `merkle_min_leaves_metal` (defolt: miras `merkle_min_leaves_gpu`).
  - `merkle_min_leaves_cuda` (defolt: miras `merkle_min_leaves_gpu`).
  - `prefer_cpu_sha2_max_leaves_aarch64` (defolt: 32768) – SHA2 ilə ARMv8-də bu qədər yarpaq qədər CPU SHA‑2-yə üstünlük verin.
  - `prefer_cpu_sha2_max_leaves_x86` (defolt: 32768) – x86/x86_64-də bu qədər yarpaq qədər CPU SHA‑NI-yə üstünlük verin.

Qeydlər
- Əvvəlcə determinizm: sürətlənmə heç vaxt müşahidə edilə bilən nəticələri dəyişdirmir; arxa uçlar
  init-də qızıl testləri həyata keçirin və uyğunsuzluqlar aşkar edildikdə skalyar/SIMD-ə qayıdın.
- `iroha_config` vasitəsilə konfiqurasiya edin; istehsalda mühit dəyişənlərindən qaçın.