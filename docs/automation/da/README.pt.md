---
lang: pt
direction: ltr
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9e5fce128259ae2b2c40782b3c96c38048fce6f3b4522319bd60b59db87a8252
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Automação do modelo de ameaças de Data Availability (DA-1)

O item DA-1 do roadmap e o `status.md` pedem um loop de automação determinístico
que produza os resumos do modelo de ameaças Norito PDP/PoTR exibidos em
`docs/source/da/threat_model.md` e no espelho do Docusaurus. Este diretório
captura os artefatos referenciados por:

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (que executa `scripts/docs/render_da_threat_model_tables.py`)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## Fluxo

1. **Gere o relatório**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   O resumo JSON registra a taxa simulada de falhas de replicação, limites do
   chunker e quaisquer violações de política detectadas pelo harness PDP/PoTR em
   `integration_tests/src/da/pdp_potr.rs`.
2. **Renderize as tabelas Markdown**
   ```bash
   make docs-da-threat-model
   ```
   Isto executa `scripts/docs/render_da_threat_model_tables.py` para reescrever
   `docs/source/da/threat_model.md` e `docs/portal/docs/da/threat-model.md`.
3. **Arquive o artefato** copiando o relatório JSON (e o log opcional da CLI)
   para `docs/automation/da/reports/<timestamp>-threat_model_report.json`. Quando
   decisões de governança dependerem de uma execução específica, inclua o hash do
   commit e a seed do simulador em um `<timestamp>-metadata.md` adjacente.

## Expectativas de evidência

- Arquivos JSON devem permanecer <100 KiB para caberem no git. Rastros maiores
  devem ficar em armazenamento externo; referencie o hash assinado na nota de
  metadados quando necessário.
- Cada arquivo arquivado deve listar a seed, o caminho de configuração e a versão
  do simulador para que as reexecuções sejam reproduzíveis.
- Vincule o arquivo arquivado em `status.md` ou na entrada do roadmap sempre que
  os critérios de aceitação DA-1 avançarem, garantindo que revisores possam
  verificar o baseline sem reexecutar o harness.

## Reconciliação de compromissos (omissão do sequenciador)

Use `cargo xtask da-commitment-reconcile` para comparar recibos de ingestão DA
com registros de compromissos DA, detectando omissão ou adulteração do
sequenciador:

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- Aceita recibos em Norito ou JSON e compromissos de `SignedBlockWire`, `.norito`
  ou bundles JSON.
- Falha quando qualquer ticket está ausente no log de blocos ou quando os hashes
  divergem; `--allow-unexpected` ignora tickets apenas-em-bloco quando você
  intencionalmente restringe o conjunto de recibos.
- Anexe o JSON emitido a pacotes de governança/Alertmanager para alertas de
  omissão; o padrão é `artifacts/da/commitment_reconciliation.json`.

## Auditoria de privilégios (revisão trimestral de acesso)

Use `cargo xtask da-privilege-audit` para escanear os diretórios de manifest/replay
DA (com caminhos extras opcionais) em busca de entradas ausentes, não-diretórios
ou com permissões world-writable:

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- Lê os caminhos de ingestão DA da configuração do Torii e inspeciona permissões
  Unix quando disponíveis.
- Sinaliza caminhos ausentes/não-diretórios/world-writable e retorna um código de
  saída não zero quando há problemas.
- Assine e anexe o bundle JSON (`artifacts/da/privilege_audit.json` por padrão)
  a pacotes e dashboards de revisão trimestral.
