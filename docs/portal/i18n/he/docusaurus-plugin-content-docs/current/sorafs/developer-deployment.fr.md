---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-deployment.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: פריסת מפתח
כותרת: Notes de déploiement SoraFS
sidebar_label: Notes de déploiement
תיאור: רשימת בדיקה לקידום צינור SoraFS de la CI לעומת הייצור.
---

:::הערה מקור קנוניק
:::

# הערות דה-ploiement

זרימת העבודה של האריזה SoraFS מחזקת את הדטרמיניזם, מעביר את ה-CI על ייצור חובה להמשך הפעילות הגארדית. לנצל את רשימת התיוג למחסום ה-outillage sur des gateways and fournisseurs de stockage reels.

## פרה כרך

- **Alignement du registre** — confirmez que les profils de chunker et les manifests référencent le même tuple `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Politique d'admission** - revoyez les adverts de fournisseurs signnés et les alias proofs nécessaires pour `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook du pin registry** — gardez `docs/source/sorafs/runbooks/pin_registry_ops.md` à portée pour les scénarios de reprise (סיבוב ד'כינוי, échecs de réplication).

## Configuration de l'environnement

- Les gateways doivent activer l'endpoint de proof streaming (`POST /v2/sorafs/proof/stream`) pour que le CLI puisse émettre des resumés de télémétrie.
- הגדר את מדיניות `sorafs_alias_cache` en utilisant les valeurs par défaut de `iroha_config` או le helper CLI (`sorafs_cli manifest submit --alias-*`).
- אסימוני זרם של Fournissez les (או מזהים Torii) באמצעות un gestionnaire de secrets sécurisé.
- Activez les Exporters de télémétrie (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) ו-envoyez-les vers votre stack Prometheus/OTel.

## אסטרטגיית השקה

1. **מתבטא בכחול/ירוק**
   - Utilisez `manifest submit --summary-out` pour archiver les réponses de chaque rollout.
   - Surveillez `torii_sorafs_gateway_refusals_total` pour détecter tôt les mismatches de capacité.
2. **validation des proofs**
   - Traitez les échecs de `sorafs_cli proof stream` comme des bloqueurs de déploiement; les pics de latence indiquent souvent un throttling fournisseur ou des tiers mal configurés.
   - `proof verify` doit faire partie du בדיקת עשן פוסט-pin pour s'assurer que le CAR hébergé par les fournisseurs correspond toujours au digest du manifest.
3. **לוחות מחוונים דה télémétrie**
   - ייבוא `docs/examples/sorafs_proof_streaming_dashboard.json` ב-Grafana.
   - Ajoutez des panneaux pour la santé du pin registry (`docs/source/sorafs/runbooks/pin_registry_ops.md`) et les stats de chunk range.
4. **הפעלה מרובה מקורות**
   - Suivez les étapes de rollout progressif dans `docs/source/sorafs/runbooks/multi_source_rollout.md` lors de l'activation de l'orchestrateur, et Archivez les artefactsboard/télémétrie pour les audits.

## התקריות- Suivez les chemins d'escalade dans `docs/source/sorafs/runbooks/` :
  - `sorafs_gateway_operator_playbook.md` pour les pannes de gateway et l'épuisement des stream tokens.
  - `dispute_revocation_runbook.md` lors de litiges de réplication.
  - `sorafs_node_ops.md` pour la maintenance au niveau des nœuds.
  - `multi_source_rollout.md` pour les overrides d'orchestrateur, le blacklisting des peers et les rollouts par étapes.
- Enregistrez les échecs de proofs et les anomalies de latence dans GovernanceLog via les API de PoR tracker existantes afin que la governance puisse évaluer les performances des fournisseurs.

## Prochaines étapes

- Intégrez l'automatisation de l'orchestrateur (`sorafs_car::multi_fetch`) lorsque l'orchestrateur multi-source fach (SF-6b) sera disponible.
- Suivez les mises à jour PDP/PoTR sous SF-13/SF-14 ; le CLI et les docs évolueront pour exposer les deadlines et la sélection de tiers une fois ces proofs stabilisés.

בהערות משולבות של ביצוע פעולות עם התחלה מהירה ו-Cettes CI, ציוד מתאים עובר ניסויים מקומיים על צינורות SoraFS בהפקה עם עיבוד répétable וניתן לצפייה.