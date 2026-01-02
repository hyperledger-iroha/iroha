---
lang: pt
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-11-04T16:28:48.303168+00:00"
translation_last_reviewed: 2026-01-01
---

# Playbook de resposta a brownout / downgrade

1. **Detectar**
   - O alerta `soranet_privacy_circuit_events_total{kind="downgrade"}` dispara ou o webhook de brownout chega via governance.
   - Confirmar via `kubectl logs soranet-relay` ou systemd journal em 5 min.

2. **Estabilizar**
   - Congelar guard rotation (`relay guard-rotation disable --ttl 30m`).
   - Habilitar override direct-only para clientes afetados
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`).
   - Capturar o hash atual da config de compliance (`sha256sum compliance.toml`).

3. **Diagnosticar**
   - Coletar o snapshot mais recente do directory e o bundle de metricas do relay:
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - Anotar profundidade da fila PoW, contadores de throttling e picos de categoria GAR.
   - Identificar se o evento foi causado por deficit de PQ, override de compliance ou falha do relay.

4. **Escalar**
   - Notificar a ponte de governance (`#soranet-incident`) com resumo e hash do bundle.
   - Abrir ticket de incidente linkando o alerta, incluindo timestamps e passos de mitigacao.

5. **Recuperar**
   - Uma vez resolvida a causa raiz, reativar a rotation
     (`relay guard-rotation enable`) e reverter overrides direct-only.
   - Monitorar KPIs por 30 minutos; garantir que nao aparecam novos brownouts.

6. **Postmortem**
   - Enviar relatorio de incidente em ate 48 horas usando o template governance.
   - Atualizar runbooks se um novo modo de falha for descoberto.
