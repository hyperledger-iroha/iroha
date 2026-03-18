---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-releases.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Processo de release
תקציר: בצע את השער לשחרור ב-CLI/SDK, אפליקציית גרסה פוליטית ותחזוקה ופרסום כתבות קנוניות לשחרור.
---

# תהליך שחרור

OS binarios da SoraFS (`sorafs_cli`, `sorafs_fetch`, עוזרים) e os crates de SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) sao entregues juntos. הו צינור
de release mantem o CLI e as bibliotecas alinhados, garante cobertura de lint/test e
captura artefatos para consumidores במורד הזרם. בצע רשימת בדיקה abaixo para cada
לתייג קנדידטו.

## 0. Confirmar aprovacao da revisao de seguranca

Antes de executar או שער טכניקה לשחרור, לכידת אומנות ה-Artefatos mais recentes da
revisao de seguranca:

- הבאיקס או תזכיר revisao de seguranca SF-6 mais recente ([דוחות/sf6-security-review](./reports/sf6-security-review.md))
  e registre seu hash SHA256 no ticket de release.
- Anexe o link do ticket de remediacao (לדוגמה, `governance/tickets/SF6-SR-2026.md`) e ante
  אושרו של הנדסת אבטחה וקבוצת עבודה של כלי עבודה.
