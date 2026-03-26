---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: runbook de revogação de disputa
título: Ранбук споров e отзывов SoraFS
sidebar_label: Ранбук споров e отзывов
description: Governança profissional para подачи споров по емкости SoraFS, координации отзывов и детерминированной эвакуации данных.
---

:::nota História Canônica
Esta página contém `docs/source/sorafs/dispute_revocation_runbook.md`. Faça uma cópia da sincronização, mas a documentação do Sphinx não está disponível para download.
:::

## Abençoado

Este é um conjunto de operações de governança que pode fornecer suporte para empresas SoraFS, coordenação de implementação e otimização детерминированной эвакуации данных.

## 1. Incidente de ocorrência

- **Trigger:** você usa SLA de tempo de atividade/PoR de atividade, configuração de replicação ou configuração de faturamento.
- **Termômetro:** Gere snapshots `/v1/sorafs/capacity/state` e `/v1/sorafs/capacity/telemetry` para prova.
- **Уведомить стейкхолдеров:** Equipe de Armazenamento (операции провайдера), Conselho de Governança (орган решения), Observabilidade (обновления дашбордов).

## 2. Baixe o pacote de download

1. Crie artefatos de segurança (JSON de telemetria, CLI de log, auditor de segurança).
2. Normalizar arquivos de determinação (por exemplo, tarball); зафиксируйте:
   - digerir BLAKE3-256 (`evidence_digest`)
   - tipo de mídia (`application/zip`, `application/jsonl` e etc.)
   - URI размещения (object storage, SoraFS pin ou endpoint, доступный через Torii)
3. Organize um pacote no balde de evidências de governança fornecido com gravação única.

## 3. Esporte esportivo

1. Altere a especificação JSON para `sorafs_manifest_stub capacity dispute`:

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

2. Abra CLI:

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

3. Verifique `dispute_summary.json` (tipo de resumo, resumo de dados e métodos atuais).
4. Abra o JSON em Torii `/v1/sorafs/capacity/dispute` para executar o processamento de governança. A chave de segurança é `dispute_id_hex`; оно якорит последующие действия по отзыву и аудиторские отчеты.

## 4. Evacuação e avaliação

1. **Окно льготы:** уведомите провайдера о грядущем отзыве; разрешите эвакуацию закрепленных данных, когда это допускает политика.
2. **Сгенерируйте `ProviderAdmissionRevocationV1`:**
   - Use `sorafs_manifest_stub provider-admission revoke` com seu próprio fornecedor.
   - Проверьте подписи и digerir отзыва.
3. **Exibir opções:**
   - Execute a operação no Torii.
   - Certifique-se de que os anúncios sejam testados (na posição `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **Exibir dados:** verifique o provedor como abrir, inserir IDs de conteúdo e usar a opção de login no pacote доказательств.

## 5. Post-mortem e destino final

- Зафиксируйте таймлайн, корневую причину e меры ремедиации в трекере инцидентов governança.
- Определите реституцию (slashing stake, grabbacks комиссий, возвраты клиентам).
- Документируйте выводы; verifique o SLA ou monitore alertas sobre problemas de segurança.

## 6. Materiais de expansão

-`sorafs_manifest_stub capacity dispute --help`
-`docs/source/sorafs/storage_capacity_marketplace.md` (esporos)
- `docs/source/sorafs/provider_admission_policy.md` (fluxo de trabalho definido)
- Número de identificação: `SoraFS / Capacity Providers`

## Verifique- [ ] Пакет доказательств собран и захеширован.
- [ ] Payload é validado localmente.
- [ ] Torii-транзакция спора принята.
- [ ] Отзыв выполнен (если одобрен).
- [ ] Дашборды/ранбуки обновлены.
- [ ] Post-mortem оформлен в совете governança.