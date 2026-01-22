---
lang: fr
direction: ltr
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fe5fb47af37f86d33bfa884dc920efbf66714bbe3535842a786755dd5649f65
source_last_modified: "2025-12-07T08:26:38.035018+00:00"
translation_last_reviewed: 2026-01-01
---

# Modele de paquet governance repo (Roadmap F1)

Utilisez ce modele pour preparer le bundle d'artefacts requis par l'item roadmap
F1 (documentation et tooling du cycle de vie repo). L'objectif est de fournir aux reviewers un
seul fichier Markdown qui liste chaque input, hash et bundle de preuves afin que le
conseil governance puisse rejouer les bytes references dans la proposition.

> Copiez le modele dans votre repertoire de preuves (par exemple
> `artifacts/finance/repo/2026-03-15/packet.md`), remplacez les placeholders et
> commitez/uploadez-le a cote des artefacts hashes references ci-dessous.

## 1. Metadonnees

| Champ | Valeur |
|-------|-------|
| Identifiant accord/changement | `<repo-yyMMdd-XX>` |
| Prepare par / date | `<desk lead> - 2026-03-15T10:00Z` |
| Revise par | `<dual-control reviewer(s)>` |
| Type de changement | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
| Custodian(s) | `<custodian id(s)>` |
| Proposition liee / referendum | `<governance ticket id or GAR link>` |
| Repertoire de preuves | ``artifacts/finance/repo/<slug>/`` |

## 2. Payloads d'instructions

Enregistrez les instructions Norito staged que les desks ont approuvees via
`iroha app repo ... --output`. Chaque entree doit inclure le hash du fichier emis
et une courte description de l'action qui sera soumise une fois le vote passe.

| Action | Fichier | SHA-256 | Notes |
|--------|------|---------|-------|
| Initiate | `instructions/initiate.json` | `<sha256>` | Contient les legs cash/collateral approuves par desk + counterparty. |
| Margin call | `instructions/margin_call.json` | `<sha256>` | Capture cadence + participant id qui a declenche l'appel. |
| Unwind | `instructions/unwind.json` | `<sha256>` | Preuve de la reverse-leg une fois les conditions remplies. |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json       | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 Acknowledgements custodian (tri-party only)

Completez cette section lorsqu'un repo utilise `--custodian`. Le paquet governance
doit inclure un acknowledgement signe de chaque custodian ainsi que le hash du fichier
reference a sec 2.8 de `docs/source/finance/repo_ops.md`.

| Custodian | Fichier | SHA-256 | Notes |
|-----------|------|---------|-------|
| `<ih58...>` | `custodian_ack_<custodian>.md` | `<sha256>` | SLA signe couvrant fenetre de custody, compte de routage et contact drill. |

> Stockez l'acknowledgement a cote des autres preuves (`artifacts/finance/repo/<slug>/`)
> afin que `scripts/repo_evidence_manifest.py` enregistre le fichier dans le meme arbre que
> les instructions staged et les snippets de config. Voir
> `docs/examples/finance/repo_custodian_ack_template.md` pour un template pret a remplir qui
> correspond au contrat de preuves governance.

## 3. Snippet de configuration

Collez le bloc TOML `[settlement.repo]` qui sera deploye sur le cluster (y compris
`collateral_substitution_matrix`). Stockez le hash a cote du snippet pour que les auditors
confirment la politique runtime active lorsque la reservation repo a ete approuvee.

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 Snapshots de configuration post-approval

Apres la fin du referendum ou vote governance et le rollout du changement `[settlement.repo]`,
capturez des snapshots `/v1/configuration` depuis chaque peer afin que les auditors prouvent
que la politique approuvee est active sur le cluster (voir
`docs/source/finance/repo_ops.md` sec 2.9 pour le workflow de preuves).

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v1/configuration       | jq '.'       > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| Peer / source | Fichier | SHA-256 | Block height | Notes |
|---------------|------|---------|--------------|-------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | Snapshot capture juste apres le rollout config. |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | Confirme que `[settlement.repo]` correspond au TOML staged. |

