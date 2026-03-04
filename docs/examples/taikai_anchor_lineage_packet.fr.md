---
lang: fr
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-11-21T18:09:53.463728+00:00"
translation_last_reviewed: 2026-01-01
---

# Modele de paquet de lineage d'ancrage Taikai (SN13-C)

L'element de roadmap **SN13-C - Manifests & SoraNS anchors** exige que chaque rotation d'alias
livre un bundle de preuves deterministe. Copiez ce modele dans votre repertoire d'artefacts de
rollout (par exemple
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) et remplacez les placeholders
avant de soumettre le paquet a la gouvernance.

## 1. Metadonnees

| Champ | Valeur |
|-------|--------|
| ID d'evenement | `<taikai.event.launch-2026-07-10>` |
| Stream / rendition | `<main-stage>` |
| Namespace / nom d'alias | `<sora / docs>` |
| Repertoire de preuves | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| Contact operateur | `<name + email>` |
| Ticket GAR / RPT | `<governance ticket or GAR digest>` |

## Helper de bundle (optionnel)

Copiez les artefacts du spool et emettez un resume JSON (optionnellement signe) avant de
remplir les sections restantes :

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

Le helper extrait `taikai-anchor-request-*`, `taikai-trm-state-*`, `taikai-lineage-*`,
envelopes et sentinels du repertoire de spool Taikai
(`config.da_ingest.manifest_store_dir/taikai`) afin que le dossier de preuves contienne deja
les fichiers exacts references ci-dessous.

## 2. Ledger de lineage et hint

Joignez le ledger de lineage sur disque ainsi que le JSON de hint que Torii a ecrit pour
cette fenetre. Ils proviennent directement de
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` et
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`.

| Artefact | Fichier | SHA-256 | Notes |
|----------|---------|---------|-------|
| Ledger de lineage | `taikai-trm-state-docs.json` | `<sha256>` | Prouve le digest/la fenetre du manifeste precedent. |
| Hint de lineage | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | Capture avant l'envoi vers l'ancrage SoraNS. |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. Capture du payload d'ancrage

Enregistrez le payload POST que Torii a envoye au service d'ancrage. Le payload inclut
`envelope_base64`, `ssm_base64`, `trm_base64` et l'objet inline `lineage_hint` ; les audits
s'appuient sur cette capture pour prouver le hint envoye a SoraNS. Torii ecrit maintenant ce
JSON automatiquement comme
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
dans le repertoire de spool Taikai (`config.da_ingest.manifest_store_dir/taikai/`), de sorte
que les operateurs puissent le copier directement au lieu d'extraire les logs HTTP.

| Artefact | Fichier | SHA-256 | Notes |
|----------|---------|---------|-------|
| POST d'ancrage | `requests/2026-07-10T18-00Z.json` | `<sha256>` | Requete brute copiee depuis `taikai-anchor-request-*.json` (Taikai spool). |

## 4. Accuse de digest du manifeste

| Champ | Valeur |
|-------|--------|
| Nouveau digest de manifeste | `<hex digest>` |
| Digest du manifeste precedent (depuis le hint) | `<hex digest>` |
| Fenetre debut / fin | `<start seq> / <end seq>` |
| Horodatage d'acceptation | `<ISO8601>` |

Referencez les hashes du ledger/hint enregistres plus haut pour que les reviewers puissent
verifier la fenetre remplacee.

## 5. Metriques / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` snapshot : `<Prometheus query + export path>`
- `/status taikai_alias_rotations` dump (par alias) : `<file path + hash>`

Fournissez l'export Prometheus/Grafana ou la sortie `curl` qui montre l'increment du compteur
et le tableau `/status` pour cet alias.

## 6. Manifeste pour le repertoire de preuves

Generez un manifeste deterministe du repertoire de preuves (fichiers spool, capture de payload,
extractions de metriques) pour que la gouvernance puisse verifier chaque hash sans extraire
l'archive.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| Artefact | Fichier | SHA-256 | Notes |
|----------|---------|---------|-------|
| Manifeste de preuves | `manifest.json` | `<sha256>` | Joindre au paquet de gouvernance / GAR. |

## 7. Checklist

- [ ] Ledger de lineage copie + hashe.
- [ ] Hint de lineage copie + hashe.
- [ ] Payload POST d'ancrage capture et hashe.
- [ ] Tableau de digest du manifeste rempli.
- [ ] Snapshots de metriques exportes (`taikai_trm_alias_rotations_total`, `/status`).
- [ ] Manifeste genere avec `scripts/repo_evidence_manifest.py`.
- [ ] Paquet charge dans la gouvernance avec hashes + info de contact.

Maintenir ce modele pour chaque rotation d'alias rend le bundle de gouvernance SoraNS
reproductible et relie les hints de lineage directement aux preuves GAR/RPT.
