---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/signing-ceremony.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: טקס חתימה
כותרת: Replacement de la ceremony de signature
תיאור: הערה Le Parlement Sora approuve and distribue les fixtures du chunker SoraFS (SF-1b).
sidebar_label: טקס חתימה
---

> מפת דרכים : **SF-1b — אישורים למשחקי פרלמנט סורה.**
> Le workflow du Parlement remplace l'ancienne «טקס החתימה du conseil» hors ligne.

Le rituel manuel de signature des fixtures du chunker SoraFS יצא לפנסיה. Toutes les
הסכמות passat desormais par le **Parlement Sora**, la DAO basee sur le tirage
au sort qui gouverne Nexus. Les membres du Parlement bloquent du XOR pour obtenir la
citoyennete, טורננט פאנלים והצבעות בשרשרת לאשר, לדחות או
revenir sur des releases de fixtures. Ce guide explique le processus et le tooling
למפתחים.

## Vue d'ensemble du Parlement

- **Citoyennete** - Les operateurs immobilisent le XOR requis pour s'inscriptre comme
  זכאים citoyens et devenir au tirage au sort.
- **פאנלים** - האחריות של מרכז הפנלים
  (תשתיות, מתינות, טרזוררי, ...). מעצר תשתיות Le Panel
  les approbations de fixtures SoraFS.
- **Tirage au sort et rotation** - Les sieges de panel sont reattribues selon la
  cadence specifiee dans la constitution du Parlement afin qu'aucun groupe ne
  לעשות מונופול על ההסכמות.

## Flux d'appropation des fixtures

1. **מסירת הצעה**
   - Le Tooling WG televerse le bundle candidat `manifest_blake3.json` et le diff
     de fixture dans le registre על השרשרת דרך `sorafs.fixtureProposal`.
   - La proposition enregistre le digest BLAKE3, la version semantique et les notes
     דה שינוי.
2. **Revue and vote**
   - Le Panel Infrastructure recoit l'affection via la file de taches du Parlement.
   - Les membres inspectent les artefacts CI, executent des tests de parite et
     emettent des votes ponderes on-chain.
3. **סיום**
   - Une fois le quorum atteint, le runtime emet un evenement d'approbation incluant
     le digest canonique du manifest et l'engagement Merkle du payload de fixture.
   - L'evenement est duplique dans le registry SoraFS afin que les clients puissent
     Recuperer le dernier manifest approuve par le Parlement.
4. **הפצה**
   - Les helpers CLI (`cargo xtask sorafs-fetch-fixture`) recuperent le manifest
     לאשר באמצעות Nexus RPC. Les constantes JSON/TS/Go du depot restent Synces
     en relancant `export_vectors` et en validant le digest par rapport a l'enregistrement
     על השרשרת.

## פיתוח זרימת עבודה

- מתקנים מחודשים עם:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Utiliser le helper de fetch du Parlement pour telecharger l'enveloppe approuvee,
  Verifier les חתימות ו rafraichir les fixtures locales. Pointer `--signatures`
  vers l'enveloppe publiee par le Parlement; le helper resout le manifest associe,
  חשב מחדש את לעכל BLAKE3 והטל את הפרופיל הקנוני `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```Passer `--manifest` זה המניפסט עם כתובת URL אחת. Les enveloppes non
signees sont refuses sauf si `--allow-unsigned` est active pour des smoke runs locaux.

- שפך את המניפסט התוקף דרך un gateway de staging, cibler Torii plutot que des
  מטענים locaux:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- Le CI local n'exige plus un roster `signer.json`.
  `ci/check_sorafs_fixtures.sh` השווה את L'etat du repo avec le dernier engagement
  על השרשרת et echoue lorsqu'ils משתנים.

## הערות ניהול

- La constitution du Parlement gouverne le quorum, la rotation et l'escalade;
  תצורת aucune au niveau du carte n'est necessaire.
- Les rollbacks d'urgence sont geres via le panel de moderation du Parlement. לה
  תשתיות פאנל הוציאו הצעה אחת לחזרה qui reference le digest
  תקדים du manifest, et la release est remplacee une fois approuvee.
- Les approbations historiques restent disponibles dans le registry SoraFS pour
  un replay forensique.

## שאלות נפוצות

- **Ou est passe `signer.json` ?**  
  Il a ete supprime. Toute attribution de signature vit on-chain; `manifest_signatures.json`
  Dans le depot n'est qu'un מתקן developpeur qui doit correspondre au dernier
  אירוע ד'הסכמה.

- **Faut-il encore des signatures Ed25519 locales ?**  
  לא. Les approbations du Parlement sont stockees comme חפצים ברשת. מתקנים לס
  מקומות קיימים pour la reproductibilite mais sont validees contre le digest du Parlement.

- ** תגובה les equipes surveillent-elles les approbations ?**  
  Abonnez-vous a l'evenement `ParliamentFixtureApproved` או לחקור את הרישום באמצעות
  Nexus RPC pour obtenir le digest actuel du manifest et la list des membres du panel.