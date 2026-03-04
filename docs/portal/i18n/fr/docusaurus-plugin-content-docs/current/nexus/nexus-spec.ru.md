---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-spec
titre : Spécifications techniques Sora Nexus
description: Полное отражение `docs/source/nexus.md`, охватывающее архитектуру и ограничения проектирования для Iroha 3 (Sora Nexus).
---

:::note Канонический источник
Cette page correspond à `docs/source/nexus.md`. Veuillez sélectionner les copies de synchronisation si l'écran précédent n'est pas affiché sur le portail.
:::

#! Iroha 3 - Sora Nexus Ledger : Spécifications techniques

Ce document présente l'architecte Sora Nexus Ledger pour Iroha 3, qui utilise Iroha 2 dans une logique globale. Grand livre unifié, espaces de données organisés (DS). Les espaces de données précèdent les domaines privés (« espaces de données privés ») et les espaces ouverts (« espaces de données publics »). Concevoir un grand livre global de composabilité pour une isolation et une confidentialité des données privées-DS et une gestion complète des données Vous trouverez le codage dans Kura (stockage en bloc) et WSV (World State View).

Il s'agit d'un dépôt contenant Iroha 2 (ensemble auto-hébergé) et Iroha 3 (SORA Nexus). Utilisation de la machine virtuelle Iroha (IVM) et de la chaîne d'outils Kotodama, pour des contrats et des contrats d'art. Vous pouvez utiliser le registre auto-hébergé et le grand livre global Nexus.Celi
- Il s'agit d'un grand livre logique global, qui prend en charge de nombreux validateurs coopératifs et espaces de données.
- Espaces de données privés pour les opérations autorisées (par exemple, CBDC), qui ne permettent pas d'accéder à des DS privés.
- Espaces de données publics avec accès ouvert, téléchargement sans autorisation dans le style Ethereum.
- Les contrats intelligents composables permettent d'utiliser des espaces de données pour créer des connexions avec l'activité DS privée.
- Les activités publiques, dont l'activité publique n'est pas une transition privée-DS.
- Déposez des données dans le masque : Kura et WSV codés par effacement pour permettre aux utilisateurs de pratiquer des données privées pour la sécurité des DS privés.

Не цели (начальная фаза)
- Определение токеномики или стимулов валидаторов; La planification politique et le jalonnement sont désormais disponibles.
- Découvrez les nouvelles versions d'ABI ; Les paramètres d'ABI v1 sont basés sur les appels système et le pointeur-ABI selon la politique IVM.Termines
- Grand livre Nexus : grand livre logique global, blocs de composition formés de l'espace de données (DS) dans l'histoire et l'engagement d'engagement.
- Espace de données (DS) : utilisation et gestion des domaines de validation, de gouvernance, de confidentialité classique, de politique, de commerce et de commission politique. Il existe une classe : DS publique et DS privée.
- Espace de données privé : validateurs et contrôleurs autorisés ; Cette transmission et cette maintenance ne conviennent pas à DS. Il n'y a que des engagements/métadonnées à l'échelle mondiale.
- Espace de données public : utilisation sans autorisation ; полные данные и состояние публичны.
- Manifeste d'espace de données (manifeste DS) : manifeste codé Norito, paramètres déclarés DS (validateurs/clés QC, classes privées, politique ISI, paramètres DA, rétention, quotas, politique ZK, commissions). Ancrage du manifeste de hachage sur la chaîne de connexion. Si cela n'est pas possible, les certificats de quorum DS utilisent ML-DSA-87 (classe Dilithium5) comme prévu pour le post-vente.
- Répertoire spatial : contrat d'annuaire mondial en chaîne, manifestes DS, versions et gouvernance/rotation pour la configuration et l'audit.
- DSID : espace de données d'identifiant unique mondial. Utilisation de l'espacement de noms pour tous les objets et cadres.
- Ancre : engagement cryptographique du bloc/satellite DS, inclus dans la chaîne Nexus, qui permet de consulter l'histoire de DS dans le grand livre mondial.- Kura : Blocs de construction Iroha. Permet de supprimer le stockage et les engagements de blobs codés par effacement.
- WSV : Iroha Vue de l'état mondial. Nous vous proposons une version complète, compatible avec les instantanés et codée avec effacement des segments.
- IVM : Machine virtuelle Iroha pour l'utilisation de connecteurs intelligents (bytecode Kotodama `.to`).
  - AIR : Représentation Algébrique Intermédiaire. La présentation générale des documents STARK concerne l'utilisation de traitements basés sur le champ avec des connaissances antérieures et граничными ограничениями.Modèle d'espaces de données
