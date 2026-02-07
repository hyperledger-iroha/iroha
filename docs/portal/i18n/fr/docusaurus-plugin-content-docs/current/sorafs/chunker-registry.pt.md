---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : chunker-registry
titre : Registro de perfis de chunker da SoraFS
sidebar_label : Registre du chunker
description : ID de profil, paramètres et plan de négociation pour le registre de chunker de SoraFS.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/chunker_registry.md`. Mantenha ambas comme copies synchronisées.
:::

## Registre des performances du chunker du SoraFS (SF-2a)

Une pile SoraFS négocie le comportement de chunking via un petit registre avec l'espace de noms.
Chaque profil attribue des paramètres déterministes CDC, des métadonnées semestriels et des digests/multicodecs attendus dans les manifestes et les archives CAR.

Autores de perfis devem consultar
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
pour les métadonnées requises, une liste de contrôle de validation et le modèle de proposition avant les nouvelles entrées submétriques.
Une fois que la gouvernance approuve une décision, siga o
[liste de contrôle du déploiement du registre](./chunker-registry-rollout-checklist.md) et o
[playbook de manifest em staging](./staging-manifest-playbook) pour le promoteur
os luminaires, mise en scène et production.

### Perfis| Espace de noms | Nome | SemVer | ID de profil | Min (octets) | Cible (octets) | Max (octets) | Mascara de quebra | Multihash | Alias ​​| Notes |
|---------------|------|--------|-------------|-------------|----------------|-------------|------------------|---------------|--------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Profil canonique utilisé dans les luminaires SF-1 |

Le registre vive n'est pas codifié comme `sorafs_manifest::chunker_registry` (gouverné par [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Chaque entrée
et exprimé comme un `ChunkerProfileDescriptor` avec :

* `namespace` - groupe logique de performances liées (ex., `sorafs`).
* `name` - rotulo legivel para humanos (`sf1`, `sf1-fast`, ...).
* `semver` - chaîne de sens sémantique pour le ensemble de paramètres.
* `profile` - o `ChunkProfile` réel (min/cible/max/masque).
* `multihash_code` - le multihash est utilisé pour produire des résumés de chunk (`0x1f`
  par défaut par SoraFS).Le manifeste sérialisé perfis via `ChunkingProfileV1`. Une structure enregistrée pour les métadonnées
 faire un registre (espace de noms, nom, semestre) avec les paramètres bruts CDC
et une liste d'alias affichée en haut. Les consommateurs doivent avoir une première tente
je ne cherche pas à m'enregistrer sur `profile_id` et à enregistrer les paramètres en ligne quand
IDs desconhecidos aparecerem; une liste d'alias garantissant que les clients HTTP possèdent
continuer à envoyer des poignées alternatives em `Accept-Chunker` sem adivinhar. Comme le font les regras
la charte du registre exige que le manche canonique (`namespace.name@semver`) seja a
Première entrée dans `profile_aliases`, suivie par des alias alternatifs.

Pour vérifier ou enregistrer à partir de l'outillage, exécutez l'assistant CLI :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

Tous les indicateurs font que CLI écrive JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) ainsi que `-` comme chemin, où transmettre la charge utile pour la sortie standard à chaque fois
crier un archivage. Est-ce qu'il est facile d'encadrer les dados pour l'outillage pendant ou
comportement responsable de l'impression du rapport principal.

### Matrice de déploiement et plan d'implantation


Le tableau abaixo capture l'état actuel du support pour `sorafs.sf1@1.0.0` nos
composants principaux. "Bridge" référence à faixa CARv1 + SHA-256
que requer negociacao explicita do cliente (`Accept-Chunker` + `Accept-Digest`).| Composants | Statut | Notes |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ Soutenu | Validez la poignée canonique + alias, accédez au flux de relations via `--json-out=-` et appliquez la charte d'enregistrement via `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ Retraité | Constructeur de forums de soutien manifestes ; utiliser `iroha app sorafs toolkit pack` pour empacotamento CAR/manifest e mantenha `--plan=-` para revalidacao deterministica. |
| `sorafs_provider_advert_stub` | ⚠️ Retraité | Aide à la validation des opérations hors ligne ; Les annonces des fournisseurs sont développées pour être produites par un pipeline de publication et validé via `/v1/sorafs/providers`. |
| `sorafs_fetch` (orchestrateur développeur) | ✅ Soutenu | Le `chunk_fetch_specs` comprend les charges utiles de capacité `range` et le monta dit CARv2. |
| Montages du SDK (Rust/Go/TS) | ✅ Soutenu | Régénérées via `export_vectors` ; Le manche canonique apparaît d'abord dans chaque liste d'alias et est assassiné par les enveloppes du conseil. |
| Négociation de profil sans passerelle Torii | ✅ Soutenu | Implémentez la grammaire complète de `Accept-Chunker`, y compris les en-têtes `Content-Chunker` et l'exposition du pont CARv1 dans les sollicitations explicites de rétrogradation. |

