---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-spec
titre : Spécifications techniques de Sora Nexus
description : Espejo completo de `docs/source/nexus.md`, qui cubre l'arquitectura et les restrictions de conception pour le grand livre Iroha 3 (Sora Nexus).
---

:::note Fuente canonica
Cette page reflète `docs/source/nexus.md`. Manten ambas copias alineadas hasta que el backlog de traduccion se retrouve sur le portail.
:::

#! Iroha 3 - Sora Nexus Ledger : Spécifications techniques de conception

Ce document propose l'architecture du grand livre Sora Nexus pour Iroha 3, l'évolution de Iroha 2 fait un grand livre mondial unique et logiquement unifié organisé autour des espaces de données (DS). Les espaces de données ont prouvé leurs dominios fuertes de privacidad (« espaces de données privés ») et participation ouverte (« espaces de données publics »). L'objectif est de préserver la composabilité à travers le grand livre mondial tout en assurant l'islamisation stricte et la confidentialité des données de Private-DS, et d'introduire l'augmentation de la disponibilité des données via la codification de Borrado en Kura (stockage en bloc) et WSV (World State View).Le même référentiel compile tant Iroha 2 (redes autoalojadas) que Iroha 3 (SORA Nexus). L'exécution est impulsive pour la machine virtuelle Iroha (IVM) et la chaîne d'outils Kotodama, car les contrats et les artefacts de bytecode portables permanents entre les applications automatiques et le grand livre global de Nexus.

Objectifs
- Un grand livre logique global composé de nombreux validateurs coopérants et espaces de données.
- Espaces de données privés pour une opération avec autorisation (par exemple, CBDC), avec des données qui ne sont jamais vendues par DS privées.
- Espaces de données publics avec participation ouverte, accès sans autorisation du style Ethereum.
- Contrats intelligents composables entre Data Spaces, sujets à autorisation explicite pour accéder aux actifs de private-DS.
- Aislamiento de rendimiento para que l'activité publique ne dégrade pas les transactions internes de private-DS.
- Disponibilité des données à un niveau supérieur : Kura et WSV avec codification de borrado pour prendre en charge efficacement les données de manière illimitée en maintenant les données de private-DS privados.

Aucun objetivos (phase initiale)
- Définir l'économie des jetons ou des incitations aux validateurs ; les politiques de planification et de jalonnement sont enchufables.
- Introduire une nouvelle version d'ABI ; Les changements apportés à ABI v1 avec des extensions explicites de syscalls et pointeur-ABI selon la politique de IVM.Terminologie
- Nexus Ledger : Le grand livre logique global formé par le composant bloque l'espace de données (DS) dans une histoire ordonnée et un compromis d'état.
- Espace de données (DS) : Dominio acotado de ejecucion y almacenamiento con sus propios validadores, gobernanza, clase de privacidad, politica de DA, cuotas y politica de tarifsas. Il existe deux classes : DS publique et DS privée.
- Espace de données privé : validateurs avec permis et contrôle d'accès ; les données de transaction et l’état nunca salen del DS. Il suffit de faire des compromis/métdonnées globalement.
- Espace de données public : participation sans permis ; les données complètes et l'état sont publics.
- Manifeste d'espace de données (DS Manifest) : Manifeste codifié avec Norito qui déclare les paramètres DS (validateurs/valeurs QC, classe de confidentialité, politique ISI, paramètres DA, rétention, comptes, politique ZK, tarifs). Le hachage du manifeste est ancla dans la chaîne de connexion. Salvo que se anule, les certificats de quorum DS utilisant ML-DSA-87 (classe Dilithium5) comme esquema de firma post-quantico por defecto.
- Space Directory : Contrat de directeur global en chaîne qui rastrea manifeste DS, versions et événements de gouvernance/rotation pour la résolution et les auditoires.
- DSID : identifiant global unique pour un espace de données. À utiliser pour l'espacement de noms de tous les objets et références.- Ancre : compromis cryptographique d'un bloc/en-tête DS inclus dans la chaîne de liens pour identifier l'historique DS dans le grand livre mondial.
- Kura : Installation des blocs de Iroha. Il s'étend ici avec l'accumulation de blobs codifiés avec du papier et des compromis.
- WSV : Iroha Vue de l'état mondial. Il s'étend ici avec des segments de versions actuelles, avec des instantanés et des codes codés avec Borrado.
- IVM : Machine virtuelle Iroha pour l'exécution de contrats intelligents (bytecode Kotodama `.to`).
  - AIR : Représentation Algébrique Intermédiaire. Vue algébrique de l'ordinateur pour les essais du style STARK, décrivant l'exécution comme les trazas basadas en campos avec des restrictions de transition et de frontière.Modèle d'espaces de données
