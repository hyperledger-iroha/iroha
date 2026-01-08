---
lang: fr
direction: ltr
source: docs/genesis.md
status: complete
translator: manual
source_hash: 975c403019da0b8700489610d86a75f26886f40f2b4c10963ee8269a68b4fe9b
source_last_modified: "2025-11-12T00:33:55.324259+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/genesis.md (Genesis configuration) -->

# Configuration de Genesis

Un fichier `genesis.json` définit les premières transactions exécutées au
démarrage d’un réseau Iroha. Le fichier est un objet JSON avec les champs
suivants :

- `chain` : identifiant unique de la chaîne.
- `executor` (optionnel) : chemin vers le bytecode de l’exécuteur (`.to`).
  S’il est présent, Genesis inclut une instruction `Upgrade` comme première
  transaction. S’il est omis, aucune mise à jour n’est effectuée et l’exécuteur
  intégré est utilisé.
- `ivm_dir` : répertoire contenant les bibliothèques de bytecode IVM. Par
  défaut `"."` si omis.
- `consensus_mode` : mode de consensus annoncé dans le manifest. Requis ; utilisez `"Npos"` pour le dataspace public de Sora Nexus et `"Permissioned"` ou `"Npos"` pour les autres dataspaces Iroha3. Iroha2 utilise `"Permissioned"` par défaut.
- `transactions` : liste de transactions de genesis exécutées séquentiellement.
  Chaque entrée peut contenir :
  - `parameters` : paramètres initiaux du réseau.
  - `instructions` : instructions encodées avec Norito.
  - `ivm_triggers` : triggers avec exécutables bytecode IVM.
  - `topology` : topologie initiale des pairs. Chaque entrée utilise `peer`
    (PeerId sous forme de chaîne, c’est‑à‑dire la clé publique) et `pop_hex` ;
    `pop_hex` peut être omis lors de la composition, mais doit être présent avant la signature.
- `crypto` : instantané de configuration cryptographique reflétant
  `iroha_config.crypto` (`default_hash`, `allowed_signing`,
  `allowed_curve_ids`, `sm2_distid_default`, `sm_openssl_preview`).
  `allowed_curve_ids` reflète `crypto.curves.allowed_curve_ids` afin que les
  manifests puissent annoncer quelles courbes de contrôleur le cluster
  accepte. Les outils imposent les combinaisons SM valides : les manifests
  qui indiquent `sm2` doivent également basculer le hash sur `sm3-256`,
  tandis que les builds compilées sans le feature `sm` rejettent
  systématiquement `sm2`.

Exemple (sortie de `kagami genesis generate default --consensus-mode npos`, instructions tronquées) :

```json
{
  "chain": "00000000-0000-0000-0000-000000000000",
  "ivm_dir": "defaults",
  "transactions": [
    {
      "parameters": { "sumeragi": { "block_time_ms": 2000 } },
      "instructions": [78, 82, 84, 48, 0, 0, 19, 123, ...],
      "ivm_triggers": [],
      "topology": []
    }
  ],
  "consensus_mode": "Npos",
  "crypto": {
    "default_hash": "blake2b-256",
    "allowed_signing": ["ed25519"],
    "allowed_curve_ids": [1],
    "sm2_distid_default": "1234567812345678",
    "sm_openssl_preview": false
  }
}
```

### Amorcer le bloc `crypto` pour SM2/SM3

Utilisez le helper `xtask` pour produire l’inventaire de clés et un snippet
de configuration prêt à coller en une seule étape :

```bash
cargo xtask sm-operator-snippet \
  --distid CN12345678901234 \
  --json-out sm2-key.json \
  --snippet-out client-sm2.toml
```

Le fichier `client-sm2.toml` contient alors :

```toml
# Account key material
public_key = "sm2:8626530010..."
private_key = "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569"
# public_key_pem = """\
-----BEGIN PUBLIC KEY-----
...
-----END PUBLIC KEY-----
"""
# private_key_pem = """\
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
"""

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]  # supprimer "sm2" pour rester en mode vérification uniquement
allowed_curve_ids = [1]               # ajouter de nouveaux IDs de courbe (ex. 15 pour SM2) lorsque les contrôleurs sont autorisés
sm2_distid_default = "CN12345678901234"
# enable_sm_openssl_preview = true  # optionnel : uniquement lors du déploiement du chemin OpenSSL/Tongsuo
```

Copiez les valeurs `public_key`/`private_key` dans la configuration
compte/client et mettez à jour le bloc `crypto` de `genesis.json` pour
qu’il corresponde au snippet (par exemple, définir `default_hash` sur
`sm3-256`, ajouter `"sm2"` à `allowed_signing` et inclure les
`allowed_curve_ids` appropriés). Kagami refusera les manifests où les
paramètres de hash/courbes et la liste des algorithmes autorisés sont
incohérents.

