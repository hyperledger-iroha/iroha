---
lang: fr
direction: ltr
source: docs/source/android_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da7119ab99121dbcfc268f5406f43b16ac9149cef6500a45c6717ad16c02ab80
source_last_modified: "2026-01-28T17:01:56.615899+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Runbook des opérations du SDK Android

Ce runbook prend en charge les opérateurs et les ingénieurs d'assistance gérant le SDK Android.
déploiements pour AND7 et au-delà. Associez-le au Playbook de support Android pour SLA
définitions et chemins d’escalade.

> **Remarque :** Lors de la mise à jour des procédures d'incident, actualisez également les
> matrice de dépannage (`docs/source/sdk/android/troubleshooting.md`) pour que le
> La table de scénarios, les SLA et les références de télémétrie restent alignés sur ce runbook.

## 0. Démarrage rapide (lorsque les téléavertisseurs se déclenchent)

Gardez cette séquence à portée de main pour les alertes Sev1/Sev2 avant de plonger dans les détails.
rubriques ci-dessous :

1. **Confirmez la configuration active :** Capturez la somme de contrôle du manifeste `ClientConfig`
   émis au démarrage de l'application et comparez-le au manifeste épinglé dans
   `configs/android_client_manifest.json`. Si les hachages divergent, arrêtez les versions et
   déposez un ticket de dérive de configuration avant de toucher à la télémétrie/aux remplacements (voir §1).
2. **Exécutez la porte de comparaison de schéma :** Exécutez la CLI `telemetry-schema-diff` sur
   l'instantané accepté
   (`docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`).
   Traitez toute sortie `policy_violations` comme Sev2 et bloquez les exportations jusqu'à ce que le
   la divergence est comprise (voir §2.6).
3. **Vérifiez les tableaux de bord + l'état CLI :** Ouvrez la rédaction de télémétrie Android et
   Conseils de santé des exportateurs, puis exécutez
   `scripts/telemetry/check_redaction_status.py --status-url <collector>`. Si
   les autorités sont sous le plancher ou exportent une erreur, capturent des captures d'écran et
   Sortie CLI pour le document d'incident (voir §2.4–§2.5).
4. **Décidez des remplacements :** Uniquement après les étapes ci-dessus et avec incident/propriétaire
   enregistré, émettez une dérogation limitée via `scripts/android_override_tool.sh`
   et enregistrez-le dans `telemetry_override_log.md` (voir §3). Expiration par défaut : <24h.
5. **Escalade par liste de contacts :** Appelez le TL d'astreinte Android et d'observabilité
   (contacts au §8), puis suivez l'arbre de remontée au §4.1. Si une attestation ou
   Les signaux StrongBox sont impliqués, tirez le dernier bundle et exécutez le harnais
   vérifications du §7 avant de réactiver les exportations.

## 1. Configuration et déploiement

- **ClientConfig sourcing :** Assurez-vous que les clients Android chargent le point de terminaison Torii, TLS
  politiques et réessayez les boutons à partir des manifestes dérivés de `iroha_config`. Valider
  valeurs lors du démarrage de l’application et somme de contrôle du journal du manifeste actif.
  Référence d'implémentation : `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  fils `TelemetryOptions` de `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryOptions.java`
  (plus le `TelemetryObserver` généré) afin que les autorités hachées soient émises automatiquement.
- **Rechargement à chaud :** Utilisez l'observateur de configuration pour récupérer `iroha_config`.
  mises à jour sans redémarrage de l'application. Les rechargements échoués devraient émettre le
  Événement `android.telemetry.config.reload` et nouvelle tentative de déclenchement avec exponentielle
  interruption (maximum 5 tentatives).
