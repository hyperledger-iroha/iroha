---
lang: az
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/06-verification-report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf489427d0eba2beebfdefc44092730c3963cbd77e83669853f4e9681ac9fd2d
source_last_modified: "2025-12-29T18:16:35.092552+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Operator Doğrulama Hesabatı (Faza T0)

- Operatorun adı: ______________________
- Rele deskriptorunun ID-si: ______________________
- Göndərmə tarixi (UTC): ___________________
- Əlaqə e-poçtu / matrisi: ___________________

### Yoxlama siyahısı xülasəsi

| Maddə | Tamamlandı (Y/N) | Qeydlər |
|------|-----------------|-------|
| Avadanlıq və şəbəkə təsdiqləndi | | |
| Uyğunluq bloku tətbiq | | |
| Qəbul zərfi təsdiqləndi | | |
| Mühafizə fırlanma tüstü testi | | |
| Telemetriya kazınmış və tablosuna canlı | | |
| Brownout qazma icra edildi | | |
| Hədəf daxilində PoW bilet uğuru | | |

### Metrik Snapshot

- PQ nisbəti (`sorafs_orchestrator_pq_ratio`): ________
- Son 24 saatda endirmə sayı: ________
- Orta dövrə RTT (p95): ________ ms
- PoW orta həll vaxtı: ________ ms

### Qoşmalar

Zəhmət olmasa əlavə edin:

1. Relay dəstəyi paketi hash (`sha256`): __________________________
2. Dashboard ekran görüntüləri (PQ nisbəti, dövrə müvəffəqiyyəti, PoW histoqramı).
3. İmzalanmış qazma dəsti (`drills-signed.json` + imzalayan açıq açar hex və əlavələr).
4. SNNet-10 ölçü hesabatı (`cargo xtask soranet-testnet-metrics --input <snapshot> --out metrics-report.json`).

### Operator İmzası

Yuxarıdakı məlumatların doğru olduğunu və bütün tələb olunan addımların yerinə yetirildiyini təsdiq edirəm
tamamlandı.

İmza: _______________________ Tarix: ___________________