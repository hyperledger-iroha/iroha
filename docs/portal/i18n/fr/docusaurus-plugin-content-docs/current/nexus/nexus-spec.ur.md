---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-spec
titre : Sora Nexus en cours de réalisation
description : `docs/source/nexus.md` pour le téléphone et Iroha 3 (Sora Nexus) pour le téléphone portable پابندیوں کا احاطہ کرتی ہے۔
---

:::note کینونیکل ماخذ
یہ صفحہ `docs/source/nexus.md` کی عکاسی کرتا ہے۔ ترجمے کا بیک لاگ پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

#! Iroha 3 - Sora Nexus Ledger : تکنیکی ڈیزائن اسپیسفیکیشن

Iroha 3 pour Sora Nexus Ledger et Iroha 2 Les espaces de données (DS) sont en mesure de créer des espaces de données plus larges que les autres. Espaces de données مضبوط پرائیویسی ڈومینز ("espaces de données privés") اور کھلی شمولیت ("espaces de données publics") فراہم کرتے ہیں۔ Il s'agit d'une question de composabilité et d'isolement et de confidentialité. Il s'agit de Kura (stockage en bloc) et de WSV (World State View) ainsi que du codage d'effacement et de la disponibilité des données, ainsi que de l'échelle et de la disponibilité des données.

یہی ریپوزٹری Iroha 2 (réseaux auto-hébergés) et Iroha 3 (SORA Nexus) sont en cours de build ہے۔ Voir la machine virtuelle Iroha (IVM) et la chaîne d'outils Kotodama pour les contrats et les artefacts de bytecode Appareil photo Nexus pour ordinateur portable portableاہداف
- Les validateurs et les espaces de données sont également disponibles pour vous.
- Autorisé les CBDC (ou CBDC) pour les espaces de données privés, ainsi que les DS privés et les espaces de données privés.
- Espaces de données publics pour les utilisateurs d'Ethereum sans autorisation
- Espaces de données pour les contrats intelligents composables, pour les actifs DS privés pour les contrats intelligents et les actifs DS privés.
- isolation des performances pour le public et le privé-DS pour le système d'isolation des performances
- disponibilité des données à grande échelle : Kura codé avec effacement et WSV تاکہ طور پر لامحدود ڈیٹا سپورٹ ہو جبکہ private-DS ڈیٹا نجی رہے۔

غیر اہداف (ابتدائی مرحلہ)
- économie des jetons et incitations des validateurs planification et jalonnement enfichable
- نئی ABI ورژن متعارف کرانا؛ Le IVM est un appel système explicite et des extensions pointeur-ABI pour ABI v1 et des extensions de pointeur-ABI.اصطلاحات
- Nexus Ledger : L'espace de données (DS) est utilisé pour créer un engagement de l'État et composer un compte bancaire ہے۔
- Data Space (DS) : exécution rapide et stockage, ainsi que des validateurs, gouvernance, classe de confidentialité, politique DA, quotas, politique de frais et politique de frais. Il s'agit d'un DS public et d'un DS privé.
- Espace de données privé : validateurs autorisés et contrôle d'accès ٹرانزیکشن ڈیٹا اور state کبھی DS سے باہر نہیں جاتے۔ صرف engagements/métadonnées عالمی طور پر ancre ہوتے ہیں۔
- Espace de données public : شمولیت؛ sans autorisation مکمل ڈیٹا اور state عوامی طور پر دستیاب ہیں۔
- Manifeste d'espace de données (manifeste DS) : manifeste codé en Norito et paramètres DS (validateurs/clés QC, classe de confidentialité, politique ISI, paramètres DA, rétention, quotas, politique ZK, frais) chaîne de liens de hachage manifeste پر ancre ہوتا ہے۔ Il s'agit de remplacer les certificats de quorum DS ML-DSA-87 (classe Dilithium5) et le schéma de signature post-quantique par défaut et de créer un système de signatures post-quantiques par défaut.
- Répertoire spatial : contrat d'annuaire en chaîne, manifestes DS, versions, événements de gouvernance/rotation, résolvabilité, audits et vérification des comptes.
- DSID : espace de données est disponible pour tous les utilisateurs. Objets et références pour l'espace de noms et l'espace de noms
- Ancre : bloc/en-tête DS et engagement cryptographique et historique DS et lien vers la chaîne de connexion et la chaîne de connexion.- Kura : stockage en bloc Iroha۔ Il s'agit d'un stockage blob codé par effacement et d'engagements pour un stockage blob codé par effacement.
- WSV : Iroha Vue de l'état mondial۔ Certains segments d'état versionnés, compatibles avec les instantanés et codés par effacement sont également disponibles.
- IVM : exécution de contrat intelligent ou machine virtuelle Iroha (bytecode Kotodama `.to`)
  - AIR : Représentation Algébrique Intermédiaire۔ STARK propose des preuves, un calcul, une vue algébrique, une exécution, des traces basées sur le champ, une transition et des contraintes aux limites, ainsi qu'une solution de contournement.Espaces de données
