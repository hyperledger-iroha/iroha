---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: snapshot-prioritário-2025-03
título: Snapshot de prioridades — março de 2025 (Beta)
descrição: Espejo do instantâneo de direção de Nexus 2025-03; pendente de ACKs antes do lançamento público.
---

> Fonte canônica: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> Estado: **Beta / aguardando ACKs de direcionamento** (leads de Networking, Storage, Docs).

## Resumo

O instantâneo de março mantém as iniciativas de docs/content-network alinhadas
com as faixas de entrega de SoraFS (SF-3, SF-6b, SF-9). Uma vez que todos eles
leads confirmam o instantâneo no canal de direção de Nexus, elimine a nota
“Beta” de chegada.

### Hilos de abordagem

1. **Snapshot circular de prioridades** — recopilar agradecimentos e
   registrarlos nas minutas do conselho de 2025-03-05.
2. **Cirque do início do Gateway/DNS** — testar o novo kit de facilitação
   (Seção 6 do runbook) antes do workshop 03/03/2025.
3. **Migração de runbooks de operadoras** — o portal `Runbook Index` está ativo;
   exponha o URL da visualização beta após a assinatura da integração dos revisores.
4. **Hilos de entrega de SoraFS** — alinear o trabalho restante de SF-3/6b/9 com
   el plano/roteiro:
   - Trabalhador de ingestão PoR + endpoint de estado em `sorafs-node`.
   - Pulido de ligações CLI/SDK e integrações de orquestrador Rust/JS/Swift.
   - Cabo de tempo de execução do coordenador PoR e eventos de GovernanceLog.

Consulte o arquivo fonte para a tabela completa, a lista de verificação de distribuição
e as entradas de log.