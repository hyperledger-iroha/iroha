---
lang: fr
direction: ltr
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T10:01:27.059307+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Conception de gadget de transfert FastPQ

# Aperçu

Le planificateur FASTPQ actuel enregistre chaque opération primitive impliquée dans une instruction `TransferAsset`, ce qui signifie que chaque transfert paie séparément l'arithmétique du solde, les tours de hachage et les mises à jour SMT. Pour réduire les lignes de trace par transfert, nous introduisons un gadget dédié qui vérifie uniquement les vérifications arithmétiques/engagement minimales pendant que l'hôte continue d'exécuter la transition d'état canonique.

- **Portée** : transferts uniques et petits lots émis via la surface d'appel système Kotodama/IVM `TransferAsset` existante.
- **Objectif** : réduire l'encombrement des colonnes FFT/LDE pour les transferts de gros volumes en partageant des tables de recherche et en regroupant l'arithmétique par transfert dans un bloc de contraintes compact.

# Architecture

```
Kotodama builder → IVM syscall (transfer_v1 / transfer_v1_batch)
          │
          ├─ Host (unchanged business logic)
          └─ Transfer transcript (Norito-encoded)
                   │
                   └─ FASTPQ TransferGadget
                        ├─ Balance arithmetic block
                        ├─ Poseidon commitment check
                        ├─ Dual SMT path verifier
                        └─ Authority digest equality
```

## Format de transcription

L'hôte émet un `TransferTranscript` par appel système :

```rust
struct TransferTranscript {
    batch_hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
    authority_digest: Hash,
    poseidon_preimage_digest: Option<Hash>,
}

struct TransferDeltaTranscript {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Numeric,
    from_balance_before: Numeric,
    from_balance_after: Numeric,
    to_balance_before: Numeric,
    to_balance_after: Numeric,
    from_merkle_proof: Option<Vec<u8>>,
    to_merkle_proof: Option<Vec<u8>>,
}
```

- `batch_hash` lie la transcription au hachage du point d'entrée de la transaction pour la protection contre la relecture.
- `authority_digest` est le hachage de l'hôte sur les données triées des signataires/quorum ; le gadget vérifie l'égalité mais ne refait pas la vérification de la signature. Concrètement, l'hôte Norito code le `AccountId` (qui intègre déjà le contrôleur multisig canonique) et hache `b"iroha:fastpq:v1:authority|" || encoded_account` avec Blake2b-256, stockant le `Hash` résultant.
- `poseidon_preimage_digest` = Poséidon (account_from || account_to || actif || montant || batch_hash); garantit que le gadget recalcule le même résumé que l'hôte. Les octets de pré-image sont construits sous la forme `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash` en utilisant le simple codage Norito avant de les transmettre via l'assistant partagé Poseidon2. Ce résumé est présent pour les transcriptions mono-delta et omis pour les lots multi-delta.

Tous les champs sont sérialisés via Norito afin que les garanties de déterminisme existantes soient valables.
`from_path` et `to_path` sont émis sous forme de blobs Norito à l'aide du
Schéma `TransferMerkleProofV1` : `{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`.
Les versions futures peuvent étendre le schéma pendant que le prouveur applique la balise de version
avant le décodage. Les métadonnées `TransitionBatch` intègrent la transcription codée Norito
vecteur sous la clé `transfer_transcripts` pour que le prouveur puisse décoder le témoin
sans effectuer de requêtes hors bande. Entrées publiques (`dsid`, `slot`, racines,
`perm_root`, `tx_set_hash`) sont portés dans `FastpqTransitionBatch.public_inputs`,
laisser des métadonnées pour la comptabilité du nombre de hachages/transcriptions d'entrée. Jusqu'à la plomberie hôte
atterrit, le prouveur dérive synthétiquement des preuves à partir des paires clé/solde, donc les lignes
incluez toujours un chemin SMT déterministe même lorsque la transcription omet les champs facultatifs.

## Disposition des gadgets

1. **Bloc arithmétique d'équilibre**
   - Entrées : `from_balance_before`, `amount`, `to_balance_before`.
   - Chèques :
     - `from_balance_before >= amount` (gamme gadget avec décomposition RNS partagée).
     -`from_balance_after = from_balance_before - amount`.
     -`to_balance_after = to_balance_before + amount`.
   - Emballés dans une porte personnalisée afin que les trois équations consomment un groupe de lignes.2. **Bloc d'engagement Poséidon**
   - Recalcule `poseidon_preimage_digest` en utilisant la table de recherche partagée Poséidon déjà utilisée dans d'autres gadgets. Aucun round de Poséidon par transfert dans la trace.

