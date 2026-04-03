<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: security_best_practices_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6055090fbfab93113392610b3eb78ef0a0e97d6fef4a94292c5609ec5562e925
source_last_modified: "2026-03-26T06:12:11.487656+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Rapport sur les meilleures pratiques de sécurité

Dates : 2026-03-25

## Résumé

J'ai actualisé le précédent rapport Torii/Soracloud par rapport à l'espace de travail actuel.
code et étendu l'examen au serveur, au SDK et au
surfaces de crypto/sérialisation. Cet audit a initialement confirmé trois
Problèmes d’entrée/d’authentification : deux de gravité élevée et un de gravité moyenne.
Ces trois résultats sont désormais fermés dans l'arborescence actuelle par la remédiation
décrit ci-dessous. Suivi du transport et examen de l'expédition interne confirmés
neuf problèmes supplémentaires de gravité moyenne : une liaison d'identité P2P sortante
écart, un défaut de rétrogradation P2P TLS sortant, deux bogues de limite de confiance Torii dans
livraison de webhook et répartition interne MCP, un transport sensible inter-SDK
écart dans les clients Swift, Java/Android, Kotlin et JS, un SoraFS
Lacune de politique proxy de confiance/IP client, un proxy local SoraFS
écart de liaison/authentification, un repli en texte brut de recherche géographique de télémétrie homologue,
et un espace de verrouillage/taux de saisie de débit IP à distance avec authentification par opérateur. Ceux
les découvertes ultérieures sont également fermées dans l’arborescence actuelle.Les quatre conclusions précédemment rapportées par Soracloud sur les clés privées brutes
HTTP, exécution de proxy en lecture locale uniquement interne, durée d'exécution publique illimitée
la solution de secours et la location de pièce jointe IP distante ne sont plus actives dans le code actuel.
Ceux-ci sont marqués comme fermés/remplacés ci-dessous avec des références de code mises à jour.Il s’agissait d’un audit centré sur le code plutôt que d’un exercice exhaustif de l’équipe rouge.
J'ai donné la priorité aux chemins d'entrée et d'authentification de demande Torii accessibles de l'extérieur, puis
IVM, `iroha_crypto`, `norito` vérifiés ponctuellement, le SDK Swift/Android/JS
les assistants à la signature de requêtes, le chemin géographique de télémétrie homologue et le SoraFS
assistants proxy de poste de travail ainsi que la stratégie IP client de broche/passerelle SoraFS
surfaces. Aucun problème confirmé en direct pour cette politique d'entrée/d'authentification, de sortie,
Géométrie de télémétrie homologue, valeurs par défaut du transport P2P échantillonnées, répartition MCP, SDK échantillonné
transport, verrouillage d'authentification de l'opérateur/clé de débit, proxy de confiance/IP client SoraFS
la stratégie ou la tranche de proxy local reste après les correctifs présentés dans ce rapport.
Le renforcement du suivi a également élargi les ensembles de vérité sur les démarrages fermés en cas d'échec pour
chemins d'accélérateur IVM CUDA/Métal échantillonnés ; ce travail n'a pas confirmé un nouveau
problème d'échec d'ouverture. Le métal échantillonné Ed25519
le chemin de signature est maintenant restauré sur cet hôte après avoir corrigé plusieurs dérives ref10
points dans les ports Metal/CUDA : gestion des points de base positifs en vérification,
la constante `d2`, le chemin de réduction exact `fe_sq2`, la finale parasite
`fe_mul`, étape de transport et normalisation du champ postopératoire manquante qui permettait au membre
les limites dérivent à travers l’échelle scalaire. Couverture ciblée de la régression Metal maintenant
maintient le pipeline de signature activé et vérifie `[true, false]` sur leaccélérateur par rapport au chemin de référence du CPU. La vérité de démarrage échantillonnée s'établit maintenant
sonder également directement le vecteur vivant (`vadd64`, `vand`, `vxor`, `vor`) et
noyaux par lots AES à tour unique sur Metal et CUDA avant ces backends
reste activé. L'analyse ultérieure des dépendances a ajouté sept résultats de tiers en direct
au retard, mais l'arborescence actuelle a depuis supprimé les deux `tar` actifs
avis en supprimant la dépendance `xtask` Rust `tar` et en remplaçant le
`iroha_crypto` `libsodium-sys-stable` Tests d'interopérabilité avec support OpenSSL
équivalents. L'arborescence actuelle a également remplacé les dépendances directes PQ
signalé lors de ce balayage, migrant `soranet_pq`, `iroha_crypto` et `ivm` depuis
`pqcrypto-dilithium` / `pqcrypto-kyber` à
`pqcrypto-mldsa` / `pqcrypto-mlkem` tout en préservant le ML-DSA existant /
Surface de l'API ML-KEM. Un laissez-passer de dépendance ultérieur le même jour a ensuite épinglé l'espace de travail
`reqwest` / `rustls` aux versions de correctifs corrigés, qui conservent
`rustls-webpki` sur la ligne fixe `0.103.10` dans la résolution actuelle. Le seul
Les exceptions restantes à la politique de dépendance sont les deux transitives non maintenues
caisses de macros (`derivative`, `paste`), qui sont désormais explicitement acceptées dans
`deny.toml` car il n'existe pas de mise à niveau sécurisée et leur suppression nécessiterait
remplacer ou vendre plusieurs piles en amont. Lele travail restant sur l'accélérateur est
validation d'exécution du correctif CUDA en miroir et de l'ensemble de vérité CUDA étendu sur
un hôte avec prise en charge du pilote CUDA en direct, pas une exactitude confirmée ou une ouverture en cas d'échec
problème dans l’arborescence actuelle.

## Haute gravité

### SEC-05 : La vérification canonique des demandes d'application a contourné les seuils multisig (fermé le 24/03/2026)

Impact :

- Toute clé de membre unique d'un compte contrôlé par multisig peut autoriser
  requêtes orientées vers les applications qui sont censées nécessiter un seuil ou pondérées
  quorum.
- Cela affecte tous les points de terminaison qui font confiance à `verify_canonical_request`, y compris
  Entrée de mutation signée Soracloud, accès au contenu et compte signé ZK
  location de saisie.

Preuve :

- `verify_canonical_request` étend un contrôleur multisig au membre à part entière
  liste de clés publiques et accepte la première clé qui vérifie la demande
  signature, sans évaluer de seuil ni de poids cumulé :
  `crates/iroha_torii/src/app_auth.rs:198-210`.
- Le modèle de politique multisig actuel comporte à la fois un `threshold` et un pondéré
  membres et rejette les politiques dont le seuil dépasse le poids total :
  `crates/iroha_data_model/src/account/controller.rs:92-95`,
  `crates/iroha_data_model/src/account/controller.rs:163-178`,
  `crates/iroha_data_model/src/account/controller.rs:188-196`.
- L'assistant est sur le chemin d'autorisation pour l'entrée de la mutation Soracloud dans
  `crates/iroha_torii/src/lib.rs:2141-2157`, accès au compte signé au contenu dans
  `crates/iroha_torii/src/content.rs:359-360` et location de pièce jointe dans
  `crates/iroha_torii/src/lib.rs:7962-7968`.

Pourquoi c'est important :- Le signataire de la requête est traité comme l'autorité du compte pour l'admission HTTP,
  mais l'implémentation rétrograde silencieusement les comptes multisig vers « n'importe quel compte unique ».
  le membre peut agir seul.
- Cela transforme une couche de signature HTTP de défense en profondeur en autorisation
  contourner les comptes protégés par multisig.

Recommandation :

- Soit rejeter les comptes contrôlés par multisig au niveau de la couche app-auth jusqu'à ce qu'un
  le format de témoin approprié existe, ou étendez le protocole pour que la requête HTTP
  transporte et vérifie un ensemble complet de témoins multisig qui satisfait au seuil et
  poids.
- Ajouter des régressions couvrant le middleware de mutation Soracloud, l'authentification de contenu et ZK
  pièces jointes pour les signatures multisig inférieures au seuil.

Statut de correction :

- Fermé dans le code actuel en échouant la fermeture des comptes contrôlés par multisig dans
  `crates/iroha_torii/src/app_auth.rs`.
- Le vérificateur n'accepte plus la sémantique « n'importe quel membre peut signer » pour
  autorisation HTTP multisig ; les demandes multisig sont rejetées jusqu'à ce qu'un
  Il existe un format de témoin satisfaisant au seuil.
- La couverture de régression inclut désormais un cas de rejet multisig dédié dans
  `crates/iroha_torii/src/app_auth.rs`.

