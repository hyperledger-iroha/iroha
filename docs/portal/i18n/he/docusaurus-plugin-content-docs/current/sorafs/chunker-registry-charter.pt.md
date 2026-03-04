---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-charter.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: chunker-registry-charter
כותרת: Carta do registro de chunker da SoraFS
sidebar_label: Carta do registro de chunker
תיאור: Carta de governanca para submissao e aprovacao de perfis de chunker.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/sorafs/chunker_registry_charter.md`. Mantenha ambas as copias sincronizadas.
:::

# Carta de governanca do registro de chunker da SoraFS

> **Ratificado:** 2025-10-29 pelo Sora Parliament Infrastructure Panel (veja
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Qualquer emenda requer um
> voto formal de governanca; equipes de implementacao devem tratar este documento como
> normativo ate que uma carta substituta seja aprovada.

אסטרטגיה מוגדרת o processo e os papeis para evoluir o registro de chunker da SoraFS.
Ela complementa o [Guia de autoria de perfis de chunker](./chunker-profile-authoring.md) ao descrever como novos
perfis sao propostos, revisados, ratificados e eventualmente descontinuados.

## אסקופו

A carta se aplica a cada entrada em `sorafs_manifest::chunker_registry` e
כלי עבודה של qualquer que consome o registro (Manifest CLI, ספק-מודעה CLI,
ערכות SDK). Ela impoe os invariantes de alias e handle verificados por
`chunker_registry::ensure_charter_compliance()`:

- IDs de perfil sao inteiros positivos que aumentam de forma monotona.
- ידית O canonico `namespace.name@semver` **deve** aparecer como a primeira
  entrada em `profile_aliases`. כינויים alternativos vem em seguida.
- כ-strings de alias sao aparadas, unicas e nao colidem com מטפל ב-canonicos
  de outras entradas.

## פאפייס

- **מחבר(ים)** - מכינים הצעה, מתקנים מחדש וקולטים
  evidencia de determinismo.
- **Tooling Working Group (TWG)** - valida a proposta usando as checklists
  publicadas e assegura que os invariantes do registro sejam atendidos.
- **מועצת הממשל (GC)** - עדכון היחסים ל-TWG, המעטפה או המעטפה
  e aprova os prazos de publicacao/deprecacao.
- **צוות אחסון** - פעל ברישום ופרסום
  atualizacoes de documentacao.

## Fluxo do ciclo de vida

1. **Submissao de proposta**
   - O autor executa רשימה של validacao do guia de autoria e cria
     אממ JSON `ChunkerProfileProposalV1` התייפח
     `docs/source/sorafs/proposals/`.
   - כולל CLI de:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Envie um PR contendo גופי, proposta, relatorio de determinismo ה
     atualizacoes לעשות רישום.

2. **Revisao de tooling (TWG)**
   - Repita a checklist de validacao (מתקנים, fuzz, pipeline de manifest/PoR).
   - בצע את `cargo test -p sorafs_car --chunker-registry` e garanta que
     `ensure_charter_compliance()` עובר קום חדש.
   - אימות que o comportamento do CLI (`--list-profiles`, `--promote-profile`, סטרימינג
     `--json-out=-`) reflita os aliases e handles atualizados.
   - Produza um relatorio curto resumindo achados e status de aprovacao/reprovacao.3. **Aprovacao do conselho (GC)**
   - עדכון היחס ל-TWG ו-metadados da proposta.
   - Assine o digest da proposta (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     e anexe as assinaturas ao envelope do conselho mantido junto aos גופי.
   - Registre o resultado da votacao nas atas de governanca.

4. **Publicacao**
   - Faca מיזוג לעשות יחסי ציבור, atualizando:
     - `sorafs_manifest::chunker_registry_data`.
     - Documentacao (`chunker_registry.md`, guias de autoria/conformidade).
     - מתקנים e relatorios de determinismo.
   - הודעה על מפעילים וציוד SDK כל כך חדש או פרופיל חדש או תוכנית השקה.

5. **Deprecacao / Encerramento**
   - Propostas que substituem um perfil existente devem incluir uma janela de publicacao
     dupla (periodos de carencia) e um plano de upgrade.
     אין רישום e atualize o Ledger de migracao.

6. **Mudancas de emergencia**
   - Remocoes או תיקונים חמים exigem voto do conselho com aprovacao por maioria.
   - O TWG deve documentar as etapas de mitigacao de risco e atualizar o log de incidentes.

## ציפיות של כלי עבודה

- `sorafs_manifest_chunk_store` e `sorafs_manifest_stub` תערוכת:
  - `--list-profiles` לבדיקת רישום.
  - `--promote-profile=<handle>` עבור גרר או בלוקו דה מטאדאדוס קנוניקו בארה"ב
    ao promoter um perfil.
  - `--json-out=-` עבור שידור relatorios עבור stdout, habilitando logs de revisao
    reproduziveis.
- `ensure_charter_compliance()` e invocado na inicializacao dos binarios relevantes
  (`manifest_chunk_store`, `provider_advert_stub`). Os testes de CI devem falhar se
  novas entradas violarem a carta.

## רישום

- Armazene todos os relatorios de determinismo em `docs/source/sorafs/reports/`.
- As atas do conselho que referenciam decisoes de chunker ficam em
  `docs/source/sorafs/migration_ledger.md`.
- Atualize `roadmap.md` e `status.md` apos cada mudanca maior no registro.

## אסמכתאות

- Guia de autoria: [Guia de autoria de perfis de chunker](./chunker-profile-authoring.md)
- רשימת תאימות: `docs/source/sorafs/chunker_conformance.md`
- Referencia do registro: [Registro de perfis de chunker](./chunker-registry.md)