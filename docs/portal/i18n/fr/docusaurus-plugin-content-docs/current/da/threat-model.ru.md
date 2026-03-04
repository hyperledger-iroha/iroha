---
lang: fr
direction: ltr
source: docs/portal/docs/da/threat-model.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Cette page correspond à `docs/source/da/threat_model.md`. Consultez les versions suivantes
:::

# Модель угроз Disponibilité des données Sora Nexus

_Présentation: 2026-01-19 — Période de livraison: 2026-04-19_

Частота обслуживания: Groupe de travail sur la disponibilité des données (<=90 jours). Каждая
la rédaction doit être effectuée dans `status.md` pour les tickets d'activité
смягчений и артефакты симуляций.

## Cel et область

Le programme de disponibilité des données (DA) assure la diffusion Taikai,
Nexus blobs de voie et artefacts de gouvernance byzantins, historiques et opérationnels
сбоях. Ce modèle utilise le robot autonome DA-1 (architecture et modèle
угроз) et служит базовым ориентиром для дальнейших задач DA (DA-2 .. DA-10).

Composants du programme :
- Расширение Torii DA ingest et écrivains Norito métadonnées.
- Déploiement de blobs de niveaux SoraFS (niveaux chaud/froid) et de répliques politiques.
- Engagements de bloc Nexus (formats wire, preuves, API light-client).
- Hooks принудительного PDP/PoTR pour les charges utiles DA.
- Processus opérateurs (épinglage, expulsion, slashing) et pipelines d'observabilité.
- Approbations de gouvernance pour les opérateurs et le contenu des DA.Voici la documentation :
- Modèle économique polonais (intégré au flux de travail DA-7).
- Basez sur les protocoles SoraFS et découvrez le modèle de menace SoraFS.
- L'ergonomie du SDK client est très performante.

## Архитектурный обзор

1. **Soumission :** Les clients utilisent les blobs à partir de l'API d'ingestion Torii DA. Usel
   créer des blobs, coder les manifestes Norito (type blob, voie, époque, codec de drapeaux),
   et ils partagent des morceaux dans le niveau chaud SoraFS.
2. **Publicité :** Intentions d'épinglage et conseils de réplication associés
   les fournisseurs de stockage utilisent le registre (marché SoraFS) avec les balises de stratégie,
   определяющими цели rétention chaud/froid.
3. **Engagement :** Les séquenceurs Nexus incluent les engagements blob (CID + facultatif
   Racines KZG) dans le bloc canonique. Les clients légers utilisent le hachage d'engagement et
   объявленную métadonnées для проверки disponibilité.
4. **Réplication :** Les nœuds de stockage ajoutent des partages/morceaux, puis
   Les défis PDP/PoTR et les niveaux chauds/froids peuvent être améliorés en termes de politique.
5. **Récupérer :** Les utilisateurs ayant trouvé des passerelles compatibles SoraFS ou DA,
   prouver les épreuves et répondre aux demandes de réparation lors de la création de répliques.
6. **Gouvernance :** Парламент и комитет DA утверждают операторов, barèmes de loyer
   et l'application des escalades. Les artefacts de gouvernance sont à votre disposition
   прозрачности процесса.## Activités et promotions

Шкала влияния: **Critique** ломает безопасность/живучесть grand livre ; **Élevé**
блокирует DA remblayage ou clients ; **Modéré** снижает качество, но
восстановимо; **Faible** ограниченное влияние.

| Actif | Descriptif | Intégrité | Disponibilité | Confidentialité | Propriétaire |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (morceaux + manifestes) | Taikai, voie, blobs de gouvernance dans SoraFS | Critique | Critique | Modéré | DA WG / Equipe Stockage |
| Norito Manifestes DA | Types de métadonnées des blobs | Critique | Élevé | Modéré | GT sur le protocole principal |
| Bloquer les engagements | CID + racines KZG dans les blocs Nexus | Critique | Élevé | Faible | GT sur le protocole principal |
| Calendriers PDP/PoTR | Application des lois pour les répliques DA | Élevé | Élevé | Faible | Équipe de stockage |
| Registre des opérateurs | Fournisseurs de stockage et politiques | Élevé | Élevé | Faible | Conseil de gouvernance |
| Registres des loyers et des incitations | Voir grand livre pour DA rent et штрафов | Élevé | Modéré | Faible | GT Trésorerie |
| Tableaux de bord d'observabilité | DA SLO, réplications globales, alertes | Modéré | Élevé | Faible | SRE / Observabilité |
| Intentions de réparation | Projets de réhydratation des morceaux | Modéré | Modéré | Faible | Équipe de stockage |