## Haute gravité

### SEC-06 : Les signatures de requêtes canoniques des applications étaient rejouables indéfiniment (Fermé le 24/03/2026)

Impact :- Une requête valide capturée peut être relue car le message signé n'a pas
  horodatage, nonce, expiration ou cache de relecture.
- Cela peut répéter les demandes de mutation Soracloud avec changement d'état et réémettre
  opérations de contenu/pièce jointe liées au compte longtemps après le client d'origine
  les destinait.

Preuve :

- Torii définit la requête canonique de l'application comme étant uniquement
  `METHOD + path + sorted query + body hash` dans
  `crates/iroha_torii/src/app_auth.rs:1-17` et
  `crates/iroha_torii/src/app_auth.rs:74-89`.
- Le vérificateur accepte uniquement `X-Iroha-Account` et `X-Iroha-Signature` et ne
  ne pas appliquer la fraîcheur ni maintenir un cache de relecture :
  `crates/iroha_torii/src/app_auth.rs:137-218`.
- Les assistants du SDK JS, Swift et Android génèrent le même en-tête sujet à la relecture
  associez-le sans champs nonce/horodatage :
  `javascript/iroha_js/src/canonicalRequest.js:50-82`,
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift:41-68`, et
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:67-106`.
- Le chemin de signature d'opérateur de Torii utilise déjà le modèle le plus fort
  le chemin d'accès à l'application est manquant : horodatage, nonce et cache de relecture dans
  `crates/iroha_torii/src/operator_signatures.rs:1-21` et
  `crates/iroha_torii/src/operator_signatures.rs:266-294`.

Pourquoi c'est important :

- HTTPS seul n'empêche pas la relecture par un proxy inverse, un enregistreur de débogage,
  hôte client compromis, ou tout intermédiaire capable d’enregistrer les demandes valides.
- Étant donné que le même schéma est implémenté dans tous les principaux SDK clients, la relecture
  la faiblesse est systémique plutôt que serveur uniquement.

Recommandation:- Ajouter du matériel de fraîcheur signé aux demandes d'authentification d'application, au minimum un horodatage
  et occasionnel, et rejette les tuples obsolètes ou réutilisés avec un cache de relecture limité.
- Versionner explicitement le format de demande canonique de l'application afin que Torii et les SDK puissent
  déprécier l'ancien système à deux en-têtes en toute sécurité.
- Ajouter des régressions prouvant le rejet de la relecture pour les mutations Soracloud, le contenu
  accès et pièce jointe CRUD.

Statut de correction :

- Fermé en code actuel. Torii nécessite désormais le schéma à quatre en-têtes
  (`X-Iroha-Account`, `X-Iroha-Signature`, `X-Iroha-Timestamp-Ms`,
  `X-Iroha-Nonce`) et signe/vérifie
  `METHOD + path + sorted query + body hash + timestamp + nonce` dans
  `crates/iroha_torii/src/app_auth.rs`.
- La validation de fraîcheur applique désormais une fenêtre de décalage d'horloge limitée, valide
  forme de nom occasionnel et rejette les noms occasionnels réutilisés avec un cache de relecture en mémoire dont
  les boutons font surface via `crates/iroha_config/src/parameters/{defaults,actual,user}.rs`.
- Les assistants JS, Swift et Android émettent désormais le même format à quatre en-têtes dans
  `javascript/iroha_js/src/canonicalRequest.js`,
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift`, et
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java`.
- La couverture de régression inclut désormais la vérification de signature positive et la relecture,
  cas d'horodatage périmé et de rejet de fraîcheur manquante dans
  `crates/iroha_torii/src/app_auth.rs`.

## Gravité moyenne

### SEC-07 : l'application de mTLS a fait confiance à un en-tête transféré pouvant être usurpé (fermé le 24/03/2026)

Impact :- Les déploiements qui reposent sur `require_mtls` peuvent être contournés si Torii est directement
  accessible ou le proxy frontal ne supprime pas les informations fournies par le client
  `x-forwarded-client-cert`.
- Le problème dépend de la configuration, mais lorsqu'il est déclenché, il devient un problème revendiqué.
  exigence de certificat client dans une simple vérification d'en-tête.

Preuve :

- Le déclenchement Norito-RPC applique `require_mtls` en appelant
  `norito_rpc_mtls_present`, qui vérifie uniquement si
  `x-forwarded-client-cert` existe et n'est pas vide :
  `crates/iroha_torii/src/lib.rs:1897-1926`.
- Les flux d'amorçage/de connexion d'authentification de l'opérateur appellent `check_common`, qui rejette uniquement
  lorsque `mtls_present(headers)` est faux :
  `crates/iroha_torii/src/operator_auth.rs:562-570`.
- `mtls_present` n'est également qu'un enregistrement `x-forwarded-client-cert` non vide
  `crates/iroha_torii/src/operator_auth.rs:1212-1216`.
- Ces gestionnaires d'authentification d'opérateur sont toujours exposés en tant que routes à
  `crates/iroha_torii/src/lib.rs:16658-16672`.

Pourquoi c'est important :

- Une convention d'en-tête transféré n'est digne de confiance que lorsque Torii se trouve derrière un
  proxy renforcé qui supprime et réécrit l'en-tête. Le code ne vérifie pas
  cette hypothèse de déploiement elle-même.
- Les contrôles de sécurité qui dépendent silencieusement de l'hygiène du proxy inverse sont faciles à mettre en œuvre.
  une mauvaise configuration lors des modifications de routage de préparation, de canari ou de réponse à un incident.

Recommandation:- Préférer l'application directe par l'État du transport lorsque cela est possible. Si un mandataire doit être
  utilisé, faites confiance à un canal proxy authentifié vers Torii et exigez une liste verte
  ou une attestation signée de ce proxy au lieu de la présence d'en-tête brut.
- Documentez que `require_mtls` n'est pas sûr sur les écouteurs Torii directement exposés.
- Ajout de tests négatifs pour l'entrée `x-forwarded-client-cert` falsifiée sur Norito-RPC
  et les routes d'amorçage d'authentification de l'opérateur.

Statut de correction :

- Fermé dans le code actuel en liant la confiance de l'en-tête transféré au proxy configuré
  CIDR au lieu de la seule présence d’en-tête brut.
- `crates/iroha_torii/src/limits.rs` fournit désormais le partage
  Porte `has_trusted_forwarded_header(...)` et les deux Norito-RPC
  (`crates/iroha_torii/src/lib.rs`) et authentification de l'opérateur
  (`crates/iroha_torii/src/operator_auth.rs`) utilisez-le avec le homologue TCP appelant
  adresse.
- `iroha_config` expose désormais `mtls_trusted_proxy_cidrs` pour les deux
  authentification de l'opérateur et Norito-RPC ; les valeurs par défaut sont uniquement en boucle.
- La couverture de régression rejette désormais l'entrée falsifiée `x-forwarded-client-cert` de
  une télécommande non fiable dans l'authentification de l'opérateur et dans l'assistant de limites partagées.

## Gravité moyenne

### SEC-08 : Les appels P2P sortants n'ont pas lié la clé authentifiée à l'identifiant d'homologue prévu (fermé le 25/03/2026)

Impact :- Une numérotation sortante vers le homologue `X` pourrait se terminer comme n'importe quel autre homologue `Y` dont la clé
  a signé avec succès la poignée de main au niveau de l'application, car la poignée de main
  authentifié "une clé sur cette connexion" mais n'a jamais vérifié que la clé était
  l'identifiant du pair que l'acteur du réseau avait l'intention d'atteindre.
- Dans les superpositions autorisées, les vérifications ultérieures de topologie/liste autorisée suppriment toujours le
  mauvaise clé, il s'agissait donc principalement d'un bug de substitution / d'accessibilité
  qu’un bug d’usurpation d’identité par consensus direct. Dans les superpositions publiques, cela pourrait laisser un
  L'adresse compromise, la réponse DNS ou le point de terminaison de relais remplacent un autre
  identité de l'observateur sur un cadran sortant.

Preuve :

- L'état du homologue sortant stocke le `peer_id` prévu dans
  `crates/iroha_p2p/src/peer.rs:5153-5179`, mais l'ancien flux de poignée de main
  a supprimé cette valeur avant la vérification de la signature.
- `GetKey::read_their_public_key` a vérifié la charge utile de la prise de contact signée et
  puis a immédiatement construit un `Peer` à partir de la clé publique distante annoncée
  dans `crates/iroha_p2p/src/peer.rs:6266-6355`, sans comparaison avec le
  `peer_id` initialement fourni à `connecting(...)`.
