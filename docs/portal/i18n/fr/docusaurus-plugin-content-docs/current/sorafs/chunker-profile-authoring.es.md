---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : création de profil chunker
titre : Guide de l'autorité des profils de chunker de SoraFS
sidebar_label : Guide de l'auteur de chunker
description : Liste de contrôle pour proposer de nouveaux profils et accessoires de chunker de SoraFS.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/chunker_profile_authoring.md`. Assurez-vous d'avoir des copies synchronisées jusqu'à ce que le ensemble de documents Sphinx héréditaire soit retiré.
:::

# Guide de l'autorité des profils du chunker de SoraFS

Ce guide explique comment proposer et publier de nouveaux profils de chunker pour SoraFS.
Complète le RFC d'architecture (SF-1) et la référence du registre (SF-2a)
avec les exigences concrètes de l'autorité, les étapes de validation et les plantes de propriété.
Pour un exemple canonique, consultez
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
et le registre des essais à sec associés
`docs/source/sorafs/reports/sf1_determinism.md`.

## CV

Chaque profil qui entre dans le registre doit être :

- Annoncer les paramètres déterministes CDC et ajuster les identités multihash entre
  architectures;
- entregar luminaires reproductibles (JSON Rust/Go/TS + corpus fuzz + testigos PoR) que
  les SDK en aval peuvent vérifier l'outillage à moyen ;
- Inclure des listes de métadonnées pour le gouvernement (espace de noms, nom, semestre) avec le guide de déploiement
  et les fenêtres opérationnelles ; oui
- passer la suite des différences déterminantes avant la révision du conseil.

Suivez la liste de contrôle ci-dessous pour préparer une proposition qui remplira ces règles.## Résumé de la carte du registre

Avant de rédiger une proposition, confirmez que vous remplissez la carte du registre appliqué
par `sorafs_manifest::chunker_registry::ensure_charter_compliance()` :

- Les identifiants de profil sont positifs et augmentent la forme monotone sans huecos.
- La poignée canonique (`namespace.name@semver`) doit apparaître dans la liste des alias
  et **debe** sera la première entrée. Suivez les alias heredados (p. ej., `sorafs.sf1@1.0.0`).
- Ningún alias puede colisionar con otro handle canonico ni apparaître plus d'une fois.
- Los alias doivent être no vacíos ni recortados de espacios en blanco.

Aides utiles de CLI :

```bash
# Listado JSON de todos los descriptores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadatos para un perfil por defecto candidato (handle canónico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Ces commandes maintiennent les propositions alignées sur la carte du registre et les fournissent
métadonnées canoniques nécessaires aux discussions de gouvernement.

