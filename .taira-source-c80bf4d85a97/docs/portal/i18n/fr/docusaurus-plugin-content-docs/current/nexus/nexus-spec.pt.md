---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-spec
titre : Spécifications techniques de Sora Nexus
description : Espelho completo de `docs/source/nexus.md`, cobrindo arquitetura et as restricoes de design para o ledger Iroha 3 (Sora Nexus).
---

:::note Fonte canonica
Cette page espelha `docs/source/nexus.md`. Mantenha ambas as copias alinhadas ate que o backlog de traducao chegue ao portal.
:::

#! Iroha 3 - Sora Nexus Ledger : Spécifications techniques de conception

Ce document propose une architecture de Sora Nexus Ledger pour Iroha 3, évoluant ou Iroha 2 pour un grand livre global unique et logiquement unifié organisé dans l'espace de données (DS). Espaces de données fornecem dominios fortes de privacidade (« espaces de données privés ») et participacao aberta (« espaces de données publics »). La conception préserve la composabilité pendant toute la durée du grand livre global en garantissant l'isolement et la confidentialité des données de Private-DS, et l'introduction de l'échelle de disponibilité des données via la codification de l'hébergement dans Kura (stockage en bloc) et WSV (World State View).Mon dépôt compile tanto Iroha 2 (redes auto-hospedadas) quanto Iroha 3 (SORA Nexus). Lors de l'exécution et de l'impulsion de la machine virtuelle Iroha (IVM), le partage et la chaîne d'outils Kotodama, de manière à ce que les contrats et les articles de bytecode soient portés de manière permanente entre les déploiements gérés automatiquement et le grand livre global Nexus.

Objectifs
- Un grand livre logique global composé de plusieurs validateurs coopérants et d'espaces de données.
- Espaces de données privés pour l'exploitation autorisée (ex., CBDC), avec des données qui ne sont jamais réservées au DS privé.
- Data Spaces publics avec participation ouverte, accès sans autorisation du style Ethereum.
- Des contrats intelligents composés entre les espaces de données, soumis à des autorisations explicites pour l'accès aux activités de private-DS.
- Isolement de l'emploi pour que l'activité publique nao dégrade les transacoes internes de private-DS.
- Disponibilité des données à un niveau supérieur : Kura et WSV avec codification de l'hébergement pour prendre en charge les données de manière efficace et illimitée en maintenant les données de private-DS privados.

Nao objetsivos (phase initiale)
- Définir l'économie des jetons ou les incitations des validateurs ; les politiques de planification et de jalonnement sont enfichables.
- Introduire une nouvelle version d'ABI ; Nous modifions ABI v1 avec des extensions explicites de syscalls et de pointeur-ABI conformes à la politique IVM.Terminologie
- Nexus Ledger : Un grand livre logique global formé avec des blocs de Data Space (DS) dans une histoire ordonnée uniquement et un compromis de l'état.
- Espace de données (DS) : Domaine délimité d'exécution et d'armement avec vos propriétaires validateurs, gouvernance, classe de confidentialité, politique de DA, quotas et politique de taxes. Il existe des classes doubles : DS publique et DS privée.
- Espace de données privé : validateurs autorisés et contrôle d'accès ; Les données de transacao et l'état n'est pas celui de DS. Il n'y a pas de compromis/métadados sao ancorados globalement.
- Espace de données publiques : Participacao sem permissao ; données complètes et état sao publicos.
- Manifeste d'espace de données (DS Manifest) : Manifeste codifié dans Norito qui déclare les paramètres DS (validateurs/chaves QC, classe de confidentialité, politique ISI, paramètres DA, retenue, quotas, politique ZK, taxes). Le hachage se manifeste et est ancorado na cadeia nexus. Salvo override, certifié de quorum DS par ML-DSA-87 (classe Dilithium5) comme une sorte d'assassinat post-quantique par père.
- Space Directory : Contrato de diretorio global on-chain que rastreia manifests DS, versoes e eventos de gouvernance/rotacao para resolucao e auditorias.
- DSID : identifiant global unique pour un espace de données. Utilisé pour l'espacement de noms de tous les objets et références.- Ancre : Compromis cryptographique d'un bloc/en-tête DS inclus dans le lien de connexion pour l'historique de DS vers le grand livre mondial.
- Kura : Armazenamento de blocos Iroha. Il s'agit ici d'un arsenal de blobs codifiés avec un emballage et des compromis.
- WSV : Iroha Vue de l'état mondial. Estendido ici avec les segments de l'état versionados, avec instantanés et codifiés avec apagamento.
- IVM : Machine virtuelle Iroha pour l'exécution de contrats intelligents (bytecode Kotodama `.to`).
  - AIR : Représentation Algébrique Intermédiaire. Visa algébrique de calcul pour le style STARK, décrit à exécuter comme des traces basées sur des champs avec des restrictions de transit et de frontière.Modèle d'espaces de données