- Identité : `DataSpaceId (DSID)` identifie un DS et l'espace de noms d'une tâche. Le DS peut être instantané dans deux granularités :
  - Domaine-DS : `ds::domain::<domain_name>` - expulsion et statut attribué à un domaine.
  - Asset-DS : `ds::asset::<domain_name>::<asset_name>` - éjection et état correspondant à une définition d'activité unique.
  Les ambas formas coexistent; les transactions peuvent toucher des multiples DSID de forme atomique.
- Cycle de vie du manifeste : la création de DS, les actualisations (rotation de clés, changements de politique) et le retrait de l'inscription dans Space Directory. Chaque artefact DS par emplacement référence le hachage du manifeste le plus récent.
- Classes : DS public (participation ouverte, DA publica) et DS privé (permisionado, DA confidentiel). La politique hybride est possible via les drapeaux du manifeste.
- Politiques par DS : autorisations ISI, paramètres DA `(k,m)`, cifrado, rétention, comptes (participation min/max de tx par blocage), politique de tests ZK/optimistes, tarifs.
- Gobernanza : membresia DS et rotation des validateurs définis pour la section de gouvernance du manifeste (propriétés en chaîne, multisig ou gouvernance externe annoncée pour les transactions liées et les attestations).Manifestes de capacités et UAID
- Comptes universels : chaque participant reçoit un UAID déterministe (`UniversalAccountId` et `crates/iroha_data_model/src/nexus/manifest.rs`) qui ouvre tous les espaces de données. Les manifestes de capacités (`AssetPermissionManifest`) provoquent un UAID dans un espace de données spécifique, les époques d'activation/expiration et une liste ordonnée de réglementations d'autorisation/refus `ManifestEntry` qui correspondent à `dataspace`, `program_id`, `method`, `asset` et rôles AMX optionnels. Las reglas nie siempre ganan ; l'évaluateur émet `ManifestVerdict::Denied` avec une raison d'auditoire ou une subvention `Allowed` avec les métadonnées d'allocation coïncidentes.
- Allocations : chaque entrée autorise lleva buckets déterministas `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) mais un `max_amount` facultatif. Les hôtes et le SDK utilisent la même charge utile Norito, car l'application est toujours identique entre le matériel et le SDK.
- Télémétrie des auditoires : Space Directory émet `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) lorsqu'un manifeste change d'état. La nouvelle surface `SpaceDirectoryEventFilter` permet aux abonnés de Torii/data-event de surveiller les actualisations du manifeste UAID, les révocations et les décisions de refus de gain sans plomberie personnalisée.Pour les preuves opérationnelles de bout en bout, les notes de migration du SDK et les listes de contrôle de publication des manifestes, en particulier cette section du Guide de compte universel (`docs/source/universal_accounts_guide.md`). Manten ambos documentsos alineados cuando cambien la politique o las herramientas UAID.