> **Astuce :** diffusez le snippet sur stdout avec `--snippet-out -` lorsque
> vous souhaitez simplement inspecter la sortie. Utilisez `--json-out -` pour
> émettre également l’inventaire de clés sur stdout.

Si vous préférez utiliser directement les commandes CLI de bas niveau, le
flux équivalent est :

```bash
# 1. Produire du matériel de clé déterministe (écrit un JSON sur disque)
cargo run -p iroha_cli --features sm -- \
  crypto sm2 keygen \
  --distid CN12345678901234 \
  --output sm2-key.json

# 2. Régénérer le snippet collable dans les fichiers de config/clients
cargo run -p iroha_cli --features sm -- \
  crypto sm2 export \
  --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
  --distid CN12345678901234 \
  --snippet-output client-sm2.toml \
  --emit-json --quiet
```

> **Astuce :** `jq` est utilisé ci‑dessus pour éviter un copier/coller
> manuel. S’il n’est pas disponible, ouvrez `sm2-key.json`, copiez le champ
> `private_key_hex` et passez‑le directement à `crypto sm2 export`.

> **Guide de migration :** pour migrer un réseau existant vers SM2/SM3/SM4,
> suivez
> [`docs/source/crypto/sm_config_migration.md`](source/crypto/sm_config_migration.md)
> pour les overrides `iroha_config`, la régénération des manifests et la
> planification du rollback.

## Générer et valider

1. Générer un template :

   ```bash
   cargo run -p iroha_kagami -- genesis generate \
     [--executor <path/to/executor.to>] \
     --consensus-mode npos \
     [--genesis-public-key <public-key>] \
     > genesis.json
   ```

   - `--executor` (optionnel) pointe vers un fichier `.to` d’exécuteur IVM ;
     s’il est présent, Kagami émet une instruction `Upgrade` comme première
     transaction.
   - `--genesis-public-key` définit la clé publique utilisée pour signer le
     bloc de genesis ; elle doit être un multihash reconnu par
     `iroha_crypto::Algorithm` (y compris les variantes GOST TC26 lorsque le
     build inclut le feature correspondant).
   - Sur le dataspace public de Sora Nexus, `--consensus-mode npos` est obligatoire et les basculements planifiés ne sont pas pris en charge ; les autres dataspaces Iroha3 peuvent utiliser `permissioned` ou `npos`. Sur Iroha2, le mode par défaut est `permissioned`.
   - Lorsque `npos` est sélectionné, Kagami sème la charge utile `sumeragi_npos_parameters` qui pilote la temporisation du pacemaker NPoS, le fan-out des collecteurs, la politique d’élection et les fenêtres de reconfiguration ; la normalisation/signature la transforme en instructions `SetParameter` dans le bloc signé.

2. Valider pendant l’édition :

   ```bash
   cargo run -p iroha_kagami -- genesis validate genesis.json
   ```

   Cette commande vérifie que `genesis.json` respecte le schema, que les
   paramètres sont valides, que les `Name` sont corrects et que les
   instructions Norito peuvent être décodées.

3. Signer pour le déploiement :

   ```bash
   cargo run -p iroha_kagami -- genesis sign \
     genesis.json \
     --public-key <PK> \
     --private-key <SK> \
     --out-file genesis.signed.nrt
   ```

   Lorsque `irohad` est démarré avec uniquement `--genesis-manifest-json`
   (sans bloc de genesis signé), le nœud dérive automatiquement sa
   configuration crypto runtime à partir du manifest ; si un bloc de genesis
   est également fourni, le manifest et la configuration doivent tout de
   même correspondre exactement.

`kagami genesis sign` vérifie que le JSON est valide et produit un bloc
Norito prêt à être utilisé via `genesis.file` dans la configuration du nœud.
Le fichier `genesis.signed.nrt` résultant est déjà en format wire canonique :
un octet de version suivi d’un en‑tête Norito décrivant le layout de la
payload. Distribuez toujours cette sortie encadrée. Il est recommandé
d’utiliser le suffixe `.nrt` pour les payloads signées ; si vous n’avez pas
besoin de mettre à jour l’exécuteur à genesis, vous pouvez omettre le champ
`executor` et ne pas fournir de fichier `.to`.

Lors de la signature de manifests NPoS (`--consensus-mode npos` ou un basculement planifié uniquement sur Iroha2), `kagami genesis sign` exige la charge utile `sumeragi_npos_parameters` ; générez-la avec `kagami genesis generate --consensus-mode npos` ou ajoutez le paramètre manuellement.
Par défaut, `kagami genesis sign` utilise le `consensus_mode` du manifest ; passez `--consensus-mode` pour le remplacer.