- La même pile de transport désactive explicitement le certificat TLS/QUIC
  vérification pour P2P dans `crates/iroha_p2p/src/transport.rs`, liant ainsi le
  La clé authentifiée par la couche d'application pour l'identifiant d'homologue prévu est la clé critique
  contrôle d'identité sur les connexions sortantes.

Pourquoi c'est important :- La conception pousse intentionnellement l'authentification par les pairs au-dessus du transport
  couche, ce qui fait que la clé de poignée de main vérifie la seule liaison d'identité durable
  sur les cadrans sortants.
- Sans cette vérification, la couche réseau pourrait traiter silencieusement "avec succès"
  authentifié un homologue » comme équivalent à « atteint l'homologue que nous avons appelé »,
  ce qui est une garantie plus faible et peut fausser l’état de la topologie/réputation.

Recommandation :

- Effectuer le `peer_id` sortant prévu à travers les étapes de négociation signée et
  échec de fermeture si la clé distante vérifiée ne correspond pas.
- Conservez une régression ciblée qui prouve une poignée de main valablement signée de la part du
  une mauvaise clé est rejetée alors qu'une poignée de main signée normale réussit toujours.

Statut de correction :

- Fermé en code actuel. `ConnectedTo` et les états de prise de contact en aval maintenant
  transporter le `PeerId` sortant attendu, et
  `GetKey::read_their_public_key` rejette une clé authentifiée incompatible avec
  `HandshakePeerMismatch` dans `crates/iroha_p2p/src/peer.rs`.
- La couverture de régression ciblée inclut désormais
  `outgoing_handshake_rejects_unexpected_peer_identity` et l'existant
  chemin positif `handshake_v1_defaults_to_trust_gossip` dans
  `crates/iroha_p2p/src/peer.rs`.

### SEC-09 : la livraison des webhooks HTTPS/WSS a résolu à nouveau les noms d'hôtes vérifiés au moment de la connexion (fermé le 25/03/2026)

Impact :- Livraison sécurisée du webhook, réponses DNS de destination validées par rapport au webhook
  politique de sortie, mais a ensuite rejeté ces adresses vérifiées et laissé le client
  La pile résout à nouveau le nom d'hôte lors de la connexion HTTPS ou WSS réelle.
- Un attaquant capable d'influencer le DNS entre la validation et le temps de connexion pourrait
  potentiellement relier un nom d'hôte précédemment autorisé à un nom d'hôte privé ou bloqué.
  destination réservée à l’opérateur et contourner la protection des webhooks basée sur CIDR.

Preuve :

- Le garde de sortie résout et filtre les adresses de destination candidates dans
  `crates/iroha_torii/src/webhook.rs:1746-1829`, et les chemins de livraison sécurisés
  transmettez ces listes d'adresses vérifiées aux assistants HTTPS / WSS.
- L'ancien assistant HTTPS a ensuite construit un client générique par rapport à l'URL d'origine
  hôte dans `crates/iroha_torii/src/webhook.rs` et n'a pas lié la connexion
  à l'ensemble d'adresses vérifié, ce qui signifiait que la résolution DNS se produisait à nouveau à l'intérieur
  le client HTTP.
- L'ancien assistant WSS également appelé `tokio_tungstenite::connect_async(url)`
  par rapport au nom d'hôte d'origine, ce qui a également résolu à nouveau l'hôte au lieu de
  en réutilisant l'adresse déjà approuvée.

Pourquoi c'est important :

- Les listes autorisées de destination ne fonctionnent que si l'adresse qui a été vérifiée est celle-là.
  auquel le client se connecte réellement.
- La nouvelle résolution après l'approbation de la politique crée un écart de liaison DNS/TOCTOU sur un
  chemin auquel les opérateurs sont susceptibles de faire confiance pour un confinement de style SSRF.

Recommandation:- Épinglez les réponses DNS vérifiées dans le chemin de connexion HTTPS réel tout en préservant
  le nom d'hôte d'origine pour la validation SNI/certificat.
- Pour WSS, connectez le socket TCP directement à une adresse vérifiée et exécutez le TLS
  poignée de main Websocket sur ce flux au lieu d'appeler un nom d'hôte basé sur
  connecteur pratique.

Statut de correction :

- Fermé en code actuel. `crates/iroha_torii/src/webhook.rs` dérive désormais
  `https_delivery_dns_override(...)` et
  `websocket_pinned_connect_addr(...)` de l’ensemble d’adresses vérifiées.
- La livraison HTTPS utilise désormais `reqwest::Client::builder().resolve_to_addrs(...)`
  Ainsi, le nom d'hôte d'origine reste visible par TLS pendant que la connexion TCP est
  épinglé aux adresses déjà approuvées.
- La livraison WSS ouvre désormais un `TcpStream` brut à une adresse vérifiée et effectue
  `tokio_tungstenite::client_async_tls_with_config(...)` sur ce flux,
  ce qui évite une deuxième recherche DNS après la validation de la politique.
- La couverture de régression inclut désormais
  `https_delivery_dns_override_pins_vetted_domain_addresses`,
  `https_delivery_dns_override_skips_ip_literals`, et
  `websocket_pinned_connect_addr_pins_secure_delivery_when_guarded` dans
  `crates/iroha_torii/src/webhook.rs`.

### SEC-10 : répartition de route interne MCP bouclage estampillé et privilège de liste blanche hérité (fermé le 25/03/2026)

Impact :- Lorsque Torii MCP a été activé, la répartition de l'outil interne a réécrit chaque demande comme
  bouclage quel que soit l’appelant réel. Routes qui font confiance au CIDR de l'appelant pour
  le privilège ou le contournement de la limitation pourraient donc considérer le trafic MCP comme
  `127.0.0.1`.
- Le problème était lié à la configuration car MCP est désactivé par défaut et le
  les routes affectées dépendent toujours d'une liste verte ou d'une confiance de bouclage similaire
  politique, mais cela a transformé MCP en un pont d'élévation des privilèges une fois que ces
  les boutons ont été activés ensemble.

Preuve :

- `dispatch_route(...)` dans `crates/iroha_torii/src/mcp.rs` précédemment inséré
  `x-iroha-remote-addr: 127.0.0.1` et bouclage synthétique `ConnectInfo` pour
  chaque demande envoyée en interne.
- `iroha.parameters.get` est exposé sur la surface MCP en mode lecture seule, et
  `/v1/parameters` contourne l'authentification normale lorsque l'adresse IP de l'appelant appartient au
  liste verte configurée dans `crates/iroha_torii/src/lib.rs:5879-5888`.
- `apply_extra_headers(...)` a également accepté les entrées arbitraires `headers` de
  l'appelant MCP, donc des en-têtes de confiance internes réservés tels que
  `x-iroha-remote-addr` et `x-forwarded-client-cert` n'étaient pas explicitement
  protégé.

Pourquoi c'est important :- Les couches de pont internes doivent préserver la limite de confiance d'origine. Remplacement
  le véritable appelant avec bouclage traite efficacement chaque appelant MCP comme un
  client interne une fois que la demande traverse le pont.
- Le bug est subtil car le profil MCP visible de l'extérieur peut toujours paraître
  en lecture seule tandis que la route HTTP interne voit une origine plus privilégiée.

Recommandation :

- Préserver l'adresse IP de l'appelant que la requête externe `/v1/mcp` a déjà reçue
  à partir du middleware d'adresse distante de Torii et synthétiser `ConnectInfo` à partir de
  cette valeur au lieu du bouclage.
- Traitez les en-têtes de confiance d'entrée uniquement tels que `x-iroha-remote-addr` et
  `x-forwarded-client-cert` en tant qu'en-têtes internes réservés afin que les appelants MCP ne puissent pas
  faites-les passer clandestinement ou remplacez-les via l'argument `headers`.

Statut de correction :

- Fermé en code actuel. `crates/iroha_torii/src/mcp.rs` dérive désormais le
  IP distante distribuée en interne à partir de la requête externe injectée
  `x-iroha-remote-addr` et synthétise `ConnectInfo` à partir de ce réel
  IP de l'appelant au lieu du bouclage.
- `apply_extra_headers(...)` supprime désormais à la fois `x-iroha-remote-addr` et
  `x-forwarded-client-cert` comme en-têtes internes réservés, donc les appelants MCP
  ne peut pas usurper la confiance du bouclage/proxy d'entrée via les arguments de l'outil.
