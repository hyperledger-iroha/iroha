---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Révision de sécurité SF-6
résumé : Achados et éléments d'accompagnement de l'aval indépendant de signature sans clé, de diffusion de preuves et de pipelines d'envoi de manifestes.
---

# Révision de sécurité SF-6

**Janela de avaliacao:** 2026-02-10 -> 2026-02-18  
**Lires de révision :** Security Engineering Guild (`@sec-eng`), Tooling Working Group (`@tooling-wg`)  
**Escopo :** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), API de streaming de preuve, gestion des manifestes Torii, intégration Sigstore/OIDC, crochets de déverrouillage CI.  
**Artefacts :**  
- Fonte do CLI et tests (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Gestionnaires de manifeste/preuve Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Automatisation des versions (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Harnais de parité déterministe (`crates/sorafs_car/tests/sorafs_cli.rs`, [SoraFS Orchestrator GA Parity Report](./orchestrator-ga-parity.md))

## Méthodologie

1. **Ateliers de modélisation des menaces** mapearam capacités d'attaque pour les postes de travail des développeurs, les systèmes CI et les nœuds Torii.  
2. **Révision du code** axé sur les surfaces d'identification (échange de jetons OIDC, signature sans clé), la validation de Norito manifeste la contre-pression dans le streaming de preuves.  
3. **Tests dynamiques** L'appareil réexécuté manifeste les modes de défaillance simulés (relecture de jeton, falsification de manifeste, flux de preuve tronqués) en utilisant le harnais de parité et les lecteurs fuzz en cours de route.  
4. **Inspection de la configuration** validation des valeurs par défaut `iroha_config`, gestion des indicateurs CLI et scripts de publication pour garantir des exécutions déterministes et des audits.  
5. **Entretien de processus** confirmant le flux de remédiation, les chemins d'escalade et la capture des preuves d'audit des propriétaires de versions effectués par Tooling WG.

## Résumé des résultats| ID | Gravité | Zone | Trouver | Résolution |
|----|----------|------|---------|------------|
| SF6-SR-01 | Élevé | Signature sans clé | Les valeurs par défaut de l'audience du jeton OIDC sont implicites dans les modèles CI, avec le risque de relecture entre locataires. | J'ai ajouté une application explicite de `--identity-token-audience` dans les hooks de publication et les modèles CI ([processus de publication](../developer-releases.md), `docs/examples/sorafs_ci.md`). O CI agora falha quando a audience e omitida. |
| SF6-SR-02 | Moyen | Preuve en streaming | Les chemins de contre-pression utilisent les tampons des abonnés sans limite, permettant l'épuisement de la mémoire. | `sorafs_cli proof stream` tailles de canal appliquées limitées avec la troncature déterminée, enregistrer les résumés Norito et abandonner le flux ; o Le miroir Torii a été mis à jour pour limiter les morceaux de réponse (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Moyen | Soumission du manifeste | La CLI se manifeste sans vérifier les plans de blocs intégrés lorsque `--plan` est ausente. | `sorafs_cli manifest submit` a ensuite recalculé et comparé CAR digère les moindres détails que `--expect-plan-digest` a fournis, rejeté les discordances et fourni des conseils de remédiation. Tests cobrem casos de sucesso/falha (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Faible | Piste d'audit | La liste de contrôle de publication contient un journal d'approbation signé pour la révision de la sécurité. | J'ai ajouté une section dans [processus de publication] (../developer-releases.md) en exigeant des hachages annexes pour le mémo de révision et l'URL du ticket de signature avant l'AG. |

Tous les achados à foram élevé/moyen corrigés au cours d'une semaine de révision et validés pour le harnais de parité existant. Nenhum émet une critique latente permanente.

## Validation du contrôle

- **Portée des informations d'identification :** Les modèles CI exigent actuellement un public et des émetteurs explicites ; o CLI et release helper falham rapido a menos que `--identity-token-audience` accompagne `--identity-token-provider`.  
- **Relecture déterministe :** Les tests ont été actualisés sur les flux positifs/négatifs de la soumission du manifeste, garantissant que les résumés incompatibles continuent d'envoyer des erreurs non déterministes et d'être exposés avant de commencer à rouge.  
- **Preuve de contre-pression de streaming :** Torii maintenant pour diffuser les PoR/PoTR dans des canaux limités, et la CLI conserve des échantillons de latence simples tronqués + 5 exemples d'échec, empêchant une croissance sans limite et des résumés de durée déterministes.  
- **Observabilité :** Compteurs de diffusion de preuves (`torii_sorafs_proof_stream_*`) et les résumés CLI capturent les raisons d'abandon, fournissant le fil d'Ariane d'audit aux opérateurs.  
- **Documentation :** Guides du développeur ([index du développeur](../developer-index.md), [référence CLI](../developer-cli.md)) les indicateurs de caméra sensibles servent à sécuriser les flux de travail d'escalade.

## Ajouts à la liste de contrôle de publication

Les gestionnaires de versions **développent** examinent les preuves suivantes du promoteur d'un candidat GA :1. Hash do security review memo le plus récent (ce document).  
2. Lien vers le ticket de remédiation rastreado (ex. : `governance/tickets/SF6-SR-2026.md`).  
3. Sortie de `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` montrant les arguments explicites du public/émetteur.  
4. Les journaux capturés font un harnais de parité (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Confirmation que les notes de version Torii incluent des compteurs de télémétrie de streaming à preuve limitée.

Ne collez pas les artefacts qui bloquent la signature de GA.

**Hashages d'artefacts de référence (approbation le 20/02/2026) :**

- `sf6_security_review.md` - `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Suivis exceptionnels

- **Actualisation du modèle de menace :** Répétez cette révision trimestrielle ou avant les grands indicateurs CLI.  
- **Couverture fuzzing :** Preuve des encodages de transport en streaming également fuzzés via `fuzz/proof_stream_transport`, identité des charges utiles cobrindo, gzip, deflate et zstd.  
- **Répétition de l'incident :** Planifiez un exercice d'opérateurs simulant une compromission de jeton et une annulation manifeste, garantissant qu'un document renvoie les procédures pratiques.

## Approbation

- Représentant de la Security Engineering Guild : @sec-eng (2026-02-20)  
- Représentant du groupe de travail sur l'outillage : @tooling-wg (2026-02-20)

Guarde approuve les assassins ainsi que la publication d'un ensemble d'artefacts.