- Identité : `DataSpaceId (DSID)` DS pour l'espace de noms et l'espace de noms DS et granularités pour instancier et créer des liens :
  - Domaine-DS : `ds::domain::<domain_name>` - exécution et état du domaine تک محدود۔
  - Asset-DS : `ds::asset::<domain_name>::<asset_name>` - exécution et état et définition de l'actif en cours
  دونوں شکلیں ساتھ موجود ہیں؛ Les DSID sont atomiquement connectés et tactiles.
- Cycle de vie du manifeste : création DS, mises à jour (rotation des clés, changements de politique) et retrait de Space Directory. Un artefact DS par emplacement est un hachage manifeste et une référence.
- Cours : DS public (participation ouverte, DA publique) et DS privé (autorisé, DA confidentiel) ۔ les politiques hybrides manifestent des drapeaux
- Politiques par DS : autorisations ISI, paramètres DA `(k,m)`, cryptage, rétention, quotas (par partage de transmission de bloc کی min/max), politique ZK/preuve optimiste, frais.
- Gouvernance : adhésion au DS et manifeste de rotation des validateurs et gouvernance et attestations de gouvernance externe ancrée dans les propositions en chaîne (propositions en chaîne, multisig et transactions nexus et attestations).Manifestations de capacités اور UAID
- Comptes universels : participant et UAID déterministe (`UniversalAccountId` dans `crates/iroha_data_model/src/nexus/manifest.rs`) et espaces de données différents. Manifestes de capacités (`AssetPermissionManifest`) UAID pour l'espace de données, les époques d'activation/d'expiration et autoriser/refuser `ManifestEntry` pour le stockage des données Utilisez les rôles `dataspace`, `program_id`, `method`, `asset` pour les rôles AMX et les rôles AMX. ہے۔ Deny قواعد ہمیشہ غالب رہتے ہیں؛ évaluateur et raison de l'audit et `ManifestVerdict::Denied` et métadonnées de l'allocation de correspondance et `Allowed` subvention et
- Autorisations : permettre l'entrée dans les seaux déterministes `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) et les allocations `max_amount` en ligne Hôtes et SDK avec charge utile Norito pour les applications matérielles et les implémentations du SDK
- Télémétrie d'audit : Space Directory est un manifeste et un état défini par `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) actuellement disponible Le `SpaceDirectoryEventFilter` surface Torii/abonnés aux événements de données et les mises à jour du manifeste UAID, les révocations et les refus de victoires pour la plomberie personnalisée. دیتی ہے۔Preuves de bout en bout de l'opérateur, notes de migration du SDK et listes de contrôle de publication de manifestes, ainsi que le guide de compte universel (`docs/source/universal_accounts_guide.md`) et le miroir miroir. Politique UAID et outils pour le développement et la gestion des ressources humaines