- La couverture de régression inclut désormais
  `dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks`
  `dispatch_route_blocks_remote_addr_spoofing_from_extra_headers`, et
  `apply_extra_headers_blocks_reserved_internal_headers` dans
  `crates/iroha_torii/src/mcp.rs`.### SEC-11 : les clients SDK ont autorisé les requêtes sensibles sur des transports non sécurisés ou entre hôtes (fermé le 25/03/2026)

Impact :

- Les clients Swift, Java/Android, Kotlin et JS échantillonnés ne l'ont pas fait
  traiter systématiquement toutes les formes de demandes sensibles comme étant sensibles au transport.
  En fonction de l'assistant, les appelants peuvent envoyer des en-têtes de porteur/jeton API, bruts
  Champs JSON `private_key*` ou matériel de signature d'authentification canonique de l'application sur
  simple `http` / `ws` ou via des remplacements d'URL absolus entre hôtes.
- Dans le client JS spécifiquement, les en-têtes `canonicalAuth` ont été ajoutés après
  `_request(...)` a terminé ses contrôles de transport, et `private_key` carrosserie seule
  JSON n’était pas du tout considéré comme un transport sensible.

Preuve:- Swift centralise désormais la garde
  `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift` et l'applique à partir de
  `IrohaSwift/Sources/IrohaSwift/NoritoRpcClient.swift`,
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`, et
  `IrohaSwift/Sources/IrohaSwift/ConnectClient.swift` ; avant ce passage
  ces assistants ne partageaient pas une seule porte d’entrée en matière de politique des transports.
- Java/Android centralise désormais la même politique dans
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java`
  et l'applique à partir de `NoritoRpcClient.java`, `ToriiRequestBuilder.java`,
  `OfflineToriiClient.java`, `SubscriptionToriiClient.java`,
  `stream/ToriiEventStreamClient.java`, et
  `websocket/ToriiWebSocketClient.java`.
- Kotlin reflète désormais cette politique dans
  `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt`
  et l'applique à partir du client/request-builder/event-stream / JVM correspondant
  Surfaces des sockets Web.
- JS `ToriiClient._request(...)` traite désormais les corps `canonicalAuth` plus JSON
  contenant des champs `private_key*` comme matériel de transport sensible dans
  `javascript/iroha_js/src/toriiClient.js` et la forme de l'événement de télémétrie dans
  `javascript/iroha_js/index.d.ts` enregistre désormais `hasSensitiveBody` /
  `hasCanonicalAuth` lorsque `allowInsecure` est utilisé.

Pourquoi c'est important :

- Les assistants de développement mobile, de navigateur et locaux pointent souvent vers un développement/staging mutable
  URL de base. Si le client n'épingle pas les requêtes sensibles sur le serveur configuré
  schéma/hôte, un remplacement d'URL absolu ou une base HTTP simple peut
  transformez le SDK en un chemin d'exfiltration de secret ou de demande signée.
- Le risque est plus large que les jetons au porteur. `private_key` JSON brut et frais
  les signatures d'authentification canonique sont également sensibles à la sécurité sur le fil et
  ne devrait pas contourner silencieusement la politique des transports.

Recommandation:- Centraliser la validation du transport dans chaque SDK et l'appliquer avant les E/S réseau
  pour toutes les formes de requêtes sensibles : en-têtes d'authentification, signature d'authentification canonique d'application,
  et JSON `private_key*` brut.
- Conserver `allowInsecure` comme trappe d'évacuation locale/dev explicite uniquement et émettre
  télémétrie lorsque les appelants l'acceptent.
- Ajoutez des régressions ciblées sur les générateurs de requêtes partagées plutôt que uniquement sur
  méthodes pratiques de niveau supérieur, de sorte que les futurs assistants héritent de la même garde.

Statut de correction :- Fermé en code actuel. Les échantillons Swift, Java/Android, Kotlin et JS
  les clients rejettent désormais les transports non sécurisés ou entre hôtes pour les données sensibles
  demander les formes ci-dessus, sauf si l'appelant opte pour le développement uniquement documenté
  mode non sécurisé.
- Les régressions ciblées Swift couvrent désormais les en-têtes d'authentification Norito-RPC non sécurisés,
  transport Websocket Connect non sécurisé et demande raw-`private_key` Torii
  corps.
- Les régressions ciblées sur Kotlin couvrent désormais les en-têtes d'authentification Norito-RPC non sécurisés,
  corps `private_key` hors ligne/abonnement, en-têtes d'authentification SSE et websocket
  en-têtes d'authentification.
- Les régressions centrées sur Java/Android couvrent désormais l'authentification Norito-RPC non sécurisée
  en-têtes, corps `private_key` hors ligne/abonnement, en-têtes d'authentification SSE et
  en-têtes d'authentification Websocket via le faisceau Gradle partagé.
- Les régressions axées sur JS couvrent désormais les environnements non sécurisés et multi-hôtes
  Requêtes `private_key`-body plus requêtes `canonicalAuth` non sécurisées dans
  `javascript/iroha_js/test/transportSecurity.test.js`, tandis que
  `javascript/iroha_js/test/toriiCanonicalAuth.test.js` exécute désormais le positif
  chemin d'authentification canonique sur une URL de base sécurisée.

### SEC-12 : Le proxy QUIC local SoraFS accepte les liaisons sans bouclage sans authentification du client (Fermé le 25/03/2026)

Impact :- `LocalQuicProxyConfig.bind_addr` pouvait auparavant être défini sur `0.0.0.0`, un
  IP LAN, ou toute autre adresse sans bouclage, qui expose le "local"
  proxy de poste de travail en tant qu'écouteur QUIC accessible à distance.
- Cet auditeur n'a pas authentifié les clients. Tout homologue accessible qui pourrait
  terminer la session QUIC/TLS et envoyer une poignée de main correspondant à la version pourrait
  puis ouvrez les flux `tcp`, `norito`, `car` ou `kaigi` selon lesquels
  les modes pont ont été configurés.
- En mode `bridge`, cela transformait une mauvaise configuration de l'opérateur en un TCP distant
  surface de relais et de streaming de fichiers locaux sur le poste opérateur.

Preuve :

- `LocalQuicProxyConfig::parsed_bind_addr(...)` dans
  `crates/sorafs_orchestrator/src/proxy.rs` analysait auparavant uniquement le socket
  adresse et n’a pas rejeté les interfaces sans bouclage.
- `spawn_local_quic_proxy(...)` dans le même fichier démarre un serveur QUIC avec un
  certificat auto-signé et `.with_no_client_auth()`.
- `handle_connection(...)` accepté tout client dont `ProxyHandshakeV1`
  la version correspondait à la version unique du protocole pris en charge, puis a entré le
  boucle de flux d’application.
- `handle_tcp_stream(...)` compose des valeurs `authority` arbitraires via
  `TcpStream::connect(...)`, tandis que `handle_norito_stream(...)`,
  `handle_car_stream(...)` et `handle_kaigi_stream(...)` diffusent des fichiers locaux
  à partir des répertoires spool/cache configurés.

Pourquoi c'est important :- Un certificat auto-signé protège l'identité du serveur uniquement si le client
  choisit de le vérifier. Il n'authentifie pas le client. Une fois le proxy
  était accessible hors bouclage, le chemin de la prise de contact se résumait à la version uniquement
  admission.
- L'API et la documentation décrivent cet assistant comme un proxy de poste de travail local pour
  intégrations navigateur/SDK, donc autoriser les adresses de liaison accessibles à distance était
  une incompatibilité de limite de confiance, et non un mode de service à distance prévu.

Recommandation :

- Échec fermé sur tout `bind_addr` sans bouclage afin que l'assistant actuel ne puisse pas être
  exposés au-delà du poste de travail local.
- Si l'exposition aux proxys à distance devient une exigence du produit, introduisez
  authentification explicite du client/admission des capacités en premier au lieu de
  relâcher la protection de liaison.

Statut de correction :

- Fermé en code actuel. `crates/sorafs_orchestrator/src/proxy.rs` maintenant
  rejette les adresses de liaison sans bouclage avec `ProxyError::BindAddressNotLoopback`
  avant le démarrage de l'écouteur QUIC.
- La documentation du champ de configuration dans
  `docs/source/sorafs/developer/orchestrator.md` et
  `docs/portal/docs/sorafs/orchestrator-config.md` documente désormais
  `bind_addr` en bouclage uniquement.
- La couverture de régression inclut désormais
  `spawn_local_quic_proxy_rejects_non_loopback_bind_addr` et l'existant
  test de pont local positif
  `proxy::tests::tcp_stream_bridge_transfers_payload` dans
  `crates/sorafs_orchestrator/src/proxy.rs`.

