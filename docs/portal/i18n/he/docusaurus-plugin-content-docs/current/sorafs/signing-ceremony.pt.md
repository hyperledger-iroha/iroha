---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/signing-ceremony.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: טקס חתימה
כותרת: Substituicao da cerimonia de assinatura
תיאור: רכיבי Como o Parlamento Sora aprova e distribui לעשות chunker SoraFS (SF-1b).
sidebar_label: Cerimonia de assinatura
---

> מפת דרכים: **SF-1b - תומכי מתקנים של Parlamento Sora.**
> O fluxo do Parlamento substitui a antiga "cerimonia de assinatura do conselho" במצב לא מקוון.

O מדריך טקסים דה assinatura usado עבור אביזרי OS לעשות chunker SoraFS foi aposentado.
Todas as aprovacoes agora passam pelo **Parlamento Sora**, a DAO baseada em sorteio que
גוברנה או Nexus. Membros do Parlamento bloqueiam XOR para obter cidadania, rotacionam
Entre paineis e votam on-chain para aprovar, rejeitar או reverter releases de fixtures.
Este guia explica o processo e o tooling עבור מפתחים.

## Visao geral do Parlamento

- **Cidadania** - Operadores bloqueiam o XOR necessario para se inscrever como cidadaos e
  se tornar elegiveis ao sorteio.
- **Paineis** - As responsabilidades sao divididas entre paineis rotativos (Infraestrutura,
  Moderacao, Tesouraria, ...). O Painel de Infraestrutura e o dono das aprovacoes de
  מתקנים לעשות SoraFS.
- **Sorteio e rotacao** - As cadeiras de painel sao redesenhadas na cadencia definida na
  constituicao do Parlamento para que nenhum grupo מונופול כמו אפרובאקו.

## פלקסו דה אפרובאקאו דה גופי

1. **Submissao da proposta**
   - O Tooling WG envia o חבילה מועמדת `manifest_blake3.json` מאי או מתקן הבדל
     עבור רישום ברשת דרך `sorafs.fixtureProposal`.
   - A proposta registra o digest BLAKE3, a versao semantica e as notas de mudanca.
2. **Revisao e votacao**
   - O Painel de Infraestrutura recebe a atribuicao pela fila de tarefas do Parlamento.
   - Membros do painel inspecionam artefatos de CI, rodam testes de paridade e
     registram votos ponderados ברשת.
3. **Finalizacao**
   - Quando o quorum e atingido, o runtime emite um evento de aprovacao que inclui o
     לעכל canonico לעשות מניפסט e o compromisso Merkle לעשות מטען מתקן.
   - O evento e espelhado no registry SoraFS para que clientes possam buscar o
     manifest mais recente aprovado pelo Parlamento.
4. **הפצה**
   - Helpers de CLI (`cargo xtask sorafs-fetch-fixture`) פוקסאם או אישור מניפסט באמצעות
     Nexus RPC. כמו קבועים JSON/TS/Go לעשות repositorio ficam sincronizadas ao
     reexecuter `export_vectors` e validar o digest contra o registro on-chain.

## Fluxo de trabalho de developer

- Regenere fixtures com:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- השתמש ב- o helper de fetch do Parlamento para baixar o aprovado envelope, verificar
  assinaturas e atualizar fixtures locais. Aponte `--signatures` למעטפה
  publicado pelo Parlamento; o עוזר לפתור o Manifest associado, recomputa o
  לעכל BLAKE3 e impoe o perfil canonico `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

העבר את `--manifest` כדי להציג את כתובת ה-URL מחוץ למניפסט. מעטפות sem assinatura
sao recusados, a menos que `--allow-unsigned` seja definido para smoke runs locais.- Ao validar um manifest via gateway de staging, aponte para Torii em vez de
  מקום מטענים:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- O CI local nao exige mais um roster `signer.json`.
  `ci/check_sorafs_fixtures.sh` השוואת מקום לביצוע ריפו עם פשרה אולטימטיבית
  על השרשרת e falha quando divergem.

## הערות דה גוברננקה

- A constituicao do Parlamento governa quorum, rotacao e escalonamento - nao e
  necessaria configuracao no nivel do crate.
- Rollbacks de emergencia sao tratados pelo painel de moderacao do Parlamento. O
  Painel de Infraestrutura abre uma proposta de revert que referencia o digest
  anterior do manifest, substituindo a release quando aprovada.
- Aprovacoes historicas permanecem disponiveis ללא רישום SoraFS עבור שידור חוזר
  פורנס.

## שאלות נפוצות

- **Para onde foi `signer.json`?**  
  Foi removido. Toda a atribuicao de assinaturas vive על השרשרת; `manifest_signatures.json`
  אין מאגר e apenas um fixture de developer que deve corresponder ao ultimo
  evento de aprovacao.

- **Ainda exigimos assinaturas Ed25519 locais?**  
  נאו. כמו אפרובאקוים עושים Parlamento sao armazenadas como artefatos על השרשרת. מתקנים
  locais existem para reprodutibilidade, mas sao validados contra o digest do Parlamento.

- **Como as equipes monitoram aprovacoes?**  
  Assinem o evento `ParliamentFixtureApproved` או ייעוץ של רישום דרך Nexus RPC
  להחלים או לעכל את עצמו לעשות ביטוי e chamada do painel.