- Identification : `DataSpaceId (DSID)` identifie DS et l'espace de noms. DS peut utiliser des installations dans deux niveaux :
  - Domaine-DS : `ds::domain::<domain_name>` - utilisation et gestion du domaine.
  - Asset-DS : `ds::asset::<domain_name>::<asset_name>` - utilisation et gestion de l'activité définie.
  Обе формы сосуществуют ; Les transferts peuvent être effectués par le DSID.
- Le manifeste du groupe de personnes est : l'association DS, la notification (rotation du clavier, la politique) et l'utilisation de l'espace de stockage dans l'annuaire. L'article DS par emplacement est défini sur le hachage manifeste suivant.
- Classes : DS public (открытое участие, публичная DA) et Private DS (autorisé, конфиденциальная DA). Les politiques politiques s'appellent des drapeaux manifestes.
- Politiques sur DS : autorisations ISI, paramètres DA `(k,m)`, précision, rétention, valeurs (min/max pour les tx sur le bloc), politique de preuve ZK/optimiste, commissions.
- Gouvernance : les vérificateurs DS et rotatifs ouvrent le manifeste de gouvernance sexuelle (préparation en chaîne, multisig ou gouvernance globale, transition et attestations).Manifestes de capacités et UAID
- Comptes universels : vous pouvez utiliser l'UAID (`UniversalAccountId` et `crates/iroha_data_model/src/nexus/manifest.rs`) pour définir vos espaces de données. Les manifestes de capacité (`AssetPermissionManifest`) связывают UAID с конкретным dataspace, époque активации/истечения и упорядоченным списком autoriser/refuser `ManifestEntry` правил, `dataspace`, `program_id`, `method`, `asset` et rôles AMX professionnels. Refuser правила всегда побеждают; l'évaluateur utilise `ManifestVerdict::Denied` avec l'audit personnel ou la subvention `Allowed` avec les métadonnées d'allocation pertinentes.
- Allocations : vous pouvez autoriser l'utilisation de seaux `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) et optionnels. `max_amount`. Les hôtes et le SDK prennent en charge la charge utile Norito, permettant ainsi l'application des paramètres de configuration du SDK.
- Télémétrie d'audit : Space Directory transmet `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) pour la création du manifeste. La nouvelle version `SpaceDirectoryEventFilter` met à jour Torii/data-event pour supprimer les manifestes UAID, les révocations et les décisions de refus de victoires plomberie кастомного.