Enregistrez les digests a cote des peer ids dans `hashes.txt` (ou le resume equivalent)
pour que les reviewers puissent tracer les nodes qui ont ingere le changement. Les snapshots
vivent sous `config/peers/` a cote du snippet TOML et seront automatiquement collectes par
`scripts/repo_evidence_manifest.py`.

## 4. Artefacts de tests deterministes

Joignez les derniers outputs de:

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

Enregistrez les chemins de fichiers + hashes pour les bundles de logs ou JUnit XML produits par votre CI.

| Artefact | Fichier | SHA-256 | Notes |
|----------|------|---------|-------|
| Log lifecycle proof | `tests/repo_lifecycle.log` | `<sha256>` | Capture avec `--nocapture`. |
| Log integration test | `tests/repo_integration.log` | `<sha256>` | Inclut substitution + cadence margin. |

## 5. Snapshot de lifecycle proof

Chaque paquet doit inclure le snapshot deterministe exporte par
`repo_deterministic_lifecycle_proof_matches_fixture`. Lancez le harness avec les
knobs d'export actives pour que les reviewers puissent comparer le frame JSON et le
digest a la fixture dans `crates/iroha_core/tests/fixtures/` (voir
`docs/source/finance/repo_ops.md` sec 2.7).

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json     REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt     cargo test -p iroha_core       -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

Ou utilisez le helper fige pour regenerer les fixtures et les copier dans votre bundle de preuves en une seule etape:

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain>       --bundle-dir artifacts/finance/repo/<slug>
```

| Artefact | Fichier | SHA-256 | Notes |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | Frame lifecycle canonique emis par le proof harness. |
| Digest file | `repo_proof_digest.txt` | `<sha256>` | Digest hex uppercase miroir de `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`; joindre meme si inchang. |

## 6. Evidence manifest

Generez le manifest pour tout le repertoire de preuves afin que les auditors puissent verifier
les hashes sans decompresser l'archive. Le helper reproduit le workflow decrit dans
`docs/source/finance/repo_ops.md` sec 3.2.

```bash
python3 scripts/repo_evidence_manifest.py       --root artifacts/finance/repo/<slug>       --agreement-id <repo-identifier>       --output artifacts/finance/repo/<slug>/manifest.json
```

| Artefact | Fichier | SHA-256 | Notes |
|----------|------|---------|-------|
| Evidence manifest | `manifest.json` | `<sha256>` | Inclure le checksum dans le ticket governance / notes de referendum. |

## 7. Snapshot telemetrie & events

Exportez les entrees pertinentes `AccountEvent::Repo(*)` et tout dashboard ou export CSV
reference dans `docs/source/finance/repo_ops.md`. Enregistrez fichiers + hashes ici pour que
les reviewers accedent directement aux preuves.

| Export | Fichier | SHA-256 | Notes |
|--------|------|---------|-------|
| Repo events JSON | `evidence/repo_events.ndjson` | `<sha256>` | Flux Torii brut filtre sur les comptes desk. |
| Telemetry CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | Exporte depuis Grafana via le panneau Repo Margin. |

## 8. Approbations & signatures

- **Signataires dual-control:** `<names + timestamps>`
- **GAR / minutes digest:** `<sha256>` du PDF GAR signe ou de l'upload minutes.
- **Emplacement de stockage:** `governance://finance/repo/<slug>/packet/`

## 9. Checklist

Cochez chaque item une fois complete.

- [ ] Payloads d'instructions staged, hashes et attaches.
- [ ] Hash du snippet de configuration enregistre.
- [ ] Logs de tests deterministes captures + hashes.
- [ ] Snapshot lifecycle + digest exporte.
- [ ] Evidence manifest genere et hash enregistre.
- [ ] Exports events/telemetrie captures + hashes.
- [ ] Acknowledgements dual-control archives.
- [ ] GAR/minutes uploadees; digest enregistre ci-dessus.

Maintenir ce template avec chaque paquet garde le DAG governance deterministe et fournit aux auditors un manifest portable pour les decisions de cycle de vie repo.
