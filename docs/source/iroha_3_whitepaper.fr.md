---
lang: fr
direction: ltr
source: docs/source/iroha_3_whitepaper.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 07e149429887b0dfc38cf0619552cbefcbae4dd1ec9fe9e9d47a05371ed08f29
source_last_modified: "2026-01-03T18:07:57.204376+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v3.0 (Aperçu Nexus)

Ce document présente l'architecture prospective Hyperledger Iroha v3, en se concentrant sur l'architecture multivoie
pipeline, les espaces de données Nexus et Asset Exchange Toolkit (AXT). Il complète le livre blanc Iroha v2 en
décrivant les capacités à venir qui sont activement en cours de développement.

---

## 1. Aperçu

Iroha v3 étend la base déterministe de la v2 avec une évolutivité horizontale et un interdomaine plus riche
flux de travail. La version, nommée **Nexus**, introduit :

- Un réseau unique partagé à l'échelle mondiale appelé **SORA Nexus**. Tous les pairs Iroha v3 participent à ce programme universel
  grand livre plutôt que d’opérer des déploiements isolés. Les organisations rejoignent en enregistrant leurs propres espaces de données,
  qui restent isolés pour des raisons de politique et de confidentialité tout en s’ancrant dans le grand livre commun.
- Une base de code partagée : le même référentiel construit à la fois Iroha v2 (réseaux auto-hébergés) et Iroha v3 (SORA Nexus).
  La configuration sélectionne le mode cible afin que les opérateurs puissent adopter les fonctionnalités Nexus sans changer de logiciel.
  des piles. La machine virtuelle Iroha (IVM) est identique dans les deux versions, donc les contrats et le bytecode Kotodama
  les artefacts fonctionnent de manière transparente sur les réseaux auto-hébergés et sur le grand livre mondial Nexus.
- Production de blocs à plusieurs voies pour traiter des charges de travail indépendantes en parallèle.
- Des espaces de données (DS) qui isolent les environnements d'exécution tout en restant composables via des ancres en chaîne.
- L'Asset Exchange Toolkit (AXT) pour les transferts de valeurs atomiques et inter-espaces et les swaps contrôlés par contrat.
- Fiabilité améliorée grâce à des voies Reliable Broadcast Commit (RBC), des délais déterministes et des preuves
  budgets d’échantillonnage.

Ces fonctionnalités restent en développement actif ; Les API et les mises en page peuvent évoluer avant la v3 générale
jalon de disponibilité. Reportez-vous à `nexus.md`, `nexus_transition_notes.md` et `new_pipeline.md` pour
détails au niveau de l’ingénierie.

## 2. Architecture multivoie

- **Planificateur :** Les partitions du planificateur Nexus fonctionnent en voies en fonction des identifiants d'espace de données et
  groupes de composabilité. Les voies s'exécutent en parallèle tout en préservant les garanties d'ordre déterministe au sein
  chaque voie.
- **Groupes de voies :** Les espaces de données associés partagent un `LaneGroupId`, permettant une exécution coordonnée pour les flux de travail qui
  couvrent plusieurs composants (par exemple, une CBDC DS et son dApp DS de paiement).
- **Délais :** Chaque voie suit des délais déterministes (blocage, preuve, disponibilité des données) pour garantir
  progrès et utilisation limitée des ressources.
- **Télémétrie :** les métriques au niveau des voies exposent le débit, la profondeur de la file d'attente, les violations de délais et l'utilisation de la bande passante.
  Les scripts CI affirment la présence de ces compteurs pour maintenir les tableaux de bord alignés avec le planificateur.

## 3. Espaces de données (Nexus)- **Isolement :** Chaque espace de données conserve sa propre voie de consensus, son segment d'état mondial et son stockage Kura. Ceci
  prend en charge les domaines de confidentialité tout en gardant le grand livre mondial SORA Nexus cohérent grâce à des ancres.
- **Anchors :** Les commits réguliers produisent des artefacts d'ancrage qui résument l'état de DS (racines Merkle, preuves,
  engagements) et les publier sur la voie mondiale à des fins d’audit.
- **Groupes de voies et composabilité :** Les espaces de données peuvent déclarer des groupes de composabilité autorisant l'AXT atomique
  transactions entre participants agréés. La gouvernance contrôle les changements d’adhésion et les époques d’activation.
- **Stockage avec codage d'effacement :** Les instantanés Kura et WSV adoptent les paramètres de codage d'effacement `(k, m)` pour mettre à l'échelle les données.
  disponibilité sans sacrifier le déterminisme. Les routines de récupération restaurent les fragments manquants de manière déterministe.

## 4. Boîte à outils d'échange d'actifs (AXT)