- Identité : `DataSpaceId (DSID)` identifie un DS et permet l'espacement de noms de tout. DS peut être instanciados em deux granularités :
  - Domaine-DS : `ds::domain::<domain_name>` - exécuté et défini dans un domaine.
  - Asset-DS : `ds::asset::<domain_name>::<asset_name>` - exécuté et déterminé à une définition d'activité unique.
  Les ambas comme formes coexistent ; Les transacoes peuvent tocar multiplier les DSID de forme atomique.
- Cycle de vie du manifeste : criacao de DS, atualizacoes (rotacao de chaves, mudancas de politica) et aposentadoria sao registradas no Space Directory. Chaque artefato DS par référence de slot ou hash est le manifeste le plus récent.
- Classes : Public DS (participacao aberta, DA publica) et Private DS (permissionado, DA confidencial). La politique hybride sao possiveis via les drapeaux se manifeste.
- Politiques par DS : autorisations ISI, paramètres DA `(k,m)`, cryptographie, retenue, quotas (participation min/max de tx par bloc), politique de preuve ZK/otimiste, taxes.
- Gouvernance : adhésion DS et rotation des validateurs définies pour la section de gouvernance du manifeste (propositions en chaîne, multisig ou gouvernance externe ancorada por transacoes nexus e atestacoes).Manifestes de capacités et UAID
- Contas universais: chaque participant reçoit un UAID déterminé (`UniversalAccountId` dans `crates/iroha_data_model/src/nexus/manifest.rs`) qui modifie tous les espaces de données. Les manifestes de capacités (`AssetPermissionManifest`) désignent un UAID dans un espace de données spécifique, les époques d'activation/expiration et une liste ordonnée d'inscriptions autorisent/refusent `ManifestEntry` qui délimitent `dataspace`, `program_id`, `method`, `asset` et rôles AMX optionnels. Regras nie sempre ganham ; L'avalisateur émet `ManifestVerdict::Denied` avec un motif de salle ou une subvention `Allowed` avec les métadonnées du correspondant d'allocation.
- Allocations : chaque entrée autorise les seaux de carrega déterministes `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) mais un `max_amount` facultatif. Les hôtes et les SDK utilisent la même charge utile Norito, ce qui permet une application permanente identique entre le matériel et le SDK.
- Télémétrie des auditoires : Space Directory transmet `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) toujours comme un manifeste de l'état. La nouvelle surface `SpaceDirectoryEventFilter` permet aux assassins Torii/data-event de surveiller l'actualisation du manifeste UAID, de modifier et de décider de refuser des gains sans plomberie personnalisée.Pour les preuves opérationnelles de bout en bout, les notes de migration du SDK et les listes de contrôle de publication du manifeste, en particulier cette section du Guide de compte universel (`docs/source/universal_accounts_guide.md`). Mantenha ambos os documentos alinhados semper que a politica ou as ferramentas UAID mudarem.

