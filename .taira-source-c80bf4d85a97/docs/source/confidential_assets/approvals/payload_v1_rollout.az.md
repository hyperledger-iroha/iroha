---
lang: az
direction: ltr
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T14:35:37.742189+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Faydalı yük v1 buraxılışının təsdiqi (SDK Şurası, 28-04-2026).
//!
//! `roadmap.md:M1` tərəfindən tələb olunan SDK Şurasının qərar yaddaşını çəkir, beləliklə
//! şifrlənmiş faydalı yük v1 buraxılışı yoxlanıla bilən qeydə malikdir (çatdırılan M1.4).

# Faydalı Yük v1 Təqdimat Qərarı (28-04-2026)

- **Sədr:** SDK Şurasının rəhbəri (M. Takemiya)
- **Səs verən üzvlər:** Swift Lider, CLI Maintainer, Confidential Assets TL, DevRel WG
- **Müşahidəçilər:** Proqram Mgmt, Telemetriya Əməliyyatları

## Daxiletmələr nəzərdən keçirildi

1. **Swift bağlamalar və təqdim edənlər** — `ShieldRequest`/`UnshieldRequest`, asinxron göndərənlər və Tx qurucu köməkçiləri paritet testləri və sənədlər.【IrohaSwift/Mənbələr/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/Mənbələr/IrohaSwift/TxBuilder.swift:1006】
2. **CLI erqonomikası** — `iroha app zk envelope` köməkçisi yol xəritəsinin erqonomikası tələbinə uyğunlaşdırılmış kodlaşdırma/təftiş iş axınları və uğursuzluq diaqnostikasını əhatə edir.【crates/iroha_cli/src/zk.rs:1256】
3. **Deterministik qurğular və paritet dəstləri** — Norito bayt/səhv səthlərini saxlamaq üçün paylaşılan qurğu + Rust/Swift doğrulaması uyğunlaşdırılmışdır.【qurğular/məxfi/şifrələnmiş_payload_v1.json:1】【crates/iroha_data_model/tests/confidential_en crypted_payload_vectors.rs:1】【IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift:73】

## Qərar

- **SDK və CLI üçün faydalı yük v1 buraxılışını təsdiq edin**, Swift pul kisələrinə sifarişli santexnika olmadan məxfi zərflər yaratmağa imkan verin.
- **Şərtlər:** 
  - Paritet qurğularını CI sürüşmə siqnalları altında saxlayın (`scripts/check_norito_bindings_sync.py` ilə bağlıdır).
  - Əməliyyat kitabçasını `docs/source/confidential_assets.md`-də sənədləşdirin (artıq Swift SDK PR vasitəsilə yenilənib).
  - İstənilən istehsal bayraqlarını çevirməzdən əvvəl kalibrləmə + telemetriya sübutlarını qeyd edin (M2 altında izlənilir).

## Fəaliyyət elementləri

| Sahibi | Maddə | Vaxtı |
|-------|------|-----|
| Swift Lead | GA mövcudluğunu elan edin + README fraqmentləri | 2026-05-01 |
| CLI Maintainer | `iroha app zk envelope --from-fixture` köməkçi əlavə edin (isteğe bağlı) | Arxa plan (bloklamayan) |
| DevRel WG | Pul kisəsinin sürətli başlanğıclarını faydalı yük v1 təlimatları ilə yeniləyin | 2026-05-05 |

> **Qeyd:** Bu memo `roadmap.md:2426`-də müvəqqəti “şuranın təsdiqini gözləyir” çağırışını əvəz edir və M1.4 izləyici elementini təmin edir. Növbəti əməliyyat elementləri bağlandıqda `status.md`-i yeniləyin.