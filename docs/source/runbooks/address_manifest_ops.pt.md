---
lang: pt
direction: ltr
source: docs/source/runbooks/address_manifest_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb5d84c6939c186ebb4cd1b622e5ab66872349f5c177191c940a9e9fd63d1a17
source_last_modified: "2025-12-14T09:53:36.233782+00:00"
translation_last_reviewed: 2025-12-28
---

# Runbook de operações do manifesto de endereços (ADDR-7c)

Este runbook operacionaliza o item de roadmap **ADDR-7c** detalhando como
verificar, publicar e aposentar entradas no manifesto de contas/aliases do Sora
Nexus. Ele complementa o contrato técnico em
[`docs/account_structure.md`](../../account_structure.md) §4 e as expectativas
de telemetria registradas em `dashboards/grafana/address_ingest.json`.

## 1. Escopo e entradas

| Entrada | Fonte | Notas |
|-------|--------|-------|
| Bundle de manifesto assinado (`manifest.json`, `manifest.sigstore`, `checksums.sha256`, `notes.md`) | Pin do SoraFS (`sorafs://address-manifests/<CID>/`) e espelho HTTPS | Bundles são emitidos pela automação de release; mantenha a estrutura de diretórios ao espelhar. |
| Digest + sequência do manifesto anterior | Bundle anterior (mesmo padrão de caminho) | Necessário para provar monotonicidade/imutabilidade. |
| Acesso à telemetria | Dashboard `address_ingest` do Grafana + Alertmanager | Necessário para monitorar a retirada de Local‑8 e picos de endereços inválidos. |
| Ferramentas | `cosign`, `shasum`, `b3sum` (ou `python3 -m blake3`), `jq`, CLI `iroha`, `scripts/account_fixture_helper.py` | Instale antes de executar o checklist. |

## 2. Estrutura de artefatos

Todo bundle segue o layout abaixo; não renomeie arquivos ao copiar entre
ambientes.

```
address-manifest-<REVISION>/
├── manifest.json              # canonical JSON (UTF-8, newline-terminated)
├── manifest.sigstore          # Sigstore bundle from `cosign sign-blob`
├── checksums.sha256           # one-line SHA-256 sum for each artifact
└── notes.md                   # change log (reason codes, tickets, owners)
```

Campos de cabeçalho de `manifest.json`:

| Campo | Descrição |
|-------|-----------|
| `version` | Versão do esquema (atualmente `1`). |
| `sequence` | Número de revisão monotônico; deve incrementar exatamente em um. |
| `generated_ms` | Timestamp UTC de publicação (milissegundos desde epoch). |
| `ttl_hours` | Vida máxima de cache que Torii/SDKs podem honrar (padrão 24). |
| `previous_digest` | BLAKE3 do corpo do manifesto anterior (hex). |
| `entries` | Array ordenado de registros (`global_domain`, `local_alias` ou `tombstone`). |

## 3. Procedimento de verificação

1. **Baixar o bundle.**

   ```bash
   export REV=2025-04-12
   sorafs_cli fetch --id sorafs://address-manifests/${REV} --out artifacts/address_manifest_${REV}
   cd artifacts/address_manifest_${REV}
   ```

2. **Guardrail de checksum.**

   ```bash
   shasum -a 256 -c checksums.sha256
   ```

   Todos os arquivos devem reportar `OK`; trate divergências como adulteração.

3. **Verificação Sigstore.**

   ```bash
   cosign verify-blob \
     --bundle manifest.sigstore \
     --certificate-identity-regexp 'governance\.sora\.nexus/addr-manifest' \
     --certificate-oidc-issuer https://accounts.google.com \
     manifest.json
   ```

4. **Prova de imutabilidade.** Compare `sequence` e `previous_digest` com o
   manifesto arquivado:

   ```bash
   jq '.sequence, .previous_digest' manifest.json
   b3sum -l 256 ../address-manifest_<prev>/manifest.json
   ```

   O digest impresso deve casar com `previous_digest`. Não são permitidos saltos
   de sequência; reemita o manifesto se houver violação.

5. **Conformidade de TTL.** Garanta que `generated_ms + ttl_hours` cubra as
   janelas de deploy esperadas; caso contrário a governança deve republicar antes
   do vencimento do cache.

6. **Sanidade das entradas.**
   - Entradas `global_domain` DEVEM incluir `{ "domain": "example", "chain": "sora:nexus:global", "selector": "global" }`.
   - Entradas `local_alias` DEVEM embutir o digest de 12 bytes produzido pelo Norm v1
     (use `iroha address convert <address-or-account_id> --format json --expect-prefix 753`
     para confirmar; o resumo JSON ecoa o domínio fornecido via `input_domain` e
     `--append-domain` reproduz a codificação convertida como `<ih58>@<domain>` para manifestos).
   - Entradas `tombstone` DEVEM referenciar exatamente o selector que será retirado,
     incluir `reason_code`, `ticket` e `replaces_sequence`.

7. **Paridade de fixtures.** Regenere vetores canônicos e assegure que a tabela
   de digests Local não mudou inesperadamente:

   ```bash
   cargo xtask address-vectors
   python3 scripts/account_fixture_helper.py check --quiet
   ```

