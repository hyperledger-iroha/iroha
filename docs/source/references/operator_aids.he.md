<!-- Hebrew translation of docs/source/references/operator_aids.md -->

---
lang: he
direction: rtl
source: docs/source/references/operator_aids.md
status: needs-update
translator: manual
---

<div dir="rtl">

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to `docs/source/references/operator_aids.md` for current semantics.

# נקודות הקצה של Torii — עזרי מפעיל (דף מקוצר)

העמוד מרכז נקודות קצה שאינן קונצנזוסיות ומסייעות לצפייה ופתרון תקלות מצד המפעיל. אלא אם צוין אחרת, התגובות בפורמט JSON.

## קונצנזוס (Sumeragi)

- `GET /v1/sumeragi/new_view`
  - צילום מצב של מיספרי NEW_VIEW לכל `(height, view)`.
  - צורה: `{ "ts_ms": <u64>, "items": [{ "height": <u64>, "view": <u64>, "count": <u64> }, ...] }`
  - דוגמה: `curl -s http://127.0.0.1:8080/v1/sumeragi/new_view | jq .`
- `GET /v1/sumeragi/new_view/sse` ‏(SSE)
  - זרם SSE (≈שנייה) של אותו המטען לדשבורדים.
  - דוגמה: `curl -Ns http://127.0.0.1:8080/v1/sumeragi/new_view/sse`
- מדדים: מדדי `sumeragi_new_view_receipts_by_hv{height,view}` משקפים את הספירות.
- `GET /v1/sumeragi/status`
  - צילום מצב של אינדקס המוביל, Highest/Locked commit certificates (`highest_qc`/`locked_qc`, גובה/תצוגה/hash), מוני אספנים/VRF, דחיות פייסמייקר, עומק תור טרנזקציות ובריאות חנות ה-RBC (`rbc_store.{sessions,bytes,pressure_level,evictions_total,recent_evictions[...]}`).
- `GET /v1/sumeragi/status/sse`
  - זרם SSE (≈שנייה) של אותו מטען כמו `/v1/sumeragi/status` למעקב בזמן אמת.
- `GET /v1/sumeragi/qc`
  - צילום מצב של highest/locked commit certificates; כולל `subject_block_hash` עבור highest commit certificate אם ידוע.
- `GET /v1/sumeragi/pacemaker`
  - טיימרים והגדרות פייסמייקר: `{ backoff_ms, rtt_floor_ms, jitter_ms, backoff_multiplier, rtt_floor_multiplier, max_backoff_ms, jitter_frac_permille }`.
- `GET /v1/sumeragi/leader`
  - אינדקס המוביל הנוכחי. במצב NPoS נכלל הקשר PRF: `{ height, view, epoch_seed }`.
- `GET /v1/sumeragi/collectors`
  - תכנית אספנים דטרמיניסטית מן הטופולוגיה והפרמטרים על השרשרת: כולל `mode`, התכנית `(height, view)` (גובה = גובה השרשרת הנוכחי), ‏`collectors_k`, ‏`redundant_send_r`, ‏`proxy_tail_index`, ‏`min_votes_for_commit`, רשימת האספנים המסודרת ו-`epoch_seed` (hex) כאשר NPoS פעיל.
- `GET /v1/sumeragi/params`
  - צילום מצב של פרמטרי Sumeragi על השרשרת `{ block_time_ms, commit_time_ms, max_clock_drift_ms, collectors_k, redundant_send_r, da_enabled, next_mode, mode_activation_height, chain_height }`.
  - כאשר `da_enabled` הוא true, ה-commit ממתין ל-`availability evidence` (ולא לאירוע `DELIVER` מקומי של RBC); בדקו את מצב ה-RBC דרך נקודות הקצה הבאות.
- `GET /v1/sumeragi/rbc`
  - מוני שידור אמין במצטבר: `{ sessions_active, sessions_pruned_total, ready_broadcasts_total, ready_rebroadcasts_skipped_total, deliver_broadcasts_total, payload_bytes_delivered_total, payload_rebroadcasts_skipped_total }`.
- `GET /v1/sumeragi/rbc/sessions`
  - צילום מצב לפי סשן (hash בלוק, גובה/תצוגה, מספר והתקדמות chunks, דגלי `ready`, `delivered`, `invalid`, hash מטען, שדה `recovered`) כדי לזהות עיכובים או שחזור לאחר אתחול.
  - קיצור CLI: ‏`iroha sumeragi rbc sessions --summary` מדפיס `hash`, ‏`height/view`, התקדמות chunks, מוני ready ודגלי invalid/delivered.

