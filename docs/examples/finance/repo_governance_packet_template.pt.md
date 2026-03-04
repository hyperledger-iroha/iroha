---
lang: pt
direction: ltr
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fe5fb47af37f86d33bfa884dc920efbf66714bbe3535842a786755dd5649f65
source_last_modified: "2025-12-07T08:26:38.035018+00:00"
translation_last_reviewed: 2026-01-01
---

# Template de pacote de governance de repo (Roadmap F1)

Use este template ao preparar o bundle de artefatos exigido pelo item da roadmap
F1 (documentacao e tooling do ciclo de vida de repo). O objetivo e entregar aos reviewers um
unico arquivo Markdown que liste cada input, hash e bundle de evidencia para que o
conselho de governance possa reproduzir os bytes referenciados na proposta.

> Copie o template para seu proprio diretorio de evidencia (por exemplo
> `artifacts/finance/repo/2026-03-15/packet.md`), substitua os placeholders e
> commite/faca upload ao lado dos artefatos hasheados referenciados abaixo.

## 1. Metadados

| Campo | Valor |
|-------|-------|
| Identificador de acordo/mudanca | `<repo-yyMMdd-XX>` |
| Preparado por / data | `<desk lead> - 2026-03-15T10:00Z` |
| Revisado por | `<dual-control reviewer(s)>` |
| Tipo de mudanca | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
| Custodian(s) | `<custodian id(s)>` |
| Proposta vinculada / referendum | `<governance ticket id or GAR link>` |
| Diretorio de evidencia | ``artifacts/finance/repo/<slug>/`` |

## 2. Payloads de instrucoes

Registre as instrucoes Norito staged que os desks aprovaram via
`iroha app repo ... --output`. Cada entrada deve incluir o hash do arquivo emitido
e uma descricao curta da acao que sera submetida quando o voto passar.

| Acao | Arquivo | SHA-256 | Notas |
|--------|------|---------|-------|
| Initiate | `instructions/initiate.json` | `<sha256>` | Contem as pernas cash/collateral aprovadas por desk + counterparty. |
| Margin call | `instructions/margin_call.json` | `<sha256>` | Captura cadence + participant id que disparou a chamada. |
| Unwind | `instructions/unwind.json` | `<sha256>` | Prova da reverse-leg quando as condicoes forem atendidas. |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json       | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 Custodian Acknowledgements (tri-party only)

Preencha esta secao sempre que um repo usar `--custodian`. O pacote de governance
deve incluir um acknowledgement assinado de cada custodian e o hash do arquivo
referenciado em sec 2.8 de `docs/source/finance/repo_ops.md`.

| Custodian | Arquivo | SHA-256 | Notas |
|-----------|------|---------|-------|
| `<ih58...>` | `custodian_ack_<custodian>.md` | `<sha256>` | SLA assinado cobrindo janela de custody, conta de roteamento e contato de drill. |

> Armazene o acknowledgement ao lado das outras evidencias (`artifacts/finance/repo/<slug>/`)
> para que `scripts/repo_evidence_manifest.py` registre o arquivo na mesma arvore que
> as instrucoes staged e os snippets de config. Veja
> `docs/examples/finance/repo_custodian_ack_template.md` para um template pronto que
> corresponde ao contrato de evidencias de governance.

## 3. Snippet de configuracao

Cole o bloco TOML `[settlement.repo]` que sera aplicado no cluster (incluindo
`collateral_substitution_matrix`). Guarde o hash ao lado do snippet para que os auditors
confirmem a politica runtime ativa quando o booking do repo foi aprovado.

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 Snapshots de configuracao pos-aprovacao

Depois que o referendum ou voto de governance for concluido e a mudanca `[settlement.repo]`
for aplicada, capture snapshots de `/v1/configuration` de cada peer para que os auditors
comprovem que a politica aprovada esta ativa no cluster (ver
`docs/source/finance/repo_ops.md` sec 2.9 para o workflow de evidencias).

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v1/configuration       | jq '.'       > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| Peer / source | Arquivo | SHA-256 | Block height | Notas |
|---------------|------|---------|--------------|-------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | Snapshot capturado logo apos o rollout da config. |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | Confirma que `[settlement.repo]` corresponde ao TOML staged. |