## Protections et protections| Acteur | Возможности | Mots-clés | Première |
| --- | --- | --- | --- |
| Client malveillant | Supprimer les blobs mal formés, relire les manifestes obsolètes, ingérer DoS. | Срыв Taikai Broadcast, инъекция невалидных данных. | Il n'y a pas de clé privilégiée. |
| Nœud de stockage byzantin | Supprimez des répliques, forgez des preuves PDP/PoTR, conspirez. | Срезать DA rétention, избежать loyer, удерживать данные. | Obtenez les informations d'identification de l'opérateur valides. |
| Séquenceur compromis | Оmitez les engagements, équivoquez les blocs, réorganisez les métadonnées. | Скрыть DA soumissions, создать несогласованность. | Ограничен консенсусом большинства. |
| Opérateur initié | Améliorer la gouvernance, les politiques de rétention et les informations d'identification. | Экономическая выгода, саботаж. | Ouvrir l'installation chaud/froid. |
| Adversaire du réseau | Utilisations de partition, réplication de sauvegarde, trafic MITM. | Снижение disponibilité, деградация SLO. | Vous ne pouvez pas utiliser TLS, mais vous pouvez le faire/déplacer. |
| Attaquant d'observabilité | Affichage des tableaux de bord/alertes, suivi des incidents. | Скрыть DA pannes. | Требует доступа к pipeline de télémétrie. |

## Granités de paiement- ** Limite d'entrée : ** Client -> Extension Torii DA. Нужна аутентификация на
  Ensuite, limitation du débit et validation de la charge utile.
- ** Limite de réplication : ** Les nœuds de stockage regroupent des morceaux et des preuves. Узлы
  Il est tout à fait authentique que vous puissiez être byzantin.
- ** Limite du grand livre : ** Données de bloc validées par rapport au stockage hors chaîne. Consensus
  En raison de la rapidité, la disponibilité entraîne une application hors chaîne.
- ** Limite de gouvernance : ** Решения Conseil/Parlement по операторам, бюджетам и
  coupant. La prochaine étape consiste à effectuer le déploiement de DA.
- ** Limite d'observabilité : ** Pour les métriques/journaux et l'exportation vers les tableaux de bord/alertes
  outillage. Falsification des pannes ou des pannes.

## Scénarios pour les humains et les contremers

### Données pour ingérer du contenu

