---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pack en mode direct
titre : Paquet disponible pour la demande SoraFS (SNNet-5a)
sidebar_label : Paquet de programme
description : Configuration de la configuration, vérification de la sécurité et configuration du fonctionnement du robot SoraFS selon le programme Torii/QUIC Il s'agit de l'ancien SNNet-5a.
---

:::note Канонический источник
:::

Les contours SoraNet assurent le transport pour SoraFS, mais les cartes **SNNet-5a** ne peuvent pas être réglées de manière régulière, par exemple. Les opérateurs peuvent prendre en charge le dosage du détergent à l'heure actuelle, afin de régler les problèmes de manière anonyme. Ce paquet contient des paramètres de configuration CLI/SDK, des configurations de profils, des preuves de résolution de problèmes et une liste de contrôle des mises à jour, des problèmes pour les robots SoraFS dans le cadre du programme Torii/QUIC sans surveillance des transports privés.

Fallback est utilisé pour la mise en scène et la production régulière, car SNNet-5 à SNNet-9 ne fournit pas vos portes. Pour les articles d'art qui concernent la modification matérielle SoraFS, les opérateurs peuvent se connecter de manière anonyme et des remèdes pour le stress.

## 1. Drapeau CLI et SDK- `sorafs_cli fetch --transport-policy=direct-only ...` ouvre le rapport de planification et utilise principalement le transport Torii/QUIC. La CLI met en œuvre le `direct-only` en cas de problème.
- Le SDK permet d'utiliser `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` à ce moment-là, en prévoyant le « programme ». Les liaisons générées dans `iroha::ClientOptions` et `iroha_android` correspondent à cette énumération.
- Gateway-harness (`sorafs_fetch`, Python-bindinging) peut activer directement directement uniquement en utilisant Norito JSON, qui est automatique получала такое же поведение.

Ajoutez le drapeau dans le runbook pour les partenaires et envoyez directement le code `iroha_config`, qui n'est pas disponible de manière temporaire.

## 2. Passerelle de profils politiques

Utilisez Norito JSON pour déterminer la configuration de l'opérateur. Le profil principal dans `docs/examples/sorafs_direct_mode_policy.json` correspond :

- `transport_policy: "direct_only"` — Ouvrez les fournisseurs qui récupèrent les transports liés à SoraNet.
- `max_providers: 2` — Sélectionnez les entrées Torii/QUIC. Assurez-vous de respecter les normes régionales.
- `telemetry_region: "regulated-eu"` — Mesures de marquage, de bord et d'audits de secours.
- Les ports de conservation (`retry_budget: 2`, `provider_failure_threshold: 3`) pour cette raison ne sont pas masqués par la passerelle existante.Ajoutez JSON à partir de `sorafs_cli fetch --config` (automatique) ou de liaison SDK (`config_from_json`) pour les opérateurs politiques de publication. Placez votre tableau d'affichage (`persist_path`) pour les auditeurs.

Les paramètres de configuration de la passerelle de stockage sont indiqués dans `docs/examples/sorafs_gateway_direct_mode.toml`. Shablon vous donne accès au `iroha app sorafs gateway direct-mode enable`, en ouvrant l'enveloppe/l'admission, en ajoutant les valeurs par défaut au taux limite et en inscrivant le tableau `direct_mode` des hôtes. плана и digest значениями manifeste. Indiquez les paramètres d'espace réservé pour votre déploiement plan actuel avant de fragmenter la configuration du système.

## 3. La réponse est la suivante

La procédure à suivre doit inclure l'achat par l'opérateur, ainsi que par les touches CLI :- `direct_only_policy_rejects_soranet_only_providers` garantit que `TransportPolicy::DirectOnly` est en charge, lorsque l'annonce du candidat prend en charge ce rôle. SoraNet.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` garantit l'utilisation de Torii/QUIC, ainsi que le téléchargement et l'utilisation de SoraNet. sessions.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` parsite `docs/examples/sorafs_direct_mode_policy.json`, pour que la documentation soit disponible avec утилитами-хелперами.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` a mis en place un `sorafs_cli fetch --transport-policy=direct-only` pour le moteur de passerelle Torii, un test de fumée préalable pour un câblage régulier, des spécifications прямые транспорты.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` utilise votre tableau de bord de configuration et de synchronisation JSON pour le déploiement automatique.

Veuillez consulter les tests avant la publication :

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Si l'espace de travail de votre entreprise permet une gestion en amont, bloquez l'ouverture de votre session dans `status.md` et ouvrez-la après la mise à jour. зависимости.

## 4. Programmes de fumée automatiquesLa CLI ne permet pas de régression spécifique à l'ouverture (par exemple, la passerelle politique ou les manifestes non souhaités). Un ancien script de fumée a été ajouté au `scripts/sorafs_direct_mode_smoke.sh` et au `sorafs_cli fetch` de l'organisateur politique du régime, du tableau de bord et du tableau de bord. résumé.

