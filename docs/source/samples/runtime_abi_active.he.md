<!-- Hebrew translation of docs/source/samples/runtime_abi_active.md -->

---
lang: he
direction: rtl
source: docs/source/samples/runtime_abi_active.md
status: complete
translator: manual
---

<div dir="rtl">

# ‏ABI בזמן ריצה — גרסאות פעילות (Torii)

נקודת קצה
- `GET /v2/runtime/abi/active`

תגובה (מהדורה ראשונה; ABI יחיד)
```json
{
  "active_versions": [1],
  "default_compile_target": 1
}
```

הערות
- הרשימה מוחזרת במיון עולה. `default_compile_target` הוא המספר הגבוה ביותר מבין הגרסאות הפעילות, והוא 1 במהדורה הראשונה.

</div>