اعلی سطحی معماری
1) Couche de composition globale (chaîne Nexus)
- 1 bloc Nexus Blocs de données pour le stockage des données Espaces (DS) pour finaliser les espaces atomiques Il s'agit d'un État mondial engagé (racines par DS et vecteur)
- Il y a des métadonnées et des preuves agrégées/QC pour une composabilité et une finalité et une détection de fraude (touchez les DSID par DSID et les racines d'état par DS). پہلے/بعد، Engagements DA, preuves de validité par DS, اور ML-DSA-87 et certificat de quorum DS)۔ کوئی données privées شامل نہیں ہوتا۔
- Consensus : un comité BFT mondial en pipeline a été créé pour 22 (3f+1 avec f=7) et environ 200 000 validateurs, un pool et un mécanisme VRF/enjeu d'époque. منتخب ہوتا ہے۔ Comité Nexus ٹرانزیکشنز کو séquence کرتا ہے اور بلاک کو 1s میں finaliser کرتا ہے۔2) Couche d'espace de données (publique/privée)
- Les fragments globaux par DS exécutent des fragments WSV locaux DS et mettent à jour les artefacts de validité par bloc (preuves agrégées par DS et engagements DA) en 1 seconde Nexus Bloc enroulable en haut
- Données DS privées au repos et données en vol et validateurs autorisés et chiffrement des données privées صرف engagements اور PQ preuves de validité DS سے باہر جاتے ہیں۔
- Corps de données publics DS (via DA) et exportations de preuves de validité PQ3) Transactions atomiques inter-espaces de données (AMX)
- Modèle : transaction utilisateur par DS et par contact tactile (par domaine DS et par actif DS) یہ ایک ہی Nexus Bloquer la validation atomique ہوتی ہے یا abandonner؛ جزوی اثرات نہیں۔
- Préparer-Commit dans les 1s : la transaction candidate est directement touchée par DS et un instantané (emplacement pour les racines DS) et une exécution parallèle est également effectuée pour les preuves de validité PQ par DS (FASTPQ-ISI) اور DA engagements بناتے ہیں۔ Nexus Committee ٹرانزیکشن کو تبھی commit کرتا ہے جب تمام مطلوبہ DS proofs verify ہوں اور DA certificates وقت پر پہنچیں (ہدف <=300 ms)؛ ورنہ ٹرانزیکشن اگلے slot کے لئے reprogrammer ہوتی ہے۔
- Cohérence : les ensembles en lecture-écriture déclarent ہوتے ہیں؛ détection de conflit racines de début d'emplacement کے خلاف commit پر ہوتی ہے۔ exécution optimiste sans verrouillage par DS global stalls سے بچاتی ہے؛ règle de validation du lien d'atomicité
- Confidentialité : DS privé avec racines pré/post DS et exportation de preuves/engagements کرتے ہیں۔ Données privées brutes DS en ligne4) Disponibilité des données (DA) avec codage d'effacement
- Corps de blocs Kura et instantanés WSV et blobs codés par effacement blobs publics وسیع پیمانے پر shard ہوتے ہیں؛ les blobs privés sont des validateurs privés-DS et des morceaux chiffrés sont des éléments clés en main.
- Engagements DA Artefacts DS comme Nexus Blocs pour les garanties de récupération de contenu privé et de contenu privé کیے۔

بلاک اور کمٹ ڈھانچہ
- Artefact de preuve d'espace de données (emplacement 1 s, DS)
  - Exemples : dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Corps de données d'artefacts Private-DS pour l'exportation de fichiers public DS DA کے ذریعے body recovery کی اجازت دیتے ہیں۔

- Bloc Nexus (cadence 1s)
  - Exemples : block_number, parent_hash, slot_time, tx_list (les DSID touchés dans les transactions atomiques cross-DS), ds_artifacts[], nexus_qc.
  - Fonction : et la fonction atomique est finalisée et les artefacts DS sont vérifiés. vecteur d'état mondial global et racines DS ici et maintenant mise à jourConsensus sur la programmation