### SEC-13 : TLS-over-TCP P2P sortant rétrogradé silencieusement en texte brut par défaut (fermé le 25/03/2026)

Impact :- L'activation de `network.tls_enabled=true` n'appliquait pas réellement TLS uniquement
  transport sortant à moins que les opérateurs n'aient également découvert et défini
  `tls_fallback_to_plain=false`.
- Tout échec de négociation TLS ou délai d'attente sur le chemin sortant donc
  a rétrogradé le cadran en TCP en clair par défaut, ce qui a supprimé le transport
  confidentialité et intégrité contre les attaquants sur le chemin ou les comportements inappropriés
  boîtiers de médiation.
- La poignée de main d'application signée authentifie toujours l'identité du homologue, donc
  il s’agissait d’un déclassement de la politique des transports plutôt que d’un contournement de l’usurpation d’identité par les pairs.

Preuve :

- `tls_fallback_to_plain` est par défaut `true` dans
  `crates/iroha_config/src/parameters/user.rs`, donc le repli était actif
  à moins que les opérateurs ne l'aient explicitement remplacé dans config.
- `Connecting::connect_tcp(...)` dans `crates/iroha_p2p/src/peer.rs` tente un
  Composez le numéro TLS chaque fois que `tls_enabled` est défini, mais en cas d'erreur ou de délai d'attente TLS.
  enregistre un avertissement et revient au TCP en clair à chaque fois
  `tls_fallback_to_plain` est activé.
- L'exemple de configuration destiné à l'opérateur dans `crates/iroha_kagami/src/wizard.rs` et
  les documents de transport public P2P dans `docs/source/p2p*.md` ont également été annoncés
  secours en texte brut comme comportement par défaut.

Pourquoi c'est important :- Une fois que les opérateurs activent TLS, l'attente la plus sûre est la fermeture en cas d'échec : si le TLS
  la session ne peut pas être établie, le cadran devrait échouer plutôt que silencieusement
  perte des protections de transport.
- Laisser le downgrade activé par défaut rend le déploiement sensible à
  bizarreries de chemin réseau, interférences de proxy et perturbation de la prise de contact active dans un
  manière facile à manquer lors du déploiement.

Recommandation :

- Conserver le texte en clair comme bouton de compatibilité explicite, mais par défaut
  `false` donc `network.tls_enabled=true` signifie TLS uniquement, sauf si l'opérateur choisit
  dans un comportement de déclassement.

Statut de correction :

- Fermé en code actuel. `crates/iroha_config/src/parameters/user.rs` maintenant
  la valeur par défaut de `tls_fallback_to_plain` est `false`.
- L'instantané de l'appareil de configuration par défaut, l'exemple de configuration Kagami et celui par défaut
  Les assistants de test P2P/Torii reflètent désormais cette valeur par défaut d'exécution renforcée.
- Les documents `docs/source/p2p*.md` dupliqués décrivent désormais la solution de secours en texte brut comme
  un opt-in explicite au lieu de la valeur par défaut fournie.

### SEC-14 : La recherche géographique de télémétrie par les pairs est revenue silencieusement au HTTP tiers en texte brut (Fermé le 25/03/2026)

Impact :- Activation de `torii.peer_geo.enabled=true` sans point de terminaison explicite provoqué
  Torii pour envoyer les noms d'hôtes homologues à un texte en clair intégré
  Service `http://ip-api.com/json/...`.
- Cela a divulgué des cibles de télémétrie homologue sur un HTTP tiers non authentifié
  dépendance et laissez tout attaquant sur le chemin ou tout point de terminaison compromis se falsifier
  métadonnées de localisation dans Torii.
- La fonctionnalité était facultative, mais les documents de télémétrie publics et les exemples de configuration
  a annoncé la valeur par défaut intégrée, ce qui a rendu le modèle de déploiement non sécurisé
  probablement une fois que les opérateurs ont activé les recherches géographiques entre pairs.

Preuve :

- `crates/iroha_torii/src/telemetry/peers/monitor.rs` défini précédemment
  `DEFAULT_GEO_ENDPOINT = "http://ip-api.com/json"` et
  `construct_geo_query(...)` a utilisé cette valeur par défaut à chaque fois
  `GeoLookupConfig.endpoint` était `None`.
- Le moniteur de télémétrie homologue génère `collect_geo(...)` à chaque fois
  `geo_config.enabled` est vrai dans
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`, donc le texte en clair
  la solution de secours était accessible dans le code d'exécution fourni plutôt que dans le code de test uniquement.
- La configuration par défaut est `crates/iroha_config/src/parameters/defaults.rs` et
  `crates/iroha_config/src/parameters/user.rs` laisse `endpoint` désactivé et le
  documents de télémétrie dupliqués dans `docs/source/telemetry*.md` plus
  `docs/source/references/peer.template.toml` a explicitement documenté le
  solution de repli intégrée lorsque les opérateurs ont activé la fonctionnalité.

Pourquoi c'est important :- La télémétrie homologue ne doit pas faire sortir silencieusement les noms d'hôtes homologues via HTTP en texte brut
  à un service tiers une fois que les opérateurs ont activé un drapeau de commodité.
- Un défaut caché et non sécurisé compromet également la révision des modifications : les opérateurs peuvent
  activer la recherche géographique sans se rendre compte qu'ils ont introduit des
  divulgation de métadonnées tierces et gestion des réponses non authentifiées.

Recommandation :

- Supprimez la valeur par défaut du point de terminaison géographique intégré.
- Exiger un point de terminaison HTTPS explicite lorsque les recherches géographiques homologues sont activées et
  sinon, ignorez les recherches.
- Gardez les régressions ciblées prouvant que les points de terminaison manquants ou non HTTPS échouent.

Statut de correction :

- Fermé en code actuel. `crates/iroha_torii/src/telemetry/peers/monitor.rs`
  rejette désormais les points de terminaison manquants avec `MissingEndpoint`, rejette les non-HTTPS
  points de terminaison avec `InsecureEndpoint` et ignore la recherche géographique homologue au lieu de
  revenir silencieusement à un service intégré en texte clair.
- `crates/iroha_config/src/parameters/user.rs` n'injecte plus de code implicite
  point de terminaison au moment de l'analyse, de sorte que l'état de configuration non défini reste explicite tout le temps.
  moyen de validation d’exécution.
- Les documents de télémétrie dupliqués et l'exemple de configuration canonique dans
  `docs/source/references/peer.template.toml` indique maintenant que
  `torii.peer_geo.endpoint` doit être explicitement configuré avec HTTPS lorsque le
  la fonctionnalité est activée.
- La couverture de régression inclut désormais
  `construct_geo_query_requires_explicit_endpoint`,
  `construct_geo_query_rejects_non_https_endpoint`,
  `collect_geo_requires_explicit_endpoint_when_enabled`, et
  `collect_geo_rejects_non_https_endpoint` dans
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`.### SEC-15 : La stratégie de broche et de passerelle SoraFS ne disposait pas d'une résolution IP client compatible avec les proxys de confiance (fermé le 25/03/2026)

Impact :

- Les déploiements Torii avec proxy inverse pouvaient auparavant effondrer des SoraFS distincts.
  les appelants sur l'adresse du socket proxy lors de l'évaluation du CIDR de la broche de stockage
  listes d'autorisation, limitations de broches de stockage par client et client de passerelle
  empreintes digitales.
- Cela a affaibli les contrôles d'abus sur `/v1/sorafs/storage/pin` et SoraFS
  les surfaces de téléchargement de passerelle dans les topologies de proxy inverse courantes en créant
  plusieurs clients partagent un compartiment ou une identité de liste verte.
- Le routeur par défaut injecte toujours des métadonnées distantes internes, ce n'était donc pas un problème.
  nouveau contournement d'entrée non authentifié, mais il s'agissait d'un véritable écart de limite de confiance
  pour les déploiements compatibles proxy et une confiance excessive dans les limites du gestionnaire
  en-tête IP distant interne.

Preuve:- `crates/iroha_config/src/parameters/user.rs` et
  `crates/iroha_config/src/parameters/actual.rs` n'avait auparavant aucun
  Bouton `torii.transport.trusted_proxy_cidrs`, donc client canonique compatible proxy
  La résolution IP n’était pas configurable à la limite d’entrée générale Torii.
-`inject_remote_addr_header(...)` dans `crates/iroha_torii/src/lib.rs`
  précédemment écrasé l'en-tête interne `x-iroha-remote-addr` de
  `ConnectInfo` seul, qui a supprimé les métadonnées IP client transférées de confiance de
  de vrais proxys inverses.
