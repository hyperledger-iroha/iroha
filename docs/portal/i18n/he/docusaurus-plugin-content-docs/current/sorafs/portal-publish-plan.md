---
id: portal-publish-plan
lang: he
direction: rtl
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/portal_publish_plan.md`. עדכנו את שתי הגרסאות כאשר ה-workflow משתנה.
:::

פריט ה-roadmap DOCS-7 מחייב שכל ארטיפקט docs (build של הפורטל, מפרט OpenAPI,
SBOMs) יעבור בצינור manifest של SoraFS ויוגש דרך `docs.sora` עם כותרות `Sora-Proof`.
רשימת בדיקה זו מחברת את ה-helpers הקיימים כדי ש-Docs/DevRel, Storage ו-Ops יוכלו
להריץ ריליס בלי לחפש בכמה runbooks.

## 1. Build ואריזת Payloads

הפעילו את helper האריזה (יש אפשרויות skip ל-dry-run):

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- `--skip-build` משתמש ב-`docs/portal/build` אם CI כבר הפיק אותו.
- הוסיפו `--skip-sbom` כאשר `syft` לא זמין (למשל רפטיציה בסביבה מבודדת).
- הסקריפט מריץ בדיקות פורטל, מפיק זוגות CAR + manifest עבור `portal`, `openapi`,
  `portal-sbom`, `openapi-sbom`, מאמת כל CAR כאשר `--proof` פעיל, ומפיק Sigstore bundles
  כאשר `--sign` פעיל.
- מבנה פלט:

```json
{
  "generated_at": "2026-02-19T13:00:12Z",
  "output_dir": "artifacts/devportal/sorafs/20260219T130012Z",
  "artifacts": [
    {
      "name": "portal",
      "car": ".../portal.car",
      "plan": ".../portal.plan.json",
      "car_summary": ".../portal.car.json",
      "manifest": ".../portal.manifest.to",
      "manifest_json": ".../portal.manifest.json",
      "proof": ".../portal.proof.json",
      "bundle": ".../portal.manifest.bundle.json",
      "signature": ".../portal.manifest.sig"
    }
  ]
}
```

שמרו את כל התיקיה (או symlink דרך `artifacts/devportal/sorafs/latest`) כדי שסקירות
הגובראננס יוכלו לעקוב אחרי ארטיפקטי הבילד.

## 2. Pin ל-Manifests ו-Aliases

השתמשו ב-`sorafs_cli manifest submit` כדי לדחוף manifests ל-Torii ולקשור aliases.
הגדירו `${SUBMITTED_EPOCH}` לאפוק הקונצנזוס העדכני (מ-
`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` או מהדשבורד).

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="<i105-account-id>"
KEY_FILE="secrets/docs-admin.key"
ALIAS_PROOF="secrets/docs.alias.proof"
SUBMITTED_EPOCH="$(curl -s ${TORII_URL}/v1/status | jq '.sumeragi.epoch')"

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest="${OUT}/portal.manifest.to" \
  --chunk-plan="${OUT}/portal.plan.json" \
  --torii-url="${TORII_URL}" \
  --submitted-epoch="${SUBMITTED_EPOCH}" \
  --authority="${AUTHORITY}" \
  --private-key-file "${KEY_FILE}" \
  --alias-namespace docs \
  --alias-name portal \
  --alias-proof "${ALIAS_PROOF}" \
  --summary-out "${OUT}/portal.manifest.submit.json" \
  --response-out "${OUT}/portal.manifest.response.json"
```

- חזרו על הפעולה עבור `openapi.manifest.to` ו-SBOM manifests (השמיטו flags של alias לחבילות SBOM
  אלא אם הגובראננס הקצה namespace).
- חלופה: `iroha app sorafs pin register` עובד עם digest מתוך submit summary אם הבינארי מותקן.
- ודאו את מצב ה-registry עם
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- דשבורדים למעקב: `sorafs_pin_registry.json` (מדדי `torii_sorafs_replication_*`).

## 3. Gateway Headers ו-Proofs

צרו בלוק כותרות HTTP + metadata של binding:

```bash
iroha app sorafs gateway route-plan \
  --manifest-json "${OUT}/portal.manifest.json" \
  --hostname docs.sora \
  --alias docs:portal \
  --route-label docs-portal-20260219 \
  --proof-status ok \
  --headers-out "${OUT}/portal.gateway.headers.txt" \
  --out "${OUT}/portal.gateway.plan.json"
```

- התבנית כוללת את `Sora-Name`, `Sora-CID`, `Sora-Proof`, `Sora-Proof-Status` וכן
  CSP/HSTS/Permissions-Policy ברירת מחדל.
- השתמשו ב-`--rollback-manifest-json` כדי להפיק סט כותרות ל-rollback.

לפני חשיפת תעבורה, הריצו:

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- ה-probe מאכף טריות חתימת GAR, מדיניות alias וטביעות TLS.
- ה-self-cert מוריד את ה-manifest עם `sorafs_fetch` ושומר לוגים של CAR replay; שמרו את הפלטים כהוכחות ביקורת.

## 4. Guardrails של DNS וטלמטריה

1. רעננו את שלד ה-DNS כדי שהגובראננס תוכל להוכיח את ה-binding:

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. ניטור בזמן rollout:

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   Dashboards: `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json` והלוח של pin registry.

3. הריצו smoke לכללי ההתראות (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) ושמרו
   לוגים/צילומי מסך לארכיון הריליס.

## 5. חבילת ראיות

כללו את הפריטים הבאים בכרטיס הריליס או בחבילת הגובראננס:

- `artifacts/devportal/sorafs/<stamp>/` (CARs, manifests, SBOMs, proofs,
  Sigstore bundles, submit summaries).
- פלטי gateway probe + self-cert
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  `artifacts/sorafs_gateway_self_cert/<stamp>/`).
- שלד DNS + תבניות headers (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`, `portal.dns-cutover.json`).
- צילומי דשבורד + אישורי התראות.
- עדכון `status.md` עם digest של ה-manifest וזמן alias binding.

מעקב אחר רשימת הבדיקה הזו מספק את DOCS-7: ה-payloads של portal/OpenAPI/SBOM
ארוזים דטרמיניסטית, מקובעים באמצעות aliases, מוגנים בכותרות `Sora-Proof`,
ומנוטרים end-to-end באמצעות מערך observability הקיים.
