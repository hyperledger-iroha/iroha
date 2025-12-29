<!-- Hebrew translation of docs/source/generated/sumeragi_da_report.md -->

---
lang: he
direction: rtl
source: docs/source/generated/sumeragi_da_report.md
status: needs-update
translator: manual
---

<div dir="rtl">

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to `docs/source/generated/sumeragi_da_report.md` for current semantics.

# דוח זמינות נתונים של Sumeragi

עובדו 3 קובצי סיכום מתוך `artifacts/sumeragi-da/20251005T190335Z`.

## סיכום

| תרחיש | ריצות | מספר צמתים | גודל מטען (MiB) | זמן RBC deliver חציון (ms) | זמן RBC deliver מקסימלי (ms) | זמן Commit חציון (ms) | זמן Commit מקסימלי (ms) | חציון קצב העברה (MiB/s) | מינימום קצב העברה (MiB/s) | RBC≤Commit | עומק תור BG מקס' | כמות נפילות P2P מקס' |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| sumeragi_rbc_da_large_payload_four_peers | 1 | 4 | 10.50 | 3120 | 3120 | 3380 | 3380 | 3.28 | 3.28 | yes | 18 | 0 |
| sumeragi_rbc_da_large_payload_six_peers | 1 | 6 | 10.50 | 3280 | 3280 | 3560 | 3560 | 3.21 | 3.21 | yes | 24 | 0 |
| sumeragi_rbc_recovers_after_peer_restart | 1 | 4 | 10.50 | 3340 | 3340 | 3620 | 3620 | 3.16 | 3.16 | yes | 19 | 0 |

### sumeragi_rbc_da_large_payload_four_peers

- מספר ריצות: 1
- מספר צמתים: 4
- מטען: ‎11010048‎ בתים (10.50 MiB)
- מספר מקטעי RBC שנצפו: 168
- ספירת הצבעות READY: 4
- RBC deliver מאובטח ע"י Commit: yes
- ממוצע RBC deliver (ms): ‎3120.00‎
- ממוצע Commit (ms): ‎3380.00‎
- ממוצע קצב העברה (MiB/s): ‎3.28‎
- עומק תור BG פוסט מקס'/חציון: ‎18 / 18‎
- נפילות תור P2P מקס'/חציון: ‎0 / 0‎
- בתים לצומת: ‎11010048 - 11010048‎
- שידורי deliver לצומת: ‎1 - 1‎
- שידורי READY לצומת: ‎1 - 1‎

| ריצה | מקור | בלוק | גובה | View | RBC deliver (ms) | Commit (ms) | קצב העברה (MiB/s) | RBC≤Commit | READY | מספר מקטעים | התקבל | עומק BG מקס' | נפילות P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_four_peers.summary.json | 0x8f2fd8804f... | 58 | 2 | 3120 | 3380 | 3.28 | yes | 4 | 168 | 168 | 18 | 0 |

### sumeragi_rbc_da_large_payload_six_peers

- מספר ריצות: 1
- מספר צמתים: 6
- מטען: ‎11010048‎ בתים (10.50 MiB)
- מספר מקטעי RBC שנצפו: 168
- ספירת הצבעות READY: 5
- RBC deliver מאובטח ע"י Commit: yes
- ממוצע RBC deliver (ms): ‎3280.00‎
- ממוצע Commit (ms): ‎3560.00‎
- ממוצע קצב העברה (MiB/s): ‎3.21‎
- עומק תור BG פוסט מקס'/חציון: ‎24 / 24‎
- נפילות תור P2P מקס'/חציון: ‎0 / 0‎
- בתים לצומת: ‎11010048 - 11010048‎
- שידורי deliver לצומת: ‎1 - 1‎
- שידורי READY לצומת: ‎1 - 1‎

| ריצה | מקור | בלוק | גובה | View | RBC deliver (ms) | Commit (ms) | קצב העברה (MiB/s) | RBC≤Commit | READY | מספר מקטעים | התקבל | עומק BG מקס' | נפילות P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_six_peers.summary.json | 0x71e3cdebcf... | 59 | 3 | 3280 | 3560 | 3.21 | yes | 5 | 168 | 168 | 24 | 0 |

### sumeragi_rbc_recovers_after_peer_restart

- מספר ריצות: 1
- מספר צמתים: 4
- מטען: ‎11010048‎ בתים (10.50 MiB)
- מספר מקטעי RBC שנצפו: 168
- ספירת הצבעות READY: 4
- RBC deliver מאובטח ע"י Commit: yes
- ממוצע RBC deliver (ms): ‎3340.00‎
- ממוצע Commit (ms): ‎3620.00‎
- ממוצע קצב העברה (MiB/s): ‎3.16‎
- עומק תור BG פוסט מקס'/חציון: ‎19 / 19‎
- נפילות תור P2P מקס'/חציון: ‎0 / 0‎
- בתים לצומת: ‎11010048 - 11010048‎
- שידורי deliver לצומת: ‎1 - 1‎
- שידורי READY לצומת: ‎1 - 1‎

| ריצה | מקור | בלוק | גובה | View | RBC deliver (ms) | Commit (ms) | קצב העברה (MiB/s) | RBC≤Commit | READY | מספר מקטעים | התקבל | עומק BG מקס' | נפילות P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_recovers_after_peer_restart.summary.json | 0xeaf2198957... | 60 | 3 | 3340 | 3620 | 3.16 | yes | 4 | 168 | 168 | 19 | 0 |

</div>