-`PinSubmissionPolicy::enforce(...)` dans
  `crates/iroha_torii/src/sorafs/pin.rs` et
  `gateway_client_fingerprint(...)` dans `crates/iroha_torii/src/sorafs/api.rs`
  n'a pas partagé une étape de résolution IP canonique compatible avec un proxy de confiance au niveau
  limite du gestionnaire.
- Limitation des broches de stockage dans `crates/iroha_torii/src/sorafs/pin.rs` également saisie
  sur le jeton du porteur uniquement chaque fois qu'un jeton était présent, ce qui signifiait plusieurs
  les clients mandatés partageant un jeton PIN valide ont été contraints au même tarif
  bucket même après la distinction des adresses IP de leurs clients.

Pourquoi c'est important :

- Les proxys inverses constituent un modèle de déploiement normal pour Torii. Si le temps d'exécution
  ne peut pas systématiquement distinguer un proxy de confiance d'un appelant non fiable, IP
  les listes autorisées et les limitations par client ne signifient plus ce que les opérateurs pensent qu'ils
  méchant.
- Les chemins de broches et de passerelle SoraFS sont des surfaces explicitement sensibles aux abus, donc
  regroupement des appelants dans l'adresse IP du proxy ou confiance excessive dans les transferts obsolètes
  les métadonnées sont significatives sur le plan opérationnel même lorsque l'itinéraire de base est toujours
  nécessite une autre admission.

Recommandation:- Ajoutez une surface de configuration générale Torii `trusted_proxy_cidrs` et résolvez le
  IP client canonique une fois à partir de `ConnectInfo` plus tout transfert préexistant
  en-tête uniquement lorsque le homologue du socket est dans cette liste verte.
- Réutilisez cette résolution IP canonique dans les chemins du gestionnaire SoraFS au lieu de
  faire aveuglément confiance à l'en-tête interne.
- Étendez les limitations des broches de stockage des jetons partagés par jeton plus l'adresse IP canonique du client
  quand les deux sont présents.

Statut de correction :

- Fermé en code actuel. `crates/iroha_config/src/parameters/defaults.rs`,
  `crates/iroha_config/src/parameters/user.rs`, et
  `crates/iroha_config/src/parameters/actual.rs` expose maintenant
  `torii.transport.trusted_proxy_cidrs`, en le définissant par défaut sur une liste vide.
- `crates/iroha_torii/src/lib.rs` résout désormais l'adresse IP canonique du client avec
  `limits::ingress_remote_ip(...)` à l'intérieur du middleware d'entrée et réécritures
  l'en-tête interne `x-iroha-remote-addr` uniquement à partir de proxys de confiance.
- `crates/iroha_torii/src/sorafs/pin.rs` et
  `crates/iroha_torii/src/sorafs/api.rs` résout désormais les adresses IP canoniques des clients
  par rapport à `state.trusted_proxy_nets` à la limite du gestionnaire pour la broche de stockage
  empreintes digitales de la politique et du client de passerelle, de sorte que les chemins directs du gestionnaire ne peuvent pas
  faire trop confiance aux métadonnées IP transférées obsolètes.
