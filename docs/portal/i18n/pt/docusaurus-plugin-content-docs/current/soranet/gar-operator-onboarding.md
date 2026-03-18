---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

Use este brief para fazer o rollout da configuracao de compliance SNNet-9 com um processo repetivel e amigavel a auditoria. Combine com a revisao jurisdicional para que cada operador use os mesmos digests e o mesmo layout de evidencias.

## Passos

1. **Montar configuracao**
   - Importe `governance/compliance/soranet_opt_outs.json`.
   - Mescle suas `operator_jurisdictions` com os digests de attestation publicados
     na [revisao jurisdicional](gar-jurisdictional-review).
2. **Validar**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - Opcional: `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **Capturar evidencias**
   - Guarde em `artifacts/soranet/compliance/<YYYYMMDD>/`:
     - `config.json` (bloco de compliance final)
     - `attestations.json` (URIs + digests)
     - logs de validacao
     - referencias a PDFs/Norito envelopes assinados
4. **Ativar**
   - Marque o rollout (`gar-opt-out-<date>`), redeploy as configs de orchestrator/SDK,
     e confirme que eventos `compliance_*` aparecem nos logs esperados.
5. **Encerrar**
   - Arquive o bundle de evidencias com o Governance Council.
   - Registre a janela de ativacao e aprovadores no GAR logbook.
   - Agende as proximas revisoes a partir da tabela de revisao jurisdicional.

## Checklist rapido

- [ ] `jurisdiction_opt_outs` corresponde ao catalogo canonico.
- [ ] Digests de attestation copiados exatamente.
- [ ] Comandos de validacao executados e arquivados.
- [ ] Bundle de evidencias guardado em `artifacts/soranet/compliance/<date>/`.
- [ ] Tag de rollout e GAR logbook atualizados.
- [ ] Lembretes de proxima revisao configurados.

## Veja tambem

- [GAR Jurisdictional Review](gar-jurisdictional-review)
- [GAR Compliance Playbook (source)](../../../source/soranet/gar_compliance_playbook.md)
