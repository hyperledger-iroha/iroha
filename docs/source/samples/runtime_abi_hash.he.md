<!-- Hebrew translation of docs/source/samples/runtime_abi_hash.md -->

---
lang: he
direction: rtl
source: docs/source/samples/runtime_abi_hash.md
status: complete
translator: manual
---

<div dir="rtl">

# ‏ABI בזמן ריצה — גיבוב קנוני (Torii)

נקודת קצה
- `GET /v1/runtime/abi/hash`

תגובה (מהדורה ראשונה; מדיניות יחידה V1)
```json
{
  "policy": "V1",
  "abi_hash_hex": "e2ca8bbdaec17330a417a248faacb6a01931245591e28e44bf3f83da98dde01f"
}
```

הערות
- הגיבוב מייצג דיג'סט קנוני של משטח קריאות המערכת המותר במדיניות.
- חוזים מטמיעים את הערך בשדה `abi_hash` במניפסט כדי להבטיח התאמה ל־ABI של הצומת.

</div>
