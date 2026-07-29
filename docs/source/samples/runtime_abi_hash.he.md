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
  "abi_hash_hex": "17c61cb3a6ee164213afe410169161def1d7025b84f0b9e385a93619a862513b"
}
```

הערות
- הגיבוב מייצג דיג'סט קנוני של משטח קריאות המערכת המותר במדיניות.
- חוזים מטמיעים את הערך בשדה `abi_hash` במניפסט כדי להבטיח התאמה ל־ABI של הצומת.

</div>