## Ce que Genesis peut faire

Genesis prend en charge les opérations suivantes. Kagami les assemble en
transactions dans un ordre bien défini pour que les pairs exécutent la même
séquence de manière déterministe :

- **Paramètres** : définir les valeurs initiales pour Sumeragi (temps de
  bloc/commit, dérive), pour Block (nombre maximal de transactions), pour
  Transaction (nombre maximal d’instructions, taille de bytecode), pour
  l’exécuteur et les contrats intelligents (carburant, mémoire, profondeur)
  ainsi que pour les paramètres personnalisés. Kagami sème `Sumeragi::NextMode`
  et la payload `sumeragi_npos_parameters` (temporisation NPoS, élection, reconfiguration) dans le bloc `parameters`, et le
  bloc signé inclut les instructions `SetParameter` générées afin qu’au
  démarrage les réglages de consensus soient appliqués à partir de l’état
  on‑chain.
- **Instructions natives** : enregistrer/désenregistrer Domain, Account,
  Asset Definition ; mint/burn/transfer d’assets ; transférer la propriété
  de domaines et de définitions d’assets ; modifier les métadonnées ;
  accorder des permissions et des rôles.
- **Triggers IVM** : enregistrer des triggers qui exécutent du bytecode IVM
  (voir `ivm_triggers`). Les exécutables de triggers sont résolus relativement
  à `ivm_dir`.
- **Topologie** : fournir l’ensemble initial de pairs via le tableau
  `topology` dans n’importe quelle transaction (généralement la première ou la dernière).
  Chaque entrée est `{ "peer": "<public_key>", "pop_hex": "<hex>" }` ; `pop_hex`
  peut être omis lors de la composition, mais doit être présent avant la signature.
- **Mise à niveau de l’exécuteur (optionnel)** : si `executor` est présent,
  Genesis insère une instruction `Upgrade` unique comme première transaction ;
  sinon, Genesis commence directement avec paramètres/instructions.

### Ordonnancement des transactions

Conceptuellement, les transactions de Genesis sont traitées dans cet ordre :

1. (Optionnel) Upgrade de l’exécuteur  
2. Pour chaque transaction de `transactions` :
   - Mises à jour de paramètres
   - Instructions natives
   - Enregistrement de triggers IVM
   - Entrées de topologie

Kagami et le code du nœud garantissent cet ordonnancement afin que, par
exemple, les paramètres soient appliqués avant les instructions suivantes
dans la même transaction.

## Workflow recommandé

- Partir d’un template généré par Kagami :
  - ISI intégrées uniquement :  
    `kagami genesis generate --ivm-dir <dir> --genesis-public-key <PK> --consensus-mode npos > genesis.json` (dataspace public de Sora Nexus ; utilisez `--consensus-mode permissioned` pour Iroha2 ou Iroha3 privé)
  - Avec upgrade d’exécuteur (optionnel) : ajouter `--executor <path/to/executor.to>`
- `<PK>` est n’importe quel multihash reconnu par `iroha_crypto::Algorithm`,
  y compris les variantes GOST TC26 lorsque Kagami est compilé avec
  `--features gost` (par exemple `gost3410-2012-256-paramset-a:...`).
- Valider en cours d’édition : `kagami genesis validate genesis.json`
- Signer pour le déploiement :
  `kagami genesis sign genesis.json --public-key <PK> --private-key <SK> --out-file genesis.signed.nrt`
- Configurer les pairs : définir `genesis.file` vers le fichier Norito signé
  (par exemple `genesis.signed.nrt`) et `genesis.public_key` sur le même
  `<PK>` utilisé pour signer.

Notes :
- Le template “default” de Kagami enregistre un domaine et des comptes
  d’exemple, mint quelques assets et accorde un minimum de permissions en
  utilisant uniquement des ISI intégrées – aucun `.to` requis.
- Si vous incluez un upgrade d’exécuteur, il doit être la première
  transaction. Kagami impose cette règle lors de la génération/signature.
- Utilisez `kagami genesis validate` pour détecter des valeurs `Name`
  invalides (ex. espaces) et des instructions mal formées avant la
  signature.

## Exécution avec Docker/Swarm

Les fichiers Docker Compose et Swarm fournis gèrent les deux cas :

- Sans exécuteur : la commande compose supprime un champ `executor`
  manquant/vide et signe le fichier.
- Avec exécuteur : elle résout le chemin relatif de l’exécuteur en un
  chemin absolu dans le conteneur puis signe le fichier.

Cela simplifie le développement sur des machines sans échantillons IVM
précompilés tout en permettant les upgrades d’exécuteur lorsque nécessaire.
