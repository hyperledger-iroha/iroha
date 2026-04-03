<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/source/soracloud/cli_local_control_plane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 567b63e9b61afaecfa5d85aa60f0348c856557e171559885ffaba45168ce61dc
source_last_modified: "2026-03-26T06:12:11.480025+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud CLI et plan de contrôle

Soracloud v1 est un runtime faisant autorité, uniquement IVM.

- `iroha app soracloud init` est la seule commande hors ligne. Il échafaude
  `container_manifest.json`, `service_manifest.json` et modèle facultatif
  artefacts pour les services Soracloud.
- Toutes les autres commandes Soracloud CLI sont uniquement sauvegardées sur le réseau et nécessitent
  `--torii-url`.
- La CLI ne maintient aucun miroir ou état local du plan de contrôle Soracloud
  fichier.
- Torii sert le statut public de Soracloud et les routes de mutation directement depuis
  État mondial faisant autorité et gestionnaire d'exécution Soracloud intégré.

## Portée d'exécution- Soracloud v1 accepte uniquement `SoraContainerRuntimeV1::Ivm`.
- `NativeProcess` reste rejeté.
- Les exécutions ordonnées de boîtes aux lettres admettent directement les gestionnaires IVM.
- L'hydratation et la matérialisation proviennent plutôt du contenu SoraFS/DA engagé
  que les instantanés locaux synthétiques.
- `SoraContainerManifestV1` porte désormais `required_config_names` et
  `required_secret_names`, plus `config_exports` explicite. Déployer, mettre à niveau,
  et l'échec de la restauration est fermé lorsque l'ensemble de documents faisant autorité effectif le ferait
  ne satisfait pas à ces liaisons déclarées ou lorsqu'une exportation de configuration cible un
  configuration non requise ou une destination env/fichier en double.
- Les entrées de configuration de service validées sont désormais matérialisées sous
  `services/<service>/<version>/configs/<config_name>` en tant que JSON canonique
  fichiers de charge utile.
- Les exportations explicites d'environnement de configuration sont projetées dans
  `services/<service>/<version>/effective_env.json` et les exportations de fichiers sont
  matérialisé sous
  `services/<service>/<version>/config_exports/<relative_path>`. Exporté
  les valeurs utilisent le texte canonique de charge utile JSON de l’entrée de configuration référencée.
- Les gestionnaires Soracloud IVM peuvent désormais lire ces charges utiles de configuration faisant autorité
  directement via la surface de l'hôte d'exécution `ReadConfig`, si ordinaire
  Les gestionnaires `query`/`update` n'ont pas besoin de deviner les chemins de fichiers locaux du nœud juste pour
  consommer la configuration de service validée.
- Les enveloppes de secret de service engagées sont désormais matérialisées sous
  `services/<service>/<version>/secret_envelopes/<secret_name>` comme
  fichiers d'enveloppes faisant autorité.- Les gestionnaires Soracloud IVM ordinaires peuvent désormais lire les secrets commis.
  enveloppes directement via la surface `ReadSecretEnvelope` de l'hôte d'exécution.
- L'ancienne arborescence de secours d'exécution privée est désormais synchronisée à partir des validations.
  deployment state under `secrets/<service>/<version>/<secret_name>` so the
  l'ancien chemin de lecture du secret brut et le plan de contrôle faisant autorité pointent vers le
  mêmes octets.
- L'environnement d'exécution privé `ReadSecret` résout désormais le déploiement faisant autorité
  `service_secrets` revient d'abord et uniquement à l'ancien nœud local
  Arborescence de fichiers matérialisée `secrets/<service>/<version>/...` lorsqu'elle n'est pas validée
  une entrée de secret de service existe pour la clé demandée.
- L'ingestion secrète est toujours intentionnellement plus étroite que l'ingestion de configuration :
  `ReadSecretEnvelope` est le contrat de manutentionnaire ordinaire de sécurité publique, tandis que
  `ReadSecret` reste uniquement à exécution privée et renvoie toujours le commit
  enveloppe des octets de texte chiffré plutôt qu'un contrat de montage en texte brut.