Déploiement de la télémétrie :- **Télémétrie pour récupérer les fragments** - La CLI Iroha `sorafs toolkit pack` émet des résumés de fragment, des métadonnées CAR et augmente le PoR pour ingérer les tableaux de bord.
- **Annonces du fournisseur** - les charges utiles des annonces incluent des métadonnées de capacité et des alias ; valider la cobertura via `/v1/sorafs/providers` (ex., presenca da capacidade `range`).
- **Monitoramento de gateway** - les opérateurs doivent signaler les pareamentos `Content-Chunker`/`Content-Digest` pour détecter les déclassements indésirables ; J'espère que l'utilisation du bridge tend à zéro avant la dépréciation.

Politique de dépréciation : une fois qu'un profil successeur est ratifié, l'agenda d'une nouvelle publication est double
bridge CARv1 dos gateways dans la production.

Pour vérifier un test de PoR spécifique, fournir des indices de chunk/segment/folha et facultativement
persista a prova no disco :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Vous pouvez sélectionner un profil par identifiant numérique (`--profile-id=1`) ou par poignée d'enregistrement.
(`--profile=sorafs.sf1@1.0.0`); un formulaire de poignée et pratique pour les scripts que
Encadeiam namespace/name/semver directement dos metadados de gouvernance.

Utilisez `--promote-profile=<handle>` pour émettre un bloc de métadonnées JSON (y compris tous les alias
enregistrés) qui peuvent être colado em `chunker_registry_data.rs` pour promouvoir un nouveau profil padrao :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```Le rapport principal (et l'archive de preuve facultative) comprend le résumé des données, les octets des folhas amis
(codifiés en hexadécimal) et les résumés irmaos de segmento/chunk pour que les vérificateurs puissent le faire
recalculer le hachage des caméras de 64 KiB/4 KiB contre la valeur `por_root_hex`.

Pour valider une preuve d'existence contre une charge utile, passez le chemin via
`--por-proof-verify` (ou CLI ajouté `"por_proof_verified": true` lorsque vous testez
correspond à un calcul calculé):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Pour stocker beaucoup de choses, utilisez `--por-sample=<count>` et éventuellement pour fournir un chemin de graine/saida.
La CLI garantit un ordre déterministe (ensemencé avec `splitmix64`) et démarre automatiquement quand
a requisicao exceder as folhas disponiveis :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

O manifest stub espelha os mesmos dados, o que e conveniente ao automatizar a selecao de
`--chunker-profile-id` em pipelines. Ambos os CLIs de chunk store tambem aceitam a forma de handle canonico
(`--profile=sorafs.sf1@1.0.0`) para que scripts de build evitem hard-codear IDs numericos:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "ID_profil": 1,
    "espace de noms": "sorafs",
    "nom": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "taille_cible": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

O campo `handle` (`namespace.name@semver`) corresponde ao que os CLIs aceitam via
`--profile=...`, tornando seguro copiar direto para automacao.

### Negociar chunkers

Gateways e clientes anunciam perfis suportados via provider adverts:

```
FournisseurAnnonceBodyV1 {
    ...
    chunk_profile : profile_id (implicite via le registre)
    capacités : [...]
}
```

O agendamento de chunks multi-source e anunciado via a capacidade `range`. O CLI aceita isso com
`--capability=range[:streams]`, onde o sufixo numerico opcional codifica a concorrencia preferida
para fetch por range do provider (por exemplo, `--capability=range:64` anuncia um budget de 64 streams).
Quando omitido, consumidores recorrem ao hint geral `max_streams` publicado em outro ponto do advert.

Ao solicitar dados CAR, clientes devem enviar um header `Accept-Chunker` listando tuplas
`(namespace, name, semver)` em ordem de preferencia:

```Les passerelles sélectionnent un profil pris en charge mutuellement (par défaut `sorafs.sf1@1.0.0`)
Il reflète la décision via l'en-tête de réponse `Content-Chunker`. Manifestes
embutem o perfil escolhido para que nos aval possam validar o layout de chunks
mais cela dépend de la négociation HTTP.

### Supporte la CAR

Nous avons un chemin d'exportation CARv1+SHA-2 :

* **Caminho primario** - CARv2, résumé de la charge utile BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, profil de morceau enregistré comme acima.
  PODEM expor esta variante lorsque le client omite `Accept-Chunker` ou sollicite
  `Accept-Digest: sha2-256`.

adicionais para transicao, mais nao devem substituir o digest canonico.

### Conformité

* Le profil `sorafs.sf1@1.0.0` mapeia para os luminaires publicos em
  `fixtures/sorafs_chunker` et les sociétés enregistrées dans
  `fuzz/sorafs_chunker`. Une parité de bout en bout et exercée sur Rust, Go et Node
  via os testes fornecidos.
* `chunker_registry::lookup_by_profile` confirme que les paramètres du descripteur
  correspondem a `ChunkProfile::DEFAULT` pour éviter toute divergence acide.
* Les manifestes produits par `iroha app sorafs toolkit pack` et `sorafs_manifest_stub` incluent les métadonnées du registre.