- Consensus de la chaîne Nexus : BFT global unique en pipeline (classe Sumeragi) et comité à 22 nœuds (3f + 1 avec f = 7) entre blocs de 1s et finalité de 1s et finalité de 1s. membres du comité ~200 000 candidats dans le pool et dans le VRF/pieu d'époque rotation décentralisation et résistance à la censure
- Consensus sur l'espace de données : les validateurs DS et les validateurs BFT ainsi que les artefacts par emplacement (preuves, engagements DA et DS QC) pour chaque emplacement. Comités de relais de voie `3f+1` pour l'espace de données `fault_tolerance` pool de validateurs d'espace de données pour l'époque et l'échantillon déterministe La graine d'époque VRF est `(dataspace_id, lane_id)` et est liée à la graine d'époque VRF. DS privé autorisé ہیں؛ public DS anti-Sybil پالیسیز کے تحت open liveness دیتے ہیں۔ comité de connexion mondial تبدیل نہیں ہوتا۔
- Planification des transactions : les systèmes atomiques soumettent des fichiers DSID touchés et des ensembles de lecture-écriture déclarent des fichiers DSID Emplacement DS pour exécution parallèle Nexus Committee ٹرانزیکشن کو 1s بلاک میں تبھی شامل کرتا ہے جب تمام Les artefacts DS vérifient et les certificats DA sont effectués (<= 300 ms)۔- Isolation des performances : DS et mempools indépendants et exécution quotas par DS یہ حد باندھتی ہیں کہ کسی DS کو touch کرنے والی کتنی ٹرانزیکشنز فی بلاک commit ہو سکتی ہیں Le blocage de la tête de ligne est un problème de latence DS privée.

