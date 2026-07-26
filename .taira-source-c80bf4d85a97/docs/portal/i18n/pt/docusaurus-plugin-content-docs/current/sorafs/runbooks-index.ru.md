---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: índice de runbooks
título: Индекс операторских ранбуков
sidebar_label: Индекс ранбуков
descrição: Каноническая точка входа для мигрированных операторских ранбуков SoraFS.
---

> Abra o arquivo владельцев, que está localizado em `docs/source/sorafs/runbooks/`.
> A nova versão da operação SoraFS está disponível para uso público em
> сборке портала.

Use este site, isso é garantido, como o serviço irá migrar do local
você precisa de documentação no portal. Каждая запись содержит владельцев, канонический путь источника
e copiando no portal, esses programas podem ser configurados para uma nova operação no período beta.

##Host бета‑превью

O DocOps está disponível para versão beta `https://docs.iroha.tech/`.
Quando você precisa de um operador ou de uma transferência para uma migração, use-o
Então, você está trabalhando no portal de segurança, para obter uma reunião de controle. Produtores
publique/отката находятся em
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Ранбук | Владельцы | Copiar no portal | Estocado |
|--------|-----------|-----------------|----------|
| Gateway de segurança e DNS | TL de rede, automação de operações, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Operação do cabo SoraFS | Documentos/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Caixa de câmbio | Tesouraria / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Operações de restauração de pinos | GT Ferramentaria | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Verifique o funcionamento da operação | Equipe de armazenamento, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Esporos e отзывов | Conselho de Governança | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Encenação de plástico‑manifestos | Documentos/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Наблюдаемость якоря Taikai | GT Plataforma de Mídia / Programa DA / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Verifique as receitas

- [x] Сборка портала ссылается на этот индекс (элемент боковой панели).
- [x] Каждый мигрированный ранбук указывает канонический путь источника, чтобы держать
  ревьюеров согласованными во время ревизии документации.
- [x] Пайплайн предварительного просмотра DocOps блокирует слияния, когда перечисленный
  ранбук отсутствует em seu portal.

Minha migração (por exemplo, nova mudança ou privilégio de atualização) é necessária para a transferência
abra sua tabela e verifique o DocOps, acessando
`docs/examples/docs_preview_request_template.md`.