Registre os digests junto aos peer ids em `hashes.txt` (ou resumo equivalente)
para que os reviewers possam rastrear quais nodes ingeriram a mudanca. Os snapshots
ficam em `config/peers/` ao lado do snippet TOML e serao coletados automaticamente por
`scripts/repo_evidence_manifest.py`.

## 4. Artefatos de testes deterministas

Anexe os ultimos outputs de:

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

Registre caminhos de arquivos + hashes para bundles de logs ou JUnit XML gerados pelo CI.

| Artefato | Arquivo | SHA-256 | Notas |
|----------|------|---------|-------|
| Log de lifecycle proof | `tests/repo_lifecycle.log` | `<sha256>` | Capturado com `--nocapture`. |
| Log de integration test | `tests/repo_integration.log` | `<sha256>` | Inclui cobertura de substitution + cadence de margem. |

## 5. Snapshot de lifecycle proof

Todo pacote deve incluir o snapshot determinista exportado de
`repo_deterministic_lifecycle_proof_matches_fixture`. Execute o harness com os
knobs de export habilitados para que os reviewers comparem o frame JSON e o
digest com a fixture em `crates/iroha_core/tests/fixtures/` (ver
`docs/source/finance/repo_ops.md` sec 2.7).

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json     REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt     cargo test -p iroha_core       -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

Ou use o helper fixo para regenerar as fixtures e copia-las para o bundle de evidencias em um passo:

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain>       --bundle-dir artifacts/finance/repo/<slug>
```

| Artefato | Arquivo | SHA-256 | Notas |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | Frame de lifecycle canonico emitido pelo proof harness. |
| Digest file | `repo_proof_digest.txt` | `<sha256>` | Digest hex em uppercase espelhado de `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`; anexe mesmo quando inalterado. |

## 6. Evidence Manifest

Gere o manifest do diretorio de evidencias para que os auditors possam verificar hashes
sem descompactar o arquivo. O helper espelha o workflow descrito em
`docs/source/finance/repo_ops.md` sec 3.2.

```bash
python3 scripts/repo_evidence_manifest.py       --root artifacts/finance/repo/<slug>       --agreement-id <repo-identifier>       --output artifacts/finance/repo/<slug>/manifest.json
```

| Artefato | Arquivo | SHA-256 | Notas |
|----------|------|---------|-------|
| Evidence manifest | `manifest.json` | `<sha256>` | Inclua o checksum no ticket de governance / notas do referendum. |

## 7. Snapshot de telemetria e eventos

Exporte as entradas relevantes `AccountEvent::Repo(*)` e quaisquer dashboards ou exports CSV
referenciados em `docs/source/finance/repo_ops.md`. Registre os arquivos + hashes aqui para que
os reviewers possam ir direto as evidencias.

| Export | Arquivo | SHA-256 | Notas |
|--------|------|---------|-------|
| Repo events JSON | `evidence/repo_events.ndjson` | `<sha256>` | Stream Torii bruto filtrado para contas de desk. |
| Telemetry CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | Exportado do Grafana usando o painel Repo Margin. |

## 8. Aprovacoes e assinaturas

- **Signatarios dual-control:** `<names + timestamps>`
- **GAR / minutes digest:** `<sha256>` do PDF GAR assinado ou upload de minutes.
- **Local de storage:** `governance://finance/repo/<slug>/packet/`

## 9. Checklist

Marque cada item quando estiver completo.

- [ ] Payloads de instrucoes staged, hasheados e anexados.
- [ ] Hash do snippet de configuracao registrado.
- [ ] Logs de testes deterministas capturados + hasheados.
- [ ] Snapshot de lifecycle + digest exportado.
- [ ] Evidence manifest gerado e hash registrado.
- [ ] Exports de eventos/telemetria capturados + hasheados.
- [ ] Acknowledgements dual-control arquivados.
- [ ] GAR/minutes enviados; digest registrado acima.

Manter este template junto a cada pacote mantem o DAG de governance determinista e fornece aos auditors um manifest portavel para decisoes do ciclo de vida de repo.
