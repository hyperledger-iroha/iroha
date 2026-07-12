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
  "abi_hash_hex": "e7ed1a6ebb7606d41c25f872546994499b56e7b72091ba52e8223e6de4926ad5"
}
```

הערות
- הגיבוב מייצג דיג'סט קנוני של משטח קריאות המערכת המותר במדיניות.
- חוזים מטמיעים את הערך בשדה `abi_hash` במניפסט כדי להבטיח התאמה ל־ABI של הצומת.

</div>