## Métadonnées requises| Champ | Description | Exemple (`sorafs.sf1@1.0.0`) |
|-------|-------------|------------------------------|
| `namespace` | Agrupación logique de perfiles relacionados. | `sorafs` |
| `name` | Étiquette lisible pour les humains. | `sf1` |
| `semver` | Chaîne de version sémantique pour le ensemble de paramètres. | `1.0.0` |
| `profile_id` | Un identifiant numérique unique est attribué une fois que le profil entre. Réservez la suite sans toutefois réutiliser de nombreux nombres existants. | `1` |
| `profile_aliases` | Gère les options supplémentaires (nombres heredados, abreviaturas) expuestos a clients durante la negociación. Incluez toujours la poignée canonique comme la première entrée. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | Longitude minimale du morceau en octets. | `65536` |
| `profile.target_size` | Longitude objet du chunk en octets. | `262144` |
| `profile.max_size` | Longitude maximale du morceau en octets. | `524288` |
| `profile.break_mask` | Masque adaptatif utilisé pour le hash roulant (hex). | `0x0000ffff` |
| `profile.polynomial` | Constante del polinomio gear (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Seed utilisée pour dériver le tableau Gear de 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Code multihash pour digérer par morceau. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Résumé du bundle canónico de luminaires. | `13fa...c482` || `fixtures_root` | Directeur relatif aux matchs régénérés. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Seed para el muestreo PoR determinista (`splitmix64`). | `0xfeedbeefcafebabe` (exemple) |

Les métadonnées doivent apparaître dans le document de propriété à l'intérieur des appareils.
régénérés pour que le registre, les outils de CLI et l'automatisation de la gestion puissent être effectués
confirmer les valeurs sans les cruces manuelles. Si vous êtes des mecs, exécutez les CLI de chunk-store et
manifeste avec `--json-out=-` pour transmettre les métadonnées calculées aux notes de révision.

### Points de contact de CLI et d'enregistrement

- `sorafs_manifest_chunk_store --profile=<handle>` — retourner les métadonnées du chunk,
  digest de manifest y checks PoR con los parámetros propuestos.
- `sorafs_manifest_chunk_store --json-out=-` — transmettre le rapport de chunk-store à
  sortie standard pour comparaisons automatisées.
- `sorafs_manifest_stub --chunker-profile=<handle>` — confirmer que les manifestes et les avions CAR
  embeben el handle canónico plus los alias.
- `sorafs_manifest_stub --plan=-` — retourner à l'alimentation du `chunk_fetch_specs` précédent pour
  vérifier les compensations/digestes après le changement.

Enregistrez la sortie de commandes (digests, racines PoR, hachages de manifeste) à la bonne adresse
Les réviseurs peuvent reproduire le texte.

## Checklist de déterminisme et de validation1. **Régénérer les luminaires**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Exécuter la suite de parité** — `cargo test -p sorafs_chunker` et le diff arnés
   entre les langues (`crates/sorafs_chunker/tests/vectors.rs`) doivent être en vert avec les
   nouveaux luminaires sur votre lieu.
3. **Reproduire des corps fuzz/back-pression** — ejecuta `cargo fuzz list` et l'arnés de
   streaming (`fuzz/sorafs_chunker`) contre les actifs régénérés.
4. **Verificar testigos Proof-of-Retrievability** — ejecuta
   `sorafs_manifest_chunk_store --por-sample=<n>` utiliser le profil et confirmer que les
   les courses coïncident avec le manifeste des rencontres.
5. **Dry run de CI** — invoque `ci/check_sorafs_fixtures.sh` localement ; le script
   vous devrez tenir un succès avec les nouveaux luminaires et le `manifest_signatures.json` existant.
6. **Confirmation cross-runtime** — assurez-vous que les liaisons Go/TS consomment le JSON régénéré
   et émet des limites et digère des identicos.

Documentation des commandes et des résumés résultants proposés pour que le Tooling WG puisse
répéter sans conjeturas.

### Confirmation du manifeste / PoR

Après avoir régénéré les appareils, exécutez le pipeline complet du manifeste pour garantir que les
métadonnées CAR et les essais PoR sigan siendo cohérents :

```bash
# Validar metadata de chunk + PoR con el nuevo perfil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Generar manifest + CAR y capturar chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Reejecutar usando el plan de fetch guardado (evita offsets obsoletos)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Remplacer le fichier d'entrée avec tout corpus représentatif utilisé pour vos luminaires
(par exemple, le flux déterministe de 1 GiB) et ajoute les résumés résultants à la proposition.

## Plante de propriétéLes propositions sont envoyées comme enregistrés Norito `ChunkerProfileProposalV1` enregistrés en
`docs/source/sorafs/proposals/`. La plante JSON de bas illustre la forme attendue
(supstituye tus valores según sea necesario):


Proportionne un rapport Markdown correspondant (`determinism_report`) qui capture la
salida de comandos, los digestes de chunk et tout ce qui se passe lors de la validation.

## Flujo de gobernanza

1. **Enviar PR con propuesta + luminaires.** Inclut les actifs générés, la propuesta
   Norito et les mises à jour `chunker_registry_data.rs`.
2. **Révision du Tooling WG.** Les réviseurs révisent la liste de contrôle de validation et
   confirman que la propriété est alignée avec les règles du registre (sans réutilisation de votre pièce d'identité,
   déterminisme satisfaisant).
3. **Sobre del consejo.** Une fois approuvé, les membres du conseil confirment le résumé de
   la propuesta (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) et anexan sus
   firmas al sobre del perfil guardado junto a los luminaires.
4. **Publicación del registro.** La fusion actualise le registre, les documents et les appareils. El
   CLI par défaut persiste dans le profil précédent jusqu'à ce que le gouvernement déclare la migration
   liste.
5. **Suivi de dépréciation.** Après la vente de migration, actualisez le registre
   registre des migrations.

## Conseils d'autorité- Préférer les limites de puissance des deux pare-chocs pour minimiser le comportement de chunking dans les cas limites.
- Évitez de modifier le code multihash sans coordonner les utilisateurs du manifeste et de la passerelle ; y compris une
  nota operativa cuando lo hagas.
- Mantén les graines du tabla gear lisibles pour les humains mais globalement uniques pour simplifier
  auditoriums.
- Garder tout artefact de benchmarking (par exemple, comparaisons de débit) bas
  `docs/source/sorafs/reports/` pour référence future.

Pour les attentes opérationnelles pendant le déploiement, consultez le registre de migration
(`docs/source/sorafs/migration_ledger.md`). Pour les règles de conformité à la version d'exécution
`docs/source/sorafs/chunker_conformance.md`.