---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Registro de migração SoraFS
descrição: Journal des changements canonique qui suit chaque jalon de migração, les responsables et les suivis requis.
---

> Adaptado de [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# Registro de migração SoraFS

Este registro representa o diário de captura de migrações na arquitetura RFC
SoraFS. Les entrees sont groupees par jalon et indiquent la fenetre effect,
as equipes impactadas e as ações necessárias. Les mises a jour du plan de
migração DOIVENT modificador desta página e RFC
(`docs/source/sorafs_architecture_rfc.md`) para cuidar de consumidores em avaliação
alinha.

| Jalão | Fenetre eficaz | Resumo da mudança | Equipes impactadas | Ações | Estatuto |
|---|-------------------|---------------------|------------------|---------|--------|
| M1 | Semanas 7–12 | Le CI impõe luminárias determinísticas; as referências de alias estão disponíveis no teste; As ferramentas expõem os sinalizadores de atenção explícitos. | Documentos, armazenamento, governança | Certifique-se de que os dispositivos restantes tenham sido assinados, registre o alias no registro de teste e apresente as listas de verificação de lançamento com a exigência `--car-digest/--root-cid`. | ⏳ Atente |

As minutas do plano de controle de governo que fazem referência a esses jalons vivent sous
`docs/source/sorafs/`. Les equipes doivent ajouter des puces datees sous chaque ligne
lorsque des evenements notables surviennent (ex: novos registros de alias,
retrospectivas de incidentes de registro) para fornecer um rastreamento auditável.

## Mises a jour recentes

- 2025-11-01 — Difusão de `migration_roadmap.md` no conselho de governo e nas listas
  operadores para revisão; na tentativa de validação durante a sessão prochaine du
  conselho (ref: suivi `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — L'ISI d'enregistrement du Pin Registry applique desormais la validação
  partagee chunker/politique via les helpers `sorafs_manifest`, gardant les chemins
  on-chain se alinha com as verificações Torii.
- 13/02/2026 — Adição das fases de lançamento do anúncio do provedor (R0–R3) no registro e
  publicação de painéis e orientações operacionais associadas
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).