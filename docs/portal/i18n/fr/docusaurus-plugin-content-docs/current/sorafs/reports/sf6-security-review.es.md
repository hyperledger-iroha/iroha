---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Révision de sécurité SF-6
résumé : Hallazgos et tâches de suivi de l'évaluation indépendante de la signature sans clé, du streaming de preuves et des pipelines d'envoi des manifestes.
---

# Révision de sécurité SF-6

**Vente d'évaluation :** 2026-02-10 -> 2026-02-18  
** Responsable de la révision : ** Security Engineering Guild (`@sec-eng`), Tooling Working Group (`@tooling-wg`)  
**Alcance :** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), API de diffusion de preuves, gestion des manifestes et Torii, intégration Sigstore/OIDC, crochets de libération en CI.  
**Artefacts :**  
- Source de CLI et tests (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Gestionnaires de manifeste/preuve en Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Automatisation de la version (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Harness de paridad determinista (`crates/sorafs_car/tests/sorafs_cli.rs`, [Reporte de Paridad GA del Orchestrator SoraFS](./orchestrator-ga-parity.md))

## Méthodologie

1. **Ateliers de modélisation des menaces** mapearon capacités d'attaque pour les postes de travail des développeurs, systèmes CI et nœuds Torii.  
2. **Révision du code** enfoco superficies de credenciales (échange de jetons OIDC, signature sans clé), validation des manifestes Norito et contre-pression et diffusion de preuves.  
3. **Test dynamique** reproduction des manifestes des appareils et simulation des modes de chute (relecture de jetons, falsification du manifeste, flux de preuve tronqués) en utilisant le harnais de parité et les lecteurs de fuzz à moyen.  
4. **Inspection de configuration** valide les valeurs par défaut de `iroha_config`, gestion des indicateurs de la CLI et des scripts de publication pour garantir les exécutions déterministes et auditables.  
5. **Entrevista de proceso** confirme le flux de remédiation, les itinéraires d'escalade et la capture des preuves de l'auditoire avec les propriétaires de libération de Tooling WG.

## Résumé des hallazgos| ID | Sévérité | Zone | Hallazgo | Résolution |
|----|----------|------|---------|------------|
| SF6-SR-01 | Haute | Signature sans clé | Les valeurs par défaut de l'audience du jeton OIDC étaient implicites dans les modèles de CI, avec des risques de relecture entre les locataires. | Ajoutez l'application explicite de `--identity-token-audience` aux hooks de version et aux modèles de CI ([processus de publication](../developer-releases.md), `docs/examples/sorafs_ci.md`). CI ahora falla si se omite l'audience. |
| SF6-SR-02 | Médias | Preuve en streaming | Les chemins de contre-pression acceptent les tampons des abonnés sans limite, permettant une agotamiento de mémoire. | `sorafs_cli proof stream` impose des tamanos de canal acotados con truncamiento déterminista, enregistre la reprise Norito et interrompt le flux ; L'objet Torii est mis à jour pour ajouter des morceaux de réponse (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Médias | Envio de manifestes | La CLI acceptée se manifeste sans vérifier les plans des morceaux intégrés lorsque `--plan` est activé. | `sorafs_cli manifest submit` ahora recalcula et comparera les résumés de la salve CAR qui se prouvent `--expect-plan-digest`, rechazando incohérences et montre les pistes de remédiation. Les tests contiennent des cas de sortie/d'erreur (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Basse | Piste d'audit | La liste de contrôle de libération prend en compte un journal d'approbation ferme pour la révision de la sécurité. | Il y a une section dans le [processus de publication](../developer-releases.md) qui nécessite des hachages supplémentaires pour le mémo de révision et l'URL du ticket de signature avant l'AG. |

Tous les niveaux élevés/moyens sont corrigés pendant la période de révision et validés avec le harnais de parité existant. Aucun problème n'est émis par des critiques latentes.

## Validation des contrôles

- **Taux d'accréditation :** Les modèles de CI exigent désormais une audience et des informations explicites sur l'émetteur ; la CLI et l'aide à la libération tombent rapidement en salve que `--identity-token-audience` accompagne un `--identity-token-provider`.  
- **Replay déterministe :** Les tests actualisés contiennent des flux positifs/négatifs de l'envoi des manifestes, garantissant que les digestions desalineados siendo fallas no déterministas et se détectent avant de toucher le rouge.  
- **Contre-pression et diffusion en continu :** Torii transmet désormais les éléments PoR/PoTR sur les canaux adjacents, et la CLI ne conserve que des valeurs tronquées de latence + 5 exemples de chute, évitant ainsi une croissance sans limite et en maintenant des reprises déterministes.  
- **Observabilité :** Les contrôleurs de diffusion de preuves (`torii_sorafs_proof_stream_*`) et les résumés de la CLI capturent les raisons de l'avortement, entrent le fil d'Ariane de l'auditoire et les opérateurs.  
- **Documentation :** Guides pour les développeurs ([index du développeur](../developer-index.md), [référence CLI](../developer-cli.md)) indiquant les indicateurs sensibles à la sécurité et aux flux de travail d'escalade.

## Ajouts à la liste de contrôle de la version

Les gestionnaires de versions **deben** ajoutent la preuve suivante au promoteur d'un candidat GA :1. Hash del memo mas recente de revision de seguridad (ce documento).  
2. Liez le ticket de correction suivi (par exemple, `governance/tickets/SF6-SR-2026.md`).  
3. Sortie de `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` montrant les arguments explicites du public/émetteur.  
4. Journaux capturés du harnais de parité (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Confirmation que les notes de version de Torii incluent des contadores de télémétrie de preuve en streaming acotado.

Ne pas récupérer les artefacts antérieurs qui bloquent la signature de GA.

**Hashes d'artefacts de référence (signature 2026-02-20) :**

-`sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Suivis pendants

- **Actualisation du modèle de menace :** Répétez cette révision trimestriellement ou avant les grandes ajouts de drapeaux de la CLI.  
- **Cobertura de fuzzing :** Les encodages du transport de preuve en streaming sont fuzzearon via `fuzz/proof_stream_transport`, cubriendo payloads Identity, gzip, deflate et zstd.  
- **Ensayo de incidents:** Programmer un exercice d'opérateurs qui simule une compromission de jeton et une restauration du manifeste, garantissant que la documentation reflète les procédures mises en pratique.

## Approbation

- Représentant de Security Engineering Guild : @sec-eng (2026-02-20)  
- Représentant du groupe de travail Tooling : @tooling-wg (2026-02-20)

Enregistrez les autorisations confirmées conjointement avec le lot d'artefacts de libération.