Pour la documentation sur le fonctionnement de bout en bout, les notes de migration du SDK et la liste de contrôle des publications du manifeste doivent être synchronisées avec le Guide de compte universel (`docs/source/universal_accounts_guide.md`). Vous trouverez des documents sur les réponses aux questions politiques ou aux instruments de l'UAID.Архитектура верхнего уровня
1) Chaîne de composition globale (chaîne Nexus)
- Il s'agit d'une période canonique de 1 seconde Nexus qui permet de finaliser les transitions atomiques, de planifier des opérations ou des opérations. plus les espaces de données (DS). Каждая engagé транзакция обновляет единый глобальный l'état mondial (vecteur par racines DS).
- Des métadonnées minimes et des preuves/QC supplémentaires pour la composabilité, la finalité et la détection de fraude (les DSID, les racines d'état par DS avant/après, les engagements DA, les preuves de validité par DS et le certificat de quorum DS) ML-DSA-87). Les données privées ne sont pas disponibles.
- Consensus : le comité mondial BFT en pipeline en direct mesure 22 (3f+1 à f=7), qui s'étend à l'époque VRF/enjeu pour environ 200 000 candidats. Le comité Nexus gère les transitions et finalise le bloc pour 1s.

2) Espace de données SLOй (Public/Privé)
- Mettre en œuvre des fragments par DS pour les transitions globales, mettre à jour DS WSV local et générer des artefacts de validité par bloc (preuves agrégées par DS et engagements DA), qui correspondent à vos besoins. Bloc 1-секундный Nexus.
- Les données privées DS sont fournies au repos et en vol par les autorités de validation ; наружу выходят только engagements et preuves de validité PQ.
- Les corps de données publics DS экспортируют полные (via DA) et les preuves de validité PQ.3) Transmissions atomiques cross-data-space (AMX)
- Modèle : каждая пользовательская транзакция может касаться нескольких DS (par exemple, domaine DS et один ou несколько actif DS). Она коммитится атомарно в одном Nexus Block ou откатывается; частичных эфектов нет.
- Prepare-Commit pour 1s : pour chaque transfert candidat, la DS est également utilisée en parallèle sur un instantané actuel (racines DS sur l'emplacement) et vérifie les preuves de validité par DS PQ. (FASTPQ-ISI) et engagements DA. Nexus comité коммитит транзакцию только если все требуемые DS preuves проверяются и DA certificats приходят вовремя (цель <=300 ms); иначе транзакция переносится на следующий слот.
- Cohérence : lecture-écriture наборы объявляются ; il détecte les conflits lors de la validation des racines de début d'emplacement. Optimiste sans verrouillage исполнение по DS избегает глобальных stall; atomicity est utilisé pour le nexus commit (tout ou rien pour DS).
- Confidentialité : exportation privée de DS uniquement avec des preuves/engagements, привязанные к racines pré/post DS. Никакие сырые private данные не покидают DS.4) Disponibilité des données (DA) avec codage d'effacement
- Kura prend en charge les corps de bloc et les instantanés WSV ainsi que les blobs codés par effacement. Публичные blobs широко shard-ятся; Les blobs privés contiennent également des validateurs private-DS, avec des morceaux ajoutés.
- Les engagements DA sont inclus dans les artefacts DS et dans les blocs Nexus, offrant des garanties d'échantillonnage et de récupération sans autorisation privée.

Structure du bloc et commit
- Artefact de preuve d'espace de données (sur l'emplacement 1s, sur DS)
  - Nom : dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Private-DS exporte des artefacts hors des corps de données ; public DS позволяют получение body через DA.

- Bloc Nexus (cadence 1s)
  - Éléments : block_number, parent_hash, slot_time, tx_list (transmissions cross-DS avec DSID), ds_artifacts[], nexus_qc.
  - Fonction : finaliser toutes les transitions atomiques, grâce auxquelles les artefacts DS sont prouvés ; обновляет globalный вектор DS racines одним шагом.Consensus et planification
