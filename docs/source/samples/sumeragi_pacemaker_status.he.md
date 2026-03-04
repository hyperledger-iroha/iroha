<!-- Hebrew translation of docs/source/samples/sumeragi_pacemaker_status.md -->

---
lang: he
direction: rtl
source: docs/source/samples/sumeragi_pacemaker_status.md
status: complete
translator: manual
---

<div dir="rtl">

# ‏Sumeragi — מצב פייסמייקר (Torii)

נקודת קצה
- `GET /v1/sumeragi/pacemaker`

תגובה (דוגמה)
```json
{
  "backoff_ms": 0,
  "rtt_floor_ms": 0,
  "backoff_multiplier": 0,
  "rtt_floor_multiplier": 0,
  "max_backoff_ms": 0,
  "jitter_ms": 0,
  "jitter_frac_permille": 0
}
```

הערות
- נקודת הקצה זמינה רק כאשר הבינארי קומפל עם טלמטריה פעילה.
- מספקת את חלון הבק-אוף הנוכחי ואת פרמטרי התזמון של הפייסמייקר.
- נועדה לשקיפות תפעולית; הערכים הם מדדים תלוּי-מימוש.

</div>
