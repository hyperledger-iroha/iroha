---
lang: ar
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

# تقرير توافر البيانات في Sumeragi

تمت معالجة 3 ملفات ملخّص من المسار `artifacts/sumeragi-da/20251005T190335Z`.

## الملخّص

| السيناريو | عدد التشغيلات | عدد الأقران | حمولة البيانات (MiB) | زمن RBC deliver الوسيط (ms) | زمن RBC deliver الأقصى (ms) | زمن Commit الوسيط (ms) | زمن Commit الأقصى (ms) | معدل النقل الوسيط (MiB/s) | معدل النقل الأدنى (MiB/s) | RBC<=Commit | أقصى عمق لقائمة BG | أقصى عدد إسقاطات P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| sumeragi_rbc_da_large_payload_four_peers | 1 | 4 | 10.50 | 3120 | 3120 | 3380 | 3380 | 3.28 | 3.28 | yes | 18 | 0 |
| sumeragi_rbc_da_large_payload_six_peers | 1 | 6 | 10.50 | 3280 | 3280 | 3560 | 3560 | 3.21 | 3.21 | yes | 24 | 0 |
| sumeragi_rbc_recovers_after_peer_restart | 1 | 4 | 10.50 | 3340 | 3340 | 3620 | 3620 | 3.16 | 3.16 | yes | 19 | 0 |

### sumeragi_rbc_da_large_payload_four_peers

- عدد التشغيلات: 1  
- عدد الأقران: 4  
- الحمولة: ‎11010048‎ بايت (10.50 MiB)  
- عدد مقاطع RBC المرصودة: 168  
- عدد أصوات READY: 4  
- تم تأمين التسليم (RBC deliver) عبر Commit: yes  
- متوسط زمن RBC deliver (ms): ‎3120.00‎  
- متوسط زمن Commit (ms): ‎3380.00‎  
- متوسط معدل النقل (MiB/s): ‎3.28‎  
- أقصى/وسيط عمق لقائمة BG post: ‎18 / 18‎  
- أقصى/وسيط عدد إسقاطات قائمة P2P: ‎0 / 0‎  
- عدد البايتات لكل قرين: ‎11010048 - 11010048‎  
- عدد بثّات deliver لكل قرين: ‎1 - 1‎  
- عدد بثّات READY لكل قرين: ‎1 - 1‎  

| التشغيل | المصدر | الكتلة | الارتفاع | View | RBC deliver (ms) | Commit (ms) | معدل النقل (MiB/s) | RBC<=Commit | READY | إجمالي المقاطع | المستلَم | أقصى BG | إسقاطات P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_four_peers.summary.json | 0x8f2fd8804f... | 58 | 2 | 3120 | 3380 | 3.28 | yes | 4 | 168 | 168 | 18 | 0 |

### sumeragi_rbc_da_large_payload_six_peers

- عدد التشغيلات: 1  
- عدد الأقران: 6  
- الحمولة: ‎11010048‎ بايت (10.50 MiB)  
- عدد مقاطع RBC المرصودة: 168  
- عدد أصوات READY: 5  
- تم تأمين التسليم عبر Commit: yes  
- متوسط زمن RBC deliver (ms): ‎3280.00‎  
- متوسط زمن Commit (ms): ‎3560.00‎  
- متوسط معدل النقل (MiB/s): ‎3.21‎  
- أقصى/وسيط عمق لقائمة BG post: ‎24 / 24‎  
- أقصى/وسيط عدد إسقاطات قائمة P2P: ‎0 / 0‎  
- عدد البايتات لكل قرين: ‎11010048 - 11010048‎  
- عدد بثّات deliver لكل قرين: ‎1 - 1‎  
- عدد بثّات READY لكل قرين: ‎1 - 1‎  

| التشغيل | المصدر | الكتلة | الارتفاع | View | RBC deliver (ms) | Commit (ms) | معدل النقل (MiB/s) | RBC<=Commit | READY | إجمالي المقاطع | المستلَم | أقصى BG | إسقاطات P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_six_peers.summary.json | 0x71e3cdebcf... | 59 | 3 | 3280 | 3560 | 3.21 | yes | 5 | 168 | 168 | 24 | 0 |

### sumeragi_rbc_recovers_after_peer_restart

- عدد التشغيلات: 1  
- عدد الأقران: 4  
- الحمولة: ‎11010048‎ بايت (10.50 MiB)  
- عدد مقاطع RBC المرصودة: 168  
- عدد أصوات READY: 4  
- تم تأمين التسليم عبر Commit: yes  
- متوسط زمن RBC deliver (ms): ‎3340.00‎  
- متوسط زمن Commit (ms): ‎3620.00‎  
- متوسط معدل النقل (MiB/s): ‎3.16‎  
- أقصى/وسيط عمق لقائمة BG post: ‎19 / 19‎  
- أقصى/وسيط عدد إسقاطات قائمة P2P: ‎0 / 0‎  
- عدد البايتات لكل قرين: ‎11010048 - 11010048‎  
- عدد بثّات deliver لكل قرين: ‎1 - 1‎  
- عدد بثّات READY لكل قرين: ‎1 - 1‎  

| التشغيل | المصدر | الكتلة | الارتفاع | View | RBC deliver (ms) | Commit (ms) | معدل النقل (MiB/s) | RBC<=Commit | READY | إجمالي المقاطع | المستلَم | أقصى BG | إسقاطات P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_recovers_after_peer_restart.summary.json | 0xeaf2198957... | 60 | 3 | 3340 | 3620 | 3.16 | yes | 4 | 168 | 168 | 19 | 0 |

</div>
