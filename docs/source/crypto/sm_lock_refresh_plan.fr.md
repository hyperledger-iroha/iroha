---
lang: fr
direction: ltr
source: docs/source/crypto/sm_lock_refresh_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3065571b34a226a5871c4fb68063f9419e48074b20096de215f440bdf54a4e59
source_last_modified: "2026-01-03T18:07:57.085103+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Procédure de planification de l'actualisation Cargo.lock requise par le pic SM.

# Fonctionnalité SM Plan de rafraîchissement `Cargo.lock`

Le pic de fonctionnalité `sm` pour `iroha_crypto` ne pouvait initialement pas compléter `cargo check` alors que `--locked` était appliqué. Cette note enregistre les étapes de coordination pour une mise à jour `Cargo.lock` sanctionnée et suit l'état actuel de ce besoin.

> **Mise à jour du 12/02/2026 :** Une validation récente montre que la fonctionnalité facultative `sm` est désormais construite avec le fichier de verrouillage existant (`cargo check -p iroha_crypto --features sm --locked` réussit en 7,9 s à froid/0,23 s à chaud). L'ensemble de dépendances contient déjà `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pkcs8`, `polyval`, `primeorder`, `sm2`, `sm3`, `sm4` et `sm4-gcm`, aucune actualisation immédiate du verrouillage n'est donc requise. Gardez la procédure ci-dessous en attente pour les futures augmentations de dépendance ou les nouvelles caisses facultatives.

## Pourquoi l'actualisation est nécessaire
- Les itérations précédentes du Spike nécessitaient l'ajout de caisses facultatives qui manquaient dans le fichier de verrouillage. Les instantanés de verrouillage actuels incluent déjà la pile RustCrypto (`sm2`, `sm3`, `sm4`, codecs pris en charge et assistants AES).
- La politique du référentiel bloque toujours les modifications opportunistes des fichiers de verrouillage ; si une future mise à niveau des dépendances est nécessaire, la procédure ci-dessous reste applicable.
- Conservez ce plan afin que l'équipe puisse exécuter une actualisation contrôlée lorsque de nouvelles dépendances liées à SM sont introduites ou que celles existantes nécessitent des changements de version.

## Étapes de coordination proposées
1. ** Demande d'augmentation dans la synchronisation Crypto WG + Release Eng (propriétaire : @crypto-wg lead).**
   - Référencez `docs/source/crypto/sm_program.md` et notez le caractère facultatif de la fonctionnalité.
   - Confirmez qu'il n'y a pas de fenêtres de modification simultanées du fichier de verrouillage (par exemple, gel des dépendances).
2. **Préparez le patch avec le diff de verrouillage (propriétaire : @release-eng).**
   - Exécutez `scripts/sm_lock_refresh.sh` (après approbation) pour mettre à jour uniquement les caisses requises.
   - Capturez la sortie `cargo tree -p iroha_crypto --features sm` (le script émet `target/sm_dep_tree.txt`).
3. **Examen de sécurité (propriétaire : @security-reviews).**
   - Vérifier que les nouvelles caisses/versions correspondent au registre d'audit et aux attentes en matière de licence.
   - Enregistrez les hachages dans le suivi de la chaîne d'approvisionnement.
4. **Fusionner l'exécution de la fenêtre.**
   - Soumettez un PR contenant uniquement le delta du fichier de verrouillage, l'instantané de l'arborescence des dépendances (joint en tant qu'artefact) et les notes d'audit mises à jour.
   - Assurez-vous que CI s'exécute avec `cargo check -p iroha_crypto --features sm` avant la fusion.
5. **Tâches de suivi.**
   - Mettre à jour la liste de contrôle des éléments d'action `docs/source/crypto/sm_program.md`.
   - Informez l'équipe SDK que la fonctionnalité peut être compilée localement avec `--features sm`.## Chronologie et propriétaires
| Étape | Cible | Propriétaire | Statut |
|------|--------|-------|--------|
| Demander un créneau à l'ordre du jour lors du prochain appel du Crypto WG | 2025-01-22 | Responsable du GT Crypto | ✅ Terminé (le pic d'examen conclu peut se poursuivre sans actualisation) |
| Projet de commande sélective `cargo update` + diff de santé mentale | 2025-01-24 | Ingénierie des versions | ⚪ En veille (réactiver si de nouvelles caisses apparaissent) |
| Examen de sécurité des nouvelles caisses | 2025-01-27 | Avis de sécurité | ⚪ En veille (réutilisation de la liste de contrôle d'audit à la reprise de l'actualisation) |
| Fusionner la mise à jour du fichier de verrouillage PR | 2025-01-29 | Ingénierie des versions | ⚪ En veille |
| Mettre à jour la liste de contrôle de la documentation du programme SM | Après fusion | Responsable du GT Crypto | ✅ Adressé via l'entrée `docs/source/crypto/sm_program.md` (2026-02-12) |

## Remarques
- Limitez toute actualisation future aux caisses liées à SM répertoriées ci-dessus (et aux assistants de prise en charge tels que `rfc6979`), en évitant `cargo update` à l'échelle de l'espace de travail.
- Si des dépendances transitives introduisent une dérive MSRV, faites-la surface avant la fusion.
- Une fois fusionné, activez une tâche CI éphémère pour surveiller les temps de construction de la fonctionnalité `sm`.