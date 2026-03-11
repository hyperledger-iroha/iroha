---
lang: he
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f708c9c597c0455761049a17989369498d318be348e28f71196bb82761dd36b
source_last_modified: "2026-01-03T18:07:58.297179+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: staging-manifest-playbook-he
slug: /sorafs/staging-manifest-playbook-he
---

:::הערה מקור קנוני
מראות `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. שמור את שני העותקים מיושרים על פני מהדורות.
:::

## סקירה כללית

ספר משחק זה עובר דרך הפעלת פרופיל ה-chunker שאושר על ידי הפרלמנט בפריסת Torii בימוי לפני קידום השינוי לייצור. היא מניחה שאמנת הממשל SoraFS אושררה והמתקנים הקנוניים זמינים במאגר.

## 1. דרישות מוקדמות

1. סנכרן את המתקנים והחתימות הקנוניים:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. הכן את ספריית מעטפות הקבלה ש-Torii יקרא בעת ההפעלה (נתיב לדוגמא): `/var/lib/iroha/admission/sorafs`.
3. ודא שתצורת Torii מאפשרת את מטמון הגילוי ואכיפת קבלה:

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2. פרסם מעטפות כניסה

1. העתק את מעטפות הקבלה של ספק המאושר לספרייה שאליה מפנה `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. הפעל מחדש את Torii (או שלח SIGHUP אם עטפת את המטען עם טעינה חוזרת תוך כדי תנועה).
3. התאם את היומנים להודעות קבלה:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. אימות הפצת גילוי

1. פרסם את מטען הפרסומות של הספק החתום (Norito בתים) שהופק על ידי
   צינור ספק:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. חפש את נקודת הקצה של הגילוי ואשר שהמודעה מופיעה עם כינויים קנוניים:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   ודא ש-`profile_aliases` כולל את `"sorafs.sf1@1.0.0"` בתור הערך הראשון.

## 4. מניפסט תרגיל ותכנון נקודות קצה

1. אחזר את המטא-נתונים של המניפסט (דורש אסימון סטרימינג אם קבלה נאכפת):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. בדוק את פלט ה-JSON וודא:
   - `chunk_profile_handle` הוא `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` תואם את דוח הדטרמיניזם.
   - `chunk_digests_blake3` התיישר עם המתקנים המחודשים.

## 5. בדיקות טלמטריה

- אשר ש-Prometheus חושף את מדדי הפרופיל החדשים:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- לוחות מחוונים צריכים להראות את ספק ה-Staging תחת הכינוי הצפוי ולהשאיר את מונים ה-Brownout באפס בזמן שהפרופיל פעיל.

## 6. מוכנות להפעלה

1. צלם דוח קצר עם כתובות האתרים, מזהה המניפסט ותמונת המצב של הטלמטריה.
2. שתף את הדוח בערוץ ההשקה של Nexus לצד חלון הפעלת הייצור המתוכנן.
3. המשך לרשימת הבדיקה של הייצור (סעיף 4 ב-`chunker_registry_rollout_checklist.md`) לאחר חתימת בעלי העניין.

שמירה על עדכון של ספר המשחק הזה מבטיחה שכל השקת צ'אנקר/קבלה יבצעו את אותם שלבים דטרמיניסטיים לאורך הבמה והפקה.