- La limitation des broches de stockage clé désormais les jetons de porteur partagés par `token + canonique
  IP du client lorsque les deux sont présents, préservant les compartiments par client pour les partages
  jetons d’épingle.
- La couverture de régression inclut désormais
  `limits::tests::ingress_remote_ip_preserves_trusted_forwarded_header`,
  `limits::tests::ingress_remote_ip_ignores_forwarded_header_from_untrusted_peer`,
  `sorafs::pin::tests::rate_key_scopes_shared_tokens_by_ip`,
  `storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`,
  `car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`, et le
  appareil de configuration
  `torii_transport_trusted_proxy_cidrs_default_to_empty`.### SEC-16 : Le verrouillage de l'authentification de l'opérateur et la saisie de la limite de débit sont revenus à un compartiment anonyme partagé lorsque l'en-tête IP distant injecté était manquant (Fermé le 25/03/2026)

Impact :

- L'admission d'authentification de l'opérateur a déjà reçu l'adresse IP du socket acceptée, mais le
  La clé de verrouillage/limite de débit l'a ignoré et a réduit les requêtes dans un fichier partagé.
  Compartiment `"anon"` chaque fois que l'en-tête interne `x-iroha-remote-addr` était
  absent.
- Il ne s'agissait pas d'un nouveau contournement d'entrée publique sur le routeur par défaut, car
  Le middleware d'entrée réécrit l'en-tête interne avant l'exécution de ces gestionnaires.
  Il s’agissait encore d’un véritable écart de confiance qui ne s’ouvrait pas pour des raisons internes plus étroites.
  chemins de gestionnaire, tests directs et tout itinéraire futur qui atteint
  `OperatorAuth` avant le middleware d'injection.
- Dans ces cas, un appelant peut consommer le budget limité ou bloquer un autre
  appelants qui auraient dû être isolés par l’adresse IP source.

Preuve:- `OperatorAuth::check_common(...)` dans
  `crates/iroha_torii/src/operator_auth.rs` déjà reçu
  `remote_ip: Option<IpAddr>`, mais auparavant appelé `auth_key(headers)`
  et a complètement abandonné l'adresse IP de transport.
- `auth_key(...)` dans `crates/iroha_torii/src/operator_auth.rs` précédemment
  analysé uniquement `limits::REMOTE_ADDR_HEADER` et renvoyé autrement `"anon"`.
- L'assistant général Torii dans `crates/iroha_torii/src/limits.rs` avait déjà
  la bonne règle de résolution après échec dans `effective_remote_ip(headers,
  distant)`, préférant l’en-tête canonique injecté mais retombant sur le
  IP de socket acceptée lorsque les appels directs du gestionnaire contournent le middleware.

Pourquoi c'est important :

- L'état de verrouillage et de limitation de débit doit utiliser la même identité d'appelant effective
  que le reste de Torii utilise pour les décisions politiques. Revenir à un partage
  Un bucket anonyme transforme un saut de métadonnées interne manquant en un saut multi-client
  interférence au lieu de localiser l’effet sur le véritable appelant.
- L'authentification de l'opérateur est une limite sensible aux abus, donc même une gravité moyenne
  le problème de collision de seau mérite d'être résolu explicitement.

Recommandation :

- Dérivez la clé d'authentification de l'opérateur à partir de `limits::effective_remote_ip(headers,
  remote_ip)` donc l'en-tête injecté gagne toujours lorsqu'il est présent, mais direct
  les appels du gestionnaire reviennent à l’adresse de transport au lieu de `"anon"`.
- Conservez `"anon"` uniquement comme solution de secours finale lorsque l'en-tête interne et
  les IP de transport ne sont pas disponibles.

Statut de correction :- Fermé en code actuel. `crates/iroha_torii/src/operator_auth.rs` appelle maintenant
  `auth_key(headers, remote_ip)` de `check_common(...)` et `auth_key(...)`
  dérive désormais la clé de verrouillage/limite de débit de
  `limits::effective_remote_ip(headers, remote_ip)`.
- La couverture de régression inclut désormais
  `operator_auth_key_uses_remote_ip_when_internal_header_missing` et
  `operator_auth_key_prefers_injected_header_over_transport_remote_ip` dans
  `crates/iroha_torii/src/operator_auth.rs`.

## Constatations clôturées ou remplacées du rapport précédent

- Conclusion antérieure de Soracloud sur la clé privée brute : fermée. Entrée de mutation actuelle
  rejette les champs `authority` / `private_key` en ligne dans
  `crates/iroha_torii/src/soracloud.rs:5305-5308`, lie le signataire HTTP au
  provenance de la mutation dans `crates/iroha_torii/src/soracloud.rs:5310-5315`, et
  renvoie un brouillon d'instructions de transaction au lieu de soumettre au serveur un document signé
  transaction dans `crates/iroha_torii/src/soracloud.rs:5556-5565`.
- Résultat antérieur d'exécution de proxy en lecture locale interne uniquement : fermé. Publique
  la résolution de route ignore désormais les gestionnaires de mise à jour non publics et de mise à jour/privés dans
  `crates/iroha_torii/src/soracloud.rs:8445-8463` et le runtime rejette
  itinéraires de lecture locale non publics dans
  `crates/irohad/src/soracloud_runtime.rs:5906-5923`.
- Résultat de secours non mesuré antérieur et d'exécution publique : clôturé comme écrit. Publique
  L'entrée d'exécution applique désormais des limites de débit et des plafonds en vol dans
  `crates/iroha_torii/src/lib.rs:8837-8852` avant de résoudre une voie publique dans
  `crates/iroha_torii/src/lib.rs:8858-8860`.
- Constatation antérieure de location de pièce jointe IP distante : fermée. Location de pièce jointe maintenant
  nécessite un compte signé vérifié dans
  `crates/iroha_torii/src/lib.rs:7962-7968`.
  Location de pièces jointes précédemment héritée de SEC-05 et SEC-06 ; cet héritage
  est fermé par les corrections actuelles d'authentification d'application ci-dessus.## Résultats de dépendance- `cargo deny check advisories bans sources --hide-inclusion-graph` fonctionne désormais
  directement contre le `deny.toml` suivi et signale désormais trois live
  résultats de dépendance à partir d’un fichier de verrouillage d’espace de travail généré.
- Les avis `tar` ne sont plus présents dans le graphe de dépendances actives :
  `xtask/src/mochi.rs` utilise désormais `Command::new("tar")` avec un argument fixe
  vecteur, et `iroha_crypto` n'extrait plus `libsodium-sys-stable` pour
  Tests d'interopérabilité Ed25519 après avoir échangé ces vérifications vers OpenSSL.
- Constatations actuelles :
  - `RUSTSEC-2024-0388` : `derivative` n'est pas maintenu.
  - `RUSTSEC-2024-0436` : `paste` n'est pas maintenu.
- Tri des impacts :
  - Les avis `tar` précédemment signalés sont fermés pour l'actif
    graphique de dépendance. `cargo tree -p xtask -e normal -i tar`,
    `cargo tree -p iroha_crypto -e all -i tar`, et
    `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` échouent maintenant tous
    avec "la spécification de l'ID du package... ne correspond à aucun package", et
    `cargo deny` ne signale plus `RUSTSEC-2026-0067` ou
    `RUSTSEC-2026-0068`.
  - Les avis de remplacement direct du PQ précédemment signalés sont désormais clôturés.
    l'arbre actuel. `crates/soranet_pq/Cargo.toml`,
    `crates/iroha_crypto/Cargo.toml` et `crates/ivm/Cargo.toml` dépendent désormais
    sur `pqcrypto-mldsa` / `pqcrypto-mlkem`, et les ML-DSA / ML-KEM touchés
    les tests d'exécution réussissent toujours après la migration.
  - L'avis `rustls-webpki` signalé précédemment n'est plus actif dans
    la résolution actuelle. L'espace de travail épingle désormais `reqwest` / `rustls` auversions de correctifs corrigés qui conservent `rustls-webpki` sur `0.103.10`, qui est
    en dehors de la plage consultative.
  - `derivative` et `paste` ne sont pas utilisés directement dans la source de l'espace de travail.
    Ils entrent transitivement via la pile BLS/arkworks sous
    `w3f-bls` et plusieurs autres caisses en amont, leur suppression nécessite donc
    modifications en amont ou de la pile de dépendances plutôt qu’un nettoyage de macro local.
    L'arborescence actuelle accepte désormais explicitement ces deux avis dans
    `deny.toml` avec motifs enregistrés.

## Notes de couverture- Serveur/runtime/config/réseau : SEC-05, SEC-06, SEC-07, SEC-08, SEC-09,
  SEC-10, SEC-12, SEC-13, SEC-14, SEC-15 et SEC-16 ont été confirmés au cours
  l'audit et sont maintenant fermés dans l'arborescence actuelle. Durcissement supplémentaire dans
  l'arborescence actuelle fait désormais également échouer l'admission du websocket/session Connect
  fermé lorsque l'en-tête IP distant injecté interne est manquant, au lieu de
  définir cette condition par défaut sur le bouclage.
- IVM/crypto/sérialisation : aucun résultat supplémentaire confirmé de cet audit
  tranche. Les preuves positives incluent la remise à zéro des éléments clés confidentiels dans
  `crates/iroha_crypto/src/confidential.rs:53-60` et Soranet PoW compatible avec la relecture
  validation du ticket signé dans `crates/iroha_crypto/src/soranet/pow.rs:823-879`.
  Le durcissement ultérieur rejette désormais également la sortie de l'accélérateur mal formée en deux
  chemins Norito échantillonnés : `crates/norito/src/lib.rs` valide le JSON accéléré
  Les bandes de l'étape 1 avant `TapeWalker` déréférencent les décalages et nécessitent désormais également
  Assistants Metal/CUDA Stage-1 chargés dynamiquement pour prouver la parité avec le
  générateur d'index structurel scalaire avant l'activation, et
  `crates/norito/src/core/gpu_zstd.rs` valide les longueurs de sortie signalées par le GPU
  avant de tronquer les tampons d'encodage/décodage. `crates/norito/src/core/simd_crc64.rs`
  désormais également auto-teste les assistants GPU CRC64 chargés dynamiquement par rapport au
  solution de secours canonique avant que `hardware_crc64` ne leur fasse confiance, donc mal formée
  les bibliothèques d'assistance échouent à la fermeture au lieu de modifier silencieusement la somme de contrôle Noritocomportement. Les résultats d'assistance invalides retombent désormais au lieu d'une libération panique
  builds ou dérive de la parité de la somme de contrôle. Côté IVM, accélérateur échantillonné
  les portes de démarrage couvrent désormais également le CUDA Ed25519 `signature_kernel`, CUDA BN254
  noyaux add/sub/mul, CUDA `sha256_leaves` / `sha256_pairs_reduce`, le live
  Noyaux de lots vectoriels/AES CUDA (`vadd64`, `vand`, `vxor`, `vor`,
  `aesenc_batch`, `aesdec_batch`), et le métal correspondant
  Les noyaux batch `sha256_leaves`/vector/AES avant que ces chemins ne soient approuvés. Le
  Le chemin de signature échantillonné Metal Ed25519 est maintenant également de retour
  à l'intérieur de l'accélérateur en direct défini sur cet hôte : l'échec de parité précédent était
  corrigé en restaurant la normalisation liée aux membres ref10 à travers l'échelle scalaire,
  et la régression Metal ciblée vérifie désormais `[s]B`, `[h](-A)`, le
  échelle de points de base puissance de deux et vérification complète des lots `[true, false]`
  sur Metal par rapport au chemin de référence du CPU. La source CUDA en miroir change
  compiler sous `--features cuda --tests` et la vérité de démarrage CUDA est maintenant définie
  échoue à la fermeture si les noyaux feuille/paire Merkle en direct dérivent du processeur
  chemin de référence. La validation d'exécution CUDA reste limitée à l'hôte dans ce cas
  environnement.
- SDK/exemples : SEC-11 a été confirmé lors de l'échantillonnage axé sur le transport
  transmettre les clients Swift, Java/Android, Kotlin et JS, et celaLa recherche est maintenant fermée dans l'arborescence actuelle. Le JS, Swift et Android
  les assistants de requête canonique ont également été mis à jour vers le nouveau
  système à quatre en-têtes soucieux de la fraîcheur.
  L’examen échantillonné du transport en continu QUIC n’a pas non plus produit de diffusion en direct.
  recherche d'exécution dans l'arborescence actuelle : `StreamingClient::connect(...)`,
  `StreamingServer::bind(...)`, et les assistants de négociation de capacités sont
  actuellement exercé uniquement à partir du code de test dans `crates/iroha_p2p` et
  `crates/iroha_core`, donc le vérificateur permissif auto-signé dans cet assistant
  path est actuellement réservé aux tests/auxiliaires plutôt qu'à une surface d'entrée livrée.
- Les exemples et les exemples d'applications mobiles ont été examinés uniquement à un niveau de vérification ponctuelle et
  ne doit pas être considéré comme audité de manière exhaustive.

## Lacunes en matière de validation et de couverture- `cargo deny check advisories bans sources --hide-inclusion-graph` fonctionne désormais
  directement avec le `deny.toml` suivi. Sous cette exécution du schéma actuel,
  `bans` et `sources` sont propres, tandis que `advisories` échoue avec les cinq
  résultats de dépendance énumérés ci-dessus.
- Validation du nettoyage du graphe de dépendances pour les résultats fermés `tar` réussie :
  `cargo tree -p xtask -e normal -i tar`,
  `cargo tree -p iroha_crypto -e all -i tar`, et
  `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` maintenant tous les rapports
  "La spécification de l'ID du package... ne correspond à aucun package", tandis que
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p xtask create_archive_packages_bundle_directory -- --nocapture`,
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo check -p xtask`,
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_verify -- --nocapture`,
  et
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_sign -- --nocapture`
  tout est passé.
- `bash scripts/fuzz_smoke.sh` exécute désormais de vraies sessions libFuzzer via
  `cargo +nightly fuzz`, mais la moitié IVM du script ne s'est pas terminée dans les délais
  ce passe car la première version nocturne pour `tlv_validate` était toujours en cours
  progrès au moment du transfert. Depuis, cette version est suffisamment terminée pour exécuter le
  généré directement le binaire libFuzzer :
  `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
  atteint maintenant la boucle d'exécution de libFuzzer et se termine proprement après 200 exécutions à partir d'un
  corpus vide. Le Norito s'est terminé à moitié avec succès après avoir corrigé le
  dérive harnais/manifeste et compilation fuzz-target `json_from_json_equiv`
  pause.