**Économie :** Un client malveillant utilise des charges utiles Norito mal formées ou
blobs surdimensionnés pour la création de ressources ou de métadonnées.**Contrôleurs**
- Validation du schéma Norito avec les versions précédentes ; rejeter les drapeaux inconnus.
- Limitation du débit et authentification pour le point de terminaison d'ingestion Torii.
- Gestion de la taille des chunks et détermination de l'encodage dans le chunker SoraFS.
- Le pipeline d'admission сохраняет manifeste только после совпадения la somme de contrôle.
- Cache de relecture déterministe (`ReplayCache`) отслеживает окна `(lane, epoch, sequence)`,
  supprimer les marques de haute mer sur le disque et supprimer les doublons/replays périmés ; propriété
  et le fuzz exploite les empreintes digitales divergentes et les soumissions dans le désordre.
  [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Problèmes spéciaux**
- Torii ingest permet d'ouvrir le cache de relecture lors de l'admission et de sauvegarder la séquence
  curseurs между рестартами.
- Les schémas Norito DA représentent votre harnais de fuzz (`fuzz/da_ingest_schema.rs`) pour
  проверки encoder/décoder инвариантов; tableaux de bord de couverture должны сигнализировать
  при регрессии.

### Réplications supplémentaires

**Éléments :** Les opérateurs de stockage byzantins attribuent des broches, mais ne les abandonnent pas.
des morceaux et le PDP/PoTR sont des réponses falsifiées ou une collusion.**Contrôleurs**
- Le calendrier des défis PDP/PoTR est défini pour les charges utiles DA selon l'époque.
- Réplication multi-sources et seuils de quorum ; récupérer l'orchestrateur
  fragments manquants et réparation.
- La réduction de la gouvernance entraîne des preuves échouées et des répliques manquantes.
- Travail de rapprochement automatisé (`cargo xtask da-commitment-reconcile`)
  ingérer les reçus avec les engagements DA (SignedBlockWire/`.norito`/JSON), formulaire
  Ensemble de preuves JSON pour la gouvernance et pour les tickets manquants/incohérents,
  Alertmanager peut détecter toute omission/altération.

**Problèmes spéciaux**
- Harnais de simulation `integration_tests/src/da/pdp_potr.rs` (tests :
  `integration_tests/tests/da/pdp_potr_simulation.rs`) теперь покрывает collusion
  et la partition, проверяя детерминированное выявление byzantin. Produire
  расширение вместе с DA-5.
- Les expulsions politiques à froid trahissent une piste d'audit signée, ce qui implique des gouttes secrètes.

### Engagements à venir

**Élément :** Un séquenceur compromis publie des blocs avec le propriétaire/l'utilisateur de DA
engagements, vous récupérez les échecs et les incohérences légères des clients.**Contrôleurs**
- Консенсус проверяет bloquer les propositions против DA files d'attente de soumission ; pairs
  отвергают предложения без обязательных engagements.
- Les clients légers prouvent les preuves d'inclusion avant de récupérer les poignées.
- Piste d'audit сравнивает les reçus de soumission et bloquer les engagements.
- Travail de rapprochement automatisé (`cargo xtask da-commitment-reconcile`)
  ingérer les reçus et les engagements DA (SignedBlockWire/`.norito`/JSON), formulaire
  Ensemble de preuves JSON et support pour les tickets manquants/incohérents pour Alertmanager.

**Problèmes spéciaux**
- Travail de réconciliation Закрыто + crochet Alertmanager ; les paquets de gouvernance sont là
  Vous pouvez ingérer un ensemble de preuves JSON.

### Partition réseau et partition

**Élément :** L'adversaire exploite le réseau de réplication, je peux l'utiliser
Créez des morceaux ou récupérez des défis PDP/PoTR.

**Contrôleurs**
- Le fournisseur multirégional s'occupe des chemins réseau.
- Défiez Windows contre la gigue et le repli en cas de réparation hors bande.
- Les tableaux de bord d'observabilité surveillent la profondeur de réplication, le succès des défis et
  récupérer la latence et les seuils d'alerte.

**Problèmes spéciaux**
- Simulations de partitions pour les événements en direct de Taikai; нужны tests de trempage.
- La politique de récupération de la bande passante n'est pas formalisée.

### Внутреннее злоупотребление**Сценарий:** Opérateur с доступом к registre манипулирует rétention политиками,
liste blanche des fournisseurs malveillants ou diffusion d'alertes.

**Contrôleurs**
- Les actions de gouvernance impliquent des signatures multipartites et des actes notariés Norito.
- Les changements de politique sont publiés dans les journaux de surveillance et d'archivage.
- Application du pipeline d'observabilité avec ajout uniquement des journaux Norito et chaînage de hachage.
- Automatisation de l'examen trimestriel des accès (`cargo xtask da-privilege-audit`)
  manifest/replay директории (плюс пути от операторов), отмечает manquant/non-répertoire/
  entrées inscriptibles dans le monde entier et un ensemble JSON signé pour les tableaux de bord de gouvernance.

**Problèmes spéciaux**
- La preuve d'inviolabilité du tableau de bord détecte les instantanés signés.

## Récupérer les risques liés à l'obésité| Risque | Probabilité | Impact | Propriétaire | Plan d'atténuation |
| --- | --- | --- | --- | --- |
| Replay DA manifeste pour le cache de séquence DA-2 | Possible | Modéré | GT sur le protocole principal | Réaliser le cache de séquence + validation occasionnelle dans DA-2 ; faire des tests de régression. |
| Collusion PDP/PoTR pour les transactions >f uslovs | Peu probable | Élevé | Équipe de stockage | Découvrez le nouveau calendrier des défis avec l'échantillonnage multi-fournisseurs ; валидировать через harnais de simulation. |
| Écart d’audit d’expulsion de niveau froid | Possible | Élevé | SRE / Equipe Stockage | Prendre des journaux d'audit signés et des reçus en chaîne pour les expulsions ; surveiller les tableaux de bord. |
| Séquenceur d'omission de latence | Possible | Élevé | GT sur le protocole principal | Actuellement, `cargo xtask da-commitment-reconcile` compare les reçus par rapport aux engagements (SignedBlockWire/`.norito`/JSON) et assure la gouvernance des tickets manquants/incohérents. |
| Résilience de partition pour les diffusions en direct de Taikai | Possible | Critique | Réseautage TL | Perceuses à cloisons Provesti; зарезервировать réparer la bande passante; documenter le basculement SOP. |
| Dérive des privilèges de gouvernance | Peu probable | Élevé | Conseil de gouvernance | Trimestriel `cargo xtask da-privilege-audit` (répertoires de manifeste/relecture + chemins supplémentaires) avec JSON signé + porte du tableau de bord ; ancrer les artefacts d’audit sur la chaîne. |

## Suivis requis1. Publier les schémas Norito pour l'ingestion DA et les exemples de vecteurs (dans DA-2).
2. Sauvegarder le cache de relecture à partir de l'ingestion Torii DA et sauvegarder les curseurs de séquence
   при рестартах узлов.
3. **Terminé (2026-02-05) :** Harnais de simulation PDP/PoTR pour le modèle
   collusion + partition с Modélisation du backlog de QoS ; см. `integration_tests/src/da/pdp_potr.rs`
   (tests : `integration_tests/tests/da/pdp_potr_simulation.rs`) avec des produits nettoyants.
4. **Terminé (2026-05-29) :** `cargo xtask da-commitment-reconcile` сравнивает
   ingérer les reçus avec les engagements DA (SignedBlockWire/`.norito`/JSON), эмитирует
   `artifacts/da/commitment_reconciliation.json` et module Alertmanager/gouvernance
   paquets pour les alertes d’omission/falsification (`xtask/src/da.rs`).
5. **Terminé (2026-05-29) :** `cargo xtask da-privilege-audit` проходит manifeste/replay
   spool (et chemins d'accès des opérateurs), indique manquant/non-répertoire/inscriptible dans le monde entier et
   Générer un bundle JSON signé pour les examens du tableau de bord/de la gouvernance
   (`artifacts/da/privilege_audit.json`), écart entre l'automatisation de l'examen des accès.

**Где смотреть дальше:**- Cache de relecture et curseurs de persistance atterris dans DA-2. Réalisation dans
  `crates/iroha_core/src/da/replay_cache.rs` (logique de cache) et intégration Torii dans
  `crates/iroha_torii/src/da/ingest.rs`, les contrôles d'empreintes digitales sont effectués à partir de `/v1/da/ingest`.
- Les simulations de streaming PDP/PoTR utilisent le harnais proof-stream dans
  `crates/sorafs_car/tests/sorafs_cli.rs`, recherche des flux de requêtes PoR/PDP/PoTR et
  scénarios d'échec из модели угроз.
- Capacité et réparation des résultats de trempage
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, matrice de trempage Sumeragi
  `docs/source/sumeragi_soak_matrix.md` (variantes localisées). Eti
  les artefacts фиксируют долгие forets из реестра рисков.
- Réconciliation + automatisation des audits de privilèges
  `docs/automation/da/README.md` et nouvelles commandes
  `cargo xtask da-commitment-reconcile`/`cargo xtask da-privilege-audit` ; utiliser
  les sorties peuvent être intégrées dans `artifacts/da/` en ajoutant des preuves aux paquets de gouvernance.

## Preuves de simulation et modélisation de la qualité de service (2026-02)

En effectuant le suivi DA-1 #3, vous pouvez déterminer le PDP/PoTR
faisceau de simulation в `integration_tests/src/da/pdp_potr.rs` (tests :
`integration_tests/tests/da/pdp_potr_simulation.rs`). Harnais распределяет
Dans 3 régions, il y a des partitions/collusions qui sont conformes à la feuille de route,
Réduire le retard du PoTR et le modèle de retard de réparation, en utilisant le niveau chaud
budget de réparation. Scénario par défaut (12 époques, 18 défis PDP + 2 PoTR)
Windows par époque) pour les mesures suivantes :<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrique | Valeur | Remarques |
| --- | --- | --- |
| Pannes PDP détectées | 48 / 49 (98,0%) | Les partitions все еще детектируются; единственный недетектированный сбой связан с честным gigue. |
| Latence de détection moyenne PDP | 0,0 époques | Сбои фиксируются в исходном époque. |
| Défaillances PoTR détectées | 28 / 77 (36,4%) | Détectez les erreurs pour un projet >=2 fenêtres PoTR, en installant un grand nombre de personnes dans le registre des risques résiduels. |
| Latence de détection moyenne PoTR | époques 2.0 | Соответствует Seuil de retard de 2 époques dans l'escalade des archives. |
| Pic de la file d'attente de réparation | 38 manifestes | L'arriéré est terminé, les partitions s'accumulent 4 réparations/époque. |
| Latence de réponse p95 | 30 068 ms | Ouvre une fenêtre de défi de 30 s avec une gigue de +/-75 ms pour l'échantillonnage QoS. |
<!-- END_DA_SIM_TABLE -->

Ces résultats vous permettront de créer des tableaux de bord DA et de définir des critères
Les exemples de "faisceau de simulation + modélisation QoS" sont une feuille de route.

Автоматизация находится за
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
Vous pouvez obtenir le harnais et émettre Norito JSON dans
`artifacts/da/threat_model_report.json` à utiliser. Les nouvelles utilisations
C'est un guide pour l'information sur les documents et les alertes concernant la dérive des taux de détection,
files d'attente de réparation ou échantillons de QoS.Для обновления таблицы выше используйте `make docs-da-threat-model`, что вызывает
`cargo xtask da-threat-model-report`, avant
`docs/source/da/_generated/threat_model_report.json`, и переписывает секцию через
`scripts/docs/render_da_threat_model_tables.py`. Verre `docs/portal`
(`docs/portal/docs/da/threat-model.md`) обновляется в том же проходе для синхронизации.