Comment utiliser l'espace de noms
- ID qualifiés DS : les entités (domaines, comptes, actifs, rôles) `dsid` sont qualifiées pour chaque personne. Modèle : `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Références globales : référence globale comme tuple `(dsid, object_id, version_hint)` et couche de nexus en chaîne et cross-DS pour les descripteurs AMX et les descripteurs AMX. ہے۔
- Sérialisation Norito : messages cross-DS (descripteurs AMX, preuves) Codecs Norito chemins de production میں serde کا استعمال نہیں ہوتا۔Smart Contracts selon IVM
- Contexte d'exécution : contexte d'exécution IVM pour `dsid`. Kotodama contrats ہمیشہ کسی مخصوص Data Space کے اندر exécuter ہوتے ہیں۔
- Primitives atomiques Cross-DS :
  - `amx_begin()` / `amx_commit()` IVM hôte multi-DS atomique et délimiter les liens
  - Détection de conflits `amx_touch(dsid, key)` pour les racines de l'instantané de slot et pour la déclaration d'intention de lecture/écriture.
  - `verify_space_proof(dsid, proof, statement)` -> booléen
  - `use_asset_handle(handle, op, amount)` -> résultat (opération تبھی autorisée ہے جب politique اجازت دے اور handle valide ہو)
- Gestion des actifs et frais :
  - Opérations sur les actifs DS et politiques ISI/rôle et autorisations d'utilisation frais DS et jeton de gaz jetons de capacité facultatifs pour des politiques riches (multi-approbateurs, limites de débit, géorepérage) pour un système atomique
- Déterminisme : il y a des entrées d'appels système et des ensembles de lecture/écriture AMX déclarés, c'est pur ou déterministe. وقت یا ماحول کے effets cachés نہیں ہوتے۔Preuves de validité post-quantique (ISI généralisées)
- FASTPQ-ISI (PQ, pas de configuration fiable) : argument kernelisé, basé sur le hachage et transfert de familles ISI pour un matériel de classe GPU pour des lots à l'échelle de 20 000 secondes. prouver کو ہدف بناتا ہے۔
  - Profil opérationnel :
    - Le prouveur de nœuds de production est `fastpq_prover::Prover::canonical` pour la construction du backend de production et l'initialisation du backend de production. maquette déterministe ہٹا دیا گیا ہے۔ [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) et les opérateurs `irohad --fastpq-execution-mode` pour l'exécution CPU/GPU et les broches déterministes pour les audits de flotte de crochets d'observateur et les triples demandés/résolus/backend. ریکارڈ کرتا ہے۔ [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Arithmétisation :
  - KV-Update AIR : WSV et Poseidon2-SMT utilisent une carte clé-valeur typée validée pour traiter les problèmes. ہر Clés ISI (comptes, actifs, rôles, domaines, métadonnées, approvisionnement) پر lignes de lecture-vérification-écriture کے چھوٹے سیٹ میں développer ہوتا ہے۔
  - Contraintes liées à l'opcode : colonnes de sélection et table AIR selon l'ISI (conservation, compteurs monotones, autorisations, vérifications de plage, mises à jour de métadonnées limitées)- Arguments de recherche : autorisations/rôles, précisions des actifs, paramètres de politique et tables de hachage transparentes, contraintes au niveau du bit lourdes, etc.
- Engagements et mises à jour de l'État :
  - Preuve SMT agrégée : les touches touchées (pré/post) `old_root`/`new_root` sont une frontière compressée et prouvent que les frères et sœurs sont dédoublés ہوں۔
  - Invariants : les invariants globaux (actifs et offre totale) effectuent des lignes et des compteurs suivis pour l'égalité multiensemble et l'égalité multiensemble.
- Système de preuve :
  - Engagements polynomiaux de style FRI (DEEP-FRI) de haute arité (8/16) et explosion 8-16 pouces Hachages Poséidon2؛ Transcription Fiat-Shamir SHA-2/3 کے ساتھ۔
  - Récursivité facultative : micro-lots, preuve de preuve et compression de slot, agrégation récursive locale DS
- Portée et exemples couverts :
  - Actifs : transfert, création, gravure, enregistrement/désenregistrement des définitions d'actifs, définition de la précision (limitée), définition des métadonnées
  - Comptes/domaines : créer/supprimer, définir une clé/un seuil, ajouter/supprimer des signataires (les contrôles de signature effectués uniquement par l'État sont attestés par les validateurs DS کرتے ہیں، AIR et prouvent نہیں ہوتے)۔
  - Rôles/Autorisations (ISI) : accorder/révoquer des rôles et des autorisations tables de recherche et vérifications de politique monotones et application d'applications
  - Contrats/AMX : marqueurs de début/commit AMX, capacité d'activation/révocation et activation transitions d'état et compteurs politiques کے طور پر prouver ہوتے ہیں۔- Vérifications Out-of-AIR pour préserver la latence :
  - Signatures et cryptographie lourde (signatures d'utilisateurs ML-DSA) Les validateurs DS vérifient les points et DS QC et attestent les points de contrôle. preuve de validité صرف cohérence de l'état اور conformité à la politique کو couverture کرتا ہے۔ یہ preuves کو PQ اور تیز رکھتا ہے۔
- Objectifs de performances (à titre indicatif, CPU 32 cœurs + un seul GPU moderne) :
  - 20 000 ISI mixtes avec petites touches (<= 8 touches/ISI) : ~0,4-0,9 s de preuve, ~150-450 Ko de preuve, ~5-15 ms de vérification۔
  - ISI plus lourds (plus de clés/contraintes riches) : micro-batch (environ 10x2k) + récursion par emplacement <1 s par emplacement
- Configuration du manifeste DS :
  -`zk.policy = "fastpq_isi"`
  -`zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  -`state.commitment = "smt_poseidon2"`
  -`zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (signatures DS QC vérifier کرتا ہے)
  - `attestation.qc_signature = "ml_dsa_87"` (alternatives par défaut et explicites ou déclaration de déclaration)
- Solutions de repli :
  - ISI complexes/personnalisés STARK (`zk.policy = "stark_fri_general"`) استعمال کر سکتے ہیں جس میں preuve différée اور 1 s finalité QC attestation + slashing کے ذریعے ہوتی ہے۔
  - Options non PQ (par exemple Plonk avec KZG) configuration fiable et version par défaut prise en charge par défautAIR تعارف (Nexus کے لئے)
- Trace d'exécution : matrice et largeur (colonnes de registre) et longueur (étapes) et largeur (colonnes de registre) et longueur (étapes) ہر ligne de traitement ISI کا منطقی قدم ہے؛ colonnes, valeurs pré/post, sélecteurs, et drapeaux, etc.
- Contraintes :
  - Contraintes de transition : relations ligne à ligne par rapport à la ligne de débit (post_balance = pre_balance - montant par `sel_transfer = 1` et ligne de débit)
  - Contraintes de limite : E/S publiques (ancienne_racine/nouvelle_racine, compteurs) et lignes/lignes liées et liaisons entre les lignes.
  - Recherches/permutations : tables validées (autorisations, paramètres d'actifs) et adhésions et égalités multiensembles et circuits à forte densité de bits.
- Engagement et vérification :
  - Traces du prouveur et encodages basés sur le hachage et validations et polynômes de bas degré et contraintes et contraintes valides.
  - Vérificateur FRI (basé sur le hachage, post-quantique) et contrôle de faible degré pour les ouvertures Merkle et les ouvertures Merkle étapes de coût کے log پر منحصر ہے۔
- Exemple (Transfert) : enregistre le pre_balance, le montant, le post_balance, le nonce, les sélecteurs et les sélecteurs Contraintes de non-négativité/gamme, de conservation, et de monotonie ponctuelle, de type de feuilles SMT agrégées multi-preuves pré/post et de racines anciennes/nouvelles.ABI et Syscall (ABI v1)
- Syscalls à ajouter (noms illustratifs) :
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Types de pointeur-ABI à ajouter :
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Mises à jour requises :
  - `ivm::syscalls::abi_syscall_list()` میں شامل کریں (commande de commande) et politique de porte d'entrée
  - numéros inconnus et hôtes comme `VMError::UnknownSyscall` sur la carte
  - mise à jour des tests comme : liste d'appels système dorée, hachage ABI, ID de type de pointeur doré, et tests de politique
  - documents : `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Confidentialité
