---
lang: pt
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-11-21T18:09:53.463728+00:00"
translation_last_reviewed: 2026-01-01
---

# Modelo de pacote de linhagem de ancora Taikai (SN13-C)

O item do roadmap **SN13-C - Manifests & SoraNS anchors** exige que cada rotacao de alias
entregue um bundle de evidencias deterministico. Copie este modelo para o diretorio de
artefatos de rollout (por exemplo
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) e substitua os placeholders
antes de enviar o pacote para a governanca.

## 1. Metadados

| Campo | Valor |
|-------|-------|
| ID do evento | `<taikai.event.launch-2026-07-10>` |
| Stream / rendition | `<main-stage>` |
| Namespace / nome do alias | `<sora / docs>` |
| Diretorio de evidencia | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| Contato do operador | `<name + email>` |
| Ticket GAR / RPT | `<governance ticket or GAR digest>` |

## Helper de bundle (opcional)

Copie os artefatos do spool e emita um resumo JSON (opcionalmente assinado) antes de
preencher as secoes restantes:

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

O helper extrai `taikai-anchor-request-*`, `taikai-trm-state-*`, `taikai-lineage-*`,
envelopes e sentinels do diretorio spool Taikai
(`config.da_ingest.manifest_store_dir/taikai`) para que a pasta de evidencia ja contenha os
arquivos exatos referenciados abaixo.

## 2. Ledger de linhagem e hint

Anexe tanto o ledger de linhagem em disco quanto o hint JSON que Torii escreveu para esta
janela. Eles vem diretamente de
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` e
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`.

| Artefato | Arquivo | SHA-256 | Notas |
|----------|---------|---------|-------|
| Ledger de linhagem | `taikai-trm-state-docs.json` | `<sha256>` | Prova o digest/janela do manifesto anterior. |
| Hint de linhagem | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | Capturado antes de enviar para a ancora SoraNS. |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. Captura do payload de ancora

Registre o payload POST que Torii enviou ao servico de ancora. O payload inclui
`envelope_base64`, `ssm_base64`, `trm_base64` e o objeto inline `lineage_hint`; auditorias
se apoiam nesta captura para comprovar o hint enviado a SoraNS. Torii agora grava este JSON
automaticamente como
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
dentro do diretorio spool Taikai (`config.da_ingest.manifest_store_dir/taikai/`), de modo
que os operadores possam copia-lo diretamente em vez de extrair logs HTTP.

| Artefato | Arquivo | SHA-256 | Notas |
|----------|---------|---------|-------|
| POST do anchor | `requests/2026-07-10T18-00Z.json` | `<sha256>` | Requisicao crua copiada de `taikai-anchor-request-*.json` (Taikai spool). |

## 4. Reconhecimento do digest do manifesto

| Campo | Valor |
|-------|-------|
| Novo digest do manifesto | `<hex digest>` |
| Digest do manifesto anterior (do hint) | `<hex digest>` |
| Janela inicio / fim | `<start seq> / <end seq>` |
| Timestamp de aceitacao | `<ISO8601>` |

Referencie os hashes do ledger/hint registrados acima para que os reviewers possam verificar
a janela substituida.

## 5. Metricas / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` snapshot: `<Prometheus query + export path>`
- `/status taikai_alias_rotations` dump (por alias): `<file path + hash>`

Forneca o export do Prometheus/Grafana ou a saida do `curl` que mostre o incremento do contador
e o array `/status` para este alias.

## 6. Manifesto para o diretorio de evidencia

Gere um manifesto deterministico do diretorio de evidencia (arquivos spool, captura de
payload, snapshots de metricas) para que a governanca possa verificar cada hash sem
descompactar o arquivo.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| Artefato | Arquivo | SHA-256 | Notas |
|----------|---------|---------|-------|
| Manifesto de evidencia | `manifest.json` | `<sha256>` | Anexe isto ao pacote de governanca / GAR. |

## 7. Checklist

- [ ] Ledger de linhagem copiado + hasheado.
- [ ] Hint de linhagem copiado + hasheado.
- [ ] Payload POST do anchor capturado e hasheado.
- [ ] Tabela de digest do manifesto preenchida.
- [ ] Snapshots de metricas exportados (`taikai_trm_alias_rotations_total`, `/status`).
- [ ] Manifesto gerado com `scripts/repo_evidence_manifest.py`.
- [ ] Pacote enviado a governanca com hashes + contato.

Manter este modelo para cada rotacao de alias mantem o bundle de governanca SoraNS
reproduzivel e liga os hints de linhagem diretamente as evidencias GAR/RPT.
