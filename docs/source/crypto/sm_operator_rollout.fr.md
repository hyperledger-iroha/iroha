---
lang: fr
direction: ltr
source: docs/source/crypto/sm_operator_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dffc2cf6c6e59f54d1fc22136ba93f75466509c699a4361a381bf7e0ce0d1dda
source_last_modified: "2026-01-03T18:07:57.089544+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Liste de contrôle de déploiement des fonctionnalités SM et de télémétrie

Cette liste de contrôle aide les équipes SRE et opérateurs à activer la fonctionnalité SM (SM2/SM3/SM4).
réglé en toute sécurité une fois les portes de l’audit et de la conformité franchies. Suivez ce document
aux côtés du brief de configuration dans `docs/source/crypto/sm_program.md` et du
conseils juridiques/d’exportation dans `docs/source/crypto/sm_compliance_brief.md`.

## 1. Préparation avant le vol
- [ ] Confirmez que les notes de version de l'espace de travail indiquent `sm` comme vérification uniquement ou signature,
      en fonction de l'étape de déploiement.
- [ ] Vérifiez que la flotte exécute des binaires construits à partir d'un commit qui inclut le
      Compteurs de télémétrie SM et boutons de configuration. (Lancement cible à déterminer ; piste
      dans le ticket de déploiement.)
-[ ] Exécutez `scripts/sm_perf.sh --tolerance 0.25` sur un nœud intermédiaire (par cible
      architecture) et archivez le résultat récapitulatif. Le script sélectionne désormais automatiquement
      la ligne de base scalaire comme cible de comparaison pour les modes d'accélération
      (`--compare-tolerance` est par défaut de 5,25 pendant que le travail SM3 NEON atterrit) ;
      enquêter ou bloquer le déploiement si le serveur principal ou le serveur de comparaison
      la garde échoue. Lors de la capture sur du matériel Linux/aarch64 Neoverse, transmettez
      `--baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_<mode>.json --write-baseline`
      pour écraser les médianes `m3-pro-native` exportées par la capture de l'hôte
      avant l'expédition.
-[ ] Assurez-vous que `status.md` et le ticket de déploiement enregistrent les déclarations de conformité pour
      tous les nœuds opérant dans les juridictions qui les exigent (voir la note de conformité).
-[ ] Préparez les mises à jour KMS/HSM si les validateurs stockent les clés de signature SM dans
      modules matériels.

## 2. Modifications de configuration
1. Exécutez l'assistant xtask pour générer l'inventaire des clés SM2 et l'extrait prêt à coller :
   ```bash
   cargo xtask sm-operator-snippet \
     --distid CN12345678901234 \
     --json-out sm2-key.json \
     --snippet-out client-sm2.toml
   ```
   Utilisez `--snippet-out -` (et éventuellement `--json-out -`) pour diffuser les sorties vers la sortie standard lorsque vous avez simplement besoin de les inspecter.
   Si vous préférez piloter manuellement les commandes CLI de niveau inférieur, le flux équivalent est le suivant :
   ```bash
   cargo run -p iroha_cli --features sm -- \
     crypto sm2 keygen \
     --distid CN12345678901234 \
     --output sm2-key.json

   cargo run -p iroha_cli --features sm -- \
     crypto sm2 export \
     --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
     --distid CN12345678901234 \
     --snippet-output client-sm2.toml \
     --emit-json --quiet
   ```
   Si `jq` n'est pas disponible, ouvrez `sm2-key.json`, copiez la valeur `private_key_hex` et transmettez-la directement à la commande d'exportation.
2. Ajoutez l'extrait résultant à la configuration de chaque nœud (valeurs affichées pour le
   étape de vérification uniquement ; ajuster par environnement et garder les clés triées comme indiqué) :
