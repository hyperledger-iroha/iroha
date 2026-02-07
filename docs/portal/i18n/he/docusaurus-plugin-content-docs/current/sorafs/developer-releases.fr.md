---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-releases.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Processus de release
תקציר: Exécutez le gate de release CLI/SDK, appliquez la politique de versioning partagée et publiez des notes de release canoniques.
---

# תהליך השחרור

Les binaires SoraFS (`sorafs_cli`, `sorafs_fetch`, עוזרים) et les crates SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) אנסמבל sont livrés. Le צינור
de release garde le CLI et les bibliothèques alignés, להבטיח לה couverture lint/test
et capture des artefacts pour les consommateurs במורד הזרם. בצע את רשימת הבדיקה
ci-dessous pour chaque tag candidat.

## 0. מאשר לה אימות דה לה רוויה

Avant d'exécuter le gate technique de release, capturez les derniers artefacts de
revue securité:

- Téléchargez le mémo de revue sécurité SF-6 le plus récent ([דוחות/sf6-security-review](./reports/sf6-security-review.md))
  et enregistrez son hash SHA256 dans le ticket de release.
- Joignez le lien du ticket de remédiation (נ.ג. `governance/tickets/SF6-SR-2026.md`) et notez
  les approbateurs de Security Engineering et du Tooling Working Group.
- Verifiez que la checklist de remediation du mémo est clôturée ; les éléments non résolus bloquent la release.
- Preparez l'upload des logs du harness de parité (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  avec le bundle de manifest.
- Confirmez que la commande de signature que vous comptez exécuter inclut à la fois `--identity-token-provider` et
  un `--identity-token-audience=<aud>` מפורש pour capturer le scope Fulcio dans les preuves de release.

כולל חפצי אמנות lors de la notification à la governance et de la publication.

## 1. מבצע את שער השחרור/בדיקות

Le helper `ci/check_sorafs_cli_release.sh` לבצע את הפורמט, Clippy et les בדיקות
sur les crates CLI ו-SDK עם יעד מקומי או סביבת עבודה (`.target`)
pour éviter les conflits de permissions lors de l'exécution dans des conteneurs CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

התסריט משפיע על הטענות הבאות:

- `cargo fmt --all -- --check` (סביבת עבודה)
- `cargo clippy --locked --all-targets` pour `sorafs_car` (avec la feature `cli`),
  `sorafs_manifest` et `sorafs_chunker`
- `cargo test --locked --all-targets` ארגזי ces mêmes

Si une étape échoue, corrigez la régression avant de tagger. Les builds de release
doivent être continus avec main; ne cherry-pickez pas de correctifs dans des branches
לשחרר. Le vérifie aussi que les flags de signature keyless (`--identity-token-issuer`,
`--identity-token-audience`) sont fournis quand requis ; les arguments manquants גופן
échouer l'execution.

## 2. Appliquer la politique de versioning

Tous les cartes CLI/SDK SoraFS שימושי SemVer :- `MAJOR` : Introduit pour la première גרסה 1.0. Avant 1.0, le bump mineur `0.y`
  **indique des changements cassants** dans la surface du CLI ou les schémas Norito.
- `MINOR` : Nouvelles fonctionnalités (נובו פקודות/דגלים, אלופי נובו Norito
  derrière une politique optionnelle, ajouts de télémétrie).
- `PATCH` : תיקוני באגים, משחרר תיעוד ייחודי et mises à jour de
  מאפיינים ניתנים לצפייה.

Gardez toujours `sorafs_car`, `sorafs_manifest` et `sorafs_chunker` גרסת א-לה-ממה
pour que les consommateurs SDK במורד הזרם puissent dépendre d'une seule chaîne de version
alignée. Lors des bumps de version:

1. Mettez à jour les champs `version =` dans chaque `Cargo.toml`.
2. Régénéres le `Cargo.lock` דרך `cargo update -p <crate>@<new-version>` (le workspace
   להטיל גרסאות מפורשות).
3. Relancez le gate de release afin d'éviter les artefacts périmés.

## 3. Preparer les notes de release

שחרור צ'אק דויט מפרסם ו-Changelog ב-Markdown עם שיפור השינויים
Impact le CLI, le SDK et la governance. נצל את התבנית
`docs/examples/sorafs_release_notes.md` (העתק של רפרטואר חפצי אמנות
release et remplissez les sections avec des détails concrets).

תוכן מינימלי:

- **הדגשים** : titres de fonctionnalités pour les consommateurs CLI et SDK.
- **תאימות**: שינויים קסנטים, שדרוגים של פוליטיקה, דרישות מינימליות
  gateway/nœud.
- **Etapes d'upgrade** : מצווה TL;DR pour mettre à jour les dépendances cargo et
  relancer les fixtures déterministes.