3. **Bloc de chemin Merkle**
   - Étend le gadget Kaigi SMT existant avec un mode "mise à jour couplée". Deux feuilles (expéditeur, destinataire) partagent la même colonne pour les hachages frères et sœurs, réduisant ainsi les lignes en double.

4. **Vérification du résumé d'autorité**
   - Contrainte d'égalité simple entre le résumé fourni par l'hôte et la valeur du témoin. Les signatures restent dans leur gadget dédié.

5. **Boucle par lots**
   - Les programmes appellent `transfer_v1_batch_begin()` avant une boucle de builders `transfer_asset` et `transfer_v1_batch_end()` après. Pendant que l'oscilloscope est actif, l'hôte met en mémoire tampon chaque transfert et les relit comme un seul `TransferAssetBatch`, en réutilisant le contexte Poséidon/SMT une fois par lot. Chaque delta supplémentaire ajoute uniquement l'arithmétique et deux vérifications de feuilles. Le décodeur de transcription accepte désormais les lots multi-delta et les présente comme `TransferGadgetInput::deltas` afin que le planificateur puisse plier les témoins sans relire Norito. Les contrats qui disposent déjà d'une charge utile Norito à portée de main (par exemple, CLI/SDK) peuvent ignorer complètement la portée en appelant `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)`, qui remet à l'hôte un lot entièrement codé en un seul appel système.