```toml
[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]   # remove "sm2" to stay in verify-only mode
sm2_distid_default = "1234567812345678"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```
3. Redémarrez le nœud et confirmez que `crypto.sm_helpers_available` et (si vous avez activé le backend d'aperçu) `crypto.sm_openssl_preview_enabled` apparaissent comme prévu dans :
   -`/status` JSON (`"crypto":{"sm_helpers_available":true,"sm_openssl_preview_enabled":true,...}`).
   - Le rendu `config.toml` pour chaque nœud.
4. Mettez à jour les entrées manifestes/genèse pour ajouter les algorithmes SM à la liste verte si
   la signature est activée plus tard dans le déploiement. Lors de l'utilisation de `--genesis-manifest-json`
   sans bloc Genesis pré-signé, `irohad` amorce désormais le crypto d'exécution
   instantané directement à partir du bloc `crypto` du manifeste : assurez-vous que le manifeste est
   vérifié votre plan de changement avant de continuer.## 3. Télémétrie et surveillance
- Grattez les points de terminaison Prometheus et assurez-vous que les compteurs/jauges suivants apparaissent :
  -`iroha_sm_syscall_total{kind="verify"}`
  -`iroha_sm_syscall_total{kind="hash"}`
  -`iroha_sm_syscall_total{kind="seal|open",mode="gcm|ccm"}`
  - `iroha_sm_openssl_preview` (jauge 0/1 signalant l'état de bascule d'aperçu)
  -`iroha_sm_syscall_failures_total{kind="verify|hash|seal|open",reason="..."}`
- Chemin de signature Hook une fois la signature SM2 activée ; ajouter des compteurs pour
  `iroha_sm_sign_total` et `iroha_sm_sign_failures_total`.
- Créer des tableaux de bord/alertes Grafana pour :
  - Spikes dans les compteurs de pannes (fenêtre 5m).
  - Chutes soudaines du débit des appels système SM.
  - Différences entre les nœuds (par exemple, activation incompatible).

## 4. Étapes de déploiement
| Phases | Actions | Remarques |
|-------|--------|-------|
| Vérification uniquement | Mettez à jour `crypto.default_hash` vers `sm3-256`, laissez `allowed_signing` sans `sm2`, surveillez les compteurs de vérification. | Objectif : exercer les chemins de vérification SM sans risquer de divergence de consensus. |
| Pilote de signature mixte | Autoriser la signature SM limitée (sous-ensemble de validateurs) ; surveiller les compteurs de signature et la latence. | Assurez-vous que la solution de secours vers Ed25519 reste disponible ; s'arrêter si la télémétrie montre des discordances. |
| Signature de l'AG | Étendez `allowed_signing` pour inclure `sm2`, mettez à jour les manifestes/SDK et publiez le runbook final. | Nécessite des résultats d’audit fermés, des dossiers de conformité mis à jour et une télémétrie stable. |

### Examens de préparation
- **Vérification de l'état de préparation uniquement (SM-RR1).** Convoquer Release Eng, Crypto WG, Ops et Legal. Exiger :
  - `status.md` note l'état de dépôt de conformité + la provenance OpenSSL.
  - `docs/source/crypto/sm_program.md` / `sm_compliance_brief.md` / cette liste de contrôle mise à jour dans la dernière fenêtre de version.
  - `defaults/genesis` ou le manifeste spécifique à l'environnement affiche `crypto.allowed_signing = ["ed25519","sm2"]` et `crypto.default_hash = "sm3-256"` (ou la variante de vérification uniquement sans `sm2` si elle est encore à la première étape).
  - Logs `scripts/sm_openssl_smoke.sh` + `scripts/sm_interop_matrix.sh` joints au ticket de déploiement.
  - Tableau de bord de télémétrie (`iroha_sm_*`) examiné pour son comportement en régime permanent.
- **Préparation du pilote de signature (SM-RR2).** Portes supplémentaires :
  - Rapport d'audit pour la pile RustCrypto SM fermée ou RFC pour les contrôles compensatoires signés par la sécurité.
  - Runbooks d'opérateur (spécifiques à l'installation) mis à jour avec les étapes de repli/restauration de signature.
  - Les manifestes Genesis pour la cohorte pilote incluent `allowed_signing = ["ed25519","sm2"]` et la liste verte est reflétée dans chaque configuration de nœud.
  - Plan de sortie/restauration documenté (revenir `allowed_signing` à Ed25519, restaurer les manifestes, réinitialiser les tableaux de bord).