- Confinement des données privées : DS privés, pour les organismes de transaction, les différences d'état, et les instantanés WSV, ainsi que le sous-ensemble de validateurs privés et pour les différents états.
- Exposition publique : exportation des en-têtes, des engagements DA et des preuves de validité PQ.
- Preuves ZK facultatives : preuves DS ZK privées (pour le solde et la politique et pour les actions inter-DS) ہوں۔
- Contrôle d'accès : autorisation DS et ISI/politiques de rôle jetons de capacité جا سکتے ہیں۔Isolation des performances et QoS
- DS et consensus, mempools et stockage
- Nexus quotas de planification par temps d'inclusion d'ancre DS pour le blocage de tête de ligne et le blocage de tête de ligne
- Budgets de ressources contractuelles par DS (calcul/mémoire/IO) IVM hôte کے ذریعے نافذ ہوتے ہیں۔ Conflit public-DS budgets DS privés استعمال نہیں کر سکتی۔
- Appels cross-DS asynchrones, exécution DS privée et attentes synchrones.

Disponibilité des données et conception du stockage
1) Codage d’effacement
- Kura bloque les instantanés WSV et le codage d'effacement au niveau blob ainsi que le système Reed-Solomon systématique (avec GF (2 ^ 16)) et les paramètres `(k, m)` et `n = k + m`. fragments ہیں۔
- Paramètres par défaut (proposés, DS publics) : `k=32, m=16` (n = 48), soit une extension d'environ 1,5x pour 16 fragments et une récupération de données. DS privé : `k=16, m=8` (n = 24) ensemble autorisé en anglais Le DS Manifest est configurable
- Blobs publics : les fragments et les nœuds/validateurs DA distribuent des fragments et des contrôles de disponibilité basés sur l'échantillonnage. en-têtes pour les engagements DA clients légers et vérifier les engagements
- Blobs privés : les fragments chiffrent les validateurs DS privés (dépositaires désignés) et les distribuent. chaîne mondiale صرف Engagements DA رکھتی ہے (emplacements des fragments یا clés نہیں)۔2) Engagements et échantillonnage
- Un blob et des fragments de racine de Merkle sont ajoutés à `*_da_commitment`. engagements à courbe elliptique pour PQ
- Attesteurs DA : attestateurs régionaux échantillonnés par VRF (région 64) avec échantillonnage de fragments réussi et attestation pour certificat ML-DSA-87 et certificat ML-DSA-87. ہدف Latence d’attestation DA <=300 ms ہے۔ les fragments du comité Nexus sont validés par les certificats validés