Exemple d'utilisation :

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- Le script apparaît et le bouton CLI, et le fichier de configuration clé=valeur (par exemple `docs/examples/sorafs_direct_mode_smoke.conf`). Avant de publier le résumé du manifeste et de publier une annonce, le fournisseur a produit des informations.
- `--policy` lors de la suppression du code `docs/examples/sorafs_direct_mode_policy.json`, vous pouvez précéder l'opérateur JSON du générateur `sorafs_orchestrator::bindings::config_to_json`. CLI est responsable de la politique concernant `--orchestrator-config=PATH`, qui s'occupe des problèmes liés aux drapeaux locaux.
- Si `sorafs_cli` n'est pas dans `PATH`, il s'agit d'un programme de fumée approuvé par `sorafs_orchestrator` (profil de version). поставляемую схему прямого режима.
- Выходные данные:
  - Charge utile interne (`--output`, pour la référence `artifacts/sorafs_direct_mode/payload.bin`).
  - Récupération du résumé (`--summary`, pour la prise en charge de la charge utile), des télémétries de région et des services de déploiement pour les bases de documentation.
  - Tableau de bord d'écran, basé sur les politiques JSON (par exemple, `fetch_state/direct_mode_scoreboard.json`). Enregistrez-le dans le résumé des billets.- Porte d'adoption automatique : après avoir récupéré le script, vous obtenez `cargo xtask sorafs-adoption-check`, en utilisant le tableau de bord et le résumé. Le personnel doit s'occuper de la nécessité de fournir des services à la course commandée ; Veuillez utiliser `--min-providers=<n>` si vous ne disposez pas d'un grand modèle. Adoption-отчеты пишутся рядом сумолчанию добавляет `--require-direct-only` (в (réponses de secours) et `--require-telemetry`, si vous n'utilisez pas le drapeau approprié. Utilisez `XTASK_SORAFS_ADOPTION_FLAGS` pour les arguments doubles xtask (par exemple, `--allow-single-source` lors de la rétrogradation de votre ordinateur, de votre porte et de votre navigateur, et repli forcé). Пропускайте adoption gate с `--skip-adoption-check` только при локальной диагностике; Il s'agit d'une feuille de route qui devrait être régulièrement mise en œuvre dans le cadre du régime d'adoption du bundle.

## 5. Liste de contrôle1. **Configuration :** définissez le profil JSON en utilisant le référentiel `iroha_config` et installez-le dans la configuration du ticket.
2. **Sur la passerelle :** assurez-vous que l'extrémité Torii utilise TLS, la capacité TLV et le journal audio à définir à l'heure prévue. Publiez le profil de la passerelle politique pour les opérateurs.
3. **Conformité :** Créez un playbook innovant avec des rapports de conformité/réglementation et précisez l'évolution du robot de manière anonyme.
4. **Dry run :** vérifiez les tests de conformité et les tests de récupération de staging fournis par les fournisseurs Torii. Enregistrez les sorties du tableau de bord et le résumé CLI.
5. **Prise en charge du produit :**, vérifiez bien les détails, sélectionnez `transport_policy` sur `direct_only` (si vous avez sélectionné `soranet-first`) et ensuite за дашбордами прямого режима (latence `sorafs_fetch`, счетчики сбоев провайдеров). Documentez le plan pour SoraNet-first après cela, comme SNNet-4/5/5a/5b/6a/7/8/12/13 avant `roadmap.md:532`.
6. **Présentation :** utilisez le tableau de bord des images, la récupération du résumé et la surveillance des résultats de la visualisation des tickets. Обновите `status.md` s'il y a des anomalies dans le système et les anomalies de lubrification.

Consultez cette liste de vérification du runbook `sorafs_node_ops` pour que les opérateurs puissent exécuter le processus de sélection de votre ordinateur. Lorsque SNNet-5 est disponible en GA, vous pouvez utiliser le système de repli après avoir activé la partition dans les systèmes de télémétrie du fournisseur.## 6. Recherche de documents et porte d'adoption

Nous sommes en train de préparer le portail d'adoption SF-6c. Téléchargez le tableau de bord, le résumé, l'enveloppe du manifeste et l'option d'adoption pour le processus d'adoption, comme `cargo xtask sorafs-adoption-check` pour vérifier la solution de secours. L'installation de la porte d'entrée doit permettre de déterminer la métadonnée des tickets d'entrée.- **Метаданные транспорта:** `scoreboard.json` doit sélectionner `transport_policy="direct_only"` (et activer `transport_policy_override=true` si vous souhaitez rétrograder). Il est possible que les politiques anonymes s'appliquent aux paramètres par défaut, ce qui permet de vérifier les vidéos d'ouverture du plan de service anonyme.
- **Fournisseurs :** Les sessions de passerelle uniquement permettent de connecter `provider_count=0` et de mettre en place le processeur `gateway_provider_count=<n>` Torii. Ne pas utiliser JSON : CLI/SDK vous permet d'accéder à des fichiers, une porte d'adoption s'ouvrant sur votre emplacement personnel.
- **Déclarez le manifeste :** Lorsque vous utilisez les passerelles Torii, avant de télécharger `--gateway-manifest-envelope <path>` (ou SDK), vous devez utiliser `gateway_manifest_provided` et `gateway_manifest_id`/`gateway_manifest_cid` sont utilisés dans `scoreboard.json`. Alors, que `summary.json` correspond à `manifest_id`/`manifest_cid` ; проверка adoption провалится, если любой файл не содержит пару.
- **Ожидания телеметрии:** Когда телеметрия сопровождает захват, запускайте gate с `--require-telemetry`, чтобы adoption-отчет подтвердил отправку métrique. Les répétitions à intervalle d'air peuvent produire un drapeau, mais les CI et les billets fournissent des documents de sortie.

Exemple :

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

Utilisez `adoption_report.json` pour le tableau de bord, le résumé, l'enveloppe du manifeste et le paquet de logos de fumée. Cet article propose un travail d'adoption chez CI (`ci/check_sorafs_orchestrator_adoption.sh`) et permet d'auditer le déclassement.