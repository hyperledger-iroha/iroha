---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: runbook de revogação de disputa
título: Runbook de litígios e revogações SoraFS
sidebar_label: Runbook litígios e revogações
descrição: Fluxo de governança para depor litígios de capacidade SoraFS, coordenar revogações e evacuar dados de forma determinada.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/dispute_revocation_runbook.md`. Gardez as duas cópias sincronizadas até que a documentação herdada do Sphinx tenha sido retirada.
:::

## Objetivo

Este runbook guia os operadores de governo na criação de litígios de capacidade SoraFS, na coordenação de revogações e na garantia de uma evacuação determinada de donativos.

## 1. Avaliar o incidente

- **Condições de encerramento:** detecção de violação de SLA (disponibilidade/échec PoR), déficit de replicação ou cancelamento de faturação.
- **Confirme a telemetria:** capture os instantâneos `/v1/sorafs/capacity/state` e `/v1/sorafs/capacity/telemetry` para o fornecedor.
- **Notificador das partes interessadas:** Equipe de armazenamento (operações do fornecedor), Conselho de governança (órgão de decisão), Observabilidade (mises à jour des dashboards).

## 2. Preparando o pacote de testes

1. Colete os artefatos brutos (JSON de telemetria, CLI de registros, notas de auditoria).
2. Normalizador em um arquivo determinado (por exemplo, um tarball); expedidor:
   - digerir BLAKE3-256 (`evidence_digest`)
   - tipo de mídia (`application/zip`, `application/jsonl`, etc.)
   - URI de armazenamento (armazenamento de objetos, pin SoraFS ou endpoint acessível via Torii)
3. Armazene o pacote no balde de coleta de pré-governança com um acesso de gravação única.

## 3. Depor o litígio

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

2. Lance a CLI:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=soraカタカナ... \
     --private-key=ed25519:<key>
   ```

3. Verifique `dispute_summary.json` (confirme o tipo, o resumo das recomendações, os carimbos de data e hora).
4. Insira o JSON de solicitação em Torii `/v1/sorafs/capacity/dispute` por meio do arquivo de transações de governo. Capture o valor de resposta `dispute_id_hex` ; elle ancre les ações de revogação seguintes e os relatórios de auditoria.

## 4. Evacuação e revogação

1. **Fenêtre de grâce :** avisa o fornecedor da revogação iminente; autorize a evacuação de dados pinglées quando a política permitir.
2. **Générez `ProviderAdmissionRevocationV1`:**
   - Utilize `sorafs_manifest_stub provider-admission revoke` com o motivo aprovado.
   - Verifique as assinaturas e o resumo da revogação.
3. **Publiez la révocation :**
   - Envie o pedido de revogação para Torii.
   - Certifique-se de que os anúncios do fornecedor estejam bloqueados (assista a uma casa de `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **Atualize os painéis:** sinalize o fornecedor como revogado, consulte o ID do litígio e leia o pacote de perguntas.

## 5. Post-mortem e suivi- Registrar a cronologia, a causa racine e as ações de remediação no rastreador de incidentes de governo.
- Determinar a restituição (corte de participação, reembolsos de dinheiro, reembolsos de clientes).
- Documente as lecções; atualize seus SLAs ou alertas de monitoramento, se necessário.

## 6. Documentos de referência

-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (seção de litígios)
- `docs/source/sorafs/provider_admission_policy.md` (fluxo de trabalho de revogação)
- Painel de observabilidade: `SoraFS / Capacity Providers`

## Lista de verificação

- [ ] Pacote de amostras capturadas e capturadas.
- [] Carga útil do local válido do litígio.
- [ ] Transação judicial Torii aceita.
- [ ] Révocation exécutée (si approuvée).
- [] Painéis/runbooks atualizados.
- [ ] Deposição post-mortem após o conselho de governo.