- **Descripteur et liaison :** Les clients construisent des descripteurs AXT déterministes. Les ancres de hachage `axt_binding`
  descripteurs dans des enveloppes individuelles, empêchant la relecture et garantissant que les participants au consensus valident l'octet pour-
  octet de charges utiles Norito.
- **Appels système :** Le IVM expose les appels système `AXT_BEGIN`, `AXT_TOUCH` et `AXT_COMMIT`. Les contrats déclarent leur
  ensembles de lecture/écriture par espace de données, permettant à l'hôte d'appliquer l'atomicité sur toutes les voies.
- **Poignées et époques :** Les portefeuilles obtiennent des descripteurs de capacité liés à `(dataspace_id, epoch_id, sub_nonce)`.
  Concurrent utilise les conflits de manière déterministe, renvoyant les codes canoniques `AxtTrap` lorsque les contraintes sont
  violé.
- **Application de la politique :** Les hôtes principaux dérivent désormais des instantanés de politique AXT à partir des manifestes Space Directory dans WSV,
  application des contrôles de racine de manifeste, de voie cible, d'ère d'activation, de sous-nonce et d'expiration (`current_slot >= expiry_slot`
  abandons) même dans les hôtes de test minimaux. Les politiques sont saisies par identifiant d'espace de données et construites à partir du catalogue de voies.
  les handles ne peuvent pas échapper à leur voie d’émission ou utiliser des manifestes périmés.
  - Les raisons de rejet sont déterministes : espace de données inconnu, inadéquation de racine manifeste, inadéquation de voie cible,
    handle_era en dessous de l'activation du manifeste, sub_nonce en dessous du plancher de la politique, handle expiré, contact manquant pour
    l'espace de données du handle, ou une preuve manquante si nécessaire.
- **Preuves et délais :** Durant une fenêtre active Δ, les validateurs collectent des preuves, des échantillons de disponibilité des données,
  et se manifeste. Le non-respect des délais interrompt l'AXT de manière déterministe avec des conseils pour les tentatives des clients.
- **Intégration de la gouvernance :** Les modules de politique définissent quels espaces de données peuvent participer à AXT, limite de débit
  gère et publie des manifestes conviviaux pour les auditeurs capturant les engagements, les annulateurs et les journaux d'événements.

## 5. Voies de validation de diffusion fiable (RBC)- **DA spécifique à la voie :** Les voies RBC reflètent les groupes de voies, garantissant que chaque pipeline à plusieurs voies dispose de données dédiées
  garanties de disponibilité.
- **Budgets d'échantillonnage :** Les validateurs suivent des règles d'échantillonnage déterministes (`q_in_slot_per_ds`) pour valider les preuves
  et des documents de témoins sans coordination centrale.
- **Informations sur la contre-pression :** Les événements du stimulateur cardiaque Sumeragi sont en corrélation avec les statistiques RBC pour diagnostiquer les voies bloquées
  (voir `scripts/sumeragi_backpressure_log_scraper.py`).

## 6. Opérations et migration

- **Plan de transition :** `nexus_transition_notes.md` décrit la migration progressive d'une voie unique (Iroha v2) vers
  multivoie (Iroha v3), y compris le transfert de télémétrie, le contrôle de configuration et les mises à jour Genesis.
- **Réseau universel :** Les pairs SORA Nexus exécutent une pile de genèse et de gouvernance commune. Nouveaux opérateurs à bord par
  créer un espace de données (DS) et satisfaire aux politiques d'admission Nexus au lieu de lancer des réseaux autonomes.
- **Configuration :** Les nouveaux boutons de configuration couvrent les budgets de voie, les délais de preuve, les quotas AXT et les métadonnées de l'espace de données.
  Les valeurs par défaut restent conservatrices jusqu'à ce que les opérateurs optent pour le mode Nexus.
- **Tests :** Les tests Golden capturent les descripteurs AXT, les manifestes de voies et les listes d'appels système. Tests d'intégration
  (`integration_tests/tests/repo.rs`, `crates/ivm/tests/axt_host_flow.rs`) exercent des flux de bout en bout.
- **Outils :** `kagami` gagne en génération de genèse compatible Nexus et les scripts de tableau de bord valident le débit des voies,
  budgets de preuve et santé RBC.

## 7. Feuille de route

- **Phase 1 :** Activez l'exécution multivoie sur un seul domaine avec la prise en charge et l'audit AXT locaux.
- **Phase 2 :** Activez les groupes de composabilité pour AXT inter-domaines autorisé et étendez la couverture de télémétrie.
- **Phase 3 :** Déployez une fédération complète de l'espace de données Nexus, un stockage codé avec effacement et un partage avancé de preuves.

Les mises à jour de statut sont disponibles dans `roadmap.md` et `status.md`. Les contributions alignées sur la conception Nexus devraient suivre
les politiques d’exécution et de gouvernance déterministes établies pour la v3.