- **אימות**: hashes de sortie ou enveloppes et révision exacte de
  `ci/check_sorafs_cli_release.sh` ביצוע.

Joignez les notes de release remplies au tag (par ex. corps de la release GitHub) et
stockez-les à côté des artefacts générés de façon déterministe.

## 4. Exécuter les hooks de release

Exécutez `scripts/release_sorafs_cli.sh` pour générer le bundle de signatures et le
קורות חיים של אישור חישוב עם שחרור צ'אק. Le wrapper construit le CLI si
הכרחי, אפל `sorafs_cli manifest sign` ושמחה מיידית
`manifest verify-signature` pour faire remonter les échecs avant le tag. דוגמה:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

טיפים:- תוויות קלט לשחרור (מטען, תוכניות, סיכומים, hash de token attendu)
  dans votre repo ou config de déploiement afin de garder le script לשחזור.
  Le bundle CI sous `fixtures/sorafs_manifest/ci_sample/` montre le layout canonique.
- Basez l'automatisation CI sur `.github/workflows/sorafs-cli-release.yml` ; elle exécute
  le gate de release, invoque le script ci-dessus וארכיון חבילות/חתימות comme
  חפצי זרימת עבודה. Reproduisez le même ordre de commandes (שער → חתימה →
  vérification) dans d'autres systèmes CI pour aligner les logs d'audit avec les hashes.
- Gardez `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json` et
  אנסמבל `manifest.verify.summary.json` : ils forment le paquet référencé dans la
  הודעה על הממשל.
- Lorsque la release met à jour des fixtures canoniques, copiez le manifest rafraîchi,
  le chunk plan et les summaries dans `fixtures/sorafs_manifest/ci_sample/` (et mettez
  à jour `docs/examples/sorafs_ci_sample/manifest.template.json`) avant le tag. לס
  מפעילים במורד הזרם תלויים בוועדות מתקנים לשפוך צרור.
- Capturez le log d'exécution de la vérification des bounded-channels de
  `sorafs_cli proof stream` et joignez-le au paquet de release pour démontrer que les
  garde-fous de proof סטרימינג של פעילי restent.
- Notez l'`--identity-token-audience` שימוש מדוייק lors de la signature dans les notes
  דה שחרור; la gouvernance recoupe l'audience avec la politique Fulcio avant approbation.

Utilisez `scripts/sorafs_gateway_self_cert.sh` quand la release inclut aussi un release
שער. Pointez-le sur le même bundle de manifest pour prouver que l'attestation
מתכתב עם מועמד חפץ:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Tagger et publier

Après le passage des checks et la fin des hooks :1. Exécutez `sorafs_cli --version` et `sorafs_fetch --version` pour confirmer que les binaires
   reportent la nouvelle version.
2. Preparez la configuration de release dans un `sorafs_release.toml` versionné (preféré)
   ou un autre fichier de config suivi par votre repo de déploiement. Évitez de dépendre
   de variables d'environnement ad-hoc; passez les chemins au CLI avec `--config` (ou
   equivalent) afin que les inputs soient explicites et reproductibles.
3. Créez un tag signné (préféré) ou un tag annoté :
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. העלה חפצים (חבילות CAR, מניפסטים, קורות חיים של הוכחות, הערות לשחרור,
   פלטים של אישורים) לעומת הרישום של הפרויקט להלן רשימת בדיקות ניהול
   dans le [guide deploiement](./developer-deployment.md). כדי לשחרר מוצר
   מכשירי דה נובלס, poussez-les vers le repo de fixtures partagé ou l'object store
   afin que l'automatisation d'audit puisse comparer le bundle publié au control source.
5. הודע על תעלת ה-Governance med les liens vers le tag signné, les notes de release,
   les hashes du bundle/signatures du manifest, les résumés archivés `manifest.sign/verify`
   et tout enveloppe d'attestation. כלול כתובת ה-URL du job CI (או ארכיון היומנים) qui a
   בצע את `ci/check_sorafs_cli_release.sh` ואת `scripts/release_sorafs_cli.sh`. Mettez à
   jour le ticket de gouvernance pour que les auditeurs puissent relier les approbations
   חפצי אמנות; lorsque `.github/workflows/sorafs-cli-release.yml` שליח הודעות,
   liez les hashes enregistrés au lieu de coller des resumés ad-hoc.

## 6. Suivi לאחר השחרור

- Assurez-vous que la documentation pointant vers la nouvelle version (התחלות מהירות, תבניות CI)
  est à jour ou confirmez qu'acun changement n'est requis.
- Créez des entrées de roadmap si un travail de suivi est nécessaire (לדוגמה. flags de migration,
- Archivez les logs de sortie du gate de release pour les auditeurs : stockez-les à côté des
  סימני חפצי אמנות.

Suivre ce pipeline maintient le CLI, les ארגזים SDK et les éléments de governance
alignés à chaque cycle de release.