- Les plans de service d'exécution exposent désormais la capacité d'ingestion correspondante
  booléens plus le `config_exports` déclaré et projeté effectif
  environnement, afin que les consommateurs de statut puissent savoir si une révision matérialisée
  prend en charge les lectures de configuration de l'hôte, les lectures d'enveloppe secrète de l'hôte, le secret brut privé
  lectures et injection de configuration explicite sans la déduire du gestionnaire
  cours seuls.

## Commandes CLI-`iroha app soracloud init`
  - échafaudage hors ligne uniquement.
  - prend en charge les modèles `baseline`, `site`, `webapp` et `pii-app`.
-`iroha app soracloud deploy`
  - valide localement le règlement d'admission `SoraDeploymentBundleV1`, signe le
    demande et appelle `POST /v1/soracloud/deploy`.
  - `--initial-configs <path>` et `--initial-secrets <path>` peuvent désormais s'attacher
    la configuration de service en ligne faisant autorité/les cartes secrètes atomiquement avec le
    déployez d’abord afin que les liaisons requises puissent être satisfaites lors de la première admission.
  - la CLI signe désormais canoniquement la requête HTTP avec soit
    `X-Iroha-Account`, `X-Iroha-Signature`, `X-Iroha-Timestamp-Ms` et
    `X-Iroha-Nonce` pour les comptes ordinaires à signature unique, ou
    `X-Iroha-Account` plus `X-Iroha-Witness` lorsque
    `soracloud.http_witness_file` pointe vers une charge utile JSON témoin multisig ;
    Torii renvoie un jeu d'instructions de transaction déterministe et le
    CLI soumet ensuite la transaction réelle via le client Iroha normal
    voie.
  - Torii applique également les plafonds d'admission des hôtes SCR et la capacité de fermeture en cas de panne.
    vérifications avant que la mutation ne soit acceptée.
-`iroha app soracloud upgrade`
  - valide et signe une nouvelle révision du bundle, puis appelle
    `POST /v1/soracloud/upgrade`.
  - le même flux `--initial-configs <path>` / `--initial-secrets <path>` est
    disponible pour les mises à jour du matériel atomique lors de la mise à niveau.
  - Les mêmes contrôles d'admission de l'hôte SCR sont exécutés côté serveur avant la mise à niveau.
    admis.
-`iroha app soracloud status`- interroge l'état du service faisant autorité auprès de `GET /v1/soracloud/status`.
-`iroha app soracloud config-*`
  - `config-set`, `config-delete` et `config-status` sont uniquement pris en charge par Torii.
  - la CLI signe les charges utiles et les appels canoniques de provenance de la configuration de service
    `POST /v1/soracloud/service/config/set`,
    `POST /v1/soracloud/service/config/delete`, et
    `GET /v1/soracloud/service/config/status`.
  - les entrées de configuration sont conservées dans l'état de déploiement faisant autorité et restent
    attaché lors des modifications de révision de déploiement/mise à niveau/annulation.
  - `config-delete` échoue désormais lorsque la révision active déclare toujours
    la configuration nommée dans `container.required_config_names`.
-`iroha app soracloud secret-*`
  - `secret-set`, `secret-delete` et `secret-status` sont uniquement pris en charge par Torii.
  - la CLI signe les charges utiles et les appels canoniques de provenance de service secret
    `POST /v1/soracloud/service/secret/set`,
    `POST /v1/soracloud/service/secret/delete`, et
    `GET /v1/soracloud/service/secret/status`.
  - les entrées secrètes sont conservées en tant qu'enregistrements `SecretEnvelopeV1` faisant autorité
    en état de déploiement et survivre aux modifications normales de révision du service.
  - `secret-delete` échoue désormais lorsque la révision active déclare toujours
    le secret nommé dans `container.required_secret_names`.
-`iroha app soracloud rollback`
  - signe les métadonnées de restauration et appelle `POST /v1/soracloud/rollback`.
-`iroha app soracloud rollout`
  - signe les métadonnées de déploiement et appelle `POST /v1/soracloud/rollout`.
-`iroha app soracloud agent-*`
  - toutes les commandes de cycle de vie de l'appartement, de portefeuille, de boîte aux lettres et d'autonomie sont
    Torii uniquement.
