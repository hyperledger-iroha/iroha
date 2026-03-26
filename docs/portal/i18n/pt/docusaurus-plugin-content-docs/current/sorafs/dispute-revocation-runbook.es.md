---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: runbook de revogação de disputa
título: Runbook de disputas e revogações de SoraFS
sidebar_label: Runbook de disputas e revogações
descrição: Fluxo de governo para apresentar disputas de capacidade de SoraFS, coordenar revogações e evacuar dados de forma determinista.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/dispute_revocation_runbook.md`. Mantenha ambas as cópias sincronizadas até que a documentação herdada do Sphinx seja retirada.
:::

## Propósito

Este runbook orienta os operadores de governança para apresentar disputas de capacidade de SoraFS, coordenar revogações e garantir que a evacuação de dados seja concluída de forma determinista.

## 1. Avaliar o incidente

- **Condições de ativação:** detecção de incumprimento de SLA (tempo de atividade/falha de PoR), déficit de replicação ou cancelamento de faturação.
- **Confirmar telemetria:** captura snapshots de `/v1/sorafs/capacity/state` e `/v1/sorafs/capacity/telemetry` para o provedor.
- **Notificar as partes interessadas:** Equipe de armazenamento (operações do fornecedor), Conselho de governança (órgão decisório), Observabilidade (atualizações de painéis).

## 2. Preparando o pacote de evidências

1. Recopila artefatos brutos (JSON de telemetria, logs de CLI, notas de auditoria).
2. Normaliza em um arquivo determinista (por exemplo, um tarball); registrar:
   - digerir BLAKE3-256 (`evidence_digest`)
   - tipo de mídia (`application/zip`, `application/jsonl`, etc.)
   - URI de alojamiento (armazenamento de objetos, pino de SoraFS ou endpoint acessível por Torii)
3. Guarde o pacote no balde de coleta de evidências de governo com acesso exclusivo à escritura.

## 3. Apresentar a disputa

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

2. Execute a CLI:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=<i105-account-id> \
     --private-key=ed25519:<key>
   ```

3. Revisão `dispute_summary.json` (confirma tipo, resumo de evidências e carimbos de data e hora).
4. Envie o JSON da solicitação para Torii `/v1/sorafs/capacity/dispute` através da cola de transações de governo. Captura o valor da resposta `dispute_id_hex`; anular as ações de revogação posteriores e os relatórios de auditoria.

## 4. Evacuação e revogação

1. **Ventana de graça:** notificação ao provedor sobre a revogação iminente; permite a evacuação de dados fixados desde que a política o permita.
2. **Geração `ProviderAdmissionRevocationV1`:**
   - Usa `sorafs_manifest_stub provider-admission revoke` com razão aprovada.
   - Verifique as firmas e o resumo da revogação.
3. **Publicar a revogação:**
   - Envie a solicitação de revogação para Torii.
   - Certifique-se de que os anúncios do provedor estejam bloqueados (se espera que `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` aumentem).
4. **Atualizar painéis:** marque o provedor como revogado, faça referência ao ID da disputa e coloque o pacote de evidências.

## 5. Post-mortem e acompanhamento- Registre a linha de tempo, a causa raiz e as ações de remediação no rastreador de incidentes de governo.
- Determinar a restituição (redução de participação, reembolso de comissões, reembolso a clientes).
- Documenta aprendizagem; atualiza limites de SLA ou alertas de monitoramento, se necessário.

## 6. Materiais de referência

-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (seção de disputas)
- `docs/source/sorafs/provider_admission_policy.md` (fluxo de revogação)
- Painel de observação: `SoraFS / Capacity Providers`

## Lista de verificação

- [ ] Pacote de evidências capturadas e hashhead.
- [ ] Payload de disputa validado localmente.
- [ ] Transação de disputa em Torii aceita.
- [ ] Revogação executada (se for aprovada).
- [ ] Dashboards/runbooks atualizados.
- [ ] Post-mortem apresentado antes do conselho de governo.