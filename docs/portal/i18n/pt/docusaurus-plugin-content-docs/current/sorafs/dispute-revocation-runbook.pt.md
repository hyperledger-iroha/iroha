---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: runbook de revogação de disputa
título: Runbook de disputas e revogações da SoraFS
sidebar_label: Runbook de disputas e revogações
descrição: Fluxo de governança para registrar disputas de capacidade da SoraFS, coordenar revogações e evacuar dados de forma determinística.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/dispute_revocation_runbook.md`. Mantenha ambas as cópias sincronizadas até que a documentação Sphinx herdada seja retirada.
:::

## Propósito

Este runbook guia os operadores de governança na abertura de disputas de capacidade da SoraFS, na coordenação de revogações e em garantir que a evacuação de dados seja concluída de forma determinística.

## 1. Avaliar o incidente

- **Condições de gatilho:** detecção de violação de SLA (uptime/falha de PoR), déficit de replicação ou divergência de cobranca.
- **Confirmar telemetria:** capture snapshots de `/v1/sorafs/capacity/state` e `/v1/sorafs/capacity/telemetry` do provedor.
- **Notificar partes interessadas:** Equipe de Armazenamento (operações do provedor), Conselho de Governança (órgão decisor), Observabilidade (atualizações de dashboards).

## 2. Preparando o pacote de evidências

1. Colete artistas brutos (JSON de telemetria, logs de CLI, notas de auditoria).
2. Normalize em um arquivo determinístico (por exemplo, um tarball); registrar:
   - digerir BLAKE3-256 (`evidence_digest`)
   - tipo de mídia (`application/zip`, `application/jsonl`, etc.)
   - URI de hospedagem (object storage, pin da SoraFS ou endpoint acessível via Torii)
3. Armazene o pacote no balde de evidências da governança com acesso write-once.

## 3. Registrador em disputa

1. Crie uma especificação JSON para `sorafs_manifest_stub capacity dispute`:

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

2. Execute uma CLI:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=<katakana-i105-account-id> \
     --private-key=ed25519:<key>
   ```

3. Revise `dispute_summary.json` (confirme tipo, resumo das evidências e carimbos de data/hora).
4. Envie o JSON da requisição para Torii `/v1/sorafs/capacity/dispute` via fila de transações de governança. Capture o valor da resposta `dispute_id_hex`; ele ancora as ações de revogação posteriores e os relatórios de auditoria.

## 4. Evacuação e revogação

1. **Janela de graça:** notifique o provedor sobre uma revogação iminente; permitir a evacuação dos dados estabelecidos quando a política permitir.
2. **Gere `ProviderAdmissionRevocationV1`:**
   - Use `sorafs_manifest_stub provider-admission revoke` com o motivo aprovado.
   - Verificar assinaturas e o resumo de revogação.
3. **Publique a revogação:**
   - Envie uma requisição de revogação para Torii.
   - Garanta que os anúncios do provedor estejam bloqueados (espera-se que `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` aumente).
4. **Atualizar dashboards:** marque o provedor como revogado, referencie o ID da disputa e vincule o pacote de evidências.

## 5. Post-mortem e acompanhamento

- Registre a linha do tempo, a causa raiz e as ações de remediação no tracker de incidentes de governança.
- Determinar uma restituição (redução de participação, reembolso de taxas, reembolsos aos clientes).
- Documente os aprendizados; atualizar limites de SLA ou alertas de monitoramento se necessário.

## 6. Materiais de referência-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (seção de disputas)
- `docs/source/sorafs/provider_admission_policy.md` (fluxo de revogação)
- Painel de observação: `SoraFS / Capacity Providers`

## Lista de verificação

- [ ] Pacote de evidências capturadas e hasheado.
- [ ] Payload da disputa validado localmente.
- [ ] Transação de disputa não Torii aceita.
- [ ] Revogação realizada (se aprovada).
- [ ] Dashboards/runbooks atualizados.
- [ ] Arquivado post-mortem junto ao conselho de governança.