- Consensus de la chaîne Nexus : un BFT global en pipeline (classe Sumeragi) avec le comité 22 (3f+1, f=7) pour les blocs 1s et la finalité 1s. Le comité recherche des époques pour un VRF/enjeu d'environ 200 000 candidats ; La rotation permet la décentralisation et la décentralisation.
- Consensus sur l'espace de données : chaque DS utilise BFT pour les artefacts par emplacement (preuves, engagements DA, DS QC). Les comités de relais de voie ont défini l'espace de données `3f+1` en utilisant l'espace de données `fault_tolerance` et ont déterminé de manière simple l'époque de l'espace de données de validation avec la graine VRF, привязанным к `(dataspace_id, lane_id)`. DS privé - autorisé ; public DS - vivacité ouverte avec la politique anti-Sybil. Le comité Nexus mondial est en place.
- Planification des transactions : vous pouvez utiliser les transactions atomiques avec les DSID et les fichiers de lecture-écriture. DS est utilisé en parallèle dans le logement ; Le comité Nexus effectue la transition dans un bloc de 1 s, si tous les artefacts DS sont prouvés et les certificats DA приходят вовремя (<= 300 ms).
- Isolation des performances : vous pouvez créer des pools de mémoire et utiliser DS. Les quotas par DS contrôlent la transition sur le bloc pour le DS actuel, préviennent le blocage de tête de ligne et augmentent la latence DS privée.Modèle de données et espace de noms
- ID qualifiés DS : все сущности (domaines, comptes, actifs, rôles) квалифицированы `dsid`. Exemple : `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Références globales : le dossier global - le court `(dsid, object_id, version_hint)` et peut être utilisé pour la configuration en chaîne dans la couche Nexus ou dans les descripteurs AMX pour cross-DS.
- Sérialisation Norito : toutes les solutions cross-DS (descripteurs AMX, preuves) utilisent les codecs Norito. Serde в продакшене не применяется.Smart Contracts et mise à jour IVM
- Contexte d'exécution : ajoutez `dsid` dans le contexte en utilisant IVM. Le contrat Kotodama est utilisé dans l'espace de données concret.
- Primitives atomiques Cross-DS :
  - `amx_begin()` / `amx_commit()` gère la transition atomique multi-DS sur l'hôte IVM.
  - `amx_touch(dsid, key)` détecte l'intention de lecture/écriture pour la détection de conflits et l'emplacement des racines d'instantané.
  - `verify_space_proof(dsid, proof, statement)` -> booléen
  - `use_asset_handle(handle, op, amount)` -> résultat (l'opération de résolution est uniquement basée sur la configuration politique et la poignée valide)
- Gestion des actifs et frais :
  - Операции активов авторизуются политиками ISI/rôle DS ; комиссии оплачиваются в gas токене DS. Les jetons de capacité facultatifs et la politique plus riche (multi-approbateur, limites de débit, géorepérage) peuvent être utilisés sans modification des modèles atomiques.
- Déterminisme : tous les nouveaux appels système sont effectués et sont détectés lors de la lecture/écriture d'AMX. Les effets sur les effets ou les effets sont nets.Preuves de validité post-quantique (ISI généralisées)
- FASTPQ-ISI (PQ, pas de configuration fiable) : argument basé sur le hachage, conçu pour le transfert vers l'ISI, avec une seule preuve sous-jacente pour un transfert par lots de 20 000 $ dans la classe GPU.
  - Profil opérationnel :
    - Les nœuds de production utilisent le prouveur `fastpq_prover::Prover::canonical`, pour créer le backend de production ; детерминированный moqueur удален. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) et `irohad --fastpq-execution-mode` permettent à l'opérateur de déterminer l'utilisation du CPU/GPU, un hook d'observateur a été demandé/résolu/backend. pour les audits de flotte. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Arithmétisation :
  - KV-Update AIR : traitez WSV comme une carte clé-valeur typique, en utilisant Poseidon2-SMT. L'ISI est configuré dans le cadre des opérations de lecture-vérification-écriture sur les clés (comptes, actifs, rôles, domaines, métadonnées, approvisionnement).
  - Contraintes liées à l'opcode : les tables AIR et les colonnes de sélection sont appliquées par ISI (conservation, compteurs monotones, autorisations, vérifications de plage, mises à jour de métadonnées limitées).
  - Arguments de recherche : tableaux de hachage validés pour les autorisations/rôles, les précisions des actifs et les paramètres de politique, ainsi que l'organisation au niveau du bit.- Engagements et mises à jour de l'État :
  - Preuve SMT agrégée : tous les clés (pré/post) documentent les projets `old_root`/`new_root` avec la frontière et les frères et sœurs dédoublonnés.
  - Invariants : les investisseurs globaux (par exemple, l'offre totale par actif) imposent l'égalité multiensemble avec les lignes d'effet et les compteurs suivis.
- Système de preuve :
  - Engagements polynomiaux de style FRI (DEEP-FRI) avec arité высокой (8/16) et explosion 8-16 ; Hachages Poséidon2 ; Transcription de Fiat-Shamir selon SHA-2/3.
  - Récursivité facultative : agrégation récursive DS-locale pour créer des micro-lots dans une seule preuve sur un emplacement de nouveau.
- Portée et exemples :
  - Actifs : transfert, création, gravure, enregistrement/désenregistrement des définitions d'actifs, définition de la précision (limitée), définition des métadonnées.
  - Comptes/domaines : créer/supprimer, définir une clé/un seuil, ajouter/supprimer des signataires (état uniquement ; preuves de l'attestation des validateurs DS, non fournies dans AIR).
  - Rôles/Autorisations (ISI) : accorder/révoquer des rôles et des autorisations ; tables de recherche forcées et vérifications de politique monotones.
  - Contrats/AMX : marqueurs de début/commit AMX, capacité d'activation/révocation lors de la validation ; доказываются как transitions d'état et compteurs de politique.
- Vérifications hors AIR pour la latence de sécurité :- Les développeurs et les cryptographes (par exemple, les signatures des utilisateurs ML-DSA) vérifient les validateurs DS et attestent auprès de DS QC ; la preuve de validité indique la cohérence de l'état et la conformité aux politiques. Это оставляет preuves PQ и быстрыми.
- Objectifs de performances (processeur 32 cœurs + GPU intégré) :
  - 20 000 ISI mixtes avec une seule touche (<= 8 touches/ISI) : ~0,4-0,9 s de preuve, ~150-450 Ko de preuve, ~5-15 ms de vérification.
  - Il y a plus d'ISI : micro-batch (par exemple, 10x2k) + récursivité, qui dure <1 s par emplacement.
- Configuration du manifeste DS :
  -`zk.policy = "fastpq_isi"`
  -`zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  -`state.commitment = "smt_poseidon2"`
  -`zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (pour DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (à utiliser ; les alternatives doivent être prises en compte)
- Solutions de repli :
  - L'option ISI peut utiliser STARK (`zk.policy = "stark_fri_general"`) avec preuve différée et finalité 1s pour l'attestation QC + barre oblique à chaque fois доказательствах.
  - Les variantes non PQ (par exemple, Plonk et KZG) menacent une configuration fiable et ne sont pas compatibles avec votre ordinateur de travail.AIR Primer (pour Nexus)
- Trace d'exécution : матрица с шириной (colonnes de registre) et длиной (étapes). Каждая строка - LOGический шаг ISI ; Les options incluent les paramètres pré/post, le sélecteur et les drapeaux.
- Contraintes :
  - Contraintes de transition : appliquer le mode de paiement des transactions (par exemple, post_balance = pre_balance - montant des transactions de débit pour `sel_transfer = 1`).
  - Contraintes de limite : activez les E/S publiques (ancienne_racine/nouvelle_racine, compteurs) et les opérations précédentes.
  - Recherches/permutations : garantir l'adhésion et l'égalité multiensemble dans les tableaux validés (autorisations, paramètres d'actifs) sans aucun schéma de bits.
- Engagement et vérification :
  - Le prouveur utilise des codes basés sur le hachage et des polynômes de bas degré, validés seulement pour la compréhension de l'organisation.
  - Le vérificateur prouve le faible degré de FRI (basé sur le hachage, post-quantique) avec Merkle открытиями; стоимость логарифмична по étapes.
- Exemple (Transfert) : les registres incluent le pre_balance, le montant, le post_balance, le nonce et les sélecteurs. L'organisation applique la maintenance/dépannage, la maintenance et la maintenance monotone, un SMT agrégé multi-preuve reliant les feuilles pré/post aux anciennes/nouvelles racines.Evolution d'ABI et des appels système (ABI v1)
- Syscalls avec appel (en anglais) :
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Types de Pointer-ABI à ajouter :
  -`PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Требуемые обновления:
  - Добавить в `ivm::syscalls::abi_syscall_list()` (сохранять порядок), porte pour la politique.
  - Mapper les numéros les plus récents dans `VMError::UnknownSyscall` dans les hôtes.
  - Effectuer des tests : liste d'appels système Golden, hachage ABI, Goldens d'ID de type de pointeur et tests de politique.
  - Documents : `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Modèles privés
- Confinement des données privées : les transferts, les sauvegardes et les instantanés WSV pour DS privés ne permettent pas de sous-ensemble de validateurs privés.
- Exposition publique : exportation d'en-têtes, d'engagements DA et de preuves de validité PQ.
- Preuves ZK facultatives : les preuves privées DS peuvent être utilisées pour les preuves ZK (par exemple, les soldes de sécurité, les politiques sociales), les tests cross-DS peuvent être effectués sans avoir à ouvrir la session. состояния.
- Contrôle d'accès : авторизация appliqué политиками ISI/rôle внутри DS. Les jetons de capacité sont opérationnels et peuvent être utilisés à deux reprises.Spécifications des produits et de la QoS
- Consensus supplémentaire, mempools et stockage sur DS.
- Les quotas de planification Nexus par DS permettent de déverrouiller les ancres et de prévenir le blocage de tête de ligne.
- Les budgets de ressources contractuelles par DS (calcul/mémoire/IO) sont appliqués à l'hôte IVM. Les conflits entre DS publics et DS privés ne peuvent pas être pris en compte.
- Les cross-DS vous permettent de créer des connexions synchronisées avec l'utilisation de Private-DS.

Disponibilité des données et conception du stockage
1) Codage d’effacement
- Utilisez Reed-Solomon systématique (par exemple, GF(2^16)) pour le codage d'effacement au niveau blob des blocs Kura et des instantanés WSV : paramètres `(k, m)` et fragments `n = k + m`.
- Paramètres de configuration (DS public) : `k=32, m=16` (n = 48), utilisation de 16 fragments compatibles avec une extension d'environ 1,5x. Pour DS privé : `k=16, m=8` (n=24) est autorisé par le propriétaire. Il est configuré selon le manifeste DS.
- Blobs publics : les fragments sont utilisés pour de nombreux nœuds/validateurs DA et des contrôles de disponibilité basés sur l'échantillonnage. Les engagements DA dans les en-têtes permettent aux clients légers de s'afficher.
- Blobs privés : les fragments sont stockés et transférés jusqu'à nos validateurs privés-DS (ou les dépositaires désignés). Les engagements globaux ne nécessitent que des engagements DA (sauf emplacements de fragments ou clés).2) Engagements et échantillonnage
- Pour ce blob : ouvrez la racine Merkle sur les fragments et ouvrez le `*_da_commitment`. En prenant PQ, vous obtenez des engagements à courbe elliptique.
- Attesteurs DA : les attestateurs régionaux échantillonnés par VRF (par exemple, 64 par région) obtiennent le certificat ML-DSA-87 pour les fragments d'échantillonnage usuels. Latence d'attestation DA <=300 ms. Le comité Nexus valide les certificats pour la sélection des fragments.

3) Intégration Kura
- Bloque les organismes de transaction tels que les blobs codés par effacement et les engagements Merkle.
- Les en-têtes ne contiennent pas d'engagements blob ; bodys извлекаются через DA сеть для public DS и через private каналы для private DS.

4) Intégration WSV
- Instantanés WSV : le point de contrôle DS périodique permet d'obtenir des instantanés fragmentés et codés avec effacement et des engagements dans les en-têtes. Ces instantanés prennent en charge les journaux de modifications. Instantanés publics широко shard-ятся; les instantanés privés sont fournis par des validateurs privés.
- Accès portant sur des preuves : les contrats peuvent fournir des preuves d'état (Merkle/Verkle), des engagements instantanés ancrés. Private DS peut utiliser des attestations de connaissance nulle ainsi que des preuves brutes.5) Rétention et taille
- Élagage net pour DS public : gérer tous les corps Kura et les instantanés WSV à partir de DA (généralement masштабирование). Le DS privé peut s'occuper de la rétention des entreprises, mais les engagements d'exportation ne sont pas nécessaires. La couche Nexus contient tous les blocs Nexus et les engagements d'artefacts DS.

Ensemble et rôle des utilisateurs
- Validateurs globaux : ils participent au consensus Nexus, valident les blocs Nexus et les artefacts DS, effectuent les contrôles DA pour DS publics.
- Validateurs d'espace de données : obtenir le consensus DS, utiliser des contrats, mettre en place un Kura/WSV local et créer un DA pour votre DS.
- Noeuds DA (officiellement) : créer/publier des blobs publics, effectuer un échantillonnage. Pour les nœuds DS DA privés co-localisés avec les validateurs ou les dépositaires.Organisation et surveillance du système
- Découplage de séquençage/mempool : créez un mempool DAG (par exemple, Narwhal-style), placez BFT en pipeline sur la couche de connexion, pour analyser la latence et augmenter le débit sans modifier les modèles logiques.
- Quotas DS et équité : les quotas par bloc et les plafonds de poids empêchent le blocage de tête de ligne et réduisent la latence pour les DS privés.
- Attestation DS (PQ) : les certificats de quorum DS utilisent ML-DSA-87 (Dilithium5-class) pour l'enregistrement. Ce post-vente et plus, comme EC, ne s'appliquent pas par exemple à 1 QC sur le emplacement. DS peut être consulté sur ML-DSA-65/44 (menьше) ou EC, également dans le manifeste DS ; public DS настоятельно рекомендуется сохранять ML-DSA-87.
- Attestateurs DA : pour les DS publics, utilisez des attestateurs régionaux échantillonnés par VRF, выдающих certificats DA. Le comité Nexus valide les certificats pour l'échantillonnage de fragments bruts ; les attestations privées DS держат DA внутренними.
- Preuves de récursion et d'époque : vous pouvez augmenter de manière optionnelle certains micro-lots dans DS en utilisant une preuve récursive à l'emplacement/à l'étape pour les preuves de planification stables et en vérifiant régulièrement vos preuves. нагрузке.
- Mise à l'échelle des voies (если нужно): если единый globalный comité становится узким местом, ввести K параллельных séquençage des voies avec детерминированным fusion. C'est un projet global et mondial.- Accélération déterministe : permet aux noyaux SIMD/CUDA d'utiliser les indicateurs de fonctionnalité pour le hachage/FFT et le repli du processeur au bit exact, afin de résoudre le problème à un certain niveau.
- Seuils d'activation des voies (proposition) : 2 à 4 voies, soit (a) la finalité p95 prévaudra 1,2 s pendant 3 minutes, ou (b) l'occupation par bloc prévaudra 85 % pendant 5 minutes, ou (c) Le taux d'émission actuel est supérieur à 1,2x la capacité de bloc utilisée. Les voies déterminent la transition entre le bucket et le hachage DSID et la fusion dans le bloc Nexus.

Frais et économie (начальные дефолты)
- Unité de gaz : jeton de gaz par DS avec calcul/IO mesuré ; les frais оплачиваются в нативном gaz actif DS. La conversion du DS - ответственность приложения.
- Priorité d'inclusion : round-robin entre les quotas DS et par DS pour l'équité et le SLO 1 s ; внутри DS fee biddding может разруливать.
- Avenir : un marché mondial des frais ou une politique de minimisation du MEV sans l'atomicité ou une conception de preuve PQ.Workflow Cross-Data-Space (par exemple)
1) Пользователь отправляет AMX транзакцию, затрагивающую public DS P и private DS S: переместить actif X из S k bénéficiaire B, ce compte в P.
2) L'emplacement P et S utilise vos fragments sur l'instantané. S проверяет l'autorisation et la disponibilité, обновляет внутреннее состояние et former la preuve de validité PQ et l'engagement DA (sans données privées). P готовит соответствующее обновление состояния (par exemple, menthe/brûlure/verrouillage dans P по политике) et свое preuve.
3) Le comité Nexus vérifie les preuves DS et les certificats DA ; Si vous êtes valide sur votre emplacement, la validation de la transition est automatique dans le bloc Nexus 1s, il est désormais possible d'obtenir des racines DS dans le monde entier.
4) Si une preuve ou un certificat DA est détecté/nécessaire, l'abandon de la transaction (sans effets) et le client peut s'effectuer sur l'emplacement correspondant. Никакие private данные S не покидают на любом шаге.- Considérations de sécurité
- Exécution déterministe : les appels système IVM остаются детерминированными ; Les résultats cross-DS incluent le commit et la finalité d'AMX, une horloge murale ou des jeux de société.
- Contrôle d'accès : les autorisations ISI dans la gestion DS privée, qui peuvent gérer les transactions et les opérations. Les jetons de capacité кодируют права для cross-DS.
- Confidentialité : vérification de bout en bout pour les données DS privées, fragments codés à effacement ainsi que les preuves ZK pour les attestations personnelles.
- Résistance au DoS : l'isolation de votre pool de mémoire/consensus/stockage empêche la congestion publique du processus DS privé.

Installation des composants Iroha
- iroha_data_model : ajoutez `DataSpaceId`, les identifiants qualifiés DS, les descripteurs AMX (ensembles de lecture/écriture), les types de preuves/engagements DA. Только Norito classification en série.
- ivm : créer des appels système et des types de pointeur-ABI pour AMX (`amx_begin`, `amx_commit`, `amx_touch`) et des preuves DA ; mettre à jour les tests/docs ABI pour la version v1.