---
lang: fr
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 00108e84bfd5dc9bce6f4d32724d4b51439067072ee450d8834967a15d17db8b
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: staging-manifest-playbook
lang: he
direction: rtl
source: docs/portal/docs/sorafs/staging-manifest-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. שמרו על סנכרון עותק Docusaurus וה-Markdown הישן עד שמערך Sphinx יופסק לחלוטין.
:::

## סקירה

הפלייבוק הזה מנחה אתכם להפעיל את פרופיל ה-chunker שאושר בפרלמנט בפריסת Torii של staging לפני קידום השינוי לפרודקשן. הוא מניח שהאמנה הממשלית של SoraFS אושרה וש-fixtures קנוניים זמינים בריפו.

## 1. דרישות מקדימות

1. סנכרנו את ה-fixtures הקנוניים והחתימות:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. הכינו את ספריית envelopes של admission ש-Torii יקרא בעת ההפעלה (נתיב דוגמה): `/var/lib/iroha/admission/sorafs`.
3. ודאו שהקונפיג של Torii מפעיל discovery cache ואכיפת admission:

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

## 2. פרסום envelopes של admission

1. העתיקו את envelopes של admission המאושרים אל הספרייה המופנית על ידי `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. אתחלו את Torii (או שלחו SIGHUP אם עטפתם את ה-loader עם hot reload).
3. עקבו אחרי הלוגים עבור הודעות admission:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. אימות הפצת discovery

1. פרסמו את ה-payload החתום של provider advert (בייטים של Norito) שיוצר בצינור של הספק:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. שאבו את endpoint ה-discovery וודאו שה-advert מופיע עם aliases קנוניים:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   ודאו ש-`profile_aliases` כולל את `"sorafs.sf1@1.0.0"` כערך הראשון.

## 4. בדיקת endpoints של manifest ו-plan

1. משכו את מטא-נתוני ה-manifest (נדרש stream token אם admission נאכף):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. בדקו את פלט ה-JSON וודאו:
   - `chunk_profile_handle` הוא `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` תואם לדוח הדטרמיניזם.
   - `chunk_digests_blake3` מתיישרים עם ה-fixtures שנוצרו מחדש.

## 5. בדיקות טלמטריה

- ודאו ש-Prometheus חושף את מדדי הפרופיל החדשים:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- הדשבורדים צריכים להציג את ספק ה-staging תחת ה-alias המצופה ולהשאיר את מוני ה-brownout על אפס בזמן שהפרופיל פעיל.

## 6. מוכנות ל-rollout

1. אספו דו"ח קצר עם ה-URLs, מזהה ה-manifest ו-snapshot של הטלמטריה.
2. שתפו את הדו"ח בערוץ ה-rollout של Nexus לצד חלון ההפעלה המתוכנן בפרודקשן.
3. המשיכו לצ'קליסט הפרודקשן (Section 4 ב-`chunker_registry_rollout_checklist.md`) לאחר אישור בעלי העניין.

שמירה על הפלייבוק הזה מעודכן מבטיחה שכל rollout של chunker/admission עוקב אחרי אותם צעדים דטרמיניסטיים בין staging לפרודקשן.