-`iroha app soracloud training-*`
  - toutes les commandes de tâches de formation sont uniquement prises en charge par Torii.-`iroha app soracloud model-*`
  - toutes les commandes d'artefact de modèle, de poids, de modèle téléchargé et d'exécution privée
    sont uniquement soutenus par Torii.
  - la surface de modèle téléchargé/d'exécution privée vit désormais dans la même famille :
    `model-upload-encryption-recipient`, `model-upload-init`,
    `model-upload-chunk`, `model-upload-finalize`, `model-upload-status`,
    `model-compile`, `model-compile-status`, `model-allow`,
    `model-run-private`, `model-run-status`, `model-decrypt-output` et
    `model-publish-private`.
  - `model-run-private` masque désormais la négociation d'exécution de brouillon puis de finalisation dans
    la CLI et renvoie l'état de session post-finalisation faisant autorité.
  - `model-publish-private` prend désormais en charge à la fois un
    bundle/chunk/finalize/compile/allow publication plan et un niveau supérieur
    projet de document. Le projet porte désormais le numéro `source: PrivateModelSourceV1`,
    qui accepte soit `LocalDir { path }`, soit
    `HuggingFaceSnapshot { repo, revision }`.
  - lorsqu'elle est appelée avec `--draft-file`, la CLI normalise la source déclarée
    dans un arbre temporaire déterministe, valide le contrat v1 HF safetensors,
    sérialise et chiffre de manière déterministe le bundle par rapport à l'actif
    Le destinataire du téléchargement Torii le divise en morceaux cryptés de taille fixe,
    écrit éventuellement le plan préparé via `--emit-plan-file`, puis
    exécute la séquence télécharger/finaliser/compiler/autoriser.
  - Les révisions `HuggingFaceSnapshot` sont obligatoires et doivent être épinglées
    les SHA ; les références de type branche sont rejetées en cas de fermeture en cas d'échec.- la disposition des sources admise est volontairement étroite dans la v1 : `config.json`,
    actifs de tokenizer, un ou plusieurs fragments `*.safetensors` et facultatif
    métadonnées du processeur/préprocesseur pour les modèles compatibles avec les images. GGUF, ONNX,
    d'autres poids non sécurisés et des mises en page personnalisées imbriquées arbitraires sont
    rejeté.
  - lorsqu'elle est appelée avec `--plan-file`, la CLI consomme toujours un déjà
    document de plan de publication préparé et fermeture en cas d'échec lors du téléchargement du plan
    le destinataire ne correspond plus au destinataire Torii faisant autorité.
  - voir `uploaded_private_models.md` pour la conception qui superpose ces itinéraires
    sur le registre de modèles existant et les enregistrements d'artefacts/poids.
- Itinéraires du plan de contrôle `model-host`
  - Torii expose désormais les informations faisant autorité
    `POST /v1/soracloud/model-host/advertise`,
    `POST /v1/soracloud/model-host/heartbeat`,
    `POST /v1/soracloud/model-host/withdraw`, et
    `GET /v1/soracloud/model-host/status`.
  - ces itinéraires persistent dans les annonces de capacité d'hébergement du validateur opt-in
    État mondial faisant autorité et laisser les opérateurs inspecter quels validateurs sont
    annonce actuellement la capacité d'hébergement de modèles.
  -`iroha app soracloud model-host-advertise`,
    `model-host-heartbeat`, `model-host-withdraw` et
    `model-host-status` signe désormais les mêmes charges utiles de provenance canonique que le
    API brute et appelez directement les routes Torii correspondantes.
