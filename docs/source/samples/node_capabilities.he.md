<!-- Hebrew translation of docs/source/samples/node_capabilities.md -->

---
lang: he
direction: rtl
source: docs/source/samples/node_capabilities.md
status: complete
translator: manual
---

<div dir="rtl">

# יכולות הצומת — תמיכת ABI (Torii)

נקודת קצה
- `GET /v1/node/capabilities`

תגובה (מהדורה ראשונה; מדיניות ABI יחידה V1)
```json
{
  "abi_version": 1,
  "data_model_version": 1,
  "crypto": {
    "sm": {
      "enabled": false,
      "default_hash": "sha2_256",
      "allowed_signing": ["ed25519"],
      "sm2_distid_default": "",
      "openssl_preview": false,
      "acceleration": {
        "scalar": true,
        "neon_sm3": false,
        "neon_sm4": false,
        "policy": "scalar-only"
      }
    },
    "curves": {
      "registry_version": 1,
      "allowed_curve_ids": [1]
    }
  }
}
```

הערות
- השדה `abi_version` מציין אילו גרסאות ABI מתקבלות בעת קבלה לרשת.
- `abi_version` היא גרסת ה-ABI היחידה שהצומת מקבל, וגם מהדרי Kotodama צריכים להשתמש בה.
- השדה `data_model_version` הוא גרסת התאימות של מודל הנתונים; ה‑SDK צריכים לדחות שליחות כאשר הערך שונה מהערך המובנה.
- `crypto.curves.allowed_curve_ids` משקף את מזהי העקומות המוגדרים ב־`iroha_config.crypto.curves.allowed_curve_ids` (ראו [`address_curve_registry`](../references/address_curve_registry.md)). אם מתוכנן שימוש ב‑ML‑DSA, GOST או SM בחרו צומת שמפרסם את המזהה המתאים.

ראו גם
- לשם סיכום קומפקטי של מדדי הריצה (מספר גרסאות ABI ומדדי מחזור שדרוג), בקשו את `GET /v1/runtime/metrics`.

</div>
