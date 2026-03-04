---
lang: pt
direction: ltr
source: docs/source/governance_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9f765fbe3170f654a9c44c3cd1afc5d82a72ff49137f32b98cf9d310faf114e
source_last_modified: "2026-01-03T18:08:00.913452+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Pipeline de Governança (Iroha 2 e SORA Parlamento)

# Estado atual (v1)
- As propostas de governação funcionam como: proponente → referendo → contagem → promulgação. As janelas de referendo e os limites de participação/aprovação são aplicados conforme descrito em `gov.md`; os bloqueios são somente estendidos e desbloqueados na expiração.
- A seleção do parlamento utiliza sorteios baseados em VRF com ordenação determinística e limites de mandato; quando não existe nenhuma lista persistente, Torii deriva um substituto usando a configuração `gov.parliament_*`. As verificações de controle e quórum do conselho são exercidas nos testes `gov_parliament_bodies`/`gov_pipeline_sla`.
- Modos de votação: ZK (padrão, requer `Active` VK com bytes inline) e Simples (peso quadrático). As incompatibilidades de modo são rejeitadas; a criação/extensão de bloqueio é monotônica em ambos os modos com testes de regressão para ZK e novas votações simples.
- A má conduta do validador é resolvida por meio do pipeline de evidências (`/v1/sumeragi/evidence*`, auxiliares CLI) com transferências de consenso conjunto aplicadas por `NextMode` + `ModeActivationHeight`.
- Namespaces protegidos, ganchos de atualização de tempo de execução e admissão de manifesto de governança são documentados em `governance_api.md` e cobertos por telemetria (`governance_manifest_*`, `governance_protected_namespace_total`).

# Em voo/backlog
- Publicar artefatos de sorteio de VRF (semente, prova, escalação ordenada, suplentes) e codificar regras de substituição para não comparecimentos; adicione acessórios dourados para o sorteio e substituições.
- A aplicação faseada do SLA para os órgãos do Parlamento (regras → agenda → estudo → revisão → júri → promulgar) necessita de temporizadores explícitos, caminhos de escalada e contadores de telemetria.
- A votação secreta/compromisso-revelação do júri de políticas e as auditorias associadas à resistência ao suborno ainda não foram implementadas.
- Multiplicadores de vínculo de função, redução de má conduta para órgãos de alto risco e tempos de espera entre slots de serviço exigem encanamento de configuração e testes.
- A vedação da pista de governança e as janelas/portões de participação dos referendos são rastreados em `gov.md`/`status.md`; mantenha as entradas do roteiro atualizadas à medida que os testes de aceitação restantes chegam.