8. **Guardrail de automação.** Rode o verificador de manifesto para revalidar o
   bundle de ponta a ponta (esquema do cabeçalho, formato das entradas, checksums
   e encadeamento do previous‑digest):

   ```bash
   cargo xtask address-manifest verify \
     --bundle artifacts/address-manifest_2025-05-12 \
     --previous artifacts/address-manifest_2025-04-30
   ```

   A flag `--previous` aponta para o bundle imediatamente anterior para que a
   ferramenta confirme a monotonicidade de `sequence` e recompute a prova BLAKE3
   de `previous_digest`. O comando falha rápido quando um checksum deriva ou um
   selector `tombstone` omite campos obrigatórios, então inclua a saída no ticket
   de mudança antes de solicitar assinaturas.

## 4. Fluxo de mudanças de alias e tombstone

1. **Propor mudança.** Abra um ticket de governança informando o código de razão
   (`LOCAL8_RETIREMENT`, `DOMAIN_REASSIGNED`, etc.) e os selectors afetados.
2. **Derivar payloads canônicos.** Para cada alias a ser atualizado, execute:

   ```bash
   iroha address convert snx1...@wonderland --expect-prefix 753 --format json > /tmp/alias.json
   jq '.canonical_hex, .input_domain' /tmp/alias.json
   ```

3. **Rascunhar entrada de manifesto.** Acrescente um registro JSON como:

   ```json
   {
     "type": "tombstone",
     "selector": { "kind": "local", "digest_hex": "b18fe9c1abbac45b3e38fc5d" },
     "reason_code": "LOCAL8_RETIREMENT",
     "ticket": "ADDR-7c-2025-04-12",
     "replaces_sequence": 36
   }
   ```

   Ao substituir um alias Local por um Global, inclua tanto o registro
   `tombstone` quanto o registro `global_domain` subsequente com o discriminante
   Nexus.

4. **Validar o bundle.** Reexecute os passos de verificação acima no manifesto
   de rascunho antes de pedir assinaturas.
5. **Publicar e monitorar.** Após assinatura da governança, siga §3 e mantenha o
   padrão `true` nos clusters de produção quando as métricas confirmarem zero
   uso de Local‑8. Só altere o flag para `false` em clusters dev/test quando
   precisar de tempo extra de soak.

## 5. Monitoramento e rollback

- Dashboards: `dashboards/grafana/address_ingest.json` (painéis para
  `torii_address_invalid_total{endpoint,reason}`,
  `torii_address_local8_total{endpoint}`,
  `torii_address_collision_total{endpoint,kind="local12_digest"}`, e
  `torii_address_collision_domain_total{endpoint,domain}`) devem ficar verdes
  por 30 dias antes de bloquear permanentemente tráfego Local‑8/Local‑12.
- Evidência de gate: exporte uma consulta de 30 dias do Prometheus para
  `torii_address_local8_total` e `torii_address_collision_total` (ex.:
  `promtool query range --output=json ...`) e execute
  `cargo xtask address-local8-gate --input <file> --json-out artifacts/address_gate.json`;
  anexe o JSON + saída da CLI aos tickets de rollout para que a governança veja
  a janela de cobertura e confirme que os contadores ficaram estáveis.
- Alertas (ver `dashboards/alerts/address_ingest_rules.yml`):
  - `AddressLocal8Resurgence` — pagina quando qualquer contexto reporta novo
    incremento de Local‑8. Trate como bloqueador de release, pause rollouts de
    (substituindo o padrão) até remediar o cliente. Restaure o flag para `true`
    quando a telemetria estiver limpa.
  - `AddressLocal12Collision` — dispara no momento em que dois rótulos Local‑12
    fazem hash para o mesmo digest. Pause promoções de manifesto, execute
    `scripts/address_local_toolkit.sh` para confirmar o mapeamento do digest e
    coordene com a governança Nexus antes de reemitir a entrada afetada.
  - `AddressInvalidRatioSlo` — alerta quando envios IH58/comprimidos inválidos
    (excluindo rejeições Local‑8/strict‑mode) excedem o SLO global de 0,1 % por
    dez minutos. Investigue `torii_address_invalid_total` por contexto/razão e
    coordene com o time SDK proprietário antes de reativar o modo estrito.
- Logs: mantenha linhas de log `manifest_refresh` do Torii e o número do ticket
  de governança em `notes.md`.
- Rollback: republique o bundle anterior (mesmos arquivos, ticket incrementado
  apenas no ambiente afetado até resolver o problema; depois retorne a `true`.

## 6. Referências

- [`docs/account_structure.md`](../../account_structure.md) §§4–4.1 (contrato).
- [`scripts/account_fixture_helper.py`](../../../scripts/account_fixture_helper.py) (sincronização de fixtures).
- [`fixtures/account/address_vectors.json`](../../../fixtures/account/address_vectors.json) (digests canônicos).
- [`dashboards/grafana/address_ingest.json`](../../../dashboards/grafana/address_ingest.json) (telemetria).