# Modifications de l'hôte et du prouveur| Couche | Changements |
|-------|--------------|
| `ivm::syscalls` | Ajoutez `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`) afin que les programmes puissent encadrer plusieurs appels système `transfer_v1` sans émettre d'ISI intermédiaires, plus `transfer_v1_batch_apply` (`0x2B`) pour les lots pré-encodés. |
| `ivm::host` & essais | Les hôtes principaux/par défaut traitent `transfer_v1` comme un ajout par lot lorsque la portée est active, font apparaître `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}` et l'hôte WSV fictif met en mémoire tampon les entrées avant la validation afin que les tests de régression puissent affirmer un équilibre déterministe. mises à jour.【crates/ivm/src/core_host.rs:1001】【crates/ivm/src/host.rs:451】【crates/ivm/src/mock_wsv.rs :3713】【crates/ivm/tests/wsv_host_pointer_tlv.rs:219】【crates/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` | Émettez `TransferTranscript` après la transition d'état, créez des enregistrements `FastpqTransitionBatch` avec `public_inputs` explicite pendant `StateBlock::capture_exec_witness` et exécutez la voie de preuve FASTPQ afin que les outils Torii/CLI et le backend Stage6 reçoivent des informations canoniques. Entrées `TransitionBatch`. `TransferAssetBatch` regroupe les transferts séquentiels en une seule transcription, en omettant le résumé de Poséidon pour les lots multi-delta afin que le gadget puisse parcourir les entrées de manière déterministe. |
| `fastpq_prover` | `gadgets::transfer` valide désormais les transcriptions multi-delta (arithmétique de balance + résumé de Poséidon) et fait apparaître des témoins structurés (y compris des blobs SMT appariés d'espaces réservés) pour le planificateur (`crates/fastpq_prover/src/gadgets/transfer.rs`). `trace::build_trace` décode ces transcriptions à partir des métadonnées par lots, rejette les lots de transfert manquant de la charge utile `transfer_transcripts`, attache les témoins validés à `Trace::transfer_witnesses` et `TracePolynomialData::transfer_plan()` maintient le plan agrégé en vie jusqu'à ce que le planificateur consomme le gadget (`crates/fastpq_prover/src/trace.rs`). Le faisceau de régression du nombre de lignes est désormais livré via `fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`), couvrant des scénarios allant jusqu'à 65 536 lignes remplies, tandis que le câblage SMT apparié reste derrière le jalon d'assistance par lots TF-3 (les espaces réservés maintiennent la disposition de la trace stable jusqu'à ce que cet échange atterrisse). |
| Kotodama | Réduit l'assistant `transfer_batch((from,to,asset,amount), …)` en `transfer_v1_batch_begin`, les appels séquentiels `transfer_asset` et `transfer_v1_batch_end`. Chaque argument de tuple doit suivre la forme `(AccountId, AccountId, AssetDefinitionId, int)` ; les transferts uniques conservent le constructeur existant. |

Exemple d'utilisation de Kotodama :

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch` exécute les mêmes vérifications d'autorisation et arithmétiques que les appels `Transfer::asset_numeric` individuels, mais enregistre tous les deltas dans un seul `TransferTranscript`. Les transcriptions multi-delta échappent au résumé de Poséidon jusqu'à ce que les engagements par delta atterrissent dans un suivi. Le générateur Kotodama émet désormais automatiquement les appels système de début/fin, afin que les contrats puissent déployer des transferts par lots sans coder manuellement les charges utiles Norito.

## Harnais de régression par nombre de lignes

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) synthétise les lots de transition FASTPQ avec des nombres de sélecteurs configurables et rapporte le résumé `row_usage` résultant (`total_rows`, nombres par sélecteur, rapport) ainsi que la longueur/log₂ complétée. Capturez des références pour le plafond de 65 536 rangées avec :

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```Le JSON émis reflète les artefacts de lot FASTPQ que `iroha_cli audit witness` émet désormais par défaut (passez `--no-fastpq-batches` pour les supprimer), de sorte que `scripts/fastpq/check_row_usage.py` et la porte CI peuvent comparer les exécutions synthétiques aux instantanés précédents lors de la validation des modifications du planificateur.

# Plan de déploiement

1. **TF-1 (Transcript plumbing)** : ✅ `StateTransaction::record_transfer_transcripts` émet désormais des transcriptions Norito pour chaque `TransferAsset`/lot, `sumeragi::witness::record_fastpq_transcript` les stocke dans le témoin global et `StateBlock::capture_exec_witness` construit `fastpq_batches` avec `public_inputs` explicite pour les opérateurs et la voie des étalons (utilisez `--no-fastpq-batches` si vous avez besoin d'un sortie).【crates/iroha_core/src/state.rs:8801】【crates/iroha_core/src/sumeragi/witness.rs:280】【crates/iroha_core/src/fastpq/mod.rs:157】【crates/iroha_cli/src/audit.rs:185】
2. **TF-2 (implémentation du gadget)** : ✅ `gadgets::transfer` valide désormais les transcriptions multi-delta (arithmétique de la balance + résumé de Poséidon), synthétise les preuves SMT appariées lorsque les hôtes les omettent, expose les témoins structurés via `TransferGadgetPlan` et `trace::build_trace` intègre ces témoins dans `Trace::transfer_witnesses` lors du remplissage des colonnes SMT à partir des épreuves. `fastpq_row_bench` capture le faisceau de régression de 65 536 lignes afin que les planificateurs suivent l'utilisation des lignes sans relire Norito. charges utiles.【crates/fastpq_prover/src/gadgets/transfer.rs:1】【crates/fastpq_prover/src/trace.rs:1】【crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1】
3. **TF-3 (assistant par lots)** : activez le générateur d'appel système par lots + Kotodama, y compris l'application séquentielle au niveau de l'hôte et la boucle de gadget.
4. **TF-4 (Télémétrie et documents)** : mettez à jour les schémas `fastpq_plan.md`, `fastpq_migration_guide.md` et le tableau de bord pour afficher l'allocation des lignes de transfert par rapport à d'autres gadgets.

# Questions ouvertes

- **Limites de domaine** : le planificateur FFT actuel panique pour les traces au-delà de 2¹⁴ lignes. TF-2 devrait soit augmenter la taille du domaine, soit documenter un objectif de référence réduit.
- **Lots multi-actifs** : le gadget initial suppose le même ID d'actif par delta. Si nous avons besoin de lots hétérogènes, nous devons nous assurer que le témoin Poséidon inclut l'actif à chaque fois pour éviter la relecture entre les actifs.
- **Réutilisation du résumé d'autorité** : à long terme, nous pouvons réutiliser le même résumé pour d'autres opérations autorisées afin d'éviter de recalculer les listes de signataires par appel système.


Ce document suit les décisions de conception ; gardez-le synchronisé avec les entrées de la feuille de route lorsque les jalons arrivent.