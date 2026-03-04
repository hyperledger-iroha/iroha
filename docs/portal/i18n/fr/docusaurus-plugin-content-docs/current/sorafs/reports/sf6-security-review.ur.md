---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : SF-6 سیکیورٹی ریویو
résumé : signature sans clé, diffusion en continu de preuves, manifestes et pipelines et suivi de suivi
---

# SF-6 سیکیورٹی ریویو

**Période d'évaluation :** 2026-02-10 → 2026-02-18  
**Pistes d'examen :** Security Engineering Guild (`@sec-eng`), groupe de travail sur les outils (`@tooling-wg`)  
**Portée :** CLI/SDK SoraFS (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), API de streaming de preuve, gestion des manifestes Torii, Intégration Sigstore/OIDC, crochets de libération CI۔  
**Artefacts :**  
- Source CLI et tests (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Gestionnaires de manifeste/preuve Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Automatisation des versions (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Harnais de parité déterministe (`crates/sorafs_car/tests/sorafs_cli.rs`, [SoraFS Orchestrator GA Parity Report](./orchestrator-ga-parity.md))

## Méthodologie

1. **Ateliers de modélisation des menaces** sur les postes de travail des développeurs, les systèmes CI et les nœuds Torii et la carte des capacités des attaquants.  
2. **Révision du code** pour les surfaces d'informations d'identification (échange de jetons OIDC, signature sans clé) et validation du manifeste Norito et contre-pression de diffusion en continu de preuve pour les utilisateurs.  
3. **Tests dynamiques** Le luminaire manifeste la relecture et les modes de défaillance simulent le (relecture de jeton, la falsification manifeste, les flux de preuve tronqués) le harnais de parité et les lecteurs de fuzz sur mesure.  
4. **Inspection de la configuration** Les paramètres par défaut du `iroha_config`, la gestion des indicateurs CLI et les scripts de version valident les exécutions déterministes et vérifiables.  
5. **Entretien de processus** Flux de mesures correctives, chemins d'escalade et capture des preuves d'audit par Tooling WG et les propriétaires de versions doivent confirmer

## Résumé des résultats| ID | Gravité | Zone | Trouver | Résolution |
|----|----------|------|---------|------------|
| SF6-SR-01 | Élevé | Signature sans clé | L'audience du jeton OIDC par défaut modèle CI est implicite et relecture entre locataires. | crochets de publication et modèles CI comme `--identity-token-audience` et application explicite des modèles ([processus de publication] (../developer-releases.md), `docs/examples/sorafs_ci.md`). public omettre ہونے پر CI اب fail ہوتا ہے۔ |
| SF6-SR-02 | Moyen | Preuve en streaming | Chemins de contre-pression et tampons d'abonnés illimités en cas d'épuisement de la mémoire | Les tailles de canal limitées `sorafs_cli proof stream` appliquent une troncature déterministe et un journal des résumés Norito pour l'abandon du flux. Torii miroir et fragments de réponse mis en évidence par le miroir (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Moyen | Soumission du manifeste | La CLI présente des manifestes pour la vérification des plans de blocs intégrés et `--plan` pour la vérification des plans de blocs intégrés. | `sorafs_cli manifest submit` pour CAR digère les calculs et les comparaisons et les conseils de remédiation. ہے۔ les cas de réussite/échec des tests couvrent کرتے ہیں (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Faible | Piste d'audit | Liste de contrôle de publication et journal d'approbation signé شامل نہیں تھا۔ | [processus de publication] (../developer-releases.md) Vous pouvez télécharger un fichier et vérifier les hachages du mémo et l'URL du ticket de signature et GA et joindre le fichier joint. ہے۔ |

Fenêtre d'examen des résultats élevés/moyens pour corriger le problème et activer le harnais de parité et valider Problèmes critiques latents

## Validation du contrôle

- **Portée des informations d'identification :** Modèles CI par défaut pour audience explicite et assertions de l'émetteur pour les utilisateurs CLI pour release helper `--identity-token-audience` et `--identity-token-provider` pour un échec rapide  
- **Relecture déterministe :** Les tests mis à jour, les flux de soumission de manifestes positifs/négatifs couvrent les échecs non déterministes du réseau et les résumés non concordants. پہلے surface ہوں۔  
- **Preuve de contre-pression de streaming :** Torii pour les éléments PoR/PoTR et les canaux limités pour le flux et la CLI, ainsi que les échantillons de latence tronquée + quelques exemples d'échecs et la croissance illimitée du nombre d'abonnés. ہے جبکہ résumés déterministes برقرار رکھتا ہے۔  
- **Observabilité :** Compteurs de diffusion de preuves (`torii_sorafs_proof_stream_*`) et les résumés CLI des raisons d'abandon capturent les opérateurs et les fils d'Ariane d'audit.  
- **Documentation :** Guides du développeur ([index du développeur](../developer-index.md), [référence CLI](../developer-cli.md)) indicateurs sensibles à la sécurité et workflows de remontée d'informations

## Ajouts à la liste de contrôle de publication

Les responsables des versions et les candidats à l'AG font la promotion des preuves et des preuves ** لازمی** attachent le message :1. Ajouter un mémo et un hachage (en anglais)  
2. ticket de remédiation suivi par lien (مثال: `governance/tickets/SF6-SR-2026.md`)۔  
3. `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` produit des arguments explicites d'audience/d'émetteur.  
4. faisceau de parité کے journaux capturés (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)۔  
5. Voir les notes de version Torii et les compteurs de télémétrie en streaming à preuve limitée.

اوپر دیے گئے artefacts جمع نہ کرنا GA sign-off کو روکتا ہے۔

**Hashages d'artefacts de référence (approbation du 20/02/2026) :**

-`sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Suivis exceptionnels

- **Actualisation du modèle de menace :** Ajouts de drapeaux CLI et ajouts de drapeaux CLI.  
- **Couverture fuzzing :** Preuve des encodages de transport en continu comme `fuzz/proof_stream_transport` pour fuzz et identité, gzip, dégonflage et charges utiles zstd et couverture pour les charges utiles zstd  
- **Répétition d'incident :** compromission de jeton et annulation du manifeste et simulation d'un exercice d'opérateur et d'un exercice de l'opérateur. Les documents et les procédures pratiquées reflètent

## Approbation

- Représentant de la Security Engineering Guild : @sec-eng (2026-02-20)  
- Représentant du groupe de travail sur l'outillage : @tooling-wg (2026-02-20)

Approbations signées et libération du lot d'artefacts