- Verifique que a checklist de remediacao no memo esta fechada; itens pendentes bloqueiam o שחרור.
- הכן או העלאת יומני dos dos do harness de paridade (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  junto com o bundle de manifest.
- אשר que o comando de assinatura que voce vai executar inclui `--identity-token-provider` e
  um `--identity-token-audience=<aud>` מפורש למען השגת שחרור.

כולל אמנות או הודעה על פרסום ופרסום.

## 1. מבצע את שער השחרור/אשכים

O helper `ci/check_sorafs_cli_release.sh` roda formatacao, Clippy e testes nos crates
CLI e SDK com um diretorio target local ao סביבת עבודה (`.target`) עבור evitar conflitos
de permissao ao executar dentro de containers CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

הו תסריט יופיע בתור אימות מפורט:

- `cargo fmt --all -- --check` (סביבת עבודה)
- `cargo clippy --locked --all-targets` para `sorafs_car` (com a feature `cli`),
  `sorafs_manifest` e `sorafs_chunker`
- `cargo test --locked --all-targets` עבור ארגזי מסמוס

Se algum passo falhar, corrija a regressao antes de taguear. בונה devem שחרור
ser continuos com main; נאו faca cherry-pick de correcoes em branches de release. O
gate tambem verifica se os flags de assinatura sem chave (`--identity-token-issuer`,
`--identity-token-audience`) foram fornecidos quando aplicavel; argumentos faltando
Fazem a Execucao Falhar.

## 2. אפליקציית גרסה פוליטית

Todos OS ארגזי CLI/SDK da SoraFS usam SemVer:

- `MAJOR`: Introduzido para o primeiro גרסה 1.0. Antes de 1.0 o aumento menor
  `0.y` **indica mudancas quebradoras** עם שטחיות ל-CLI או לא אסקוומאס Norito.
- `MINOR`: Trabalho de כולל התאמה אישית (נובו קומנדוס/דגלים, נובוסים
  campos Norito protegidos por politica optional, adicoes de telemetria).
- `PATCH`: Correcoes de bugs, משחרר somente de documentacao e atualizacoes de
  dependencias que nao mudam o comportamento observavel.Mantenha semper `sorafs_car`, `sorafs_manifest` ו-`sorafs_chunker` בגרסה אחרת
para que os consumidores de SDK במורד הזרם possam depender de uma unica string de
versao alinhada. Ao atualizar פסוקים:

1. להטמיע את מערכת ההפעלה `version =` ב-`Cargo.toml`.
2. Regenere o `Cargo.lock` דרך `cargo update -p <crate>@<new-version>` (o סביבת עבודה
   exige versoes explicitas).
3. Rode novamente o gate de release para garantir que nao restem artefatos desatualizados.

## 3. הכן כתבות לשחרור

Cada release deve publicar um changelog em markdown que destaque mudancas de CLI, SDK e
השפעה דה גוברננקה. השתמש בתבנית o em `docs/examples/sorafs_release_notes.md`
(copie-o para seu diretorio de artefatos de release e preencha as secoes com detalhes
concretos).

Conteudo minimo:

- **הדגשים**: מנצ'טים דה תכונות לצרכנות של CLI e SDK.
- **תאימות**: mudancas quebradoras, upgrades de politica, requisitos minimos
  de gateway/nodo.
- **מעברי שדרוג**: comandos TL;DR para atualizar dependencias cargo e refazer
  מתקנים deterministicas.
- **Verificacao**: hashes de saida ou envelopes e a revisao exata de
  `ci/check_sorafs_cli_release.sh` executada.

תוספת כתיוג לשחרור preenchidas ao תג (לדוגמה, Corpo de release do GitHub) e
guarde junto com os artefatos gerados de forma deterministica.

## 4. מבצעים ווים לשחרור

Rode `scripts/release_sorafs_cli.sh` para gerar o bundle de assinatura e o resumo de
verificacao que acompanham cada release. אוסף עטיפה או צורך CLI,
chama `sorafs_cli manifest sign` e imediatamente reexecuta `manifest verify-signature`
para que falhas aparecam antes לעשות תיוג. דוגמה:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

Dicas:

- רישום כניסות לשחרור (מטען, תוכניות, סיכומים, אסימון hash esperado do)
  אין ריפו או פריסה מוגדרת לפריסת מנטר או שחזור סקריפט. O
  חבילת מתקנים עם `fixtures/sorafs_manifest/ci_sample/` או פריסת קנוניקו.
- Baseie a automacao de CI em `.github/workflows/sorafs-cli-release.yml`; ele roda o
  gate de release, chama o script acima e arquiva bundles/assinaturas como artefatos
  לעשות זרימת עבודה. Mantenha a mesma ordem de comandos (שער שחרור -> assinatura ->
  verificacao) em outros sistemas CI para que os logs de auditoria batam com os hashes
  gerados.
- Mantenha `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json` e
  `manifest.verify.summary.json` juntos; eles formam o pacote referenciado na
  הודעה מוגברת.
- Quando או שחרור קנוניק של מתקנים אטואליזים, העתק או מניפסט אטואליזדו, o
  סיכומים של תוכנית נתח מערכת הפעלה עבור `fixtures/sorafs_manifest/ci_sample/` (e atualize
  `docs/examples/sorafs_ci_sample/manifest.template.json`) antes de taguear. Operadores
  במורד הזרם תלויים מתקנים עבור חידושים או צרור שחרור.
- Capture o log de execucao da verificacao de bounded-channels de
  `sorafs_cli proof stream` e anexe ao pacote do release para demonstrar que as
  זרימת הוכחה רציפה.
- הירשם ל-`--identity-token-audience` exato usado durante a assinatura nas
  notas de release; a governanca cruza o קהל com a politica de Fulcio antes de
  aprovar a publicacao.השתמש ב-`scripts/sorafs_gateway_self_cert.sh` quando או שחרור טמבם כולל השקה
de gateway. Aponte para o mesmo bundle de manifest para provar que a attestation
corresponde ao artefato candidato:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. תג e publicacao

Depois que os בודק passarem e os hooks forem concluidos:

1. Rode `sorafs_cli --version` e `sorafs_fetch --version` עבור אישורים עבור בינאריוס
   דיווח הפוך.
2. הכן תצורת גרסה עם `sorafs_release.toml` גרסה (מועדפת)
   ou outro arquivo de configuracao rastreado pelo seu repo de deployment. תלוי ב-Evite
   de variaveis de ambiente אד-הוק; passe os caminhos para o CLI com `--config` (ou
   equivalente) para que os inputs לעשות שחרור sejam explicitos e reproduziveis.
3. Crie um tag assinado (preferido) או anotado:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. העלאת חומרי יצירה (חבילות CAR, מניפסטים, קורות חיים של הוכחות, הודעות שחרור,
   פלטי אישור) לרישום לעשות פרוג'טו סגווינדו רשימת בדיקה של גוברננקה
   לא [פריסה של guia](./developer-deployment.md). ראה או שחרור גופי Gerou Novas,
   envie-as para o repo de fixtures compartilhado או חנות אובייקטים para que a automacao
   de auditoria consiga comparar o bundle publicado com o control de codigo.
5. הודעה על תעלת גוברננקה com קישורים עבור תג assinado, notas de release, hashes
   do bundle/assinaturas do manifest, resumos arquivados de `manifest.sign/verify` e
   quaisquer envelopes de attestation. כולל כתובת URL לביצוע עבודת CI (או כתובות יומנים)
   que rodou `ci/check_sorafs_cli_release.sh` e `scripts/release_sorafs_cli.sh`. מימוש
   o ticket de governanca para que os auditores possam rastrear aprovacoes אכל os artefatos;
   quando o job `.github/workflows/sorafs-cli-release.yml` הודעות פומביות, קישור
   os hashes registrados וez de colar resumes ad-hoc.

## 6. לאחר שחרור

- Garanta que a documentacao apontando para a nova versao (התחלות מהירות, תבניות CI)
  esteja atualizada ou confirme que nenhuma mudanca e necessaria.
- Registre entradas no map se for necessario trabalho posterior (לדוגמה, דגלים
- Arquive os logs do gate de release para auditoria - guarde-os ao lado dos artefatos
  assinados.

Seguir este pipeline mantem o CLI, os ארגזים SDK ו o material de governanca alinhados
em cada ciclo de release.