## ראיות (ביקורת; מחוץ לקונצנזוס)

- `GET /v1/sumeragi/evidence/count` → ‏`{ "count": <u64> }`
- `GET /v1/sumeragi/evidence` → ‏`{ "total": <u64>, "items": [...] }`
  - כולל שדות בסיסיים (DoublePrepare/DoubleCommit, ‏InvalidQc, ‏InvalidProposal) לבחינה.
  - דוגמאות:
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence/count | jq .`
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence | jq .`
- `POST /v1/sumeragi/evidence` → ‏`{ "status": "accepted", "kind": "<variant>" }`
  - מסייעי CLI:
    - `iroha sumeragi evidence list --summary`
    - `iroha sumeragi evidence count --summary`
    - `iroha sumeragi evidence submit --evidence-hex <hex>` (או `--evidence-hex-file <path>`)

## אימות מפעיל (WebAuthn/mTLS)

- `POST /v1/operator/auth/registration/options`
  - מחזיר אפשרויות רישום WebAuthn (`publicKey`) לצורך רישום אישור ראשוני.
- `POST /v1/operator/auth/registration/verify`
  - מאמת את מטען ה-attestation של WebAuthn ושומר את אישור המפעיל.
- `POST /v1/operator/auth/login/options`
  - מחזיר אפשרויות אימות WebAuthn (`publicKey`) להתחברות מפעיל.
- `POST /v1/operator/auth/login/verify`
  - מאמת את ה-assertion של WebAuthn ומחזיר טוקן סשן למפעיל.
- כותרות:
  - `x-iroha-operator-session`: טוקן סשן לנקודות קצה מפעיל (מונפק ב login verify).
  - `x-iroha-operator-token`: טוקן bootstrap (מותר כאשר `torii.operator_auth.token_fallback` מאפשר).
  - `x-api-token`: נדרש כאשר `torii.require_api_token = true` או `torii.operator_auth.token_source = "api"`.
  - `x-forwarded-client-cert`: נדרש כאשר `torii.operator_auth.require_mtls = true` (מוגדר ע"י ingress proxy).
- תהליך רישום:
  1. קריאה ל-registration options עם טוקן bootstrap (מותר רק לפני רישום האישור הראשון כאשר `token_fallback = "bootstrap"`).
  2. הריצו `navigator.credentials.create` בממשק המפעיל ושלחו את ה-attestation ל-registration verify.
  3. קראו ל-login options ואז ל-login verify כדי לקבל `x-iroha-operator-session`.
  4. שלחו `x-iroha-operator-session` עם נקודות הקצה של המפעיל.

הערות
- הנקודות הללו מספקות מבט לוקאלי לצומת (בחלקו בזיכרון) ואינן משפיעות על הקונצנזוס או על התמPersistnc.
- בהתאם לתצורת Torii, ייתכן שהגישה מוגנת בטוקני API, אימות מפעיל (WebAuthn/mTLS) ובמגבלות קצב.

## קטעי CLI לניטור (bash)

- שאיבה כל 2 שניות עם הדפסת 10 הערכים האחרונים:

```bash
#!/usr/bin/env bash
set -euo pipefail
TORII="${TORII:-http://127.0.0.1:8080}"
INTERVAL="${INTERVAL:-2}"
TOKEN="${TOKEN:-}"
HDR=()
if [[ -n "$TOKEN" ]]; then HDR=(-H "x-api-token: $TOKEN"); fi
while true; do
  curl -s "${HDR[@]}" "$TORII/v1/sumeragi/new_view" \
    | jq -c '{ts_ms, items:(.items|sort_by([.height,.view])|reverse|.[:10])}'
  sleep "$INTERVAL"
done
```

- צפייה בזרם SSE והדפסה מעובדת (10 כניסות אחרונות):

```bash
#!/usr/bin/env bash
set -euo pipefail
TORII="${TORII:-http://127.0.0.1:8080}"
TOKEN="${TOKEN:-}"
HDR=()
if [[ -n "$TOKEN" ]]; then HDR=(-H "x-api-token: $TOKEN"); fi
curl -Ns "${HDR[@]}" "$TORII/v1/sumeragi/new_view/sse" \
  | awk '/^data:/{sub(/^data: /,""); print}' \
  | jq -c '{ts_ms, items:(.items|sort_by([.height,.view])|reverse|.[:10])}'
```

</div>
