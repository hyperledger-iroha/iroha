---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Отчет по безопасности SF-6
Résumé : Résultats et procédures ultérieures pour les opérations non autorisées : signature sans clé, diffusion en continu de preuves et ouverture de manifestes.
---

# Ouvrir la fenêtre de sécurité SF-6

**Détails :** 2026-02-10 → 2026-02-18  
**Provisions :** Guilde d'ingénierie de sécurité (`@sec-eng`), groupe de travail sur les outils (`@tooling-wg`)  
**Область:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), API de streaming de preuve, création de manifestes dans Torii, intégration Sigstore/OIDC, crochets de déverrouillage CI.  
**Articles :**  
- CLI et tests (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Manifeste/preuve des gestionnaires Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Automatisation des versions (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Harnais de parité déterministe (`crates/sorafs_car/tests/sorafs_cli.rs`, [Отчет о паритете GA SoraFS Orchestrator](./orchestrator-ga-parity.md))

## Métologie

1. **Ateliers de modélisation des menaces** pour les utilisateurs du système CI et des utilisateurs Torii.  
2. **Révision du code** s'intéresse aux données personnelles les plus avancées (avec les jetons OIDC, signature sans clé), valide les manifestes Norito et la contre-pression dans le streaming de preuves.  
3. **Tests dynamiques** utilisent des manifestes d'appareils et des simulations (relecture de jetons, falsification de manifeste, flux de preuve d'utilisation) avec un harnais de parité et des lecteurs de fuzz spéciaux.  
4. **Inspection de la configuration** a vérifié les valeurs par défaut `iroha_config`, en travaillant sur les drapeaux CLI et les scripts de publication, pour examiner les programmes de détection et d'écoute.  
5. **Entretien de processus** pour améliorer le flux de remédiation, les voies d'escalade et la collecte des preuves d'audit de manière globale auprès des propriétaires de versions Tooling WG.

## Сводка находок| ID | Gravité | Zone | Trouver | Résolution |
|----|----------|------|---------|------------|
| SF6-SR-01 | Élevé | Signature sans clé | Les jetons d'audit par défaut OIDC ne sont pas présents dans les modèles CI, ce qui présente un risque de relecture entre locataires. | Il est désormais possible de tester `--identity-token-audience` dans les hooks de publication et les modèles CI ([processus de publication](../developer-releases.md), `docs/examples/sorafs_ci.md`). CI теперь падает, если аудитория не указана. |
| SF6-SR-02 | Moyen | Preuve en streaming | Les chemins de contre-pression sont principalement des tampons vides qui peuvent être utilisés. | `sorafs_cli proof stream` permet de configurer les canaux avec la troncature, d'enregistrer les résumés Norito et d'abandonner le processus ; Le miroir Torii est mis en œuvre pour gérer les fragments de réponse (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Moyen | Отправка manifeste | La CLI se manifeste principalement sans la vérification des plans de fragments établis, comme `--plan`. | `sorafs_cli manifest submit` vous permet de rechercher et de corriger les résumés CAR, si vous n'utilisez pas `--expect-plan-digest`, d'éliminer les incompatibilités et de fournir des conseils de correction. Les tests effectués sur les appareils/appareils (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Faible | Piste d'audit | Dans la liste de contrôle de publication, vous ne pouvez pas passer en revue l'examen de sécurité. | Le processus de publication [processus de publication] (../developer-releases.md) a été effectué et la révision du mémo de hachage et la signature du ticket d'URL ont été effectuées avant GA. |

Les niveaux élevés/moyens sont utilisés à un moment donné et permettent de créer un harnais de parité. Les problèmes critiques ne sont pas résolus.

## Contrôle de validation

- **Portée des informations d'identification :** Les modèles CI permettent de traiter votre audience et l'émetteur ; CLI et release helper sont disponibles, si `--identity-token-audience` ne se trouve pas dans `--identity-token-provider`.  
- **Relecture déterministe :** Les tests récents révèlent les détails des manifestes/déclarations non concordantes, garantissant que les résumés incompatibles entraînent des problèmes. ошибками и выявляются до обращения к сети.  
- **Preuve de contre-pression de streaming :** Torii permet de définir les éléments PoR/PoTR à partir des canaux externes, et la CLI utilise uniquement la latence des échantillons + latence des échantillons, предотвращая неограниченный рост подписчиков и сохраняя детерминированные résumés.  
- **Observabilité :** Fichiers de diffusion de preuves (`torii_sorafs_proof_stream_*`) et résumés CLI qui fixent les options d'abandon, permettant à l'opérateur d'auditer le fil d'Ariane.  
- **Documentation :** Les informations pour les utilisateurs ([index du développeur](../developer-index.md), [référence CLI](../developer-cli.md)) suppriment les flux de travail d'escalade et d'escalade sensibles à la sécurité.

## Liste de contrôle de mise à jour et de publication

Les gestionnaires de versions **обязаны** приложить следующие доказательства при продвижении GA кандидата :

1. Hash последнего mémo examen de sécurité (этот документ).  
2. Recherchez la correction du ticket (par exemple, `governance/tickets/SF6-SR-2026.md`).  
3. Sortie `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` pour votre public/émetteur.  
4. Faisceau de parité Log (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Mise à jour, les notes de version Torii incluent les compteurs de télémétrie en streaming de preuves limitées.La suppression des éléments d'art bloque la signature de l'AG.

**Hash de référence артефактов (signature 2026-02-20) :**

-`sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Suivis d'installation

- **Actualisation du modèle de menace :** Ajoutez cette révision automatiquement ou avant d'ajouter des drapeaux CLI.  
- **Couverture fuzzing :** Les codes de transport proof streaming fuzz'ятся через `fuzz/proof_stream_transport`, охватывая payloads Identity, gzip, deflate et zstd.  
- **Répétition de l'incident :** Planification de l'exploitation de l'opérateur, simulation de la transaction et du manifeste de restauration, pour documenter l'exploitation du robot. процедуры.

## Approbation

- Guilde d'ingénierie de sécurité précédente : @sec-eng (2026-02-20)  
- Groupe de travail sur l'outillage précédent : @tooling-wg (2026-02-20)

Vous pouvez obtenir des approbations lors de la publication du lot d'artefacts.