-`iroha app soracloud hf-*`
  - `hf-deploy`, `hf-status`, `hf-lease-leave` et `hf-lease-renew` sont
    Soutenu par Torii uniquement.- `hf-deploy` et `hf-lease-renew` admettent désormais également automatiquement le déterminisme
    service d'inférence HF généré pour le `service_name` demandé, et
    admettre automatiquement l'appartement HF généré déterministe pour
    `apartment_name` lorsqu'elle est demandée, avant la mutation en bail partagé
    est soumis.
  - la réutilisation est fermée : si le service/appartement nommé existe déjà mais est
    pas le déploiement HF généré attendu pour cette source canonique, le HF
    la mutation est rejetée au lieu de lier silencieusement le bail à des personnes non liées
    Objets Soracloud.
  - lorsque le gestionnaire d'exécution intégré est connecté, `hf-status` désormais également
    renvoie une projection d'exécution pour la source canonique, y compris liée
    services/appartements, visibilité en file d'attente dans la fenêtre suivante et offre groupée/locale
    le cache d'artefacts manque ; `importer_pending` suit cette projection d'exécution
    au lieu de s'appuyer uniquement sur l'énumération source faisant autorité.
  - lorsque `hf-deploy` ou `hf-lease-renew` admet le service HF généré dans
    la même transaction que la mutation en bail partagé, la HF faisant autorité
    la source passe maintenant immédiatement à `Ready` et `importer_pending` reste
    `false` dans la réponse.
  - Le statut du bail HF et les réponses aux mutations exposent désormais également toute autorité
    instantané de placement déjà attaché à la fenêtre de bail active, y comprishôtes attribués, nombre d'hôtes éligibles, nombre d'hôtes chauds et
    champs de frais de stockage et de calcul.
  - `hf-deploy` et `hf-lease-renew` dérivent désormais la ressource canonique HF
    profil à partir des métadonnées du dépôt Hugging Face résolues avant de soumettre le
    mutation :
    - Torii inspecte le repo `siblings`, préfère `.gguf` à
      `.safetensors` sur les présentations de poids PyTorch, HEAD les fichiers sélectionnés vers
      dériver `required_model_bytes` et le mapper à une première version
      backend/format plus planchers de RAM/disque ;
    - L'admission au bail échoue lorsqu'aucune annonce d'hôte de validation en direct ne peut
      satisfaire ce profil ; et
    - lorsqu'un ensemble d'hôtes est disponible, la fenêtre active enregistre désormais un
      placement déterministe pondéré en fonction des enjeux et réservation de calcul distincte
      frais parallèlement à la comptabilité des baux de stockage existants.
  - Les membres ultérieurs rejoignant une fenêtre HF active paient désormais le stockage au prorata et
    calculer les partages uniquement pour la fenêtre restante, tandis que les membres précédents
    recevez le même remboursement de stockage déterministe et la même comptabilité de remboursement de calcul
    de cette arrivée tardive.
  - le gestionnaire d'exécution intégré peut synthétiser le bundle de stub HF généré
    localement, afin que ces services générés puissent se matérialiser sans attendre un
    charge utile SoraFS engagée uniquement pour le bundle d'inférence d'espace réservé.- Le gestionnaire d'exécution intégré importe désormais également le dépôt Hugging Face sur liste autorisée
    fichiers dans `soracloud_runtime.state_dir/hf_sources/<source_id>/files/` et
    persiste un `import_manifest.json` local avec le commit résolu, importé
    fichiers, fichiers ignorés et toute erreur d'importateur.
  - les lectures locales HF `metadata` générées renvoient désormais ce manifeste d'importation local,
    y compris l'inventaire des fichiers importés et si l'exécution locale et
    le repli du pont est activé pour le nœud.
  - Les lectures locales HF `infer` générées préfèrent désormais l'exécution sur le nœud par rapport à l'exécution sur le nœud.
    octets partagés importés :
    - `irohad` matérialise un script adaptateur Python embarqué sous le local
      Répertoire d'état d'exécution Soracloud et l'invoque via
      `soracloud_runtime.hf.local_runner_program` ;
    - le coureur intégré vérifie d'abord une strophe de luminaire déterministe dans
      `config.json` (utilisé par les tests), puis charge sinon la source importée
      répertoire via `transformers.pipeline(..., local_files_only=True)` donc
      le modèle s'exécute sur l'importation locale partagée au lieu de tirer
      nouveaux octets du Hub ; et
    - si `soracloud_runtime.hf.allow_inference_bridge_fallback = true` et
      `soracloud_runtime.hf.inference_token` est configuré, le runtime tombe
      revenir à l'URL de base d'inférence HF configurée uniquement lors de l'exécution locale
      n'est pas disponible ou échoue et l'appelant s'engage explicitement avec
      `x-soracloud-hf-allow-bridge-fallback: 1`, `true` ou `yes`.
  - la projection d'exécution conserve désormais une source HF dans `PendingImport` jusqu'à ce qu'unUn manifeste d'importation local réussi existe et les échecs de l'importateur apparaissent lorsque
    runtime `Failed` plus `last_error` au lieu de signaler silencieusement `Ready`.
  - les appartements HF générés consomment désormais une autonomie agréée parcourt le
    chemin d'exécution local du nœud :
    - `agent-autonomy-run` suit désormais un flux en deux étapes : la première signée
      la mutation enregistre l'approbation faisant autorité et renvoie un résultat déterministe
      projet de transaction, puis une deuxième demande de finalisation signée demande au
      gestionnaire d'exécution intégré pour exécuter cette exécution approuvée par rapport à la limite
      génère le service HF `/infer` et renvoie tout suivi faisant autorité
      instructions comme un autre brouillon déterministe ;
    - l'enregistrement d'exécution approuvé reste désormais également canonique
      `request_commitment`, de sorte que le reçu de service généré ultérieurement puisse être
      lié à l’approbation exacte de l’autonomie faisant autorité ;
    - les approbations peuvent désormais persister dans un `workflow_input_json` canonique facultatif
      corps; lorsqu'il est présent, le runtime intégré transmet cette charge utile JSON exacte
      au gestionnaire HF `/infer` généré, et lorsqu'il est absent, il revient à
      l'ancienne enveloppe `run_label`-as-`inputs` avec l'autorité faisant autorité
      `artifact_hash` / `provenance_hash` / `budget_units` / `run_id` porté
      comme paramètres structurés ;
    - `workflow_input_json` peut désormais également opter pour le séquentiel déterministeexécution en plusieurs étapes avec
      `{ "workflow_version": 1, "steps": [...] }`, où chaque étape exécute un
      La requête HF `/infer` générée et les étapes ultérieures peuvent faire référence aux sorties antérieures
      via `${run.*}`, `${previous.text|json|result_commitment}` et
      Espaces réservés `${steps.<step_id>.text|json|result_commitment}` ; et
    - la réponse mutationnelle et `agent-autonomy-status` font maintenant surface
      résumé de l'exécution au niveau du nœud lorsqu'il est disponible, y compris le succès/l'échec,
      la révision du service lié, les engagements de résultats déterministes, le point de contrôle
      / hachages d'artefacts de journal, le service généré `AuditReceipt` et le
      corps de réponse JSON analysé.
    - lorsque ce reçu de service généré est présent, Torii l'enregistre dans
      faisant autorité `soracloud_runtime_receipts` et expose le résultat
      reçu d'exécution faisant autorité sur l'état d'exécution récent à côté du
      résumé de l'exécution au niveau du nœud.
    - le chemin d'autonomie généré-HF enregistre désormais également une autorité dédiée
      Événement d'audit de l'appartement `AutonomyRunExecuted` et état d'exécution récente
      renvoie cet audit d'exécution avec le reçu d'exécution faisant autorité.
  - `hf-lease-renew` dispose désormais de deux modes :
    - si la fenêtre actuelle est expirée ou vidée, elle ouvre immédiatement une nouvelle
      fenêtre;
    - si la fenêtre actuelle est toujours active, elle met l'appelant en file d'attente
      sponsor de la fenêtre suivante, facture l'intégralité du stockage et du calcul de la fenêtre suivantefrais de réservation à l'avance, persiste la fenêtre suivante déterministe
      plan de placement et expose ce parrainage en file d'attente via `hf-status`
      jusqu'à ce qu'une mutation ultérieure fasse avancer le pool.
  - L'entrée publique HF `/infer` générée résout désormais le problème faisant autorité.
    placement et, lorsque le nœud de réception n'est pas le nœud principal chaud, proxy le
    demande via les messages de contrôle Soracloud P2P à l'hôte principal attribué ;
    le runtime intégré échoue toujours lors de la fermeture sur une réplique directe/locale non attribuée
    l'exécution et les reçus d'exécution HF générés portent `placement_id`,
    validateur et attribution par les pairs à partir du dossier de placement faisant autorité.
  - lorsque ce chemin proxy vers principal expire, se ferme avant une réponse, ou
    revient avec un échec d'exécution non-client de la part de l'autorité faisant autorité
    principal, le nœud d'entrée signale désormais `AssignedHeartbeatMiss` pour cela
    primaire et met en file d'attente `ReconcileSoracloudModelHosts` via le même
    voie de mutation interne.
  - La réconciliation faisant autorité entre les hôtes expirés et les enregistrements sont désormais persistants
    preuve de violation modèle-hôte, réutilise le chemin slash du validateur de voie publique,
    et applique la politique de pénalité par défaut pour les locations partagées HF :
    `warmup_no_show_slash_bps=500`,
    `assigned_heartbeat_miss_slash_bps=250`,
    `assigned_heartbeat_miss_strike_threshold=3`, et
    `advert_contradiction_slash_bps=1000`.
  - La santé d'exécution HF attribuée localement alimente désormais également ce même chemin de preuves :échecs d'importation/préchauffage au moment de la réconciliation sur un hôte `Warming` local émis
    `WarmupNoShow` et pannes des résidents-travailleurs sur le primaire chaud local
    émettre des rapports `AssignedHeartbeatMiss` limités via le canal normal
    file d'attente des transactions.
  - concilier désormais également les pré-démarrages et les enquêtes sur les travailleurs résidents du HF pour les besoins locaux
    des hôtes de réchauffement/réchauffement attribués, y compris des répliques, afin qu'une réplique puisse échouer
    fermé dans le chemin faisant autorité `AssignedHeartbeatMiss` avant tout
    La requête publique `/infer` arrive toujours sur le serveur principal.
  - lorsque cette sonde locale réussit, le runtime en émet désormais également une
    mutation `model-host-heartbeat` faisant autorité pour le validateur local lorsque
    l'hôte attribué est toujours `Warming` ou l'annonce d'hôte active nécessite un TTL
    rafraîchir, de sorte qu'une préparation locale réussie favorise la même autorité
    l'emplacement/l'annonce indique que les battements de cœur manuels seraient mis à jour.
  - lorsque le runtime émet un `WarmupNoShow` local ou
    `AssignedHeartbeatMiss`, il est désormais également mis en file d'attente
    `ReconcileSoracloudModelHosts` via la même voie de mutation interne donc
    le basculement/remplissage faisant autorité démarre immédiatement au lieu d'attendre
    un balayage périodique ultérieur de l'expiration de l'hôte.
  - lorsque l'entrée HF générée par le public échoue encore plus tôt parce que le
    le placement n'a pas de primaire chaud vers lequel proxy, Torii demande maintenant au runtime
    gérer pour mettre en file d'attente cette même autoritéInstruction `ReconcileSoracloudModelHosts` immédiatement au lieu d'attendre
    pour une expiration ultérieure ou un signal de défaillance du travailleur.
  - lorsque l'entrée HF générée par le public reçoit une réponse de réussite mandatée,
    Torii vérifie désormais que le reçu d'exécution inclus prouve toujours l'exécution par
    la primaire chaleureuse et engagée pour le placement actif ; manquant ou incompatible
    L'attribution de placement échoue désormais et laisse entendre que la même autorité fait autorité.
    Chemin `ReconcileSoracloudModelHosts` au lieu de renvoyer un
    réponse ne faisant pas autorité. Torii rejette également désormais le succès du proxy
    réponses lorsque les engagements de réception d'exécution ou la politique de certification le font
    ne correspond pas à la réponse qu'il est sur le point de renvoyer, et ce même mauvais reçu
    le chemin alimente également le reporting principal distant `AssignedHeartbeatMiss`
    crochet.
  - Les échecs d'exécution HF générés par proxy demandent désormais la même chose
    chemin `ReconcileSoracloudModelHosts` faisant autorité après avoir signalé le
    défaut de santé primaire à distance, plutôt que d'attendre une expiration ultérieure
    balayer.
  - Torii lie désormais chaque requête proxy HF générée en attente au
    homologue principal faisant autorité qu’il ciblait. Une réponse par procuration du mauvais
    le homologue est désormais ignoré au lieu d'empoisonner cette requête en attente, donc uniquement
    le responsable principal faisant autorité peut terminer ou échouer la demande. Un mandataire
    réponse du homologue attendu avec un schéma de réponse proxy non pris en chargela version échoue toujours fermée au lieu d'être acceptée uniquement parce que son
    `request_id` correspondait à une demande en attente. Si le mauvais pair a répondu
    est lui-même toujours un hôte HF généré attribué pour cet emplacement, le
    le runtime signale désormais cet hôte via le
    Chemin de preuve `WarmupNoShow` / `AssignedHeartbeatMiss` basé sur son
    statut d'affectation faisant autorité et indiquant également que
    `ReconcileSoracloudModelHosts`, donc dérive d'autorité primaire/réplique obsolète
    alimente la boucle de contrôle au lieu d'être ignoré uniquement à l'entrée.
  - L'exécution du proxy Soracloud entrant est également désormais limitée à l'exécution prévue
    Cas de requête généré-HF `infer` sur le primaire chaud validé. Hors HF
    routes publiques de lecture locale et requêtes HF générées transmises à un nœud
    ce n'est plus le primaire chaud faisant autorité, maintenant l'échec est fermé à la place
    d'exécution sur le chemin proxy P2P. La primaire faisant autorité aussi maintenant
    recalcule l'engagement canonique de la requête HF générée avant l'exécution,
    les enveloppes de procuration falsifiées ou incompatibles ne peuvent donc pas être fermées.
  - lorsqu'une réplique attribuée ou une ancienne réplique principale obsolète rejette cette réplique entrante
    exécution du proxy HF généré car il ne fait plus autorité
    primaire chaud, le temps d'exécution côté récepteur indique désormais également
    `ReconcileSoracloudModelHosts` au lieu de compter uniquement sur le côté appelant
    vue de routage.- lorsque la même défaillance de l'autorité proxy HF générée entrante se produit
    l'autorité primaire locale elle-même, le runtime le traite désormais comme un
    signal de santé hôte de premier ordre : auto-évaluation des primaires chaleureuses
    `AssignedHeartbeatMiss`, auto-évaluation du réchauffement des primaires `WarmupNoShow`,
    et les deux chemins réutilisent immédiatement la même autorité
    Boucle de contrôle `ReconcileSoracloudModelHosts`.
  - lorsque ce même récepteur non principal est toujours l'un des autorités faisant autorité
    les hôtes attribués et peuvent résoudre le primaire chaud à partir de l'état de la chaîne validée,
    il renvoie désormais la requête HF générée vers ce primaire à la place
    d'échouer immédiatement. Les validateurs non attribués échouent à la fermeture au lieu d'agir
    en tant que sauts proxy HF intermédiaires génériques, et le nœud d'entrée d'origine est toujours
    valide le reçu d'exécution renvoyé par rapport au placement faisant autorité
    état. Si ce saut en avant de la réplique assignée vers le serveur principal échoue après le
    La requête est effectivement envoyée, le runtime côté récepteur signale le
    défaut de santé primaire à distance et conseils faisant autorité
    `ReconcileSoracloudModelHosts` ; si la réplique attribuée localement ne peut même pas
    tenter ce saut car son propre transport/exécution proxy est manquant,
    l'échec est désormais traité comme une erreur d'hôte attribué localement au lieu de
    blâmer la primaire.
  - la réconciliation émet désormais également automatiquement `AdvertContradiction` lorsque leL'ID d'homologue d'exécution configuré par le validateur local n'est pas d'accord avec le
    identifiant d'homologue `model-host-advertise` faisant autorité pour ce validateur.
  - Les mutations valides de réannonce modèle-hôte synchronisent désormais également les autorités faisant autorité.
    Métadonnées `peer_id` / `host_class` de l'hôte attribué et recalculer le courant
    frais de réservation de placement en cas de changement de classe d'accueil.
  - Les mutations contradictoires de réannonce modèle-hôte émettent désormais immédiatement
    Preuve `AdvertContradiction`, appliquer la barre oblique/expulsion du validateur existant
    chemin et actualisez les emplacements concernés au lieu de simplement échouer à la validation.
  - le reste du travail d'hébergement HF est désormais :
    - des signaux de santé inter-nœuds/clusters d'exécution plus larges au-delà du niveau local
      observations directes du travailleur/d'échauffement du validateur et de l'hôte assigné
      échecs d’autorité côté récepteur lorsque l’intégrité interne d’un homologue distant devrait
      alimente également le chemin de rééquilibrage/slash faisant autorité.
  - L'exécution locale HF générée conserve désormais un travailleur Python résident par source
    vivant sous `irohad`, réutilise le modèle chargé à travers `/infer` répété
    appelle et redémarre ce travailleur de manière déterministe si l'importation locale
    des changements manifestes ou le processus se termine.
  - ces itinéraires ne sont pas le chemin privé du modèle téléchargé. Séjour des baux partagés HF
    axé sur l'adhésion partagée à la source/à l'importation plutôt que sur la chaîne chiffrée
    octets de modèle privé.- Les approbations d'autonomie HF générées prennent désormais en charge la séquence déterministe
    enveloppes de requêtes en plusieurs étapes, mais plus larges, non linéaires/utilisant des outils
    l'orchestration et l'exécution d'artefacts-graphes restent des travaux de suivi
    au-delà des étapes `/infer` enchaînées.