- La validation de remédiation Torii inclut désormais :
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo check -p iroha_torii --lib`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib --no-run`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_accepts_valid_signature -- --nocapture`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_rejects_replayed_nonce -- --nocapture`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_key_ -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib trusted_forwarded_header_requires_proxy_membership -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo check -p iroha_torii --lib --features app_api_https`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo test -p iroha_torii --lib https_delivery_dns_override_ --features app_api_https -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook2 cargo test -p iroha_torii --lib websocket_pinned_connect_addr_pins_secure_delivery_when_guarded -- --nocapture`-`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_blocks_remote_addr_spoofing_from_extra_headers -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib apply_extra_headers_blocks_reserved_internal_headers -- --nocapture`
  - le plus étroit `--no-default-features --features app_api,app_api_https` Torii
    la matrice de test a toujours des échecs de compilation existants non liés dans DA /
    Code de test lib-gated par Soracloud, donc cette passe a validé le
    chemin MCP de la fonctionnalité par défaut et le chemin du webhook `app_api_https` plutôt que
    revendiquant une couverture complète des fonctionnalités minimales.
- La validation de correction Trusted-proxy/SoraFS inclut désormais :
  -`CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib limits::tests:: -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib sorafs::pin::tests:: -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_requires_token_and_respects_allowlist_and_rate_limit -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limits_repeated_clients -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_config --test fixtures torii_transport_trusted_proxy_cidrs_default_to_empty -- --nocapture`
- La validation des remédiations P2P inclut désormais :
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib --no-run`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib outgoing_handshake_rejects_unexpected_peer_identity -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib handshake_v1_defaults_to_trust_gossip -- --nocapture`
- La validation de correction par proxy local SoraFS inclut désormais :
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-proxy cargo test -p sorafs_orchestrator spawn_local_quic_proxy_rejects_non_loopback_bind_addr -- --nocapture`
  -`/tmp/iroha-codex-target-proxy/debug/deps/sorafs_orchestrator-b3be10a343598c7b --exact proxy::tests::tcp_stream_bridge_transfers_payload --nocapture`
- La validation de correction par défaut P2P TLS inclut désormais :
  -`CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-config-tls2 cargo test -p iroha_config tls_fallback_defaults_to_tls_only -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib start_rejects_tls_without_feature_when_tls_only_outbound -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib tls_only_dial_requires_p2p_tls_feature_when_no_fallback -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-connect-gating cargo test -p iroha_torii --test connect_gating --no-run`
- La validation des corrections côté SDK inclut désormais :
  -`node --test javascript/iroha_js/test/canonicalRequest.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  -`cd IrohaSwift && swift test --filter CanonicalRequestTests`
  -`cd IrohaSwift && swift test --filter 'NoritoRpcClientTests/testCallRejectsInsecureAuthorizationHeader'`
  -`cd IrohaSwift && swift test --filter 'ConnectClientTests/testBuildsConnectWebSocketRequestRejectsInsecureTransport'`
  -`cd IrohaSwift && swift test --filter 'ToriiClientTests/testCreateSubscriptionPlanRejectsInsecureTransportForPrivateKeyBody'`
  -`cd kotlin && ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.client.TransportSecurityClientTest --console=plain`
  -`cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew android:compileDebugUnitTestJavaWithJavac --console=plain`
  -`cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.NoritoRpcClientTests,org.hyperledger.iroha.android.client.OfflineToriiClientTests,org.hyperledger.iroha.android.client.SubscriptionToriiClientTests,org.hyperledger.iroha.android.client.stream.ToriiEventStreamClientTests,org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClientTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain`
  -`node --test javascript/iroha_js/test/transportSecurity.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  -`node --test javascript/iroha_js/test/toriiSubscriptions.test.js`
- La validation de suivi Norito comprend désormais :
  -`python3 scripts/check_norito_bindings_sync.py`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_out_of_bounds_offsets -- --nocapture`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_non_structural_offsets -- --nocapture`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_encode_rejects_invalid_success_length -- --nocapture`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_decode_rejects_invalid_success_length -- --nocapture`
  - `bash scripts/fuzz_smoke.sh` (Norito cible `json_parse_string`,
    `json_parse_string_ref`, `json_skip_value` et `json_from_json_equiv`réussi après les corrections de harnais/cible)
- La validation de suivi du fuzz IVM comprend désormais :
  -`cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
- La validation de suivi de l'accélérateur IVM comprend désormais :
  -`xcrun -sdk macosx metal -c crates/ivm/src/metal_ed25519.metal -o /tmp/metal_ed25519.air`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo check -p ivm --features cuda --tests`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda-check cargo check -p ivm --features cuda --tests`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_bitwise_single_vector_matches_scalar -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_aes_batch_matches_scalar -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_ed25519_batch_matches_cpu -- --nocapture`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_sha256_leaves_matches_cpu -- --nocapture`
- L'exécution ciblée du test de bibliothèque CUDA reste limitée par l'environnement sur cet hôte :
  `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo test -p ivm --features cuda --lib selftest_covers_ -- --nocapture`
  La liaison ne parvient toujours pas car les symboles du pilote CUDA (`cu*`) ne sont pas disponibles.
- La validation du runtime Focused Metal s'exécute désormais entièrement sur l'accélérateur à ce sujet
  hôte : le pipeline de signature Ed25519 échantillonné reste activé jusqu'au démarrage
  autotests et `metal_ed25519_batch_matches_cpu` vérifie `[true, false]`
  directement sur Metal par rapport au chemin de référence du CPU.
- Je n'ai pas réexécuté un balayage de test Rust complet de l'espace de travail, `npm test` complet ou le
  suites Swift/Android complètes au cours de cette passe de remédiation.

## Carnet de remédiation hiérarchisé

### Tranche suivante- Surveiller les remplacements en amont pour le transitif explicitement accepté
  Macro-dette `derivative` / `paste` et supprimer les exceptions `deny.toml` lorsque
  des mises à niveau sûres deviennent disponibles sans déstabiliser le BLS / Halo2 / PQ /
  Piles de dépendances de l’interface utilisateur.
- Réexécutez le script nocturne complet IVM fuzz-smoke sur un cache chaud afin
  `tlv_validate` / `kotodama_lower` ont des résultats enregistrés stables à côté du
  Cibles Norito désormais vertes. Une exécution binaire directe `tlv_validate` se termine désormais,
  mais la fumée nocturne entièrement scénarisée est toujours exceptionnelle.
- Réexécutez la tranche d'auto-test CUDA lib-test ciblée sur un hôte avec le pilote CUDA
  bibliothèques installées, de sorte que l'ensemble de vérité de démarrage CUDA étendu soit validé
  au-delà de `cargo check` et du correctif de normalisation Ed25519 en miroir ainsi que du
  de nouvelles sondes de démarrage vectorielles/AES sont exercées au moment de l'exécution.
- Réexécuter des suites JS/Swift/Android/Kotlin plus larges une fois le niveau de la suite non lié
  les bloqueurs sur cette branche sont effacés, donc la nouvelle requête canonique et
  les agents de sécurité des transports sont couverts au-delà des tests d'aide ciblés ci-dessus.
- Décider si l'histoire multisig d'authentification d'application à long terme doit rester
  fermé en cas d'échec ou développez un format de témoin HTTP multisig de première classe.

### Moniteur- Poursuivre l'examen ciblé de l'accélération matérielle/des chemins dangereux `ivm`
  et les limites restantes du streaming/crypto `norito`. L'étape JSON-1
  et les transferts d'assistance GPU zstd ont maintenant été renforcés pour échouer.
  versions de version, et les ensembles de vérité de démarrage de l'accélérateur IVM échantillonnés sont maintenant
  plus large, mais l’examen plus large du risque et du déterminisme est toujours ouvert.