Architecture de haut niveau
1) Capa de composition globale (Chaîne Nexus)
- Maintenir un ordre canonique unique de blocs Nexus de 1 seconde pour finaliser les transactions atomiques qui permettent d'ouvrir un ou plusieurs espaces de données (DS). Chaque transaction confirmée actualise l'état mondial unifié (vecteur de racines par DS).
- Contiene des métadonnées minimes plus d'essais/QC agrégés pour garantir la composabilité, la finalisation et la détection de fraude (DSID mentionnés, racines de l'état par DS avant/après, compromis DA, essais de validation par DS, et le certificat de quorum DS utilisé ML-DSA-87). Il n'y a pas de données privées incluses.
- Consenso : comité BFT global avec pipeline de tamano 22 (3f+1 avec f=7), sélectionné d'un pool d'environ 200 000 validateurs potentiels via un mécanisme VRF/mise par époque. Le comité Nexus ordonne les transactions et finalise le blocage en 1s.2) Capa de Data Space (Public/Privé)
- Exécuter des fragments par DS de transactions globales, actualiser le WSV local du DS et produire des artefacts de validation par bloc (pruebas agregadas por DS et compromissos DA) qui s'agrègent dans le bloc Nexus de 1 seconde.
- Données privées DS cifran en dépôt et en transit entre validadores autorizados ; solo compromisos y pruebas de validez PQ salen del DS.
- Public DS exportan cuerpos completos de datos (via DA) y pruebas de validez PQ.3) Transactions atomiques cross-Data-Space (AMX)
- Modèle : chaque transaction de l'utilisateur peut toucher plusieurs DS (par exemple, domaine DS et un ou plusieurs actifs DS). Se confirme la forme atomique dans un seul blocage Nexus ou s'interrompt ; pas d'effets partiels de foin.
- Prepare-Commit à l'intérieur des 1 : pour chaque transaction candidate, les DS tocados sont exécutés en parallèle contre le même instantané (roots DS au début de l'emplacement) et produisent des essais de validation PQ par DS (FASTPQ-ISI) et des compromis DA. Le comité Nexus confirme la transaction seule si toutes les évaluations DS sont vérifiées et les certificats DA sont effectués à temps (objet <= 300 ms) ; de l'autre côté, la transaction est reprogrammée pour le slot suivant.
- Consistencia : les conjuntos de lectura/escritura se déclarent ; la détection de conflits survient lors de la validation des racines du lancement de la machine à sous. L'exécution optimiste sans verrouillage pour DS évite les blocages globaux ; l'atomicité est imposée par la règle de commit nexus (tout ou rien entre DS).
- Privacidad : privé DS exportan solo pruebas/compromisos ligados a racines DS pré/post. Aucune donnée de vente privée cruda del DS.4) Disponibilité des données (DA) avec codification de borrado
- Kura almacena cuerpos de bloques y snapshots WSV como blobs codificados con borrado. Les blobs publics sont largement fragmentés ; Les blobs privés se trouvent seuls dans les validateurs privés-DS, avec des morceaux cifrados.
- Les compromis DA sont enregistrés tant sur les artefacts DS que sur les blocs Nexus, permettant ainsi la récupération et la garantie de récupération sans révéler un contenu privé.

Structure des blocs et du commit
- Artefact de test de Data Space (pour slot de 1s, pour DS)
  - Champs : dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Exportation privée-DS d'artefacts sans corps de données ; public DS permet la récupération de corps via DA.

- Bloc Nexus (cadence de 1s)
  - Champs : block_number, parent_hash, slot_time, tx_list (transactions atomiques cross-DS avec DSIDs tocados), ds_artifacts[], nexus_qc.
  - Fonction : finaliza todas las transacciones atomicas cuyos artefactos DS requeridos verifican ; actualisez le vecteur de racines DS de l’état mondial en une seule étape.Consenso et planification
- Consenso de Nexus Chain : BFT global avec pipeline (classe Sumeragi) avec comité de 22 nœuds (3f+1 avec f=7) pour les blocs de 1 et la finalisation des 1. Les membres du comité sont sélectionnés par époque via VRF/enjeu entre ~ 200 000 candidats ; la rotation maintient la décentralisation et résiste à la censure.
- Consenso de Data Space : chaque DS exécute votre propre BFT entre vos validateurs pour produire des artefacts par slot (essais, compromis DA, DS QC). Les comités Lane-Relay sont dimensionnés en `3f+1` en utilisant la configuration `fault_tolerance` de l'espace de données et sont déterminés par l'époque à partir du pool de validateurs de l'espace de données en utilisant la graine de l'époque VRF liée à `(dataspace_id, lane_id)`. Privé DS fils permisionados ; public DS permet la vivacité ouverte au sujet d'une politique anti-Sybil. Le Comité Global Nexus ne change pas.
- Planification des transactions : les utilisateurs souhaitent des transactions atomiques en déclarant les DSID tocados et conjuntos de lectura/escritura. Le DS s'exécute en parallèle à l'intérieur de la fente ; le comité nexus inclut la transaction dans le bloc de 1s si tous les artefacts DS sont vérifiés et les certificats DA sont ponctuels (<=300 ms).- Aislamiento de rendimiento: chaque DS a des mémoires et une exécution indépendante. Les limites de DS limitent les transactions que vous pouvez effectuer sur une DS et peuvent être confirmées par le blocage pour éviter le blocage de la tête de ligne et protéger la latence de la DS privée.

