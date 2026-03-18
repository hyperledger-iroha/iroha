---
lang: fr
direction: ltr
source: docs/source/governance_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9f765fbe3170f654a9c44c3cd1afc5d82a72ff49137f32b98cf9d310faf114e
source_last_modified: "2026-01-03T18:08:00.913452+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% du pipeline de gouvernance (Iroha 2 et SORA Parlement)

# État actuel (v1)
- Les propositions de gouvernance sont présentées comme suit : proposant → référendum → décompte → promulgation. Les fenêtres référendaires et les seuils de participation/approbation sont appliqués comme décrit dans `gov.md` ; les verrous sont uniquement extensibles et se déverrouillent à l'expiration.
- La sélection du Parlement utilise des tirages au sort basés sur le VRF avec un ordre déterministe et des limites de termes ; lorsqu'aucune liste persistante n'existe, Torii dérive une solution de secours en utilisant la configuration `gov.parliament_*`. Le contrôle du conseil et les contrôles de quorum sont effectués dans les tests `gov_parliament_bodies` / `gov_pipeline_sla`.
- Modes de vote : ZK (par défaut, nécessite `Active` VK avec octets en ligne) et Plain (poids quadratique). Les discordances de mode sont rejetées ; la création/extension du verrou est monotone dans les deux modes avec des tests de régression pour ZK et des revotes simples.
- L'inconduite du validateur est traitée via le pipeline de preuves (`/v1/sumeragi/evidence*`, assistants CLI) avec des transferts par consensus conjoints appliqués par `NextMode` + `ModeActivationHeight`.
- Les espaces de noms protégés, les hooks de mise à niveau d'exécution et l'admission du manifeste de gouvernance sont documentés dans `governance_api.md` et couverts par la télémétrie (`governance_manifest_*`, `governance_protected_namespace_total`).

# En vol/arriéré
- Publier les artefacts du tirage VRF (seed, proof, roster commandé, suppléants) et codifier les règles de remplacement en cas de non-présentation ; ajoutez des luminaires dorés pour le tirage au sort et les remplacements.
- L'application des SLA par étapes pour les organes du Parlement (règles → ordre du jour → étude → examen → jury → promulgation) nécessite des minuteries explicites, des chemins d'escalade et des compteurs de télémétrie.
- Le vote secret/engagement-révélation du jury politique et les audits de résistance à la corruption associés doivent encore être mis en œuvre.
- Les multiplicateurs de liens de rôle, les réductions de mauvaise conduite pour les corps à haut risque et les temps de recharge entre les emplacements de service nécessitent une plomberie de configuration ainsi que des tests.
- L'étanchéité des voies de gouvernance et les fenêtres/barrières de participation référendaires sont suivies dans `gov.md`/`status.md` ; garder les entrées de la feuille de route à jour au fur et à mesure que les tests d'acceptation restants arrivent.