- **Comportement de repli :** Lorsque la configuration est manquante ou invalide, revenez à
  valeurs par défaut sécurisées (mode lecture seule, pas de soumission de file d'attente en attente) et affichage d'un utilisateur
  invite. Enregistrez l'incident pour le suivi.

### 1.1 Diagnostic de rechargement de la configuration- Le config watcher émet des signaux `android.telemetry.config.reload` avec
  `source`, `result`, `duration_ms` et champs facultatifs `digest`/`error` (voir
  `configs/android_telemetry.json` et
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ConfigWatcher.java`).
  Attendez-vous à un seul événement `result:"success"` par manifeste appliqué ; répété
  Les enregistrements `result:"error"` indiquent que l'observateur a épuisé ses 5 tentatives d'interruption
  à partir de 50 ms.
- Lors d'un incident, capturer le dernier signal de rechargement du collecteur
  (magasin OTLP/span ou point de terminaison de l'état de rédaction) et enregistrez le `digest` +
  `source` dans le document d'incident. Comparez le résumé à
  `configs/android_client_manifest.json` et le manifeste de version distribué à
  opérateurs.
- Si l'observateur continue d'émettre des erreurs, exécutez le harnais ciblé pour reproduire
  l'échec de l'analyse avec le manifeste suspect :
  `ci/run_android_tests.sh org.hyperledger.iroha.android.client.ConfigWatcherTests`.
  Joignez la sortie du test et le manifeste d'échec au groupe d'incidents afin que SRE
  peut le comparer au schéma de configuration cuit.
- Lorsque la télémétrie de rechargement est manquante, confirmez que le `ClientConfig` actif porte un
  récepteur de télémétrie et que le collecteur OTLP accepte toujours le
  ID `android.telemetry.config.reload` ; sinon, traitez-le comme une télémétrie Sev2
  régression (même chemin que §2.4) et relâchement en pause jusqu'au retour du signal.

### 1.2 Bundles d'exportation de clés déterministes
- Les exportations de logiciels émettent désormais des bundles v3 avec sel + nonce par exportation, `kdf_kind` et `kdf_work_factor`.
  L'exportateur préfère Argon2id (64 MiB, 3 itérations, parallélisme = 2) et revient à
  PBKDF2-HMAC-SHA256 avec un plancher d'itération de 350 000 lorsque Argon2id n'est pas disponible sur l'appareil. Forfait
  AAD se lie toujours à l'alias ; les phrases secrètes doivent comporter au moins 12 caractères pour les exportations v3 et le
  l'importateur rejette les graines sans sel/nonce.
  `KeyExportBundle.decode(Base64|bytes)`, importez avec la phrase secrète d'origine et réexportez vers la v3 vers
  passer au format mémoire dure. L'importateur rejette les paires sel/nonce entièrement nulles ou réutilisées ; toujours
  faites pivoter les bundles au lieu de réutiliser les anciennes exportations entre les appareils.
- Tests de chemin négatif dans `ci/run_android_tests.sh --tests org.hyperledger.iroha.android.crypto.export.DeterministicKeyExporterTests`
  rejet. Effacez les tableaux de caractères de phrase secrète après utilisation et capturez à la fois la version du bundle et `kdf_kind`.
  dans les notes d'incident lorsque la récupération échoue.

## 2. Télémétrie et rédaction

> Référence rapide : voir
> [`telemetry_redaction_quick_reference.md`](sdk/android/telemetry_redaction_quick_reference.md)
> pour la liste de contrôle condensée des commandes/seuils utilisée lors de l'activation
> sessions et ponts d'incidents.- **Inventaire des signaux :** Voir `docs/source/sdk/android/telemetry_redaction.md`
  pour la liste complète des spans/métriques/événements émis et
  `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`
  pour les détails du propriétaire/validation et les lacunes en suspens.
- **Différence de schéma canonique :** L'instantané AND7 approuvé est
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`.
  Chaque nouvelle exécution CLI doit être comparée à cet artefact afin que les réviseurs puissent voir
  que les `intentional_differences` et `android_only_signals` acceptés sont toujours
  correspondre aux tableaux de politiques documentés dans
  `docs/source/sdk/android/telemetry_schema_diff.md` §3. La CLI ajoute maintenant
  `policy_violations` lorsqu'il manque une différence intentionnelle
  `status:"accepted"`/`"policy_allowlisted"` (ou lorsque les enregistrements Android uniquement perdent
  leur statut accepté), traitez donc les violations non vides comme Sev2 et arrêtez
  exportations. Les extraits `jq` ci-dessous restent à titre de contrôle manuel de l'intégrité des fichiers archivés.
  artefacts :
  ```bash
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted")' "$OUT"
  jq '.android_only_signals[] | select(.status != "accepted")' "$OUT"
  jq '.field_mismatches[] | {signal, field, android, rust}' "$OUT"
  ```
  Traitez toute sortie de ces commandes comme une régression de schéma nécessitant un
  Bogue de préparation à AND7 avant la poursuite des exportations de télémétrie ; `field_mismatches`
  doit rester vide selon `telemetry_schema_diff.md` §5. L'assistant écrit maintenant
  `artifacts/android/telemetry/schema_diff.prom` automatiquement ; passer
  `--textfile-dir /var/lib/node_exporter/textfile_collector` (ou définir
  `ANDROID_SCHEMA_DIFF_TEXTFILE_DIR`) lors de l'exécution sur des hôtes de test/production
  donc la jauge `telemetry_schema_diff_run_status` passe à `policy_violation`
  automatiquement si la CLI détecte une dérive.
- **Assistant CLI :** `scripts/telemetry/check_redaction_status.py` inspecte
  `artifacts/android/telemetry/status.json` par défaut ; passer `--status-url` à
  préparation de requête et `--write-cache` pour actualiser la copie locale pour la mise hors ligne
  exercices. Utilisez `--min-hashed 214` (ou définissez
  `ANDROID_TELEMETRY_MIN_HASHED_AUTHORITIES=214`) pour faire respecter la gouvernance
  plancher sur les autorités hachées lors de chaque sondage de statut.
- **Hachage d'autorité :** Toutes les autorités sont hachées à l'aide de Blake2b-256 avec le
  sel de rotation trimestriel stocké dans le coffre-fort secret sécurisé. Les rotations se produisent sur
  le premier lundi de chaque trimestre à 00h00 UTC. Vérifiez que l'exportateur récupère
  le nouveau sel en vérifiant la métrique `android.telemetry.redaction.salt_version`.
- **Paquets de profils d'appareil :** uniquement `emulator`, `consumer` et `enterprise`.
  les niveaux sont exportés (avec la version majeure du SDK). Les tableaux de bord les comparent
  compte par rapport aux lignes de base de Rust ; une variance >10 % déclenche des alertes.
- **Métadonnées réseau :** Android exporte uniquement les indicateurs `network_type` et `roaming`.
  Les noms de transporteurs ne sont jamais émis ; les opérateurs ne devraient pas demander à l'abonné
  informations dans les journaux d’incidents. L'instantané nettoyé est émis en tant que
  Événement `android.telemetry.network_context` ; assurez-vous donc que les applications enregistrent un
  `NetworkContextProvider` (soit via
  `ClientConfig.Builder.setNetworkContextProvider(...)` ou la commodité
  `enableAndroidNetworkContext(...)`) avant l'émission des appels Torii.
- **Pointeur Grafana :** Le tableau de bord `Android Telemetry Redaction` est le
  vérification visuelle canonique de la sortie CLI ci-dessus : confirmez le
  Le panneau `android.telemetry.redaction.salt_version` correspond à l'époque actuelle du sel
  et le widget `android_telemetry_override_tokens_active` reste à zéro
  chaque fois qu’aucun exercice ou incident n’est en cours. Escalader si l'un des panneaux dérive
  avant que les scripts CLI ne signalent une régression.

### 2.1 Flux de travail du pipeline d'exportation1. **Distribution de configuration.** `ClientConfig.telemetry.redaction` est threadé depuis
   `iroha_config` et rechargé à chaud par `ConfigWatcher`. Chaque rechargement enregistre le
   le résumé manifeste plus l'époque du sel - capturez cette ligne lors des incidents et pendant
   répétitions.
2. **Instrumentation.** Les composants du SDK émettent des étendues/métriques/événements dans le
   `TelemetryBuffer`. Le tampon marque chaque charge utile avec le profil de l'appareil et
   l'époque actuelle du sel afin que l'exportateur puisse vérifier les entrées de hachage de manière déterministe.
3. **Filtre de rédaction.** `RedactionFilter` hache `authority`, `alias` et
   identifiants de l’appareil avant qu’ils ne quittent l’appareil. Les échecs émettent
   `android.telemetry.redaction.failure` et bloquez la tentative d'exportation.
4. **Exportateur + collecteur.** Les charges utiles désinfectées sont expédiées via Android
   Exportateur OpenTelemetry vers le déploiement `android-otel-collector`. Le
   sorties des ventilateurs du collecteur vers les traces (Tempo), les métriques (Prometheus) et Norito
   les bûches coulent.
5. **Crochets d'observabilité.** `scripts/telemetry/check_redaction_status.py` lit
   compteurs collecteurs (`android.telemetry.export.status`,
   `android.telemetry.redaction.salt_version`) et produit le bundle d'état
   référencé tout au long de ce runbook.

### 2.2 Portes de validation

- **Différence de schéma :** Exécuter
  `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json`
  chaque fois que manifeste un changement. Après chaque analyse, confirmez chaque
  Les entrées `intentional_differences[*]` et `android_only_signals[*]` sont estampillées
  `status:"accepted"` (ou `status:"policy_allowlisted"` pour les fichiers hachés/groupés
  champs) comme recommandé dans `telemetry_schema_diff.md` §3 avant de joindre le
  artefact aux incidents et aux rapports de laboratoire du chaos. Utiliser l'instantané approuvé
  (`android_vs_rust-20260305.json`) comme garde-corps et pelucher le fraîchement émis
  JSON avant son dépôt :
  ```bash
  LATEST=docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted") | {signal, field, status}' "$LATEST"
  jq '.android_only_signals[] | select(.status != "accepted") | {signal, status}' "$LATEST"
  ```
  Comparez `$LATEST` avec
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`
  pour prouver que la liste blanche est restée inchangée. `status` manquant ou vide
  entrées (par exemple sur `android.telemetry.redaction.failure` ou
  `android.telemetry.redaction.salt_version`) sont désormais traités comme des régressions et
  doit être résolu avant que l'examen puisse être clôturé ; la CLI fait apparaître les valeurs acceptées
  indiquer directement, de sorte que la référence croisée du manuel §3.4 ne s'applique que lorsque
  expliquant pourquoi un statut non-`accepted` apparaît.

  **Signaux canoniques AND7 (instantané du 05/03/2026)**| Signalisation | Chaîne | Statut | Note de gouvernance | Crochet de validation |
  |--------|---------|--------|-----------------|-----------------|
  | `android.telemetry.redaction.override` | Événement | `accepted` | Les miroirs remplacent les manifestes et doivent correspondre aux entrées `telemetry_override_log.md`. | Regardez `android_telemetry_override_tokens_active` et archivez les manifestes conformément au §3. |
  | `android.telemetry.network_context` | Événement | `accepted` | Android supprime intentionnellement les noms des opérateurs ; seuls `network_type` et `roaming` sont exportés. | Assurez-vous que les applications enregistrent un `NetworkContextProvider` et confirmez que le volume de l'événement suit le trafic Torii sur `Android Telemetry Overview`. |
  | `android.telemetry.redaction.failure` | Compteur | `accepted` | Émet chaque fois que le hachage échoue ; la gouvernance nécessite désormais des métadonnées de statut explicites dans l'artefact de différence de schéma. | Le panneau du tableau de bord `Redaction Compliance` et la sortie CLI de `check_redaction_status.py` doivent rester à zéro, sauf pendant les exercices. |
  | `android.telemetry.redaction.salt_version` | Jauge | `accepted` | Prouve que l'exportateur utilise l'époque trimestrielle actuelle du sel. | Comparez le widget salt de Grafana avec l'époque du coffre-fort de secrets et assurez-vous que les exécutions de différences de schéma conservent l'annotation `status:"accepted"`. |

  Si une entrée dans le tableau ci-dessus laisse tomber un `status`, l'artefact diff doit être
  régénéré **et** `telemetry_schema_diff.md` mis à jour avant l'AND7
  le dossier de gouvernance est diffusé. Incluez le JSON actualisé dans
  `docs/source/sdk/android/readiness/schema_diffs/` et liez-le à partir du
  incident, laboratoire de chaos ou rapport d'activation qui a déclenché la réexécution.
- **Couverture CI/unité :** `ci/run_android_tests.sh` doit être réussi avant
  publication de versions ; la suite applique le comportement de hachage/remplacement en exerçant
  les exportateurs de télémétrie avec des exemples de charges utiles.
- **Contrôles d'intégrité des injecteurs :** Utilisation
  `scripts/telemetry/inject_redaction_failure.sh --dry-run` avant les répétitions
  pour confirmer l'échec de l'injection qui fonctionne et qui alerte le feu lors du hachage des gardes
  sont déclenchés. Toujours effacer l'injecteur avec `--clear` une fois validation
  complète.

### 2.3 Mobile ↔ Liste de contrôle de parité de télémétrie Rust

Gardez les exportateurs Android et les services de nœud Rust alignés tout en respectant les
différentes exigences de rédaction documentées dans
`docs/source/sdk/android/telemetry_redaction.md`. Le tableau ci-dessous sert de
double liste autorisée référencée dans l'entrée de la feuille de route AND7 : mettez-la à jour chaque fois que le
schema diff introduit ou supprime des champs.| Catégorie | Exportateurs Android | Services de rouille | Crochet de validation |
|--------------|---------|---------------|-----------------|
| Contexte d'autorité/itinéraire | Hachez `authority`/`alias` via Blake2b-256 et supprimez les noms d'hôte Torii bruts avant l'exportation ; émettent `android.telemetry.redaction.salt_version` pour prouver la rotation du sel. | Émettez les noms d’hôte Torii complets et les ID d’homologue pour la corrélation. | Comparez les entrées `android.torii.http.request` et `torii.http.request` dans la dernière différence de schéma sous `readiness/schema_diffs/`, puis confirmez que `android.telemetry.redaction.salt_version` correspond au sel de cluster en exécutant `scripts/telemetry/check_redaction_status.py`. |
| Identité de l'appareil et du signataire | Bucket `hardware_tier`/`device_profile`, hachez les alias du contrôleur et n'exportez jamais les numéros de série. | Aucune métadonnée de l'appareil ; les nœuds émettent le validateur `peer_id` et le contrôleur `public_key` textuellement. | Mettez en miroir les mappages dans `docs/source/sdk/mobile_device_profile_alignment.md`, auditez les sorties `PendingQueueInspector` pendant les laboratoires et assurez-vous que les tests de hachage d'alias dans `ci/run_android_tests.sh` restent verts. |
| Métadonnées du réseau | Exportez uniquement les booléens `network_type` + `roaming` ; `carrier_name` est supprimé. | Rust conserve les noms d'hôtes homologues ainsi que les métadonnées complètes des points de terminaison TLS. | Stockez le dernier JSON diff dans `readiness/schema_diffs/` et confirmez que le côté Android omet toujours `carrier_name`. Alerte si le widget « Contexte réseau » de Grafana affiche des chaînes porteuses. |
| Preuve d'annulation/chaos | Émettez des événements `android.telemetry.redaction.override` et `android.telemetry.chaos.scenario` avec des rôles d'acteur masqués. | Les services Rust émettent des approbations de remplacement sans masquage de rôle et sans étendues spécifiques au chaos. | Vérifiez `docs/source/sdk/android/readiness/and7_operator_enablement.md` après chaque exercice pour vous assurer que les jetons de remplacement et les artefacts du chaos sont archivés aux côtés des événements Rust non masqués. |

Flux de travail de parité :

1. Après chaque modification de manifeste ou d'exportateur, exécutez
   `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json --textfile-dir /var/lib/node_exporter/textfile_collector`
   donc l'artefact JSON et les métriques en miroir atterrissent tous deux dans le lot de preuves
   (l'assistant écrit toujours `artifacts/android/telemetry/schema_diff.prom` par défaut).
2. Examinez la différence par rapport au tableau ci-dessus ; si Android émet maintenant un champ qui est
   uniquement autorisé sur Rust (ou vice versa), déposez un bug de préparation AND7 et mettez à jour
   le plan de rédaction.
3. Lors des contrôles hebdomadaires, exécutez
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   pour confirmer que les époques de sel correspondent au widget Grafana et notez l'époque dans le
   journal de garde.
4. Enregistrez tous les deltas dans
   `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` donc
   la gouvernance peut auditer les décisions paritaires.

### 2.4 Tableaux de bord d'observabilité et seuils d'alerte

Gardez les tableaux de bord et les alertes alignés sur les approbations des différences de schéma AND7 lorsque
examen de la sortie `scripts/telemetry/check_redaction_status.py` :

- `Android Telemetry Redaction` — Widget d'époque Salt, remplacement de la jauge de jeton.
- `Redaction Compliance` — Compteur `android.telemetry.redaction.failure` et
  panneaux de tendance des injecteurs.
- `Exporter Health` — Pannes tarifaires `android.telemetry.export.status`.
- `Android Telemetry Overview` — compartiments de profil de périphérique et volume de contexte réseau.

Les seuils suivants reflètent la fiche de référence rapide et doivent être appliqués
pendant la réponse aux incidents et les répétitions :| Métrique / panneau | Seuil | Actions |
|----------------|-----------|--------|
| `android.telemetry.redaction.failure` (carte `Redaction Compliance`) | >0 sur une fenêtre glissante de 15 minutes | Examinez le signal défaillant, exécutez l'effacement de l'injecteur, enregistrez la sortie CLI + capture d'écran Grafana. |
| `android.telemetry.redaction.salt_version` (carte `Android Telemetry Redaction`) | Diffère de l'époque du sel du coffre-fort secret | Arrêtez les versions, coordonnez-vous avec la rotation des secrets, enregistrez la note AND7. |
| `android.telemetry.export.status{status="error"}` (carte `Exporter Health`) | >1% des exportations | Inspectez l’état du collecteur, capturez les diagnostics CLI, passez à SRE. |
| Parité `android.telemetry.device_profile{tier="enterprise"}` vs Rust (`Android Telemetry Overview`) | Écart > 10 % par rapport à la ligne de base de Rust | Suivi de la gouvernance des fichiers, vérification des pools de luminaires, annotation des artefacts de différences de schéma. |
| Volume `android.telemetry.network_context` (`Android Telemetry Overview`) | Tombe à zéro tant que le trafic Torii existe | Confirmez l’enregistrement `NetworkContextProvider`, réexécutez la comparaison de schéma pour garantir que les champs restent inchangés. |
| `android.telemetry.redaction.override` / `android_telemetry_override_tokens_active` (`Android Telemetry Redaction`) | Fenêtre de remplacement/perçage approuvée à l'extérieur, non nulle | Associez le jeton à un incident, régénérez le résumé, révoquez via le workflow au §3. |

### 2.5 Préparation des opérateurs et parcours d'activation

L'élément AND7 de la feuille de route prévoit un programme d'études dédié aux opérateurs afin que le support, le SRE et
les parties prenantes de la version comprennent les tables de parité ci-dessus avant le lancement du runbook
GA. Utilisez le plan dans
`docs/source/sdk/android/telemetry_readiness_outline.md` pour la logistique canonique
(ordre du jour, présentateurs, calendrier) et `docs/source/sdk/android/readiness/and7_operator_enablement.md`
pour la liste de contrôle détaillée, les liens de preuves et le journal des actions. Conservez ce qui suit
phases synchronisées chaque fois que le plan de télémétrie change :| Phases | Descriptif | Paquet de preuves | Propriétaire principal |
|-------|-------------|-------|-------------------|
| Distribution pré-lue | Envoyez la pré-lecture de la politique, `telemetry_redaction.md`, et la carte de référence rapide au moins cinq jours ouvrables avant le briefing. Suivez les accusés de réception dans le journal de communication du plan. | `docs/source/sdk/android/telemetry_readiness_outline.md` (Logistique de session + Journal des communications) et l'e-mail archivé dans `docs/source/sdk/android/readiness/archive/<YYYY-MM>/`. | Gestionnaire de documents/support |
| Séance de préparation en direct | Offrez la formation de 60 minutes (analyse approfondie des politiques, présentation du runbook, tableaux de bord, démonstration du laboratoire du chaos) et continuez à exécuter l'enregistrement pour les spectateurs asynchrones. | Enregistrement + diapositives stockées sous `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` avec les références capturées au §2 du plan. | LLM (propriétaire par intérim d'AND7) |
| Exécution du laboratoire du chaos | Exécutez au moins C2 (remplacement) + C6 (relecture de la file d'attente) à partir de `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` immédiatement après la session en direct et joignez les journaux/captures d'écran au kit d'activation. | Rapports de scénario et captures d'écran dans `docs/source/sdk/android/readiness/labs/reports/<YYYY-MM>/` et `/screenshots/<YYYY-MM>/`. | Observabilité Android TL + SRE de garde |
| Contrôle des connaissances et présence | Collectez les soumissions aux quiz, corrigez toute personne ayant obtenu un score <90 % et enregistrez les statistiques de participation/quiz. Gardez les questions de référence rapide alignées sur la liste de contrôle de parité. | Exportations du quiz dans `docs/source/sdk/android/readiness/forms/responses/`, résumé Markdown/JSON produit via `scripts/telemetry/generate_and7_quiz_summary.py` et tableau de présence dans `and7_operator_enablement.md`. | Ingénierie d'assistance |
| Archives & suivis | Mettez à jour le journal des actions du kit d'activation, téléchargez les artefacts dans l'archive et notez l'achèvement dans `status.md`. Tous les jetons de correction ou de remplacement émis pendant la session doivent être copiés dans `telemetry_override_log.md`. | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 (journal des actions), `.../archive/<YYYY-MM>/checklist.md` et le journal de remplacement référencé au §3. | LLM (propriétaire par intérim d'AND7) |

Lorsque le programme est réexécuté (tous les trimestres ou avant des modifications majeures du schéma), actualisez
le plan avec la nouvelle date de la session, garder la liste des participants à jour et
régénérer les artefacts JSON/Markdown du résumé du quiz afin que les paquets de gouvernance puissent
faire référence à des preuves cohérentes. L'entrée `status.md` pour AND7 doit être liée au
dernier dossier d'archive une fois chaque sprint d'activation terminé.

### 2.6 Listes autorisées de différences de schéma et vérifications de stratégie

La feuille de route appelle explicitement une politique de double liste autorisée (expurgations mobiles vs
Rétention de rouille) qui est appliquée par la CLI `telemetry-schema-diff` hébergée sous
`tools/telemetry-schema-diff`. Chaque artefact différent enregistré dans
`docs/source/sdk/android/readiness/schema_diffs/` doit documenter quels champs sont
hachés/groupés sur Android, quels champs restent non hachés sur Rust et si
tout signal non autorisé s'est glissé dans la construction. Capturez ces décisions
directement dans le JSON en exécutant :

```bash
cargo run -p telemetry-schema-diff -- \
  --android-config configs/android_telemetry.json \
  --rust-config configs/rust_telemetry.json \
  --format json \
  > "$LATEST"

if jq -e '.policy_violations | length > 0' "$LATEST" >/dev/null; then
  jq '.policy_violations[]' "$LATEST"
  exit 1
fi
```Le `jq` final est évalué comme non opérationnel lorsque le rapport est propre. Traitez n'importe quelle sortie
à partir de cette commande en tant que bogue de préparation Sev2 : un `policy_violations` renseigné
tableau signifie que la CLI a découvert un signal qui ne figure ni sur la liste Android uniquement
ni sur la liste d'exemption pour Rust uniquement documentée dans
`docs/source/sdk/android/telemetry_schema_diff.md`. Lorsque cela se produit, arrêtez
exporte, dépose un ticket AND7 et réexécute la comparaison uniquement après le module de stratégie
et les instantanés du manifeste ont été corrigés. Stockez le JSON résultant dans
`docs/source/sdk/android/readiness/schema_diffs/` avec le suffixe de date et la note
le chemin à l'intérieur du rapport d'incident ou de laboratoire afin que la gouvernance puisse rejouer les contrôles.

**Matrice de hachage et de rétention**

| Signal.champ | Gestion d'Android | Traitement de la rouille | Balise de liste verte |
|--------------|-----------------|---------------|---------------|
| `torii.http.request.authority` | Blake2b-256 haché (`representation: "blake2b_256"`) | Verbatim stocké pour la traçabilité | `policy_allowlisted` (hachage mobile) |
| `attestation.result.alias` | Blake2b-256 haché | Alias ​​de texte brut (archives d'attestation) | `policy_allowlisted` |
| `attestation.result.device_tier` | En godet (`representation: "bucketed"`) | Chaîne de niveau simple | `policy_allowlisted` |
| `hardware.profile.hardware_tier` | Absent — Les exportateurs Android abandonnent complètement le terrain | Présent sans rédaction | `rust_only` (documenté au §3 de `telemetry_schema_diff.md`) |
| `android.telemetry.redaction.override.*` | Signal uniquement Android avec rôles d'acteurs masqués | Aucun signal équivalent émis | `android_only` (doit rester `status:"accepted"`) |

Lorsque de nouveaux signaux apparaissent, ajoutez-les au module de stratégie de différence de schéma **et** au
tableau ci-dessus afin que le runbook reflète la logique d’application fournie dans la CLI.
L'exécution du schéma échoue désormais si un signal Android uniquement omet un `status` explicite ou si
le tableau `policy_violations` n'est pas vide, alors gardez cette liste de contrôle synchronisée avec
`telemetry_schema_diff.md` §3 et les derniers instantanés JSON référencés dans
`telemetry_redaction_minutes_*.md`.

## 3. Remplacer le flux de travail

Les remplacements sont l'option « briser la vitre » lors du hachage des régressions ou de la confidentialité
les alertes bloquent les clients. Appliquez-les uniquement après avoir enregistré la piste de décision complète
dans le document d'incident.1. **Confirmez la dérive et la portée.** Attendez l'alerte PagerDuty ou la différence de schéma
   porte à feu, puis cours
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` à
   prouver des autorités incompatibles. Joignez la sortie CLI et les captures d'écran Grafana
   au dossier d'incident.
2. **Préparez une demande signée.** Remplir
   `docs/examples/android_override_request.json` avec l'identifiant du ticket, le demandeur,
   expiration et justification. Conservez le fichier à côté des artefacts de l'incident afin
   la conformité peut auditer les entrées.
3. **Émettez le remplacement.** Invoquez
   ```bash
   scripts/android_override_tool.sh apply \
     --request docs/examples/android_override_request.json \
     --log docs/source/sdk/android/telemetry_override_log.md \
     --out artifacts/android/telemetry/override-$(date -u +%Y%m%dT%H%M%SZ).json \
     --event-log docs/source/sdk/android/readiness/override_logs/override_events.ndjson \
     --actor-role <support|sre|docs|compliance|program|other>
   ```
   L'assistant imprime le jeton de remplacement, écrit le manifeste et ajoute une ligne
   au journal d'audit Markdown. Ne publiez jamais le jeton dans le chat ; livrez-le directement
   aux opérateurs Torii appliquant la dérogation.
4. **Surveillez l'effet.** Dans les cinq minutes, vérifiez un seul
   L'événement `android.telemetry.redaction.override` a été émis, le collecteur
   le point de terminaison d'état indique `override_active=true` et le document d'incident répertorie les
   expiration. Regardez le tableau de bord « Remplacer les jetons » du tableau de bord Présentation de la télémétrie Android.
   panneau actif »(`android_telemetry_override_tokens_active`) pour le même
   compte de jetons et continuez à exécuter la CLI d'état toutes les 10 minutes jusqu'à ce que
   le hachage se stabilise.
5. **Révoquer et archiver.** Dès que l'atténuation arrive, exécutez
  `scripts/android_override_tool.sh revoke --token <token>` donc le journal d'audit
  capture l'heure de révocation, puis exécute
  `scripts/android_override_tool.sh digest --out docs/source/sdk/android/readiness/override_logs/override_digest_$(date -u +%Y%m%dT%H%M%SZ).json`
  pour actualiser l’instantané aseptisé attendu par la gouvernance. Attachez le
  manifeste, résumé JSON, transcriptions CLI, instantanés Grafana et journal NDJSON
  produit via `--event-log` à
  `docs/source/sdk/android/readiness/screenshots/<date>/` et réticulez le
  entrée de `docs/source/sdk/android/telemetry_override_log.md`.

Les dérogations dépassant 24 heures nécessitent l'approbation du directeur SRE et de la conformité.
doivent être soulignés dans la prochaine revue hebdomadaire AND7.

### 3.1 Remplacer la matrice d'escalade

| Situation | Durée maximale | Approbateurs | Notifications requises |
|-----------|--------------|---------------|------------------------|
| Enquête sur un seul locataire (incompatibilité d'autorité hachée, client Sev2) | 4 heures | Ingénieur support + SRE d'astreinte | Ticket `SUP-OVR-<id>`, événement `android.telemetry.redaction.override`, journal des incidents |
| Panne de télémétrie à l'échelle de la flotte ou reproduction demandée par SRE | 24 heures | SRE d'astreinte + Responsable de programme | Remarque PagerDuty, remplacement de l'entrée du journal, mise à jour dans `status.md` |
| Demande de conformité/investigation ou tout cas dépassant 24 heures | Jusqu'à révocation explicite | Directeur SRE + Responsable Conformité | Liste de diffusion de gouvernance, journal de remplacement, statut hebdomadaire AND7 |

#### Responsabilités du rôle| Rôle | Responsabilités | ANS / Remarques |
|------|--------|-------------|
| Télémétrie Android d'astreinte (Incident Commander) | Pilotez la détection, exécutez les outils de remplacement, enregistrez les approbations dans le document d'incident et assurez-vous que la révocation a lieu avant l'expiration. | Accusez réception de PagerDuty dans les 5 minutes et enregistrez la progression toutes les 15 minutes. |
| Observabilité Android TL (Haruka Yamamoto) | Validez le signal de dérive, confirmez l’état de l’exportateur/collecteur et signez le manifeste de dérogation avant qu’il ne soit remis aux opérateurs. | Rejoignez le pont en 10 minutes ; déléguez au propriétaire du cluster intermédiaire s’il n’est pas disponible. |
| Liaison SRE (Liam O'Connor) | Appliquez le manifeste aux collecteurs, surveillez le retard et coordonnez-vous avec Release Engineering pour les atténuations côté Torii. | Enregistrez chaque action `kubectl` dans la demande de modification et collez les transcriptions des commandes dans le document d'incident. |
| Conformité (Sofia Martins / Daniel Park) | Approuvez les remplacements de plus de 30 minutes, vérifiez la ligne du journal d'audit et donnez des conseils sur la messagerie régulateur/client. | Publier un accusé de réception dans `#compliance-alerts` ; pour les événements de production, déposer une note de conformité avant que la dérogation ne soit émise. |
| Gestionnaire de documents/support (Priya Deshpande) | Archivez les manifestes/sorties CLI sous `docs/source/sdk/android/readiness/…`, gardez le journal de remplacement bien rangé et planifiez des laboratoires de suivi si des lacunes apparaissent. | Confirme la conservation des preuves (13 mois) et effectue AND7 suivis avant de clôturer l'incident. |

Escalader immédiatement si un jeton de remplacement approche de son expiration sans
plan de révocation documenté.

## 4. Réponse aux incidents

- **Alertes :** Le service PagerDuty `android-telemetry-primary` couvre la rédaction
  pannes, pannes d’exportateur et dérive du godet. Accuser réception dans les fenêtres SLA
  (voir playbook de support).
- **Diagnostics :** Exécutez `scripts/telemetry/check_redaction_status.py` pour collecter
  état de santé actuel des exportateurs, alertes récentes et mesures d’autorité hachées. Inclure
  sortie dans la chronologie de l’incident (`incident/YYYY-MM-DD-android-telemetry.md`).
- **Tableaux de bord :** Surveillez la rédaction de la télémétrie Android, la télémétrie Android
  Tableaux de bord Présentation, Conformité des rédactions et Santé de l'exportateur. Capturer
  captures d'écran pour les enregistrements d'incidents et annoter toute version salée ou remplacement
  écarts symboliques avant de clôturer un incident.
- **Coordination :** Engager l'ingénierie de publication pour les problèmes liés aux exportateurs et à la conformité
  pour les questions de remplacement/PII, et le responsable du programme pour les incidents de niveau 1.

### 4.1 Flux de remontée

Les incidents Android sont triés en utilisant les mêmes niveaux de gravité que les incidents Android.
Support Playbook (§2.1). Le tableau ci-dessous résume qui doit être bipé et comment
rapidement, chaque intervenant devrait rejoindre le pont.| Gravité | Impact | Intervenant principal (≤5min) | Escalade secondaire (≤10min) | Notifications supplémentaires | Remarques |
|--------------|--------|----------------------------|--------------------------------|------------------------------|-------|
| Septembre1 | Panne côté client, violation de la vie privée ou fuite de données | Télémétrie Android d'astreinte (`android-telemetry-primary`) | Torii de garde + Responsable de programme | Conformité + Gouvernance SRE (`#sre-governance`), propriétaires de clusters de transfert (`#android-staging`) | Démarrez immédiatement War Room et ouvrez un document partagé pour la journalisation des commandes. |
| Septembre2 | Dégradation de la flotte, utilisation abusive du contournement ou retard de relecture prolongé | Télémétrie Android de garde | Android Foundations TL + Docs/Support Manager | Responsable du programme, liaison avec l'ingénierie des versions | Transférer vers la conformité si les remplacements dépassent 24 heures. |
| Septembre3 | Problème de locataire unique, répétition en laboratoire ou alerte consultative | Ingénieur support | Android de garde (facultatif) | Docs/Soutien à la sensibilisation | Convertissez en Sev2 si la portée s'étend ou si plusieurs locataires sont affectés. |

| Fenêtre | Actions | Propriétaire(s) | Preuve/Notes |
|--------|--------|----------|----------------|
| 0 à 5 minutes | Accusez réception de PagerDuty, attribuez un commandant d'incident (IC) et créez `incident/YYYY-MM-DD-android-telemetry.md`. Supprimez le lien et le statut d'une ligne dans `#android-sdk-support`. | SRE d'astreinte / Ingénieur support | Capture d'écran de l'accusé de réception PagerDuty + stub d'incident commis à côté d'autres journaux d'incidents. |
| 5 à 15 minutes | Exécutez `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` et collez le résumé dans le document d'incident. Ping Android Observability TL (Haruka Yamamoto) et le responsable du support (Priya Deshpande) pour un transfert en temps réel. | IC + Android Observabilité TL | Joignez le JSON de sortie CLI, notez les URL du tableau de bord ouvertes et marquez à qui appartient les diagnostics. |
| 15 à 25 minutes | Engagez les propriétaires de clusters de préparation (Haruka Yamamoto pour l'observabilité, Liam O'Connor pour SRE) à reproduire sur `android-telemetry-stg`. Chargez avec `scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg` et capturez les vidages de file d'attente à partir de l'émulateur Pixel + pour confirmer la parité des symptômes. | Propriétaires de clusters de staging | Téléchargez la sortie `pending.queue` + `PendingQueueInspector` nettoyée dans le dossier des incidents. |
| 25 à 40 minutes | Décidez des remplacements, de la limitation Torii ou du repli StrongBox. Si une exposition de données personnelles ou un hachage non déterministe est suspecté, contactez la conformité (Sofia Martins, Daniel Park) via `#compliance-alerts` et informez le responsable du programme dans le même fil d'incident. | IC + Conformité + Responsable de programme | Jetons de remplacement de lien, manifestes Norito et commentaires d’approbation. |
| ≥40min | Fournissez des mises à jour de statut toutes les 30 minutes (notes PagerDuty + `#android-sdk-support`). Planifiez le pont de salle de guerre s'il n'est pas déjà actif, documentez l'ETA d'atténuation et assurez-vous que l'ingénierie de publication (Alexei Morozov) est en attente pour lancer les artefacts du collecteur/SDK. | CI | Mises à jour horodatées ainsi que journaux de décisions stockés dans le fichier d'incident et résumés dans `status.md` lors de la prochaine actualisation hebdomadaire. |- Toutes les escalades doivent rester reflétées dans le document d'incident en utilisant le tableau « Propriétaire / Heure de la prochaine mise à jour » du Playbook de support Android.
- Si un autre incident est déjà ouvert, rejoignez la salle de guerre existante et ajoutez le contexte Android plutôt que d'en créer un nouveau.
- Lorsque l'incident touche des lacunes du runbook, créez des tâches de suivi dans l'épopée AND7 JIRA et étiquetez `telemetry-runbook`.

## 5. Exercices de chaos et de préparation

- Exécuter les scénarios détaillés dans
  `docs/source/sdk/android/telemetry_chaos_checklist.md` trimestriel et avant
  versions majeures. Enregistrez les résultats avec le modèle de rapport de laboratoire.
- Stocker les preuves (captures d'écran, journaux) sous
  `docs/source/sdk/android/readiness/screenshots/`.
- Suivez les tickets de correction dans l'épopée AND7 avec l'étiquette `telemetry-lab`.
- Carte de scénario : C1 (erreur de rédaction), C2 (override), C3 (coupure de tension de l'exportateur), C4
  (porte de comparaison de schéma utilisant `run_schema_diff.sh` avec une configuration dérivée), C5
  (inclinaison du profil de périphérique générée via `generate_android_load.sh`), C6 (délai d'expiration Torii)
  + relecture de la file d'attente), C7 (rejet de l'attestation). Gardez cette numérotation alignée avec
  `telemetry_lab_01.md` et la liste de contrôle du chaos lors de l'ajout d'exercices.

### 5.1 Exercice de dérive de rédaction et de remplacement (C1/C2)

1. Injectez un échec de hachage via
   `scripts/telemetry/inject_redaction_failure.sh` et attendez le PagerDuty
   alerte (`android.telemetry.redaction.failure`). Capturez la sortie CLI de
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` pour
   le dossier de l'incident.
2. Effacez l'échec avec `--clear` et confirmez que l'alerte est résolue dans les délais.
   10minutes ; joignez des captures d'écran Grafana des panneaux sel/autorité.
3. Créez une demande de remplacement signée en utilisant
   `docs/examples/android_override_request.json`, appliquez-le avec
   `scripts/android_override_tool.sh apply` et vérifiez l'échantillon non haché en
   inspecter la charge utile de l'exportateur lors de la préparation (recherchez
   `android.telemetry.redaction.override`).
4. Révoquer le remplacement avec `scripts/android_override_tool.sh revoke --token <token>`,
   ajouter le hachage du jeton de remplacement ainsi que la référence du ticket à
   `docs/source/sdk/android/telemetry_override_log.md` et créez un résumé JSON
   sous `docs/source/sdk/android/readiness/override_logs/`. Cela ferme le
   scénario C2 dans la liste de contrôle du chaos et maintient les preuves de gouvernance à jour.

### 5.2 Perte de tension de l'exportateur et exercice de relecture de la file d'attente (C3/C6)1. Réduire le collecteur intermédiaire (`kubectl scale
   déployer/android-otel-collector --replicas=0`) pour simuler un exportateur
   baisse de tension. Suivez les métriques du tampon via la CLI d'état et confirmez que les alertes se déclenchent à
   la barre des 15 minutes.
2. Restaurez le collecteur, confirmez la vidange du backlog et archivez le journal du collecteur
   extrait montrant la fin de la relecture.
3. Sur le pixel intermédiaire et sur l'émulateur, suivez le scénario C6 : installez
   `examples/android/operator-console`, basculer en mode avion, soumettre la démo
   transferts, puis désactivez le mode avion et surveillez les mesures de profondeur de file d’attente.
4. Extrayez chaque file d'attente en attente (`adb shell run-as  cat files/ending.queue >
   /tmp/.queue`), compile the inspector (`gradle -p java/iroha_android
   :core:classes >/dev/null`), and run `java -cp build/classes
   org.hyperledger.iroha.android.tools.PendingQueueInspector --file
   /tmp/.queue --json > queue-replay-.json`. Joindre décodé
   enveloppes et hachages de relecture dans le journal du laboratoire.
5. Mettez à jour le rapport de chaos avec la durée de panne de l'exportateur, la profondeur de la file d'attente avant/après,
   et confirmation que `android_sdk_offline_replay_errors` est resté 0.

### 5.3 Script de chaos de cluster intermédiaire (android-telemetry-stg)

Haruka Yamamoto (Android Observability TL), propriétaires du cluster de préparation, et Liam O'Connor
(SRE) suivez ce script chaque fois qu'une répétition est programmée. La séquence reste
les participants se sont alignés sur la liste de contrôle du chaos télémétrique tout en garantissant que
les artefacts sont capturés pour la gouvernance.

**Participants**

| Rôle | Responsabilités | Contacter |
|------|--------|---------|
| IC de garde Android | Pilote la perceuse, coordonne les notes de PagerDuty, possède le journal des commandes | Téléavertisseur `android-telemetry-primary`, `#android-sdk-support` |
| Propriétaires du cluster de staging (Haruka, Liam) | Fenêtres de changement de porte, exécution des actions `kubectl`, télémétrie du cluster d'instantanés | `#android-staging` |
| Gestionnaire de documents/support (Priya) | Enregistrer les preuves, suivre la liste de contrôle des laboratoires, publier des tickets de suivi | `#docs-support` |

**Coordination avant le vol**

- 48 heures avant l'exercice, déposer une demande de modification listant les
  scénarios (C1 à C7) et collez le lien dans `#android-staging` afin que les propriétaires de cluster
  peut bloquer les déploiements conflictuels.
- Collectez le dernier hachage `ClientConfig` et `kubectl --context staging pour obtenir les pods
  -n sortie android-telemetry-stg` pour établir l'état de base, puis stocker
  tous deux sous `docs/source/sdk/android/readiness/labs/reports/<date>/`.
- Confirmer la couverture de l'appareil (Pixel + émulateur) et assurer
  `ci/run_android_tests.sh` a compilé les outils utilisés pendant le laboratoire
  (`PendingQueueInspector`, injecteurs de télémétrie).

**Points de contrôle d'exécution**

- Annoncer « chaos start » dans `#android-sdk-support`, commencer l'enregistrement du pont,
  et gardez `docs/source/sdk/android/telemetry_chaos_checklist.md` visible afin
  chaque commandement est raconté pour le scribe.
- Demandez à un propriétaire de mise en scène de refléter chaque action d'injecteur (`kubectl scale`, exportateur
  redémarrages, générateurs de charge) donc Observability et SRE confirment l'étape.
- Capturez la sortie de `scripts/telemetry/check_redaction_status.py
  --status-url https://android-telemetry-stg/api/redaction/status` après chaque
  scénario et collez-le dans le document d'incident.

**Récupération**- Ne quittez pas le pont tant que tous les injecteurs ne sont pas nettoyés (`inject_redaction_failure.sh --clear`,
  `kubectl scale ... --replicas=1`) et Grafana affichent un état vert.
- Docs/Support archive les vidages de file d'attente, les journaux CLI et les captures d'écran sous
  `docs/source/sdk/android/readiness/screenshots/<date>/` et coche l'archive
  liste de contrôle avant la clôture de la demande de modification.
- Enregistrez les tickets de suivi avec l'étiquette `telemetry-chaos` pour tout scénario qui
  a échoué ou produit des métriques inattendues, et référencez-les dans `status.md`
  lors de la prochaine revue hebdomadaire.

| Temps | Actions | Propriétaire(s) | Artefact |
|------|--------|----------|----------|
| T−30min | Vérifiez l’état de santé de `android-telemetry-stg` : `kubectl --context staging get pods -n android-telemetry-stg`, confirmez l’absence de mises à niveau en attente et notez les versions du collecteur. | Haruka | `docs/source/sdk/android/readiness/screenshots/<date>/cluster-health.png` |
| T−20min | Chargez la ligne de base (`scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg --duration 20m`) et capturez la sortie standard. | Liam | `readiness/labs/reports/<date>/load-generator.log` |
| T−15min | Copiez `docs/source/sdk/android/readiness/incident/telemetry_chaos_template.md` vers `docs/source/sdk/android/readiness/incident/<date>-telemetry-chaos.md`, répertoriez les scénarios à exécuter (C1 à C7) et attribuez des scribes. | Priya Deshpande (Support) | Démarquage d'incident commis avant le début de la répétition. |
| T−10min | Confirmez l'émulateur Pixel + en ligne, le dernier SDK installé et `ci/run_android_tests.sh` a compilé le `PendingQueueInspector`. | Haruka, Liam | `readiness/screenshots/<date>/device-checklist.png` |
| T−5min | Démarrez Zoom Bridge, commencez l’enregistrement d’écran et annoncez « début du chaos » dans `#android-sdk-support`. | IC / Documents/Support | Enregistrement enregistré sous `readiness/archive/<month>/`. |
| +0min | Exécutez le scénario sélectionné à partir de `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` (généralement C2 + C6). Gardez le guide de laboratoire visible et appelez les invocations de commandes au fur et à mesure qu'elles se produisent. | Haruka conduit, Liam reflète les résultats | Logs joints au dossier d’incident en temps réel. |
| +15min | Faites une pause pour collecter des métriques (`scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`) et récupérez des captures d'écran Grafana. | Haruka | `readiness/screenshots/<date>/status-<scenario>.png` |
| +25min | Restaurez toutes les pannes injectées (`inject_redaction_failure.sh --clear`, `kubectl scale ... --replicas=1`), rejouez les files d'attente et confirmez la fermeture des alertes. | Liam | `readiness/labs/reports/<date>/recovery.log` |
| +35min | Débriefing : mettre à jour le document d'incident avec réussite/échec par scénario, répertorier les suivis et envoyer les artefacts vers git. Informez Docs/Support que la liste de contrôle des archives peut être complétée. | CI | Document d'incident mis à jour, `readiness/archive/<month>/checklist.md` coché. |

- Gardez les propriétaires de relais sur le pont jusqu'à ce que les exportateurs soient en bonne santé et que toutes les alertes soient levées.
- Stockez les vidages bruts de la file d'attente dans `docs/source/sdk/android/readiness/labs/reports/<date>/queues/` et référencez leurs hachages dans le journal des incidents.
- Si un scénario échoue, créez immédiatement un ticket JIRA intitulé `telemetry-chaos` et reliez-le à partir de `status.md`.
- Assistant d'automatisation : `ci/run_android_telemetry_chaos_prep.sh` encapsule le générateur de charge, les instantanés d'état et la plomberie d'exportation de file d'attente. Définissez `ANDROID_TELEMETRY_DRY_RUN=false` lorsque l'accès intermédiaire est disponible et `ANDROID_PENDING_QUEUE_EXPORTS=pixel8=/tmp/pixel.queue,emulator=/tmp/emulator.queue` (etc.) afin que le script copie chaque fichier de file d'attente, émette `<label>.sha256` et exécute `PendingQueueInspector` pour produire `<label>.json`. Utilisez `ANDROID_PENDING_QUEUE_INSPECTOR=false` uniquement lorsque l'émission JSON doit être ignorée (par exemple, aucun JDK disponible). **Exportez toujours les identifiants salt attendus avant d'exécuter l'assistant** en définissant `ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH=<YYYYQ#>` et `ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION=<id>` afin que les appels `check_redaction_status.py` intégrés échouent rapidement si la télémétrie capturée s'écarte de la ligne de base de Rust.

## 6. Documentation et activation- **Kit d'activation opérateur :** `docs/source/sdk/android/readiness/and7_operator_enablement.md`
  relie le runbook, la politique de télémétrie, le guide de laboratoire, la liste de contrôle des archives et les connaissances
  s'archive dans un seul package prêt pour AND7. Référencez-le lors de la préparation du SRE
  la gouvernance pré-lit ou planifie l’actualisation trimestrielle.
- **Sessions d'activation :** Un enregistrement d'activation de 60 minutes s'exécute le 18/02/2026
  avec des mises à jour trimestrielles. Les matériaux vivent sous
  `docs/source/sdk/android/readiness/`.
- **Contrôle des connaissances :** Le personnel doit obtenir un score ≥90 % via le formulaire de préparation. Magasin
  donne `docs/source/sdk/android/readiness/forms/responses/`.
- **Mises à jour :** à chaque fois que des schémas de télémétrie, des tableaux de bord ou des politiques de remplacement
  changement, mettez à jour ce runbook, le playbook de support et `status.md` dans le même
  RP.
- **Révision hebdomadaire :** Après chaque version candidate de Rust (ou au moins une fois par semaine), vérifiez
  `java/iroha_android/README.md` et ce runbook reflètent toujours l'automatisation actuelle,
  les procédures de rotation des luminaires et les attentes en matière de gouvernance. Capturez l'avis dans
  `status.md` afin que l'audit des jalons des fondations puisse retracer la fraîcheur de la documentation.

## 7. Harnais d'attestation StrongBox- **Objectif :** Valider les ensembles d'attestations basés sur le matériel avant de promouvoir des appareils dans le
  Pool StrongBox (AND2/AND6). Le harnais consomme les chaînes de certificats capturées et les vérifie
  contre les racines de confiance en utilisant la même politique que celle exécutée par le code de production.
- **Référence :** Voir `docs/source/sdk/android/strongbox_attestation_harness_plan.md` pour l'intégralité
  capturez l'API, le cycle de vie des alias, le câblage CI/Buildkite et la matrice de propriété. Traitez ce plan comme le
  source de vérité lors de l’intégration de nouveaux techniciens de laboratoire ou de la mise à jour des artefacts financiers/conformité.
- **Flux de travail :**
  1. Collectez un ensemble d'attestations sur l'appareil (alias `challenge.hex` et `chain.pem` avec le
     feuille → ordre racine) et copiez-le sur le poste de travail.
  2. Exécutez `scripts/android_keystore_attestation.sh --bundle-dir  --trust-root 
     [--trust-root-dir ] --require-strongbox --output ` en utilisant le
     Racine Google/Samsung (les répertoires vous permettent de charger des offres groupées entières de fournisseurs).
  3. Archivez le résumé JSON avec le matériel d'attestation brut dans
     `artifacts/android/attestation/<device-tag>/`.
- **Format du pack :** Suivez `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  pour la présentation de fichier requise (`chain.pem`, `challenge.hex`, `alias.txt`, `result.json`).
- **Racines fiables :** Obtenez les PEM fournis par le fournisseur à partir du magasin de secrets du laboratoire d'appareils ; passer plusieurs
  Arguments `--trust-root` ou pointez `--trust-root-dir` vers le répertoire qui contient les ancres lorsque
  la chaîne se termine par une ancre non Google.
- **Faisceau CI :** Utilisez `scripts/android_strongbox_attestation_ci.sh` pour vérifier par lots les bundles archivés
  sur des machines de laboratoire ou des coureurs CI. Le script analyse `artifacts/android/attestation/**` et appelle le
  harnais pour chaque répertoire contenant les fichiers documentés, écriture `result.json` actualisée
  synthèses en place.
- **Voie CI :** Après avoir synchronisé les nouveaux bundles, exécutez l'étape Buildkite définie dans
  `.buildkite/android-strongbox-attestation.yml` (`buildkite-agent pipeline upload --pipeline .buildkite/android-strongbox-attestation.yml`).
  Le travail exécute `scripts/android_strongbox_attestation_ci.sh`, génère un résumé avec
  `scripts/android_strongbox_attestation_report.py`, télécharge le rapport vers `artifacts/android_strongbox_attestation_report.txt`,
  et annote la version comme `android-strongbox/report`. Enquêter immédiatement sur toute défaillance et
  liez l’URL de construction à partir de la matrice de périphériques.
- **Reporting :** Joignez la sortie JSON aux examens de gouvernance et mettez à jour l'entrée de la matrice des appareils dans
  `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` avec la date d'attestation.
- **Répétition simulée :** Lorsque le matériel n'est pas disponible, exécutez `scripts/android_generate_mock_attestation_bundles.sh`
  (qui utilise `scripts/android_mock_attestation_der.py`) pour créer des ensembles de tests déterministes ainsi qu'une racine fictive partagée afin que CI et les documents puissent exercer l'exploit de bout en bout.
- **Garde-corps intégrés au code :** `ci/run_android_tests.sh --tests
  org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProviderTests` couvre vide et contesté
  régénération de l'attestation (métadonnées StrongBox/TEE) et émet `android.keystore.attestation.failure`
  en cas d'inadéquation des défis afin que les régressions de cache/télémétrie soient détectées avant l'expédition de nouveaux bundles.

## 8. Contacts

- **Support technique sur appel :** `#android-sdk-support`
- **Gouvernance SRE :** `#sre-governance`
- **Documents/Support :** `#docs-support`
- **Arbre d'escalade :** Voir le Playbook de support Android §2.1

## 9. Scénarios de dépannageL'élément de feuille de route AND7-P2 appelle trois classes d'incidents qui pagination à plusieurs reprises
Android de garde : Torii/délais d'attente réseau, échecs d'attestation StrongBox et
`iroha_config` dérive manifeste. Parcourez la liste de contrôle pertinente avant de déposer
Suivi Sev1/2 et archive les preuves dans `incident/<date>-android-*.md`.

### 9.1 Torii et délais d'attente du réseau

**Signaux**

- Alertes sur `android_sdk_submission_latency`, `android_sdk_pending_queue_depth`,
  `android_sdk_offline_replay_errors` et le taux d'erreur Torii `/v2/pipeline`.
- Widgets `operator-console` (exemples/android) affichant une vidange de file d'attente bloquée ou
  les tentatives sont bloquées dans une interruption exponentielle.

**Réponse immédiate**

1. Accusez réception de PagerDuty (`android-networking`) et démarrez un journal des incidents.
2. Capturez des instantanés Grafana (latence de soumission + profondeur de file d'attente) couvrant la
   dernières 30 minutes.
3. Enregistrez le hachage `ClientConfig` actif à partir des journaux de l'appareil (`ConfigWatcher`
   imprime le résumé du manifeste chaque fois qu'un rechargement réussit ou échoue).

**Diagnostics**

- **Santé de la file d'attente :** Extrayez le fichier de file d'attente configuré à partir d'un périphérique intermédiaire ou du
  émulateur (`adb shell run-as  cat files/ending.queue >
  /tmp/ending.queue`). Décodez les enveloppes avec
  `OfflineSigningEnvelopeCodec` comme décrit dans
  `docs/source/sdk/android/offline_signing.md#4-queueing--replay` pour confirmer le
  le carnet de commandes correspond aux attentes des opérateurs. Attachez les hachages décodés au
  incident.
- **Inventaire de hachage :** Après avoir téléchargé le fichier de file d'attente, exécutez l'assistant de l'inspecteur
  pour capturer les hachages/alias canoniques pour les artefacts incidents :

  ```bash
  gradle -p java/iroha_android :core:classes >/dev/null  # compiles classes if needed
  java -cp build/classes org.hyperledger.iroha.android.tools.PendingQueueInspector \
    --file /tmp/pending.queue --json > queue-inspector.json
  ```

  Joignez `queue-inspector.json` et la jolie sortie standard imprimée à l'incident
  et associez-le à partir du rapport de laboratoire AND7 pour le scénario D.
- **Connectivité Torii :** Exécutez le faisceau de transport HTTP localement pour exclure le SDK
  régressions : exercices `ci/run_android_tests.sh`
  `HttpClientTransportTests`, `HttpClientTransportHarnessTests` et
  `ToriiMockServerTests`. Les échecs indiquent ici un bug client plutôt qu'un
  Panne Torii.
- **Répétition d'injection de fautes :** Sur le staging Pixel (StrongBox) et l'AOSP
  émulateur, activez la connectivité pour reproduire la croissance de la file d'attente en attente :
  `adb shell cmd connectivity airplane-mode enable` → soumettre deux démos
  transactions via la console de l'opérateur → `adb shell cmd connectivité mode avion
  désactiver` → verify the queue drains and `android_sdk_offline_replay_errors`
  reste 0. Enregistrez les hachages des transactions rejouées.
- **Parité d'alerte :** Lors du réglage des seuils ou après la modification de Torii, exécutez
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh` donc les règles Prometheus restent
  aligné avec les tableaux de bord.

**Récupération**

1. Si Torii est dégradé, engagez le Torii de garde et continuez à rejouer le
   file d'attente une fois que `/v2/pipeline` accepte le trafic.
2. Reconfigurez les clients concernés uniquement via les manifestes `iroha_config` signés. Le
   L'observateur de rechargement à chaud `ClientConfig` doit émettre un journal de réussite avant l'incident
   peut fermer.
3. Mettez à jour l'incident avec la taille de la file d'attente avant/après la relecture ainsi que les hachages de
   toute transaction abandonnée.

### 9.2 Échecs du StrongBox et des attestations

**Signaux**- Alertes sur `android_sdk_strongbox_success_rate` ou
  `android.keystore.attestation.failure`.
- La télémétrie `android.keystore.keygen` enregistre désormais le
  `KeySecurityPreference` et l'itinéraire utilisé (`strongbox`, `hardware`,
  `software`) avec un indicateur `fallback=true` lorsqu'une préférence StrongBox arrive dans
  TEE/logiciel. Les requêtes STRONGBOX_REQUIRED échouent désormais rapidement au lieu de silencieusement
  retour des clés TEE.
- Tickets d'assistance faisant référence aux appareils `KeySecurityPreference.STRONGBOX_ONLY`
  revenir aux clés logicielles.

**Réponse immédiate**

1. Accusez réception de PagerDuty (`android-crypto`) et capturez l'étiquette d'alias concernée.
   (hachage salé) plus seau de profil d'appareil.
2. Vérifiez l'entrée de la matrice d'attestation pour l'appareil dans
   `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` et
   enregistrer la dernière date vérifiée.

**Diagnostics**

- **Vérification du bundle :** Exécuter
  `scripts/android_keystore_attestation.sh --bundle-dir <bundle> --trust-root <root.pem>`
  sur l'attestation archivée pour confirmer si la panne est due à l'appareil
  une mauvaise configuration ou un changement de politique. Joignez le `result.json` généré.
- **Régénération des défis :** Les défis ne sont pas mis en cache. Chaque demande de défi régénère une nouvelle
  attestation et caches par `(alias, challenge)` ; les appels sans défi réutilisent le cache. Non pris en charge
- **CI scanning :** Exécutez `scripts/android_strongbox_attestation_ci.sh` pour que chaque
  le paquet stocké est revalidé ; cela protège contre les problèmes systémiques introduits
  par de nouvelles ancres de confiance.
- **Device Drill :** Sur du matériel sans StrongBox (ou en forçant l'émulateur),
  configurez le SDK pour qu'il nécessite uniquement StrongBox, soumettez une transaction de démonstration et confirmez
  l'exportateur de télémétrie émet l'événement `android.keystore.attestation.failure`
  avec la raison attendue. Répétez l'opération sur un Pixel compatible StrongBox pour garantir que
  le chemin heureux reste vert.
- **Contrôle de régression du SDK :** Exécutez `ci/run_android_tests.sh` et payez
  attention aux suites axées sur l'attestation (`AndroidKeystoreBackendDetectionTests`,
  `AttestationVerifierTests`, `IrohaKeyManagerDeterministicExportTests`,
  `KeystoreKeyProviderTests` pour la séparation cache/défi). Des échecs ici
  indiquer une régression côté client.

**Récupération**

1. Régénérez les ensembles d'attestations si le fournisseur a effectué une rotation des certificats ou si le
   L'appareil a récemment reçu une OTA majeure.
2. Téléchargez le bundle actualisé sur `artifacts/android/attestation/<device>/` et
   mettre à jour l'entrée de la matrice avec la nouvelle date.
3. Si StrongBox n'est pas disponible en production, suivez le flux de travail de remplacement dans
   Section 3 et documentez la durée de repli ; une atténuation à long terme nécessite
   remplacement de l'appareil ou réparation par un fournisseur.

### 9.2a Reprise déterministe des exportations

- **Formats :** Les exportations actuelles sont v3 (sel/nonce par exportation + Argon2id, enregistrées sous la forme
- **Politique de phrase secrète :** v3 applique des phrases secrètes de ≥12 caractères. Si les utilisateurs fournissent des informations plus courtes
  phrases secrètes, demandez-leur de réexporter avec une phrase secrète conforme ; Les importations v0/v1 sont
  exempté mais doit être réemballé en v3 immédiatement après l'importation.
- **Protections contre l'altération/réutilisation :** Les décodeurs rejettent les longueurs nulles/courtes ou occasionnelles et les répétitions
  Les paires sel/nonce font surface sous la forme d'erreurs `salt/nonce reuse`. Régénérez l'export pour effacer
  le garde; n'essayez pas de forcer la réutilisation.
  `SoftwareKeyProvider.importDeterministic(...)` pour réhydrater la clé, puis
  `exportDeterministic(...)` pour émettre un bundle v3 afin que les outils de bureau enregistrent le nouveau KDF
  paramètres.### 9.3 Incompatibilités de manifeste et de configuration

**Signaux**

- Échecs de rechargement `ClientConfig`, noms d'hôtes Torii incompatibles ou télémétrie
  différences de schéma signalées par l'outil de comparaison AND7.
- Les opérateurs signalent différents boutons de tentative/d'attente sur tous les appareils de la même manière.
  flotte.

**Réponse immédiate**

1. Capturez le résumé `ClientConfig` imprimé dans les journaux Android et le
   résumé attendu du manifeste de version.
2. Videz la configuration du nœud en cours d'exécution pour comparaison :
   `iroha_cli config show --actual > /tmp/iroha_config.actual.json`.

**Diagnostics**

- **Différence de schéma :** Exécutez `scripts/telemetry/run_schema_diff.sh --android-config
   --rust-config  --textfile-dir /var/lib/node_exporter/textfile_collector`
  pour générer un rapport de comparaison Norito, actualisez le fichier texte Prometheus et joignez le
  Artéfact JSON plus preuves de métriques de l'incident et journal de préparation de la télémétrie AND7.
- **Validation du manifeste :** Utilisez `iroha_cli runtime capabilities` (ou le runtime
  commande d'audit) pour récupérer les hachages crypto/ABI annoncés du nœud et garantir
  ils correspondent au manifeste mobile. Une incompatibilité confirme que le nœud a été restauré
  sans rééditer le manifeste Android.
- **Vérification de régression du SDK :** `ci/run_android_tests.sh` couvre
  `ClientConfigNoritoRpcTests`, `ClientConfig.ValidationTests` et
  `HttpClientTransportStatusTests`. Les échecs signalent que le SDK livré ne peut pas
  analyser le format du manifeste actuellement déployé.

**Récupération**

1. Régénérez le manifeste via le pipeline autorisé (généralement
   `iroha_cli runtime Capabilities` → manifeste Norito signé → bundle de configuration) et
   redéployez-le via le canal opérateur. Ne jamais modifier `ClientConfig`
   remplace sur l'appareil.
2. Une fois qu'un manifeste corrigé arrive, surveillez le message `ConfigWatcher` « reload ok »
   message sur chaque niveau de flotte et clôturer l'incident seulement après la télémétrie
   le schéma diff rapporte la parité.
3. Enregistrez le hachage du manifeste, le chemin de l'artefact de différence de schéma et le lien de l'incident dans
   `status.md` sous la section Android pour l'auditabilité.

## 10. Programme d'habilitation des opérateurs

L'élément **AND7** de la feuille de route nécessite un programme de formation reproductible afin que les opérateurs,
ingénieurs de support technique, et SRE peut adopter les mises à jour de télémétrie/rédaction sans
des conjectures. Associez cette section avec
`docs/source/sdk/android/readiness/and7_operator_enablement.md`, qui contient
la liste de contrôle détaillée et les liens vers les artefacts.

### 10.1 Modules de session (briefing de 60 minutes)

1. **Architecture de télémétrie (15 min).** Parcourez le tampon de l'exportateur,
   filtre de rédaction et outils de comparaison de schéma. Démo
   `scripts/telemetry/run_schema_diff.sh --textfile-dir /var/lib/node_exporter/textfile_collector` plus
   `scripts/telemetry/check_redaction_status.py` pour que les participants voient comment est la parité
   appliqué.
2. **Runbook + chaos labs (20 min).** Mettez en surbrillance les sections 2 à 9 de ce runbook,
   Répétez un scénario de `readiness/labs/telemetry_lab_01.md` et montrez comment
   pour archiver des artefacts sous `readiness/labs/reports/<stamp>/`.
3. ** Workflow de remplacement + conformité (10 min). ** Examinez les remplacements de la section 3,
   démontrer `scripts/android_override_tool.sh` (appliquer/révoquer/digérer), et
   mise à jour `docs/source/sdk/android/telemetry_override_log.md` plus la dernière
   digérer JSON.
4. **Q&A / contrôle des connaissances (15 min).** Utilisez la carte de référence rapide dans
   `readiness/cards/telemetry_redaction_qrc.md` pour ancrer les questions, puis
   capturer les suivis dans `readiness/and7_operator_enablement.md`.### 10.2 Cadence et propriétaires des actifs

| Actif | Cadence | Propriétaire(s) | Emplacement des archives |
|-------|---------|----------|------------------|
| Procédure pas à pas enregistrée (Zoom/Équipes) | Trimestriel ou avant chaque rotation du sel | Android Observability TL + Gestionnaire de documents/support | `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` (enregistrement + liste de contrôle) |
| Présentation de diapositives et carte de référence rapide | Mettre à jour chaque fois que la stratégie/le runbook change | Gestionnaire de documents/support | `docs/source/sdk/android/readiness/deck/` et `/cards/` (export PDF + Markdown) |
| Contrôle de connaissances + feuille de présence | Après chaque session en direct | Ingénierie d'assistance | Bloc de fréquentation `docs/source/sdk/android/readiness/forms/responses/` et `and7_operator_enablement.md` |
| Carnet de questions/réponses/journal des actions | Roulement; mis à jour après chaque session | LLM (DRI par intérim) | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 |

### 10.3 Boucle de preuves et de rétroaction

- Stocker les artefacts de session (captures d'écran, exercices d'incident, exportations de quiz) dans le
  le même répertoire daté est utilisé pour les répétitions du chaos afin que la gouvernance puisse auditer les deux
  la préparation suit ensemble.
- Lorsqu'une session est terminée, mettez à jour `status.md` (section Android) avec des liens vers
  le répertoire d'archives et notez tous les suivis ouverts.
- Les questions en suspens lors des questions-réponses en direct doivent être transformées en problèmes ou en documents
  demandes de tirage dans un délai d'une semaine ; référencer les épopées de la feuille de route (AND7/AND8) dans le
  description du billet afin que les propriétaires restent alignés.
- Les synchronisations SRE examinent la liste de contrôle des archives ainsi que l'artefact de différence de schéma répertorié dans
  Section2.3 avant de déclarer le cursus clôturé pour le trimestre.