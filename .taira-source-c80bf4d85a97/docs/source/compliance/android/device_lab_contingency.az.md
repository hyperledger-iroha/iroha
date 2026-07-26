---
lang: az
direction: ltr
source: docs/source/compliance/android/device_lab_contingency.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4016b82d86dc61a9de5e345950d02aeadf26db4cc26777c60db336c57479ba15
source_last_modified: "2025-12-29T18:16:35.923121+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Cihaz Laboratoriyası Fövqəladə Hallar Qeydiyyatı

Android cihazı-laboratoriya fövqəladə hal planının hər aktivləşdirilməsini burada qeyd edin.
Uyğunluq yoxlamaları və gələcək hazırlıq auditləri üçün kifayət qədər təfərrüatı daxil edin.

| Tarix | Tətik | Görülən tədbirlər | Təqiblər | Sahibi |
|------|---------|---------------|------------|-------|
| 2026-02-11 | Pixel8 Pro zolağı kəsildikdən və Pixel8a çatdırılması gecikdikdən sonra tutum 78%-ə düşdü (bax: `android_strongbox_device_matrix.md`). | Pixel7 zolağı əsas CI hədəfinə yüksəldi, borc götürülmüş paylaşılan Pixel6 donanması, pərakəndə pul kisəsi nümunəsi üçün planlaşdırılmış Firebase Test Laboratoriyası tüstü testləri və AND6 planı üzrə xarici StrongBox laboratoriyası işə salındı. | Pixel8 Pro üçün nasaz USB-C mərkəzini dəyişdirin (2026-02-15 tarixinə); Pixel8a gəlişi və yenidən əsas tutum hesabatını təsdiqləyin. | Hardware Laboratoriyası Rəhbəri |
| 2026-02-13 | Pixel8 Pro mərkəzi dəyişdirildi və GalaxyS24 təsdiqləndi, tutumu 85%-ə bərpa etdi. | `pixel8pro-strongbox-a` və `s24-strongbox-a` teqləri ilə ikinci dərəcəli, yenidən aktivləşdirilmiş `android-strongbox-attestation` Buildkite işinə Pixel7 zolağı qaytarıldı, yenilənmiş hazırlıq matrisi + sübut jurnalı. | Pixel8a çatdırılma ETA monitorinqi (hələ də gözləməkdədir); ehtiyat hub inventarını sənədləşdirin. | Hardware Laboratoriyası Rəhbəri |