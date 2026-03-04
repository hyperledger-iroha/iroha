---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Revue de sécurité SF-6
résumé : Constatations et actions de suivi de l'évaluation indépendante de la signature sans clé, du proof streaming et des pipelines d'envoi de manifestes.
---

# Revue de sécurité SF-6

**Fenêtre d'évaluation :** 2026-02-10 → 2026-02-18  
**Leads de revue :** Security Engineering Guild (`@sec-eng`), Tooling Working Group (`@tooling-wg`)  
**Périmètre :** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), API de proof streaming, gestion des manifestes dans Torii, intégration Sigstore/OIDC, crochets de déverrouillage CI.  
**Artefacts :**  
- Source CLI et tests (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Manifeste/preuve des gestionnaires Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Version d'automatisation (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Harness de parité déterministe (`crates/sorafs_car/tests/sorafs_cli.rs`, [Rapport de parité GA SoraFS Orchestrator](./orchestrator-ga-parity.md))

## Méthodologie

1. **Ateliers de modélisation des menaces** ont cartographié les capacités d'attaque pour les postes de devs, les systèmes CI et les nœuds Torii.  
2. **Code review** a ciblé les surfaces d'identifiants (échange de tokens OIDC, signature sans clé), la validation de manifestes Norito et la contre-pression du proof streaming.  
3. **Tests dynamiques** ont rejoué des manifestes de luminaires et simulé des modes de panne (token replay, manifest falsification, proof streams tronqués) via le harnais de parité et des fuzz drives dédiés.  
4. **Inspection de configuration** a validé les defaults `iroha_config`, la gestion des flags CLI et les scripts de release pour garantir des exécutions déterministes et auditables.  
5. **Entretien de processus** a confirmé le flux de remédiation, les chemins d'escalade et la capture d'evidence d'audit avec les propriétaires de release du Tooling WG.

## Résumé des constats| ID | Sévérité | Zone | Statistique | Résolution |
|----|----------|------|---------|------------|
| SF6-SR-01 | Élevée | Signature sans clé | Les défauts d'audience des tokens OIDC étaient implicites dans les templates CI, avec risque de replay inter-tenant. | Ajout d'une exigence explicite `--identity-token-audience` dans les hooks de release et templates CI ([release process](../developer-releases.md), `docs/examples/sorafs_ci.md`). CI échoue désormais si l'audience est omise. |
| SF6-SR-02 | Moyenne | Preuve en streaming | Les chemins de contre-pression acceptaient des tampons d'abonnés sans limite, permettant l'épuisement de la mémoire. | `sorafs_cli proof stream` impose des tailles de canal bornées avec une troncature déterministe, journalise des curriculum vitae Norito et abandonne le flux ; le miroir Torii a été mis à jour pour supporter les fragments de réponse (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Moyenne | Envoi de manifestes | Le CLI acceptait des manifestes sans vérifier les gros plans embarqués quand `--plan` était absent. | `sorafs_cli manifest submit` recalcule et compare les digests CAR sauf si `--expect-plan-digest` est fourni, rejetant les mismatches et exposant des astuces de remédiation. Des tests couvrent succès/échecs (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Faible | Piste d'audit | La checklist de release n'avait pas de log d'approbation signé pour la revue de sécurité. | Ajout d'une section [release process](../developer-releases.md) exigeant l'attachement des hashes du memo de revue et l'URL du ticket de signature avant GA. |

Tous les constats haut/moyen ont été corrigés pendant la fenêtre de revue et validés par le faisceau de parité existant. Aucune critique latente ne reste.

## Validation des contrôles

- **Portée des identifiants :** Les templates CI imposent désormais audience et émetteur explicites ; le CLI et le helper de release échouent rapidement si `--identity-token-audience` n'accompagne pas `--identity-token-provider`.  
- **Replay déterministe :** Les tests mis à jour couvrent les flux positifs/négatifs d'envoi de manifestes, garantissant que les résumés en mismatch restent des échecs non déterministes et sont signalés avant de toucher le réseau.  
- **Back-pression proof streaming :** Torii diffuse désormais les items PoR/PoTR via des canaux bornés, et le CLI ne conserve que des échantillons de latence tronqués + cinq exemples d'échec, provoquant la croissance sans limite tout en gardant des curriculum vitae déterministes.  
- **Observabilité :** Les compteurs proof streaming (`torii_sorafs_proof_stream_*`) et les résumés CLI capturent les raisons d'abort, offrant des fils d'Ariane d'audit aux opérateurs.  
- **Documentation :** Les guides devs ([index du développeur](../developer-index.md), [référence CLI](../developer-cli.md)) signalent les flags sensibles et les workflows d'escalade.

## Ajouts à la checklist de release

Les release managers **doivent** joindre les preuves suivantes lors de la promotion d'un candidat GA :1. Hash du mémo de revue de sécurité le plus récent (ce document).  
2. Lien vers le ticket de remédiation suivi (ex. `governance/tickets/SF6-SR-2026.md`).  
3. Output de `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` montrant les arguments public/émetteur explicites.  
4. Logs capturés du harnais de parité (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Confirmation que les notes de version Torii incluent les compteurs de télémétrie de preuve streaming borné.

Ne pas collecter les artefacts ci-dessus bloquent le sign-off GA.

**Hashes des artefacts de référence (sign-off 2026-02-20) :**

-`sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Suivis en attente

- **Mise à jour du modèle de menace :** Répéter cette revue chaque trimestre ou avant des ajouts majeurs de flags CLI.  
- **Couverture fuzzing :** Les encodages de transport proof streaming sont fuzzés via `fuzz/proof_stream_transport`, couvrant Identity, gzip, deflate et zstd.  
- **Répétition d'incident :** Planifier un exercice opérateur simulant une compromission de token et un rollback de manifest, pour s'assurer que la documentation reflète les procédures pratiquées.

## Approbation

- Représentant Security Engineering Guild : @sec-eng (2026-02-20)  
- Représentant Tooling Working Group : @tooling-wg (2026-02-20)

Conserver les approbations signées avec le bundle d'artefacts de release.