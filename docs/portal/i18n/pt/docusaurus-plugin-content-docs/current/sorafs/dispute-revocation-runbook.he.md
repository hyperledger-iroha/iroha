---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/sorafs/dispute-revocation-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 041673bed38d2a7f1643db45312e0dd1766facc5cd68bf678ac5ae88d3ec7ebe
source_last_modified: "2026-01-22T15:55:18+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
id: dispute-revocation-runbook
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fonte canonica
Esta pagina reflete `docs/source/sorafs/dispute_revocation_runbook.md`. Mantenha ambas as copias sincronizadas ate que a documentacao Sphinx herdada seja retirada.
:::

## Proposito

Este runbook guia operadores de governanca na abertura de disputas de capacidade da SoraFS, na coordenacao de revogacoes e em garantir que a evacuacao de dados seja concluida de forma deterministica.

## 1. Avaliar o incidente

- **Condicoes de gatilho:** deteccao de violacao de SLA (uptime/falha de PoR), deficit de replicacao ou divergencia de cobranca.
- **Confirmar telemetria:** capture snapshots de `/v1/sorafs/capacity/state` e `/v1/sorafs/capacity/telemetry` do provedor.
- **Notificar partes interessadas:** Storage Team (operacoes do provedor), Governance Council (orgao decisor), Observability (atualizacoes de dashboards).

## 2. Preparar o pacote de evidencias

1. Colete artefatos brutos (telemetry JSON, logs de CLI, notas de auditoria).
2. Normalize em um arquivo deterministico (por exemplo, um tarball); registre:
   - digest BLAKE3-256 (`evidence_digest`)
   - tipo de midia (`application/zip`, `application/jsonl`, etc.)
   - URI de hospedagem (object storage, pin da SoraFS ou endpoint acessivel via Torii)
3. Armazene o pacote no bucket de evidencias da governanca com acesso write-once.

## 3. Registrar a disputa

1. Crie um JSON spec para `sorafs_manifest_stub capacity dispute`:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. Execute a CLI:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=ih58... \
     --private-key=ed25519:<key>
   ```

3. Revise `dispute_summary.json` (confirme tipo, digest das evidencias e timestamps).
4. Envie o JSON da requisicao para Torii `/v1/sorafs/capacity/dispute` via fila de transacoes de governanca. Capture o valor de resposta `dispute_id_hex`; ele ancora as acoes de revogacao posteriores e os relatorios de auditoria.

## 4. Evacuacao e revogacao

1. **Janela de graca:** notifique o provedor sobre a revogacao iminente; permita a evacuacao dos dados fixados quando a politica permitir.
2. **Gere `ProviderAdmissionRevocationV1`:**
   - Use `sorafs_manifest_stub provider-admission revoke` com o motivo aprovado.
   - Verifique assinaturas e o digest de revogacao.
3. **Publique a revogacao:**
   - Envie a requisicao de revogacao para Torii.
   - Garanta que os adverts do provedor estejam bloqueados (espera-se que `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` aumente).
4. **Atualize dashboards:** marque o provedor como revogado, referencie o ID da disputa e vincule o pacote de evidencias.

## 5. Post-mortem e acompanhamento

- Registre a linha do tempo, a causa raiz e as acoes de remediacao no tracker de incidentes de governanca.
- Determine a restitucao (slashing de stake, clawbacks de taxas, reembolsos aos clientes).
- Documente os aprendizados; atualize limites de SLA ou alertas de monitoramento se necessario.

## 6. Materiais de referencia

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (secao de disputas)
- `docs/source/sorafs/provider_admission_policy.md` (fluxo de revogacao)
- Dashboard de observabilidade: `SoraFS / Capacity Providers`

## Checklist

- [ ] Pacote de evidencias capturado e hasheado.
- [ ] Payload da disputa validado localmente.
- [ ] Transacao de disputa no Torii aceita.
- [ ] Revogacao executada (se aprovada).
- [ ] Dashboards/runbooks atualizados.
- [ ] Post-mortem arquivado junto ao conselho de governanca.