- ** Préparation GA (SM-RR3). ** Nécessite un rapport pilote positif, des dépôts de conformité mis à jour pour toutes les juridictions de validation, des lignes de base de télémétrie signées et l'approbation du ticket de sortie par la triade Release Eng + Crypto WG + Ops/Legal.## 5. Liste de contrôle d'emballage et de conformité
- **Regroupez les artefacts OpenSSL/Tongsuo.** Expédiez les bibliothèques partagées OpenSSL/Tongsuo 3.0+ (`libcrypto`/`libssl`) avec chaque package de validation ou documentez la dépendance exacte du système. Enregistrez la version, les indicateurs de build et les sommes de contrôle SHA256 dans le manifeste de version afin que les auditeurs puissent retracer la build du fournisseur.
- **Vérifiez pendant CI.** Ajoutez une étape CI qui exécute `scripts/sm_openssl_smoke.sh` sur les artefacts packagés sur chaque plate-forme cible. Le travail doit échouer si l'indicateur d'aperçu est activé mais que le fournisseur ne peut pas être initialisé (en-têtes manquants, algorithme non pris en charge, etc.).
- **Publier les notes de conformité.** Mettre à jour les notes de version / `status.md` avec la version groupée du fournisseur, les références de contrôle des exportations (GM/T, GB/T) et tout dépôt spécifique à la juridiction requis pour les algorithmes SM.
- **Mises à jour du runbook de l'opérateur.** Documentez le flux de mise à niveau : préparez les nouveaux objets partagés, redémarrez les homologues avec `crypto.enable_sm_openssl_preview = true`, confirmez le champ `/status` et le basculement de la jauge `iroha_sm_openssl_preview` vers `true` et conservez un plan de restauration (retournez l'indicateur de configuration ou annulez le package) si la télémétrie d'aperçu s'écarte à travers la flotte.
- **Conservation des preuves.** Archivez les journaux de construction et les attestations de signature des packages OpenSSL/Tongsuo aux côtés des artefacts de version du validateur afin que les audits futurs puissent reproduire la chaîne de provenance.

## 6. Réponse aux incidents
- **Pics d'échecs de vérification :** Revenez à une version sans prise en charge SM ou supprimez `sm2`
  à partir de `allowed_signing` (en rétablissant `default_hash` si nécessaire) et basculez vers le précédent
  libération pendant l'enquête. Capturez les charges utiles ayant échoué, les hachages comparatifs et les journaux de nœuds.
- **Régressions de performances :** Comparez les métriques SM avec les références Ed25519/SHA2.
  Si le chemin intrinsèque ARM provoque une divergence, définissez `crypto.sm_intrinsics = "force-disable"`
  (fonctionnalité à bascule en attente de mise en œuvre) et rapporter les résultats.
- **Écarts de télémétrie :** Si les compteurs sont manquants ou ne sont pas mis à jour, signalez un problème
  contre Release Engineering; ne pas procéder à un déploiement plus large jusqu'à ce que l'écart soit comblé
  est résolu.

## 7. Modèle de liste de contrôle
- [ ] Configuration effectuée et redémarré par l'homologue.
- [ ] Compteurs de télémétrie visibles et tableaux de bord configurés.
- [ ] Démarches de conformité/légales enregistrées.
- [ ] Phase de déploiement approuvée par Crypto WG / Release TL.
- [ ] Examen post-déploiement terminé et conclusions documentées.

Conservez cette liste de contrôle dans le ticket de déploiement et mettez à jour `status.md` lorsque le
transitions de flotte entre les phases.