---
lang: pt
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d15f10764ee430825f4d7caa142c6664016d3934ce7c0da68314a4a4a6f700bd
source_last_modified: "2025-11-15T15:21:14.374476+00:00"
translation_last_reviewed: 2026-01-01
---

:::note Fonte canonica
Esta pagina reflete `docs/source/sns/address_checksum_failure_runbook.md`. Atualize primeiro o arquivo fonte e depois sincronize esta copia.
:::

Falhas de checksum aparecem como `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`) em Torii, SDKs e clientes wallet/explorer. Os itens de roadmap ADDR-6/ADDR-7 agora exigem que os operadores sigam este runbook sempre que alertas de checksum ou tickets de suporte forem acionados.

## Quando executar o play

- **Alertas:** `AddressInvalidRatioSlo` (definido em `dashboards/alerts/address_ingest_rules.yml`) dispara e as anotacoes listam `reason="ERR_CHECKSUM_MISMATCH"`.
- **Deriva de fixtures:** O textfile Prometheus `account_address_fixture_status` ou o dashboard Grafana reporta um checksum mismatch para qualquer copia de SDK.
- **Escalacoes de suporte:** Times de wallet/explorer/SDK citam erros de checksum, corrupcao de IME ou scans da area de transferencia que nao decodificam mais.
- **Observacao manual:** Logs do Torii mostram repetidamente `address_parse_error=checksum_mismatch` para endpoints de producao.

Se o incidente for especificamente sobre colisoes Local-8/Local-12, siga os playbooks `AddressLocal8Resurgence` ou `AddressLocal12Collision`.

## Checklist de evidencia

| Evidencia | Comando / Localizacao | Notas |
|----------|-----------------------|-------|
| Snapshot Grafana | `dashboards/grafana/address_ingest.json` | Captura a divisao das razoes invalidas e os endpoints afetados. |
| Payload de alerta | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | Inclua labels de contexto e timestamps. |
| Saude dos fixtures | `artifacts/account_fixture/address_fixture.prom` + Grafana | Prova se as copias de SDK divergiram de `fixtures/account/address_vectors.json`. |
| Consulta PromQL | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | Exporte CSV para o doc do incidente. |
| Logs | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` (ou agregacao de logs) | Limpe PII antes de compartilhar. |
| Verificacao do fixture | `cargo xtask address-vectors --verify` | Confirma que o gerador canonico e o JSON commitado batem. |
| Checagem de paridade SDK | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | Execute para cada SDK reportado em alertas/tickets. |
| Sanidade de area de transferencia/IME | `iroha address inspect <literal>` | Detecta caracteres ocultos ou reescritas IME; cite `address_display_guidelines.md`. |

## Resposta imediata

1. Reconheca o alerta, vincule snapshots de Grafana + saida PromQL no thread do incidente, e anote os contextos Torii afetados.
2. Congele promocoes de manifest / releases de SDK que mexam no parsing de enderecos.
3. Guarde snapshots de dashboard e os artefatos textfile Prometheus gerados na pasta do incidente (`docs/source/sns/incidents/YYYY-MM/<ticket>/`).
4. Colete amostras de logs mostrando payloads `checksum_mismatch`.
5. Notifique os owners de SDK (`#sdk-parity`) com payloads de exemplo para que possam fazer triage.

## Isolamento da causa raiz

### Deriva de fixture ou do gerador

- Rode `cargo xtask address-vectors --verify` novamente; regenere se falhar.
- Execute `ci/account_fixture_metrics.sh` (ou `scripts/account_fixture_helper.py check` individualmente) para cada SDK e confirme que os fixtures incluidos batem com o JSON canonico.

### Regressoes de codificadores de cliente / IME

- Inspecione literais fornecidos por usuarios via `iroha address inspect` para encontrar joins de largura zero, conversoes kana ou payloads truncados.
- Cruze os fluxos de wallet/explorer com `docs/source/sns/address_display_guidelines.md` (metas de copia dupla, avisos, helpers QR) para garantir que seguem a UX aprovada.

### Problemas de manifest ou registro

- Siga `address_manifest_ops.md` para revalidar o ultimo manifest bundle e garantir que nenhum seletor Local-8 reapareceu.

### Trafego malicioso ou malformado

- Quebre IPs/app IDs ofensores via logs do Torii e `torii_http_requests_total`.
- Preserve pelo menos 24 horas de logs para acompanhamento de Security/Governance.

## Mitigacao e recuperacao

| Cenario | Acoes |
|---------|-------|
| Deriva de fixture | Regenere `fixtures/account/address_vectors.json`, rode novamente `cargo xtask address-vectors --verify`, atualize bundles de SDK, e anexe snapshots de `address_fixture.prom` ao ticket. |
| Regressao de SDK/cliente | Abra issues referenciando o fixture canonico + saida de `iroha address inspect`, e bloqueie releases com a CI de paridade SDK (por exemplo `ci/check_address_normalize.sh`). |
| Envios maliciosos | Aplique rate-limit ou bloqueie principals ofensores, escale para Governance se for necessario tombstone de seletores. |

Depois que as mitigacoes forem aplicadas, rode novamente a consulta PromQL acima para confirmar que `ERR_CHECKSUM_MISMATCH` permanece em zero (excluindo `/tests/*`) por pelo menos 30 minutos antes de baixar o incidente.

## Encerramento

1. Arquive snapshots de Grafana, CSV de PromQL, trechos de logs e `address_fixture.prom`.
2. Atualize `status.md` (secao ADDR) e a linha do roadmap se ferramentas/docs mudarem.
3. Registre notas post-incidente em `docs/source/sns/incidents/` quando surgirem novas licoes.
4. Garanta que as notas de release de SDK mencionem correcoes de checksum quando aplicavel.
5. Confirme que o alerta fica verde por 24h e que os checks de fixture permanecem verdes antes de encerrar.