3) Intégration Kura
- Bloque les organismes de transaction et les engagements Merkle ainsi que les blobs codés à effacement ainsi que les magasins de stockage.
- Engagements blob d'en-têtes organismes publics DS ou réseau DA et privés DS ou chaînes privées ou chaînes privées

4) Intégration WSV
- Instantanés WSV : instantanés périodiques de l'état DS et instantanés fragmentés et codés avec effacement, point de contrôle et en-têtes d'engagements. instantanés et journaux de modifications pour les utilisateurs instantanés publics وسیع پیمانے پر shard ہوتے ہیں؛ instantanés privés validateurs privés کے اندر رہتے ہیں۔
- Proof-Carrying Access : les contrats indiquent les preuves (Merkle/Verkle) et les engagements instantanés sont ancrés. DS privées sont des preuves et des attestations de connaissance nulle.5) Rétention et élagage
- Élagage public DS et élagage: les corps Kura et les instantanés WSV DA conservent les images (mise à l'échelle horizontale) La rétention interne DS privée définit les engagements exportés comme les engagements immuables Couche Nexus Blocs Nexus pour les engagements d'artefacts DS

Mise en réseau et rôles des nœuds
- Validateurs globaux : consensus Nexus pour les contrôles Nexus Les blocs et les artefacts DS valident les DS publics et les contrôles DA.
- Validateurs d'espace de données : les contrats de consensus DS exécutent les contrats locaux Kura/WSV et gèrent les contrats DS et les poignées DA.
- Nœuds DA (facultatif) : stockage/publication de blobs publics et échantillonnage d'échantillons de blobs publics DS privés et validateurs de nœuds DA et dépositaires de confiance et colocalisationsAméliorations au niveau du système
- Découplage de séquençage/mempool : mempool DAG (style Narwhal) et couche de connexion et BFT en pipeline et alimentation, latence et débit et débit منطقی ماڈل بدلے۔
- Quotas DS et équité : quotas par DS et par bloc et plafonds de poids, blocage de tête de ligne et DS privé pour plus de latence et de latence. لئے۔
- Attestation DS (PQ) : certificats de quorum DS par défaut ML-DSA-87 (classe Dilithium5) یہ post-quantum ہے اور EC signatures سے بڑا ہے مگر ایک QC فی slot قابلِ قبول ہے۔ DS et DS Manifest déclarent le ML-DSA-65/44 (چھوٹا) et les signatures CE sont en vigueur. public DS et ML-DSA-87 pour le service public DS
- Attestateurs DA : attestateurs publics DS et VRF échantillonnés régionaux Certificats DA جاری کرتے ہیں۔ Nexus Committee Raw Shard Sampler les certificats valident les certificats privé DS اندرونی DA attestations رکھتے ہیں۔
- Preuves de récursion et d'époque : preuves récursives par emplacement/époque et agrégation de tailles de preuves et temps de vérification charge élevée مستحکم رہیں۔
- Mise à l'échelle des voies (اگر ضرورت ہو) : Il s'agit d'un goulot d'étranglement du comité mondial pour la fusion déterministe de K voies de séquençage parallèles. اس سے ordre mondial unique برقرار رہتا ہے جبکہ mise à l'échelle horizontale ہو جاتی ہے۔- Accélération déterministe : hachage/FFT pour les noyaux SIMD/CUDA et les noyaux à fonctionnalités limitées ainsi que le repli du processeur au bit exact et le déterminisme inter-matériel pour les utilisateurs
- Seuils d'activation des voies (proposition) : 2-4 voies فعال کریں اگر (a) p95 finalité 1,2 s زیادہ ہو >3 مسلسل منٹ، یا (b) occupation par bloc 85% زیادہ >5 millions de dollars (c) taux d'émission entrant soutenu pour une capacité de bloc supérieure à >1,2x taux de transmission élevé voies déterministes pour le hachage DSID pour les transactions et les compartiments pour le bloc nexus pour la fusion et la fusion

Frais et économie (valeurs par défaut)
- Unité de gaz : jeton de gaz par DS et compteur de calcul/IO frais DS et actif gazier natif Application de conversion DS en ligne
- Priorité d'inclusion : round-robin sur DS et quotas par DS, équité et SLO 1s pour chaque DS. DS et tie-break pour les enchères payantes
- Avenir : marché mondial facultatif des frais et politiques de minimisation du MEV explorant l'atomicité et la conception à l'épreuve du PQ.Flux de travail inter-espaces de données (مثال)
1) Le système AMX soumet le système public DS P au secteur privé DS S tactile: l'actif X et le bénéficiaire B sont les actifs X et S. کی compte P میں ہے۔
2) slot کے اندر، P اور S ہر ایک اپنا fragment slot snapshot کے خلاف exécuter کرتے ہیں۔ Autorisation S et vérification de la disponibilité et mise à jour de l'état interne et preuve de validité PQ et engagement DA (fuite de données privées) P متعلقہ state update تیار کرتا ہے (مثلاً politique کے مطابق P میں mint/burn/locking) et اور اپنی preuve۔
3) Comité de connexion pour les preuves DS et les certificats DA vérifier کرتی ہے؛ Il s'agit d'un emplacement pour vérifier que le système 1s Nexus Block est un commit atomique et un vecteur d'état mondial global est mis à jour des racines DS. ہیں۔
4) Preuve de preuve ou certificat DA invalide/invalide ou abandon de contrat (slot de client) لئے دوبارہ بھیج سکتا ہے۔ کسی مرحلے پر S سے کوئی données privées باہر نہیں جاتا۔- سکیورٹی پر غور و فکر
- Exécution déterministe : appels système IVM déterministes cross-DS permet le commit AMX et la finalité de l'horloge murale et la synchronisation du réseau.
- Contrôle d'accès : autorisations privées DS et ISI pour les autorisations d'accès et les opérations autorisées. jetons de capacité cross-DS pour l'encodage à grains fins
- Confidentialité : données DS privées, chiffrement de bout en bout, fragments codés par effacement, membres autorisés, attestations ZK facultatives et preuves ZK facultatives.
- Résistance au DoS : couches mempool/consensus/stockage, isolement, congestion publique et progrès du DS privé.

Composants Iroha
- iroha_data_model : `DataSpaceId`, identifiants qualifiés DS, descripteurs AMX (ensembles de lecture/écriture), types d'engagement preuve/DA sérialisation صرف Norito۔
- ivm : AMX (`amx_begin`, `amx_commit`, `amx_touch`) et les preuves DA, les appels système et les types pointeur-ABI pour les utilisateurs Tests/documents ABI pour v1 ici et maintenant