## Sémantique du statut

`/v1/soracloud/status` et les points de terminaison d'état d'agent/formation/modèle associés maintenant
refléter l'état d'exécution faisant autorité :

- révisions de service admises par l'état mondial engagé ;
- état d'hydratation/matérialisation du runtime à partir du gestionnaire d'exécution intégré ;
- reçus d'exécution de boîtes aux lettres réelles et état d'échec ;
- artefacts de journaux/points de contrôle publiés ;
- Santé du cache et de l'exécution au lieu de cales d'état d'espace réservé.

Si le matériel d'exécution faisant autorité est obsolète ou indisponible, les lectures échouent.
au lieu de se rabattre sur les miroirs des États locaux.

`/v1/soracloud/status` est le seul point de terminaison d'état Soracloud documenté dans la v1.
Il n’existe pas de route `/v1/soracloud/registry` distincte.

## Échafaudage local supprimé

Ces anciens concepts de simulation locale n'existent plus dans la version v1 :

- Fichiers d'état/registre local CLI ou options de chemin de registre
- Miroirs de plan de contrôle basés sur des fichiers locaux Torii

## Exemple

```bash
iroha app soracloud deploy \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080 \
  --api-token <token-if-required> \
  --timeout-secs 10
```

## Remarques- La validation locale s'exécute toujours avant que les demandes ne soient signées et soumises.
- Les points de terminaison de mutation Soracloud standard n'acceptent plus les `authority` bruts /
  `private_key` Champs JSON pour le déploiement, la mise à niveau, la restauration, le déploiement et l'agent
  chemins de cycle de vie, de formation, d’hôte de modèle et de poids de modèle ; Torii authentifie
  ces requêtes provenant des en-têtes de signature HTTP canoniques à la place.
