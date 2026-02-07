---
lang: mn
direction: ltr
source: docs/references/configuration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cff283a14bf65f185f81539f8fbcd78ddcc6447c5e9045e1b46493051febaf6a
source_last_modified: "2025-12-29T18:16:35.913045+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Хурдатгал

`[accel]` хэсэг нь IVM болон туслахуудад зориулсан нэмэлт тоног төхөөрөмжийн хурдатгалыг хянадаг. Бүгд
хурдасгасан замууд нь тодорхойлогч CPU-ийн нөөцтэй байдаг; хэрэв backend амжилтгүй болбол алтан
Ажиллаж байх үед өөрийгөө шалгах нь автоматаар идэвхгүй болж, гүйцэтгэл нь CPU дээр үргэлжилнэ.

- `enable_cuda` (анхдагч: үнэн) – CUDA-г эмхэтгэж, ашиглах боломжтой үед ашиглана.
- `enable_metal` (өгөгдмөл: үнэн) – Боломжтой үед macOS дээр Metal ашиглана уу.
- `max_gpus` (өгөгдмөл: 0) – эхлүүлэх хамгийн их GPU; `0` нь автомат/хязгааргүй гэсэн үг.
- `merkle_min_leaves_gpu` (өгөгдмөл: 8192) – Merkle-г буулгах хамгийн бага хуудас
  GPU руу навч хэшлэх. Зөвхөн ер бусын хурдан GPU-д зориулж бага.
- Нарийвчилсан (заавал биш; ихэвчлэн мэдрэмжтэй өгөгдмөлүүдийг өвлөн авдаг):
  - `merkle_min_leaves_metal` (өгөгдмөл: `merkle_min_leaves_gpu` өвлөнө).
  - `merkle_min_leaves_cuda` (өгөгдмөл: `merkle_min_leaves_gpu` өвлөнө).
  - `prefer_cpu_sha2_max_leaves_aarch64` (өгөгдмөл: 32768) – SHA2-тэй ARMv8 дээр ийм олон навч хүртэл CPU SHA‑2-г илүүд үз.
  - `prefer_cpu_sha2_max_leaves_x86` (өгөгдмөл: 32768) – x86/x86_64 дээр ийм олон навч хүртэл CPU SHA‑NI-г илүүд үзнэ үү.

Тэмдэглэл
- Эхлээд детерминизм: хурдатгал нь ажиглагдах үр дүнг хэзээ ч өөрчлөхгүй; backends
  init дээр алтан тестийг ажиллуулж, таарахгүй байдал илэрсэн үед скаляр/SIMD руу буцна.
- `iroha_config`-ээр тохируулах; үйлдвэрлэлд хүрээлэн буй орчны хувьсагчаас зайлсхийх.