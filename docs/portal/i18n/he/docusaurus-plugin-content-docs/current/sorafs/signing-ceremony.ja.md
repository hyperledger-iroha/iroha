---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/signing-ceremony.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a300b14409f1f2ee90c9c039e4d07a9aa452648b9c5b3cf5661fc5d8c27115c
source_last_modified: "2025-11-14T04:43:22.321818+00:00"
translation_last_reviewed: 2026-01-30
---

> Roadmap: **SF-1b — אישורי fixtures של פרלמנט סורה.**
> תהליך הפרלמנט מחליף את "טקס החתימה של המועצה" הישן במצב אופליין.

הטקס הידני לחתימת fixtures של chunker SoraFS הוסר. כל האישורים עוברים כעת דרך
**פרלמנט סורה**, DAO מבוסס סורטיציה שמנהל את Nexus. חברי הפרלמנט משעבדים XOR כדי
לקבל אזרחות, מסתובבים בין פאנלים, ומצביעים on-chain כדי לאשר, לדחות או לגלגל
לאחור שחרורי fixtures. מדריך זה מסביר את התהליך ואת כלי המפתחים.

## סקירת הפרלמנט

- **אזרחות** — מפעילים משעבדים את כמות ה-XOR הנדרשת כדי להירשם כאזרחים ולהיות זכאים לסורטיציה.
- **פאנלים** — האחריות מחולקת בין פאנלים מתחלפים (Infrastructure, Moderation, Treasury, ...).
  פאנל Infrastructure אחראי על אישורי fixtures של SoraFS.
- **סורטיציה ורוטציה** — מושבי הפאנלים נגרלים מחדש בקצב שנקבע בחוקת הפרלמנט כדי למנוע מונופול.

## זרימת אישור fixtures

1. **הגשת הצעה**
   - Tooling WG מעלה את bundle המועמד `manifest_blake3.json` יחד עם diff של fixture
     לרשומת on-chain דרך `sorafs.fixtureProposal`.
   - ההצעה מתעדת digest BLAKE3, גרסה סמנטית והערות שינוי.
2. **סקירה והצבעה**
   - פאנל Infrastructure מקבל את המשימה דרך תור המשימות של הפרלמנט.
   - חברי הפאנל בודקים artefacts של CI, מריצים בדיקות parity ומצביעים on-chain במשקלים.
3. **סיום**
   - כאשר מתקיים קוורום, ה-runtime מפיק אירוע אישור הכולל את digest הקנוני של ה-manifest
     ואת התחייבות Merkle ל-payload של fixture.
   - האירוע משוכפל ל-registry של SoraFS כדי שלקוחות יוכלו למשוך את ה-manifest המאושר האחרון.
4. **הפצה**
   - עוזרי CLI (`cargo xtask sorafs-fetch-fixture`) מושכים את ה-manifest המאושר דרך Nexus RPC.
     קבועי JSON/TS/Go במאגר נשמרים מסונכרנים באמצעות הרצה מחדש של `export_vectors`
     ואימות ה-digest מול הרשומה on-chain.

## תהליך עבודה למפתחים

- חידוש fixtures עם:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- השתמשו ב-helper של הפרלמנט כדי להוריד את ה-envelope המאושר, לאמת חתימות ולרענן
  fixtures מקומיים. כוונו את `--signatures` ל-envelope שפורסם על ידי הפרלמנט; ה-helper
  פותר את ה-manifest הנלווה, מחשב מחדש digest BLAKE3 ומאכף את הפרופיל הקנוני
  `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

העבירו `--manifest` אם ה-manifest נמצא ב-URL אחר. envelopes ללא חתימה נדחים אלא אם
`--allow-unsigned` הוגדר להרצות smoke מקומיות.

- בעת אימות manifest דרך gateway של staging, יעדו את Torii במקום payloads מקומיים:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- CI מקומי כבר לא דורש roster בשם `signer.json`.
  `ci/check_sorafs_fixtures.sh` משווה את מצב ה-repo להתחייבות on-chain האחרונה ונכשל כשיש סטיה.

## הערות ממשל

- חוקת הפרלמנט קובעת קוורום, רוטציה והסלמה — אין צורך בקונפיגורציה ברמת crate.
- Rollbacks חירום מטופלים דרך פאנל המודרציה של הפרלמנט. פאנל Infrastructure מגיש הצעת revert
  שמפנה ל-digest הקודם של ה-manifest, וה-release מוחלף לאחר אישור.
- אישורים היסטוריים נשארים זמינים ב-registry של SoraFS לצורך replay פורנזי.

## FAQ

- **לאן נעלם `signer.json`?**  
  הוא הוסר. כל שיוך החתימות נמצא on-chain; `manifest_signatures.json` במאגר הוא רק fixture
  למפתחים שחייב להתאים לאירוע האישור האחרון.

- **האם עדיין נדרשות חתימות Ed25519 מקומיות?**  
  לא. אישורי הפרלמנט נשמרים כ-artefacts on-chain. fixtures מקומיים קיימים לשחזור, אבל
  מאומתים מול digest הפרלמנט.

- **איך צוותים עוקבים אחרי אישורים?**  
  הירשמו לאירוע `ParliamentFixtureApproved` או שאילת ה-registry דרך Nexus RPC כדי לקבל
  את digest ה-manifest הנוכחי ואת רשימת חברי הפאנל.