- Les propriétaires de Soracloud contrôlés par Multisig utilisent désormais `X-Iroha-Witness` ; pointe
  `soracloud.http_witness_file` au témoin JSON exact que vous souhaitez que la CLI
  rejouer pour la prochaine demande de mutation, et Torii échouera fermé si le
  Le compte du sujet témoin ou le hachage de la demande canonique ne correspondent pas.
- `hf-deploy` et `hf-lease-renew` incluent désormais un auxiliaire signé par le client
  provenance des artefacts de service/appartement HF générés de manière déterministe,
  donc Torii n'a plus besoin des clés privées de l'appelant pour admettre ces suivis
  objets.
- `agent-autonomy-run` et `model/run-private` utilisent désormais un brouillon puis finalisation
  flux : la première mutation signée enregistre l'approbation/le démarrage faisant autorité,
  et une deuxième demande de finalisation signée exécute le chemin d'exécution et renvoie
  toute instruction de suivi faisant autorité en tant que projet déterministe
  transactions.
- `model/decrypt-output` renvoie désormais l'inférence privée faisant autorité
  point de contrôle en tant que projet de transaction déterministe, signé uniquement par l'extérieurtransaction plutôt que par une clé privée intégrée détenue par Torii.
- ZK attachment CRUD clé désormais la location du compte Iroha signé et toujours
  traite les jetons API comme une porte d'accès supplémentaire lorsqu'ils sont activés.
- L'entrée publique en lecture locale de Soracloud applique désormais un débit explicite par IP et
  la concurrence limite et revérifie la visibilité des itinéraires publics avant les
  exécution par procuration.
- L'application des capacités d'exécution privée s'effectue au sein de l'ABI de l'hôte Soracloud,
  pas à l’intérieur de l’échafaudage CLI ou Torii-local.
- `ram_lfe` reste un sous-système distinct à fonctions cachées. Privé téléchargé par l'utilisateur
  l'exécution du transformateur doit réutiliser la gouvernance de Soracloud FHE/décryptage et
  registres de modèles, pas le chemin de requête `ram_lfe`.
- La santé, l'hydratation et l'exécution de l'exécution proviennent de
  Configuration `[soracloud_runtime]` et état validé, pas environnement
  bascule.