---
lang: ur
direction: rtl
source: docs/source/generated/sumeragi_da_report.md
status: needs-update
translator: manual
generator: scripts/sync_docs_i18n.py
source_hash: fd381ca301bd673a337c07379e755e0cd9c390072fa60aece98b60cddcea351a
source_last_modified: "2025-11-02T04:40:40.073146+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to `docs/source/generated/sumeragi_da_report.md` for current semantics.

# Sumeragi ڈیٹا اویلیبیلٹی رپورٹ

`artifacts/sumeragi-da/20251005T190335Z` سے 3 سمری فائلیں پروسیس کی گئیں۔

## خلاصہ

| منظرنامہ | رنز | پیئرز | پی لوڈ (MiB) | RBC deliver میڈین (ms) | RBC deliver میکس (ms) | Commit میڈین (ms) | Commit میکس (ms) | تھرو پُٹ میڈین (MiB/s) | تھرو پُٹ منیمم (MiB/s) | RBC<=Commit | BG کیو میکس | P2P ڈراپس میکس |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| sumeragi_rbc_da_large_payload_four_peers | 1 | 4 | 10.50 | 3120 | 3120 | 3380 | 3380 | 3.28 | 3.28 | yes | 18 | 0 |
| sumeragi_rbc_da_large_payload_six_peers | 1 | 6 | 10.50 | 3280 | 3280 | 3560 | 3560 | 3.21 | 3.21 | yes | 24 | 0 |
| sumeragi_rbc_recovers_after_peer_restart | 1 | 4 | 10.50 | 3340 | 3340 | 3620 | 3620 | 3.16 | 3.16 | yes | 19 | 0 |

### sumeragi_rbc_da_large_payload_four_peers

- رنز: 1  
- پیئرز: 4  
- پی لوڈ: 11010048 بائٹس (10.50 MiB)  
- مشاہدہ شدہ RBC چنکس: 168  
- READY ووٹس کی تعداد: 4  
- RBC deliver کو Commit کے ذریعے محفوظ کیا گیا: yes  
- RBC deliver اوسط (ms): 3120.00  
- Commit اوسط (ms): 3380.00  
- تھرو پُٹ اوسط (MiB/s): 3.28  
- BG پوسٹ کیو ڈیپتھ میکس/میڈین: 18 / 18  
- P2P کیو ڈراپس میکس/میڈین: 0 / 0  
- فی پیئر بائٹس: 11010048 - 11010048  
- فی پیئر deliver براڈ کاسٹس: 1 - 1  
- فی پیئر READY براڈ کاسٹس: 1 - 1  

| رن | سورس | بلاک | ہائٹ | View | RBC deliver (ms) | Commit (ms) | تھرو پُٹ (MiB/s) | RBC<=Commit | READY | ٹوٹل چنکس | موصول | BG میکس | P2P ڈراپس |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_four_peers.summary.json | 0x8f2fd8804f... | 58 | 2 | 3120 | 3380 | 3.28 | yes | 4 | 168 | 168 | 18 | 0 |

### sumeragi_rbc_da_large_payload_six_peers

- رنز: 1  
- پیئرز: 6  
- پی لوڈ: 11010048 بائٹس (10.50 MiB)  
- مشاہدہ شدہ RBC چنکس: 168  
- READY ووٹس کی تعداد: 5  
- RBC deliver کو Commit کے ذریعے محفوظ کیا گیا: yes  
- RBC deliver اوسط (ms): 3280.00  
- Commit اوسط (ms): 3560.00  
- تھرو پُٹ اوسط (MiB/s): 3.21  
- BG پوسٹ کیو ڈیپتھ میکس/میڈین: 24 / 24  
- P2P کیو ڈراپس میکس/میڈین: 0 / 0  
- فی پیئر بائٹس: 11010048 - 11010048  
- فی پیئر deliver براڈ کاسٹس: 1 - 1  
- فی پیئر READY براڈ کاسٹس: 1 - 1  

| رن | سورس | بلاک | ہائٹ | View | RBC deliver (ms) | Commit (ms) | تھرو پُٹ (MiB/s) | RBC<=Commit | READY | ٹوٹل چنکس | موصول | BG میکس | P2P ڈراپس |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_six_peers.summary.json | 0x71e3cdebcf... | 59 | 3 | 3280 | 3560 | 3.21 | yes | 5 | 168 | 168 | 24 | 0 |

### sumeragi_rbc_recovers_after_peer_restart

- رنز: 1  
- پیئرز: 4  
- پی لوڈ: 11010048 بائٹس (10.50 MiB)  
- مشاہدہ شدہ RBC چنکس: 168  
- READY ووٹس کی تعداد: 4  
- RBC deliver کو Commit کے ذریعے محفوظ کیا گیا: yes  
- RBC deliver اوسط (ms): 3340.00  
- Commit اوسط (ms): 3620.00  
- تھرو پُٹ اوسط (MiB/s): 3.16  
- BG پوسٹ کیو ڈیپتھ میکس/میڈین: 19 / 19  
- P2P کیو ڈراپس میکس/میڈین: 0 / 0  
- فی پیئر بائٹس: 11010048 - 11010048  
- فی پیئر deliver براڈ کاسٹس: 1 - 1  
- فی پیئر READY براڈ کاسٹس: 1 - 1  

| رن | سورس | بلاک | ہائٹ | View | RBC deliver (ms) | Commit (ms) | تھرو پُٹ (MiB/s) | RBC<=Commit | READY | ٹوٹل چنکس | موصول | BG میکس | P2P ڈراپس |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_recovers_after_peer_restart.summary.json | 0xeaf2198957... | 60 | 3 | 3340 | 3620 | 3.16 | yes | 4 | 168 | 168 | 19 | 0 |

</div>
