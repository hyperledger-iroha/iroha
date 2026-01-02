---
lang: pt
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/06-verification-report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf489427d0eba2beebfdefc44092730c3963cbd77e83669853f4e9681ac9fd2d
source_last_modified: "2025-11-21T14:25:28.923348+00:00"
translation_last_reviewed: 2026-01-01
---

## Relatorio de verificacao do operador (fase T0)

- Nome do operador: ______________________
- ID do descriptor do relay: ______________________
- Data de envio (UTC): ___________________
- Email / matrix de contato: ___________________

### Resumo do checklist

| Item | Concluido (S/N) | Notas |
|------|-----------------|-------|
| Hardware e rede validados | | |
| Bloco de compliance aplicado | | |
| Envelope de admissao verificado | | |
| Smoke test de guard rotation | | |
| Telemetria coletada e dashboards ativos | | |
| Brownout drill executado | | |
| Sucesso de tickets PoW dentro da meta | | |

### Snapshot de metricas

- Ratio PQ (`sorafs_orchestrator_pq_ratio`): ________
- Contagem de downgrade ultimas 24h: ________
- RTT medio de circuitos (p95): ________ ms
- Tempo mediano de resolucao PoW: ________ ms

### Anexos

Por favor anexe:

1. Hash do support bundle do relay (`sha256`): __________________________
2. Capturas de dashboards (ratio PQ, sucesso de circuitos, histograma PoW).
3. Bundle de drill assinado (`drills-signed.json` + chave publica do signatario em hex e anexos).
4. Relatorio de metricas SNNet-10 (`cargo xtask soranet-testnet-metrics --input <snapshot> --out metrics-report.json`).

### Assinatura do operador

Certifico que as informacoes acima estao corretas e que todos os passos obrigatorios foram concluidos.

Assinatura: _________________________  Data: ___________________
