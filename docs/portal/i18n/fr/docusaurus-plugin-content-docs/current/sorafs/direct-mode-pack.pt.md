---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pack en mode direct
titre : Paquet de contingence de modo direct do SoraFS (SNNet-5a)
sidebar_label : Pacote de modo directo
description : Configuration obligatoire, contrôle de conformité et étapes de déploiement pour utiliser SoraFS en mode direct Torii/QUIC lors de la transition SNNet-5a.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/direct_mode_pack.md`. Mantenha ambas comme copies synchronisées.
:::

Les circuits SoraNet suivent le transport via SoraFS, mais l'élément de la feuille de route **SNNet-5a** exige une régulation de secours pour que les opérateurs aient accès à la lecture de façon déterminée en ce qui concerne le déploiement anonymisé soit complet. Ce paquet capture les boutons de CLI/SDK, les paramètres de configuration, les tests de conformité et la liste de contrôle de déploiement nécessaire pour utiliser SoraFS en mode direct Torii/QUIC sans prendre en charge les transports de confidentialité.

La solution de secours est appliquée à la mise en scène et aux environnements de production régulés selon que SNNet-5 et SNNet-9 passent par les portes de sortie. Mantenha os artefatos abaixo junto com o materiel de déploiement do SoraFS pour que les opérateurs alternent entre les modes anonymes et directement sur demande.

## 1. Indicateurs de CLI et SDK- `sorafs_cli fetch --transport-policy=direct-only ...` désactive l'agenda des relais et des transports Torii/QUIC. L'article de la CLI vient de lister `direct-only` comme valeur exacte.
- Les SDK doivent définir `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` sans toutefois utiliser une bascule "mode direct". Comme liaisons, nous gérons les `iroha::ClientOptions` et `iroha_android` encaminham ou mesmo enum.
- Les harnais de passerelle (`sorafs_fetch`, liaisons Python) peuvent interpréter ou basculer directement uniquement via les assistants Norito JSON partagés pour qu'ils reçoivent automatiquement leur même comportement.

Documentez le drapeau des runbooks en fonction des paramètres et passez les bascules via `iroha_config` à chaque changement d'environnement.

## 2. Performances politiques de la passerelle

Utilisez Norito JSON pour conserver la configuration déterminée de l'explorateur. Le profil de l'exemple selon le code `docs/examples/sorafs_direct_mode_policy.json` :

- `transport_policy: "direct_only"` - je vous ai demandé d'annoncer le transport du relais SoraNet.
- `max_providers: 2` - Limite les pairs directement aux points de terminaison Torii/QUIC plus confidentiels. Ajuste conforme aux concessoes de conformité régionaux.
- `telemetry_region: "regulated-eu"` - rotation comme mesures émises pour que les tableaux de bord et les auditoires distinguent les exécutions de secours.
- Orcamentos de retry conservateurs (`retry_budget: 2`, `provider_failure_threshold: 3`) pour éviter de masquer les passerelles mal configurées.Utilisez JSON via `sorafs_cli fetch --config` (automatique) ou les liaisons du SDK (`config_from_json`) avant d'explorer la politique des opérateurs. Persista a saya do scoreboard (`persist_path`) para trilhas de auditoria.

Les boutons de mise en application de la passerelle sont situés sur `docs/examples/sorafs_gateway_direct_mode.toml`. Le modèle s'appuie sur `iroha app sorafs gateway direct-mode enable`, désactive les contrôles d'enveloppe/admission, connecte les valeurs par défaut de limite de débit et ajoute au tableau `direct_mode` les noms d'hôtes dérivés du plan et des résumés du manifeste. Remplacez les valeurs de l'espace réservé par votre plan de déploiement avant la version ou la configuration de la configuration.

## 3. Suite de tests de conformité

La proposition de mode directement ici inclut une couverture tant pour l'explorateur que pour nos caisses de CLI :- `direct_only_policy_rejects_soranet_only_providers` garantit que `TransportPolicy::DirectOnly` échouera rapidement lorsque chaque candidat annoncera son support aux relais SoraNet. [crates/sorafs_orchestrator/src/lib.rs:7238]
- `direct_only_policy_prefers_direct_transports_when_available` garantit que les transports Torii/QUIC sont utilisés lorsqu'ils sont disponibles et que les relais SoraNet sont exclusifs de la session. [crates/sorafs_orchestrator/src/lib.rs:7285]
- `direct_mode_policy_example_is_valid` permet d'analyser `docs/examples/sorafs_direct_mode_policy.json` pour garantir que la documentation est permanente et est destinée aux assistants. [crates/sorafs_orchestrator/src/lib.rs:7509] [docs/examples/sorafs_direct_mode_policy.json:1]
- `fetch_command_respects_direct_transports` exercé `sorafs_cli fetch --transport-policy=direct-only` contre une passerelle Torii simulée, en fournissant un test de fumée pour les ambiances régulées qui doivent être transportées directement. [crates/sorafs_car/tests/sorafs_cli.rs:2733]
- `scripts/sorafs_direct_mode_smoke.sh` implique la même commande avec le JSON de politique et la persistance du tableau de bord pour le déploiement automatique.

J'ai parcouru une suite focada antes de publicar atualizacoes :

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Si une compilation de l'espace de travail falhar por mudancas en amont, enregistrez l'erreur de blocage sur `status.md` et roulez récemment lorsque la dépendance s'actualise.

## 4. Exécute des automatisations de fuméeLa couverture de la CLI ne révèle aucune régression spécifique à l'environnement (par exemple, dérive politique de la passerelle ou divergences de manifeste). Un assistant de fumée dédié à `scripts/sorafs_direct_mode_smoke.sh` et à l'implication de `sorafs_cli fetch` dans la politique de l'explorateur en mode direct, persistance du tableau de bord et capture du curriculum vitae.

Exemple d'utilisation :

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- Le script respecte les indicateurs de CLI et les fichiers de configuration key=value (voir `docs/examples/sorafs_direct_mode_smoke.conf`). Preencha o digest do manifest e as entradas de advert de provenor com valores de producao antes de rodar.
- `--policy` tem como padrao `docs/examples/sorafs_direct_mode_policy.json`, mais tout JSON de l'explorateur produit par `sorafs_orchestrator::bindings::config_to_json` peut être fourni. La CLI s'adresse à la politique via `--orchestrator-config=PATH`, habilité à exécuter la reproduction en ajustant les drapeaux manuellement.
- Quand `sorafs_cli` n'est pas `PATH`, l'aide est compila à partir de la caisse `sorafs_orchestrator` (perfil release) pour que les fumées exercent la plomberie de manière directe envoyée.
- Saïdas :
  - Charge utile montée (`--output`, padrao `artifacts/sorafs_direct_mode/payload.bin`).
  - Résumé de récupération (`--summary`, situé à côté de la charge utile) concernant la région de télémétrie et les rapports des fournisseurs utilisés comme preuves de déploiement.
  - Instantané du tableau de bord persistant dans le chemin déclaré dans JSON de politique (par exemple, `fetch_state/direct_mode_scoreboard.json`). Arquive junto ao reumo em tickets de mudanca.- Automatisation de la porte d'entrée : après avoir récupéré, ou invoqué l'aide `cargo xtask sorafs-adoption-check` en utilisant les chemins persistants du tableau de bord et du résumé. Le quorum requis par le père et le nombre de fournisseurs fournis sur la ligne de commande ; sur `--min-providers=<n>` lorsque vous précisez un ami plus grand. Les relations avec l'avocat sont liées au lieu du CV (`--adoption-report=<path>` peuvent définir une personnalisation locale) et l'assistant passe `--require-direct-only` par le responsable (aligné vers le repli) et `--require-telemetry` toujours en demandant le correspondant du drapeau. Utilisez `XTASK_SORAFS_ADOPTION_FLAGS` pour repasser les arguments supplémentaires de xtask (par exemple `--allow-single-source` lors d'une rétrogradation approuvée pour que la porte tolère et impose le repli). Alors, lancez la porte avec `--skip-adoption-check` pour les diagnostics locaux ; La feuille de route exige que chaque exécution soit réglementée directement en incluant le bundle de relations avec les avocats.

## 5. Checklist de déploiement1. **Geler la configuration :** activer le profil JSON directement dans le référentiel `iroha_config` et enregistrer le hachage sans ticket de modification.
2. **Auditorium de la passerelle :** confirmez que les points de terminaison Torii utilisent TLS, TLV de capacité et la journalisation de l'auditorium avant de virer directement. Public ou profil politique de la passerelle pour les opérateurs.
3. **Sign-off de conformité :** partagez le playbook actualisé avec les réviseurs de conformité/réglementation et capturez les autorisations pour exploiter les forums de superposition anonymisée.
4. **Dry run :** exécutez une suite de conformité pour récupérer le staging contre les fournisseurs Torii confiaves. Archiver les sorties du tableau de bord et les résumés de la CLI.
5. **Cutover en production :** annonce la fenêtre de configuration, modifiez `transport_policy` pour `direct_only` (si vous avez opté pour `soranet-first`) et surveillez les tableaux de bord de façon directe (latence de `sorafs_fetch`, contadores de faux de proveneurs). Documentez le plan de restauration pour commencer par SoraNet lorsque SNNet-4/5/5a/5b/6a/7/8/12/13 passera à `roadmap.md:532`.
6. **Revisao pos-mudanca :** des instantanés annexes du tableau de bord, des résumés de récupération et des résultats de surveillance du ticket de mudanca. Actualisez `status.md` avec des données efficaces et toutes les anomalies.Utilisez la liste de contrôle conjointement avec le runbook `sorafs_node_ops` pour que les opérateurs puissent analyser le flux avant une virée vers le vivant. Lorsque SNNet-5 arrive à GA, retirez ou repliez pour confirmer la parité de télémétrie de production.

## 6. Exigences de preuve et porte d'avocat

Capturez en mode direct et avec précision la satisfaction de la porte de l'adocao SF-6c. L'ensemble du tableau de bord, du curriculum vitae, de l'enveloppe du manifeste et du rapport d'avocat à chaque exécution pour que `cargo xtask sorafs-adoption-check` valide la position de repli. Campos ausentes fazem o gate falhar, entretao registre o metadata esperado nos tickets de mudanca.- **Métadonnées de transport :** `scoreboard.json` doit déclarer `transport_policy="direct_only"` (et virer `transport_policy_override=true` lorsque vous souhaitez effectuer une rétrogradation). Les camps politiques anonymisés sont gardés en même temps lorsqu'ils ont des défauts de paiement pour que les réviseurs soient en mesure de supprimer le plan anonymisé dans les phases.
- **Contadores des fournisseurs :** Sesssoes gateway-only devem persister `provider_count=0` et preencher `gateway_provider_count=<n>` avec le nombre de fournisseurs Torii utilisés. Évitez de modifier manuellement le JSON : le CLI/SDK est dérivé en tant que contagieux et la porte d'entrée rejette les captures qui omettent de se séparer.
- **Evidencia de manifest:** Lorsque les passerelles Torii participent, le `--gateway-manifest-envelope <path>` est supprimé (ou équivalent sans SDK) pour que `gateway_manifest_provided` mais `gateway_manifest_id`/`gateway_manifest_cid` soient enregistrés dans `scoreboard.json`. Garanta que `summary.json` quitte le même `manifest_id`/`manifest_cid` ; un chèque d'avocat falha se qualquer arquivo omitir o par.
- **Attentes de télémétrie :** Lorsque la télémétrie accompagne la capture, montez sur la porte avec `--require-telemetry` pour que le rapport prouve que les mesures sont émises. Ensaios peut omettre le drapeau, mais CI et les billets de mudanca doivent documenter l'ausencia.

Exemple :

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```Anexe `adoption_report.json`, notamment le tableau de bord, le résumé, l'enveloppe du manifeste et le paquet de journaux de fumée. C'est un artefatos espelham o que o job de adocão em CI (`ci/check_sorafs_orchestrator_adoption.sh`) applique et déclasse le mode directement vers l'audit.