Architecture de haut niveau
1) Camada de composicao global (Chaîne Nexus)
- Mantem une seule ordonnance canonique de blocs Nexus de 1 seconde pour finaliser les transactions atomiques en abrangendo un ou plus d'espaces de données (DS). Cada transacao s'est engagé à actualiser l'état mondial unifié (vétor de racines por DS).
- Contenir des métadonnées minimes mais plus prouvées/QCs agrégées pour garantir la composabilité, la finalité et la détection de fraude (DSIDs tocados, racines de l'état par DS avant/après, compromissos DA, preuves de validation par DS, et le certificat de quorum DS utilisé ML-DSA-87). Nenhum dado privado e incluido.
- Consensus : le comité BFT global dans le pipeline de tamanho 22 (3f+1 avec f=7), sélectionné dans un pool d'environ 200 000 validateurs potentiels pour un mécanisme de VRF/mise à l'époque. Le comité Nexus Sequencia transacoes et finalise le bloco em 1s.2) Camada de Data Space (Public/Privé)
- Exécuter des fragments par DS de transactions globales, actualiser WSV local par DS et produire des articles de validation par bloc (prouvés par DS agrégés et compromis DA) qui ne sont pas accumulés dans le bloc Nexus de 1 seconde.
- Private DS cryptografam dados em repouso e em transito entre validadores autorizados; apenas compromissos e provas de validade PQ saem do DS.
- Public DS exportam corps completos de données (via DA) et preuves de validation PQ.3) Transacoes atomiques cross-Data-Space (AMX)
- Modèle : chaque transacao de l'utilisateur peut tocar multiplos DS (ex., domaine DS et un ou plusieurs actifs DS). Il s'est engagé atomiquement dans un seul bloc Nexus ou avorté ; nao ha efeitos parciais.
- Prepare-Commit à l'intérieur des 1 : pour chaque transaction candidate, DS est exécuté en parallèle avec le même instantané (roots DS au début du slot) et produit une validation PQ par DS (FASTPQ-ISI) et des compromis DA. Le comité Nexus Commita a Transacao apenas se todas as provas DS exigidas verificarem e os certificados DA Chegarem a tempo (alvo <=300 ms) ; cas contraire, le transfert est reprogrammé pour le slot à proximité.
- Consistencia : conjuntos de leitura/escrita sao declarados ; la détection de conflits ne se produit pas contre les racines du début de la machine à sous. Exécution otimista sem locks por DS evita stalls globais; atomicidade et imposta pela regra de commit nexus (tout ou rien entre DS).
- Privacidade : privé DS exportam apenas provas/compromissos vinculados aos racines DS pré/post. Nenhum dado privado cru sai do DS.4) Disponibilité des données (DA) avec codage de l'hébergement
- Kura armazena corps de blocs et instantanés WSV comme blobs codifiés avec apagamento. Blobs publics sao shardados; Les blobs privés de Sao Armazenados sont des validateurs Private-DS, avec des morceaux cryptographiés.
- Compromissos DA sao registrados tanto em artefatos DS quanto em blocos Nexus, possibilitando amostragem e garantias de recuperacao sem revelar conteudo privado.

Structure de bloc et de validation
- Artefato de prova de Data Space (pour slot de 1s, pour DS)
  - Champs : dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - DS privé exporta artefatos sem corpos de dados ; public DS permet la récupération de corps via DA.

- Bloc Nexus (cadence de 1s)
  - Champs : block_number, parent_hash, slot_time, tx_list (transacoes atomiques cross-DS avec DSIDs tocados), ds_artifacts[], nexus_qc.
  - Funcao : finaliza todas as transacoes atomicas cujos artefatos DS requeridos verificam ; actualisez le vecteur de racines DS pour l'état mondial global à un moment donné.Consenso et planification