Modèle de données et espace de noms
- ID certifiés par DS : toutes les entités (domaines, comptes, actifs, rôles) sont autorisées par `dsid`. Exemple : `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Références globales : une référence globale est un tupla `(dsid, object_id, version_hint)` et peut être placée en chaîne dans le lien ou dans les descripteurs AMX pour une utilisation cross-DS.
- Sérialisation Norito : tous les messages cross-DS (descripteurs AMX, tests) utilisant les codecs Norito. Ne pas utiliser les chemins de production.Contrats intelligents et extensions de IVM
- Contexte d'éjection : ajouter `dsid` au contexte d'éjection de IVM. Les contrats Kotodama sont toujours exécutés dans un espace de données spécifique.
- Primitivas atomicas cross-DS :
  - `amx_begin()` / `amx_commit()` délimite une transaction atomique multi-DS sur l'hôte IVM.
  - `amx_touch(dsid, key)` déclare l'intention de lecture/écriture pour la détection de conflits contre l'instantané racine de l'emplacement.
  - `verify_space_proof(dsid, proof, statement)` -> booléen
  - `use_asset_handle(handle, op, amount)` -> résultat (opération autorisée seule si la politique l'autorise et la poignée est valide)
- Poignées d'actifs et de tarifs :
  - Les opérations d'activités sont autorisées par les politiques ISI/rol del DS ; les tarifs sont païens sur le jeton de gaz du DS. Les jetons de capacité optionnelle et politique principale (multi-approbateur, limites de débit, géorepérage) peuvent être ajoutés plus efficacement sans modifier le modèle atomique.
- Déterminisme : tous les nouveaux appels système sont purs et déterministes dadas les entrées et les conjuntos de lectura/escritura AMX déclarés. Sans effets occultes de temps ou d’hiver.Pruebas de validez post-cuanticas (ISI generalizados)
- FASTPQ-ISI (PQ, sans configuration fiable) : un argument basé sur le hachage qui généralise la conception du transfert à toutes les familles ISI pendant l'essai en sous-seconde pour beaucoup d'échelle 20k en classe matérielle GPU.
  - Profil opérationnel :
    - Les nœuds de production construisent le prouveur via `fastpq_prover::Prover::canonical`, qui est désormais toujours initialisé le backend de production ; Le déterministe simulé fut supprimé. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) et `irohad --fastpq-execution-mode` permettent aux opérateurs de faire une exécution CPU/GPU de manière déterminante lorsque le hook d'observateur enregistre des triples sollicitations/résultats/backend pour les salles de flottaison. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Arimétisation :
  - KV-Update AIR : il s'agit d'un WSV comme d'une carte clé-valeur compatible via Poseidon2-SMT. Chaque ISI s'étend à un petit ensemble de fils de lecture-vérification-écriture sur les clés (comptes, actifs, rôles, domaines, métadonnées, fourniture).
  - Restrictions concernant les portes d'opcode : une seule table AIR avec des sélecteurs de colonnes impose les règles de l'ISI (conservation, contrôleurs monotones, autorisations, vérifications de plage, actualisation des métadonnées acceptées).- Arguments de recherche : des tableaux transparents compromis par hachage pour les autorisations/rôles, la précision des actifs et les paramètres politiques évitent les contraintes au niveau du bit.
- Compromis et actualisations de l'état :
  - Test SMT agrégé : toutes les clés tocadas (pré/post) sont testées contre `old_root`/`new_root` en utilisant un frontière compressé avec des frères et sœurs dédupliqués.
  - Invariantes : les invariantes globales (par exemple, supply total por activo) s'imposent via l'igualdad de multiconjuntos entre filas de efecto et contadores rastreados.
- Système de test :
  - Compromis polinomiaux style FRI (DEEP-FRI) avec une haute température (8/16) et une explosion 8-16 ; hache Poséidon2 ; transcription Fiat-Shamir avec SHA-2/3.
  - Récursion facultative : agrégation récursive locale à DS pour compresser des micro-lots à une évaluation par emplacement si nécessaire.
- Alcance et exemples de cubes :
  - Activos : transférer, créer, graver, enregistrer/désenregistrer les définitions d'actifs, définir la précision (acotado), définir les métadonnées.
  - Comptes/Dominios : créer/supprimer, définir une clé/un seuil, ajouter/supprimer des signataires (seulement en état ; les vérifications de l'entreprise se font par les validateurs DS, ne se vérifient pas dans l'AIR).
  - Rôles/Permisos (ISI) : accorder/révoquer des rôles et des autorisations ; impuestos por tablas de lookup y checks de politique monotone.- Contratos/AMX : les marcadores commencent/commettent AMX, capacité à créer/révoquer si esta habilitado ; se prueban como transitions de étatado y contadores de politica.
- Vérifie l'extérieur de l'AIR pour préserver la latence :
  - Firmas y criptografia pesada (p. ej., firmas ML-DSA de usuarios) se verifican por validadores DS y se atestan en el DS QC ; la testa de validez cubre solo consistencia de estado y cumplimiento de politicas. Esto mantiene pruebas PQ y rapidas.
- Objets de rendu (illustrations, CPU de 32 cœurs + un GPU moderne) :
  - 20 000 mixages ISI avec un petit toucher de touche (<=8 touches/ISI) : ~0,4-0,9 s d'essai, ~150-450 Ko d'essai, ~5-15 ms de vérification.
  - ISI mas pesadas (mas claves/constraints ricas) : micro-lotes (p. ej., 10x2k) + récursivité pour maintenir por slot <1 s.
- Configuration du manifeste DS :
  -`zk.policy = "fastpq_isi"`
  -`zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  -`state.commitment = "smt_poseidon2"`
  -`zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (firmas verificadas por DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (par défaut ; les alternatives doivent être déclarées explicitement)
- Solutions de repli :
  - ISI complet/personnalisé peut utiliser un STARK général (`zk.policy = "stark_fri_general"`) avec essai différent et finalisation de 1 s via l'attestation QC + slashing et essais invalides.
  - Les options sans PQ (par exemple, Plonk avec KZG) nécessitent une configuration fiable et ne sont pas prises en charge dans la construction par défaut.Introduction à AIR (para Nexus)
- Traza de ejecucion: matriz con ancho (columnas de registros) et longitude (pasos). Chaque fil est une étape logique du processus ISI ; les colonnes contiennent des valeurs pré/post, des sélecteurs et des drapeaux.
- Restrictions :
  - Restrictions de transition : imposent des relations fil à fil (par exemple, post_balance = pre_balance - montant pour un fil de débit lorsque `sel_transfer = 1`).
  - Restrictions de frontière : vinculan E/S publica (old_root/new_root, contactadores) a la primera/ultima fila.
  - Recherches/permutations : assurez-vous des membres et des igualdades de multiconjuntos contre des tableaux compromis (autorisations, paramètres d'activité) sans circuits pesados ​​de bits.
- Compromis et vérification :
  - Le fournisseur compromet les trazas via les codifications hash et construit des polinomios de bas niveau qui sont validés si les restrictions sont remplies.
  - Le vérificateur comprueba bajo grado via FRI (hash-based, post-cuantico) avec quelques ouvertures de Merkle ; le coût est logaritmique aux étapes.
- Exemple (Transfert) : les registres incluent pre_balance, montant, post_balance, nonce et sélecteurs. Les restrictions n'imposent aucune négativité/rang, conservation et monotonie de nonce, alors qu'un multiprueba SMT agrégé vincula hojas pré/post aux racines anciennes/nouvelles.Évolution d'ABI et des appels système (ABI v1)
- Syscalls a agregar (nombres illustratifs) :
  -`SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Types Pointer-ABI à associer :
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Actualisations requises :
  - Agréger un `ivm::syscalls::abi_syscall_list()` (mantener orden), gatear por politica.
  - Mapear numéros découverts à `VMError::UnknownSyscall` et hôtes.
  - Actualiser les tests : liste d'appels système en or, hachage ABI, identifiants de type de pointeur et tests de politique.
  - Documents : `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Modèle de confidentialité
- Contenu des données privées : corps de transaction, différences d'état et instantanés WSV de DS privé n'est pas vendu dans le sous-ensemble privé des validateurs.
- Exposition publique : en-têtes solo, compromis DA et essais de validation PQ se exportent.
- Pruebas ZK opcionales : le DS privé peut produire des pruebas ZK (p. ej., balance suficiente, politica cumplida) habilitando acciones cross-DS sin revelar estado interno.
- Contrôle d'accès : l'autorisation est imposée par la politique ISI/rol dans le DS. Les jetons de capacité sont optionnels et peuvent être introduits de manière plus efficace.Islamisme de rendu et QoS
- Consenso, mempools et stockage séparés par DS.
- Cuotas de planification nexus por DS pour limiter le temps d'inclusion des ancres et éviter le blocage de tête de ligne.
- Présupposés de ressources de contrat pour DS (calcul/mémoire/E/S), imputables pour l'hôte IVM. La conception en DS publique ne peut pas consommer des présupposés de DS privés.
- Les appels cross-DS asincronas évitent d'espérer des sincronas largas dentro de ejecucion private-DS.

Disponibilité des données et disposition du stockage
1) Codification de borrado
- Utiliser le système Reed-Solomon (p. ej., GF(2^16)) pour la codification de la capture au niveau du blob des blocs Kura et des instantanés WSV : paramètres `(k, m)` avec les fragments `n = k + m`.
- Paramètres par défaut (propriétaires, DS publics) : `k=32, m=16` (n = 48), permettant une récupération jusqu'à 16 fragments perdus avec une extension d'environ 1,5x. Pour DS privé : `k=16, m=8` (n=24) dans le groupe autorisé. Ambos configurables par DS Manifest.
- Blobs publics : fragments distribués à travers de nombreux nœuds DA/validadores avec contrôles de disponibilité par mois. Les compromis DA et les en-têtes permettent la vérification par les clients légers.
- Blobs privés : fragments cifrados et distribués uniquement entre les validateurs privés-DS (ou les gardiens désignés). La chaîne mondiale ne contient que des compromis DA (sans emplacements de fragments ni de clés).2) Compromis et résultats
- Pour chaque blob : calculer une racine Merkle sur les fragments et l'inclure dans `*_da_commitment`. Mantener PQ évite les compromis sur la courbe elliptique.
- DA Attesters : attesters régionaux muestreados por VRF (p.ej., 64 por region) émis un certificat ML-DSA-87 attestant d'un certificat exitoso de shards. Objet de latence d’attestation DA <=300 ms. Le comité Nexus est certifié certifié à la place de fragments supplémentaires.

3) Intégration avec Kura
- Les blocs stockent les corps de transaction comme les blobs codifiés avec le compromis avec Merkle.
- Les en-têtes impliquent des compromis de blob ; les corps sont récupérés via la DA rouge pour DS publique et via les canaux privés pour DS privée.

4) Intégration avec WSV
- Snapshots WSV : il est périodiquement effectué un point de contrôle de l'état DS et des instantanés par morceaux codifiés avec des compromis enregistrés dans les en-têtes. Entre les instantanés, les journaux de modifications sont conservés. Les instantanés publics sont largement fragmentés ; les instantanés privés sont permanents à l'intérieur des validateurs privés.
- Accès aux essais : les contrats peuvent fournir (ou solliciter) des essais de l'état (Merkle/Verkle) ancladas por compromisos de snapshot. Le soldat DS peut soumettre des attestations de connaissance de zéro à la place des essais crudas.5) Rétention et taille
- Sin pruning para public DS : retenir tous les corps Kura et les instantanés WSV via DA (escalado horizontal). Private DS peut définir la rétention interne, mais les compromis exportés sont permanents et immuables. Le lien conserve tous les blocs Nexus et les compromis des artefacts DS.

Rouge et rôles de noeuds
- Validadores globales : participe au lien de consensus, valide les blocs Nexus et les artefacts DS, réalise les contrôles DA pour le public DS.
- Validadores de Data Space: ejecutan consensus DS, ejecutan contratsos, gestionan Kura/WSV local, manejan DA para su DS.
- Nodos DA (facultatif) : almacenan/publican blobs publicos, facilitan muestreo. Para private DS, los nodos DA se co-ubican con validadores o custodios confiables.Améliorations et considérations au niveau du système
- Désactiver la sécurité/mempool : adopter un mempool DAG (par exemple, style Narwhal) qui alimente un BFT avec pipeline dans le lien pour réduire la latence et améliorer le débit sans modifier le modèle logique.
- Points DS et équité : points pour DS pour bloquer et plafonds de peso pour éviter le blocage de tête de ligne et assurer la latence prévisible pour DS privé.
- Atestación DS (PQ) : les certificats de quorum DS utilisant ML-DSA-87 (classe Dilithium5) par défaut. C'est post-cuantico et plus grande que les entreprises EC mais acceptables avec un QC por slot. DS peut opter explicitement pour ML-DSA-65/44 (mais petit) ou pour les entreprises EC si elles sont déclarées dans le manifeste DS ; je recommande de maintenir ML-DSA-87 pour le public DS.
- Attestateurs DA : pour le public DS, utilisez les attestateurs régionaux muestreados por VRF qui émettent des certificados DA. Le comité Nexus validé est certifié au lieu du musée des fragments crus ; privé DS mantienen atestaciones DA internas.
- Récursion et essais par époque : agréger facultativement plusieurs micro-lots à l'intérieur d'un DS dans une étude récursive par slot/époque pour maintenir une période d'essai et un temps de vérification établi à basse température.- Escalado de lanes (si nécessaire) : si un comité mondial unique voit un cuello de botella, introduisez K lanes de secuenciacion parallèles avec une fusion déterministe. Ceci préserve un ordre mondial unique qui s’étend horizontalement.
- Accélération déterministe : noyaux de preuve SIMD/CUDA avec indicateurs de fonctionnalité pour le hachage/FFT avec bit de CPU de repli, exactement pour préserver la détermination du matériel croisé.
- Cadres d'activation des voies (proposés) : activer 2 à 4 voies si (a) p95 de finalisation dépasse 1,2 s pendant >3 minutes consécutives, ou (b) l'occupation du bloc dépasse 85 % pendant >5 minutes, ou (c) la charge entrante de transmission requiert >1,2x la capacité de blocage en niveaux. sostenidos. Les voies regroupent les transactions de forme déterminée par le hachage du DSID et sont fusionnées dans le bloc Nexus.

Tarifs et économie (valeurs initiales)
- Unité de gaz : jeton de gaz pour DS avec moyen de calcul/IO ; les tarifs sont païens en el activo de gas natif del DS. La conversion entre DS est la responsabilité de l'application.
- Priorité d'inclusion : round-robin entre DS avec des points par DS pour préserver l'équité et les SLO de 1 ; À l'intérieur d'un DS, les enchères peuvent être désemparées.
- Futuro : vous pouvez explorer un marché mondial de tarifs ou de politiques qui minimisent le MEV sans modifier l'atomicité ni le projet d'essais PQ.Flujo cross-Data-Space (exemple)
1) Un utilisateur envoie une transaction AMX qui toca public DS P y private DS S: mover activo X desde S a beneficiario B cuya cuenta esta en P.
2) À l'intérieur du slot, P et Séchez votre fragment contre l'instantané du slot. S'il vous plaît vérifier l'autorisation et la disponibilité, actualiser votre état interne et produire une vérification de validation PQ et compromis DA (sans filtrer les données privées). P préparer l'actualisation de l'état correspondant (par exemple, mint/burn/locking en P segun politica) et votre essai.
3) El comite nexus verifica ambas pruebas DS y certificados DA ; Si vous êtes en train de vérifier à l'intérieur de l'emplacement, la transaction est confirmée atomiquement dans le bloc Nexus de 1s, en actualisant les racines DS dans le vecteur de l'état mondial global.
4) Si une vérification ou un certificat DA échoue ou est invalide, la transaction est interrompue (sans effets) et le client peut être renvoyé pour le créneau suivant. Ningun dato privado sale de S en ningun paso.- Considérations de sécurité
- Éjection déterministe : les appels système IVM déterministes permanents ; Les résultats cross-DS dictent l'engagement et la finalisation d'AMX, ni la montre ni le timing de rouge.
- Contrôle d'accès : les autorisations ISI en privé DS restringen quien peuvent envoyer des transactions et que les opérations sont autorisées. Les jetons de capacité codifient les droits de grano fino pour une utilisation cross-DS.
- Confidentialité : cifré de bout en bout pour les données privées-DS, fragments codifiés avec des données enregistrées uniquement entre les membres autorisés, tests ZK optionnels pour les attestations externes.
- Résistance au DoS : l'islamiento en mempool/consenso/almacenamiento évite que la congestion publique ait un impact sur le progrès de DS privé.

Modifications des composants de Iroha
- iroha_data_model : introduction `DataSpaceId`, ID califiés pour DS, descripteurs AMX (ensembles de lecture/écriture), types de tests/compromis DA. Sérialisation seule Norito.
- ivm : regrouper les appels système et les types de pointeur-ABI pour AMX (`amx_begin`, `amx_commit`, `amx_touch`) et tester DA ; Actualiser les tests/docs d'ABI depuis la politique v1.