- Consenso de la chaîne Nexus : BFT global em pipeline (classe Sumeragi) avec un comité de 22 (3f+1 avec f=7) regroupant des blocs de 1 et une finalité de 1. Les membres du comité sao sélectionné par époque via VRF/enjeu entre ~ 200 000 candidats ; une rotation avec décentralisation et résistance à la censure.
- Consenso de Data Space : chaque DS exécute votre propre BFT entre les validateurs pour produire des articles par emplacement (propositions, compromis DA, DS QC). Les comités Lane-Relay ont été dimensionnés dans `3f+1` en utilisant `fault_tolerance` pour l'espace de données et sont reconnus comme étant de forme déterministe par époque à partir du pool de validateurs de l'espace de données en utilisant une graine VRF liée à `(dataspace_id, lane_id)`. Privé DS sao permissionados ; public DS permitem vivacité aberta sujeita a politicas anti-Sybil. Le comité mondial nexus permanece inalterado.
- Planification des transacoes : les utilisateurs submetem transacoes atomicas déclarent les DSID tocados et conjuntos de leitura/escrita. DS s'exécute en parallèle dans l'emplacement ; Le comité Nexus comprend une transaction sans bloc de 1 avec tous les articles DS vérifiés et les certificats DA forem pontuais (<= 300 ms).
- Isolation de l'emploi : chacun de ces pools de mémoire et exécutions indépendantes. Les quotas de DS limitant les quantités de transactions pouvant faire un DS peuvent être engagés pour bloquer pour éviter le blocage de tête de ligne et protéger la latence de DS privée.Modèle de données et espace de noms
- ID qualifiés pour DS : tous les entités (domaines, comptes, activités, rôles) sont qualifiés pour `dsid`. Exemple : `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Références mondiales : une référence globale et un tupla `(dsid, object_id, version_hint)` et peut être placé en chaîne sur la chaîne Nexus ou dans les descripteurs AMX pour une utilisation cross-DS.
- Serialização Norito : tous les messages cross-DS (descripteurs AMX, provas) utilisent les codecs Norito. Sem uso de serde em caminhos de producao.Contrats intelligents et étendus IVM
- Contexte d'exécution : ajouter `dsid` au contexte d'exécution IVM. Les contrats Kotodama sont toujours exécutés dans un espace de données spécifique.
- Primitivas atomicas cross-DS :
  - `amx_begin()` / `amx_commit()` délimite un transacao atomique multi-DS sans hôte IVM.
  - `amx_touch(dsid, key)` déclare l'intensité de la lecture/écriture pour détecter les conflits contre l'instantané des racines de l'emplacement.
  - `verify_space_proof(dsid, proof, statement)` -> booléen
  - `use_asset_handle(handle, op, amount)` -> résultat (l'opération permise apenas se a politica permitir e o handle for valido)
- Gestion des actifs et taxons :
  - Operacoes de activos sao autorisadas pelas politicas ISI/role do DS ; taxas sao pagas no token de gas do DS. Les jetons de capacité optionnels et politiques plus riches (multi-approbateur, limites de débit, géorepérage) peuvent être ajoutés après avoir changé le modèle atomique.
- Déterminisme : tous les nouveaux appels système sao puras et déterministes dadas as entradas e os conjuntos de leitura/escrita AMX déclarés. Sem efeitos ocultos de tempo ou ambiente.Provas de validation post-quantique (ISI generalizados)
- FASTPQ-ISI (PQ, configuration sans confiance) : un argument basé sur le hachage qui généralise la conception de transfert pour tous les ISI familiers, mais il sera ensuite testé en sous-seconde pour beaucoup à une échelle de 20 000 matériels dans la classe matérielle GPU.
  - Profil opérationnel :
    - Nos produits sont construits ou prouvés via `fastpq_prover::Prover::canonical`, qui restent toujours initialisés dans le backend de production ; Le déterminisme simulé a été supprimé. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) et `irohad --fastpq-execution-mode` permettent aux opérateurs fixes d'exécuter le CPU/GPU de manière déterminée en ce qui concerne le crochet d'observateur qui enregistre trois demandes/résolutions/backend pour les salles de froid. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Arimétisation :
  - KV-Update AIR : traite WSV comme une carte clé-valeur modifiée via Poseidon2-SMT. Chaque ISI est développé pour un petit ensemble de lignes de lecture-vérification-écriture sur les éléments (contenus, activités, rôles, domaines, métadonnées, fourniture).
  - Restreindre les portes de l'opcode : un seul tableau AIR avec des colonnes sélectionnées imposées par ISI (conservateur, contrôleurs monotones, autorisations, vérifications de plage, mises à jour des métadonnées limitées).- Arguments de recherche : tableaux transparents compromis par hachage pour les autorisations/rôles, précision des activités et paramètres politiques pour éviter les restrictions au niveau du bit.
- Compromis et actualisations de l'état :
  - Prova SMT agregada: todas as chaves tocadas (pre/post) sao provadas contra `old_root`/`new_root` en utilisant une frontière compressée avec des frères et sœurs dédupliqués.
  - Invariants : invariants globais (ex., supply total por ativo) sao impostas via igualdade de multiconjuntos entre linhas de efeito e contadores rastreados.
- Système de vérification :
  - Compromis polinomiens style FRI (DEEP-FRI) avec haute aridade (8/16) et explosion 8-16 ; hache Poséidon2 ; transcription Fiat-Shamir avec SHA-2/3.
  - Récursivité facultative : agrégation récursive locale DS pour compresser des micro-lots à une vérification par emplacement si nécessaire.
- Escopo e exemples cobertos:
  - Activités : transférer, créer, graver, enregistrer/désenregistrer les définitions d'actifs, définir la précision (limitée), définir les métadonnées.
  - Contas/Dominios : créer/supprimer, définir une clé/un seuil, ajouter/supprimer des signataires (apenas estado ; verificacoes de assinatura sao atestadas por validadores DS, nao provadas dentro do AIR).
  - Rôles/Autorisations (ISI) : accorder/révoquer des rôles et des autorisations ; taxes sur les tableaux de recherche et les chèques politiques monotones.- Contratos/AMX : les marcadores commencent/commettent AMX, la capacité est créée/révoquée se habilitado ; provados como transicoes de étatado et contadores de politica.
- Vérifie le forum AIR pour préserver la latence :
  - Assinaturas e criptografia pesada (ex., assinaturas ML-DSA de usuario) sao verificadas por validadores DS e atestadas no DS QC ; a prova de validade cobre apenas consistencia de étatado e conformidade de politica. Isso mantem provas PQ e rapidas.
- Métaux de travail (illustrations, CPU 32 cœurs + un GPU moderne) :
  - 20 000 secondes d'ISI avec une petite touche (<=8 touches/ISI) : ~0,4-0,9 s de vérification, ~150-450 Ko de vérification, ~5-15 ms de vérification.
  - ISI plus pesadas (mais chaves/contrastes ricos) : micro-lote (ex., 10x2k) + recursao para manter <1 s por slot.
- Configuration du manifeste DS :
  -`zk.policy = "fastpq_isi"`
  -`zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  -`state.commitment = "smt_poseidon2"`
  -`zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (assinaturas verificadas por DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (plutôt ; les alternatives doivent être déclarées explicitement)
- Solutions de repli :
  - Les ISI complexes/personnalisés peuvent utiliser un STARK général (`zk.policy = "stark_fri_general"`) avec preuve adiada et finalidade de 1 s via l'attestation QC + en coupant les preuves invalides.
  - Les opérations nao PQ (ex., Plonk avec KZG) exigent une configuration fiable et ne sont plus prises en charge sans build padrao.Introduction à AIR (par Nexus)
- Traco de execucao: matrice avec longueur (colonnes de registres) et comprimento (passos). Chaque ligne et une étape logique du processus ISI ; colonnes armazenam valeurs pré/post, sélection et drapeaux.
-Restrictions :
  - Restrictions de transfert : impoem relacoes de linha a linha (ex., post_balance = pre_balance - montant pour une ligne de débit quando `sel_transfer = 1`).
  - Limites de frontière : connexion d'E/S publique (old_root/new_root, contacts) comme premières/dernières lignes.
  - Recherches/permutations : garantie d'adhésion et igualdade de multiconjuntos contra tabelas compromis (autorisations, paramètres d'activité) sur des circuits pesados ​​de bits.
- Compromis et vérification :
  - O prouver des traces compromises via des codificacoes hash et construire des polinomios de bas grau que sao validos se as restricoes forem satisfeitas.
  - Survérificateur checa baixo grau via FRI (basé sur le hachage, post-quantique) avec poucas aberturas Merkle ; o custo e logaritmico nos passos.
- Exemple (Transfert) : les registres incluent le pre_balance, le montant, le post_balance, les nombres et les sélections. Les restrictions imposent des intervalles négatifs/négatifs, une conservation et une monotonie de nonce, en ce qui concerne une multiprova SMT agrégée de lignes de folhas avant/après les racines anciennes/nouvelles.Évolution d'ABI et des appels système (ABI v1)
- Syscalls un ajout (noms illustratifs) :
  -`SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Types de pointeur-ABI à ajouter :
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Mises à jour nécessaires :
  - Ajouter à `ivm::syscalls::abi_syscall_list()` (manter ordenacao), gate por politica.
  - Mapear numéros détectés pour `VMError::UnknownSyscall` nos hôtes.
  - Mettre à jour les tests : liste d'appels système en or, hachage ABI, identifiant de type de pointeur en or et tests de politique.
  - Documents : `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Modèle de confidentialité
- Contencao de dados privados: corpos de transacao, diffs de estado e snapshots WSV de private DS nunca deixam o sous-conjunto privado de validadores.
- Exposicao publica: apenas en-têtes, compromissos DA et provas de validade PQ sao exportados.
- Provas ZK opcionais : le DS privé peut produire un provas ZK (ex., saldo suficiente, politica satisfeita) habilitando acoes cross-DS sem revelar estado interno.
- Contrôle d'accès : autorisation et impôt pour les politiques ISI/rôle dans le DS. Les jetons de capacité sont optionnels et peuvent être introduits après.Isolation de l'emploi et QoS
- Consenso, mempools et armazenamento séparés por DS.
- Quotas de planification liés à DS pour limiter le temps d'inclusion des ancres et éviter le blocage de tête de ligne.
- Orcamentos de recursos de contrato por DS (compute/memory/IO), impostos pelo host IVM. Contencao em public DS ne peut pas consommer des orcamentos de private DS.
- Chamadas cross-DS assincronas evitam esperas sincronas longas dentro da execucao private-DS.

Disponibilité de données et conception d'armement
1) Codification de l'hébergement
- Utiliser le système Reed-Solomon (ex., GF(2^16)) pour codifier l'affichage au niveau du blob des blocs Kura et des instantanés WSV : paramètres `(k, m)` avec fragments `n = k + m`.
- Paramètres padrao (proposés, DS public) : `k=32, m=16` (n = 48), permettant de récupérer 16 fragments perdus avec ~ 1,5x d'expansion. Pour DS privé : `k=16, m=8` (n=24) dans le groupe autorisé. Les paramètres sont configurés par DS Manifest.
- Blobs publics : fragments distribués par plusieurs de nos DA/validadores avec contrôles de disponibilité par amostragem. Les compromis DA dans nos en-têtes permettent la vérification par les clients légers.
- Blobs privés : fragments cryptographiés et distribués uniquement entre les validateurs privés-DS (ou les gardiens désignés). Une cadeia global carrega apenas compromissos DA (sem localizacoes de shards ou chaves).2) Compromis et compromissions
- Pour chaque blob : calculez un Raiz Merkle sur les fragments et incluez-le dans `*_da_commitment`. Manter PQ évite les compromis sur la courbe elliptique.
- DA Attesters : les attestateurs régionaux agréés par VRF (ex., 64 par région) émettent un certificat ML-DSA-87 attestant l'attestation de la réussite des fragments. Méta de latence de test DA <=300 ms. Le comité Nexus validé est certifié à chaque fois que vous cherchez des fragments.

3) Intégration avec Kura
- Blocos armazenam corpos de transacao como blobs codificados com apagamento e compromissos Merkle.
- Les en-têtes font des compromis sur le blob ; corpos sao recuperaveis via rede DA para public DS et via canais privados para private DS.

4) Intégration avec WSV
- Snapshots WSV : point de contrôle périodique de l'état DS dans les instantanés fragmentés et codifiés avec l'expédition avec des compromis enregistrés dans les en-têtes. Entre les instantanés, les journaux de modifications du manteau. Instantanés publics sao amplement shardados; instantanés privés permanents à l'intérieur des validateurs privés.
- Accès à la preuve : les contrats peuvent être fournis (ou sollicités) par l'état (Merkle/Verkle) ancoradas por compromissos de snapshot. Le privé DS peut fornecer atestacoes zéro connaissance em vez de provas cruas.5) Retençao et taille
- Sem pruning para public DS : reter todos os corps Kura e snapshots WSV via DA (escalade horizontale). Private DS peut définir une rétention interne, mais des compromissos exportados permanents imutaveis. La connexion entre tous les blocs Nexus et les compromis des artéfacts DS est établie.

Rede e papeis de nos
- Validadores globais : participation au consensus nexus, validation des blocs Nexus et artefatos DS, réalisation des chèques DA pour public DS.
- Validateurs de Data Space : exécutez le consensus DS, exécutez les contrats, gerenciam Kura/WSV local, lidam com DA para votre DS.
- Nos DA (facultatif) : armazenam/publicam blobs publicos, facilitam amostragem. Para private DS, nos DA sao co-localizados com validadores ou custodios confiaveis.Melhorias et considérations du système
- Découpage du séquençage/mempool : ajouter un mempool DAG (ex., style Narwhal) en alimentant un BFT dans le pipeline dans une chaîne de connexion pour réduire la latence et améliorer le débit selon le modèle logique.
- Quotas DS et équité : quotas par DS pour blocage et plafonds de peso pour éviter le blocage de tête de ligne et garantir la latence préventive pour DS privés.
- Atestacao DS (PQ): certificados de quorum DS usam ML-DSA-87 (classe Dilithium5) par padrao. Le post-quantique est plus important que l'EC assinaturé, mais il s'agit d'un QC par slot. DS peut opter explicitement pour ML-DSA-65/44 (moins) ou assinaturas EC se déclarant dans le manifeste DS ; public DS sao fortement encorajados a manter ML-DSA-87.
- Attestateurs DA : para public DS, usar attesters regionais amostrados por VRF qui émettent des certificados DA. Le comité Nexus validé a été certifié en cas d'accident brut de fragments ; privé DS mantem atestacoes DA internas.
- Récursivité et vérification par époque : agréger facultativement plusieurs microlots à l'intérieur d'un DS dans une vérification récursive par emplacement/époque pour manter tamanho de preuve et le temps de vérification estaveis sur alta charge.
- Escalonamento de lanes (si nécessaire) : pour un seul comité global virar gargalo, introduisez K lanes de séquenciamento parallèles avec fusion déterministe. Il s'agit de préserver un ordre mondial unique en termes d'échelle horizontale.- Accélération déterministe : forner les noyaux SIMD/CUDA avec des indicateurs de fonctionnalité pour le hachage/FFT et le bit de CPU de secours pour préserver la détermination du matériel croisé.
- Limites d'activité des voies (proposées) : permettre 2 à 4 voies si (a) une finalité p95 dépasse 1,2 s pendant >3 minutes consécutives, ou (b) une occupation du bloc dépasse 85 % pendant >5 minutes, ou (c) une taxe d'entrée de route existante >1,2x la capacité de bloc dans les niveaux soutenus. Les voies fazem bucket de transacoes de forme déterministe par hash de DSID et se fazem fusionnent sans bloc nexus.

Taxes et économie (padroes iniciais)
- Unité de gaz : jeton de gaz pour DS avec moyen de calcul/IO ; taxas sao pagas no ativo de gas natif do DS. Conversation entre DS et responsabilité de l'application.
- Priorité d'inclusion : round-robin entre DS et quotas pour DS pour préserver l'équité et les SLO de 1 ; Dentro de um DS, les enchères payantes peuvent être acceptées.
- Futur : le marché mondial des taxes ou des politiques qui minimisent le MEV peut être exploré sans se soucier de l'atomicité ou de la conception du PQ.Fluxo cross-Data-Space (exemple)
1) Un utilisateur envoie une transacao AMX tocando public DS P e private DS S: mover o ativo X de S para o beneficiario B cuja conta esta em P.
2) Dans le slot, P et S exécutent votre fragment contre l'instantané du slot. S verifica autorizacao e disponibilidade, actualiza seu estado interno e produz uma prova de validade PQ e compromisso DA (nenhum dado privado vaza). P préparer l'actualisation de l'état correspondant (ex., mint/burn/locking em P conformité politica) et sa preuve.
3) Le comité nexus verifica ambas as provas DS e certificados DA ; se ambas verificarem dentro do slot, a transacao e commit atomiquement no blocko Nexus de 1s, atualizando ambosroots DS no vetor de world state global.
4) Si vous avez vérifié ou certifié un échec/invalide, la transaction a été interrompue (sem efeitos) et le client peut être renvoyé pour le créneau à proximité. Nenhum dado privado sai de S em nenhum passo.- Considérations de sécurité
- Exécution déterministe : appels système IVM déterministiques permanentes ; résultats cross-DS sao guiados por commit AMX et finalizacao, nao por relogio ou timing de rede.
- Contrôle d'accès : autorisations ISI em private DS restringem quem pode submeter transacoes e quais operacoes sao permitidas. Les jetons de capacité sont codifiés directement pour une utilisation cross-DS.
- Confidentialité : cryptographie de bout en bout pour les données Private-DS, fragments codifiés avec apagamento armazenados apenas entre membros autorizados, provas ZK opcionais para atestacoes externes.
- Résistance au DoS : l'isolement du pool de mémoire/le consentement/l'armement empêche la congestion publique d'impacter la progression du DS privé.

Mudancas nos composants Iroha
- iroha_data_model : introduction `DataSpaceId`, ID qualifiés pour DS, descripteurs AMX (conjuntos leitura/escrita), types de preuves/compromis DA. Serialização quelque chose Norito.
- ivm : ajouter des appels système et des types de pointeur-ABI pour AMX (`amx_begin`, `amx_commit`, `amx_touch`) et tester DA ; actualiser les tests/docs ABI conforme politica v1.