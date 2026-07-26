---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-operator-onboarding.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-operator-onboarding
כותרת: Integracao de operadores de data-space Sora Nexus
תיאור: Espelho de `docs/source/sora_nexus_operator_onboarding.md`, מלווה רשימת בדיקה של שחרור מקצה לקצה עבור מפעילי Nexus.
---

:::שים לב Fonte canonica
Esta pagina reflete `docs/source/sora_nexus_operator_onboarding.md`. Mantenha as duas copias alinhadas ate que as edicoes localizadas cheguem ao פורטל.
:::

# Integracao de operadores de data-space Sora Nexus

אסטרטגיית קבצים או מקצה לקצה מפעילים את מרחב הנתונים Sora Nexus פיתחה את השחרור והכרזה. Ele complementa o runbook de dupla via (`docs/source/release_dual_track_runbook.md`) e a not de selecao de artefatos (`docs/source/release_artifact_selection.md`) descrevendo como alinhar bundles/images baixados, manifests e templates de configuracao com as coproducts de la globai.

## דרישות מוקדמות לצופים
- Voce foi aprovado pelo Programa Nexus e recebeu sua atribuicao de data-space (index de lane, data-space ID/alias e requisitos de politica de routing).
- Voce consegue acessar os artefatos assinados do release publicados pelo Release Engineering (tarballs, imagens, manifests, assinaturas, chaves publicas).
- Voce gerou ou recebeu material de chaves de producao para seu papel de validator/observer (identidade de no Ed25519; chave de consenso BLS + PoP para validators; mais quaisquer toggles de recursos confidenciais).
- Voce consegue alcancar os peers Sora Nexus existentes que farao bootstrap do seu no.

## Etapa 1 - אישור או קובץ פרסום
1. מזהה או alias de rede או מזהה שרשרת fornecido.
2. הפעל את `scripts/select_release_profile.py --network <alias>` (או `--chain-id <id>`) ב-um checkout deste repositorio. O helper consulta `release/network_profiles.toml` e imprime o perfil para deploy. Para Sora Nexus a resposta deve ser `iroha3`. עבור חילוף כוחות, עבור הנדסת שחרור.
3. Anote o tag de versao referenciado no anuncio do release (por exemplo `iroha3-v3.2.0`); voce o usara para buscar artefatos e manifests.

## Etapa 2 - Recuperar e validar artefatos
1. Baixe o bundle `iroha3` (`<profile>-<version>-<os>.tar.zst`) e seus arquivos companheiros (`.sha256`, אופציונלי `.sig/.pub`, `<profile>-<version>-manifest.json`, `<profile>-<version>-manifest.json`, fizer deploy com contenedores).
2. תקף מסמך אינטגריד antes descompactar:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Substitua `openssl` מוכיח את העזרה של ארה"ב או חומרת KMS.
3. Inspecione `PROFILE.toml` dentro do tarball e os manifests JSON para confirmar:
   - `profile = "iroha3"`
   - מערכת הפעלה `version`, `commit` ו-`built_at` מתכתבים ועוד הודעה לשחרור.
   - O OS/arquitetura correspondem ao alvo deploy.
4. ראה ארה"ב תמונה של מתמודד, חזור על אימות hash/assinatura עבור `<profile>-<version>-<os>-image.tar` ואשר את מזהה התמונה הרשום ב-`<profile>-<version>-image.json`.## Etapa 3 - הכן את התצורה של תבניות
1. Extraia o Bundle e Copie `config/` para o local onde o no vai ler sua configuracao.
2. Trate os arquivos sob `config/` como תבניות:
   - Substitua `public_key`/`private_key` pelas suas chaves Ed25519 de producao. Remova chaves privadas do disco se o no vai obtelas a partir de um HSM; להגדיר הגדרות עבור חיבור HSM.
   - Ajuste `trusted_peers`, `network.address` ו-`torii.address` עבור ממשקים מיוחדים אלקנקאבייס ו-eos peers de bootstrap que lhe foram atribuidos.
   - הפעל את `client.toml` עם נקודת קצה Torii עבור מפעילים (כולל תצורת TLS עם אפליקציית) e as credenciais provisionadas para tooling operational.
3. Mantenha o chain ID fornecido no bundle a menos que Governance instrua explicitamente - a lane global espera um unico identificador canonico de cadeia.
4. Planeje iniciar o no com o flag de perfil Sora: `irohad --sora --config <path>`. O loader de configuracao rejeitara configuracoes SoraFS או ריבוי נתיבים לראות או דגל estiver ausente.

## Etapa 4 - Alinhar metadata de data-space e routing
1. ערוך את `config/config.toml` עבור `[nexus]` התכתבות וקטלוג נתונים-חלל פורנקידו של Nexus Council:
   - `lane_count` פיתח סרגל דומה לבעלי מסלולים כלליים.
   - Cada entrada em `[[nexus.lane_catalog]]` e `[[nexus.dataspace_catalog]]` deve conter um `index`/`id` unico e os aliases acordados. Nao apague as entradas globais existentes; adicione seus aliases delegados se o conselho atribuiu data-spaces adicionais.
   - Garanta que cada entrada de dataspace כולל `fault_tolerance (f)`; comites נתיב-relay sao dimensionados em `3f+1`.
2. להטמיע את `[[nexus.routing_policy.rules]]` למען תפיסה של פוליטיקה que lhe foi dada. O template padrao roteia instrucoes de governanca para a lane `1` e deploys de contratos para a lane `2`; adicione ou modifique regras para que o trafego destinado ao seu data-space seja encaminhado para a lane e o alias corretos. Coordene com Release Engineering antes de alterar a order das regras.
3. בדוק את OS limites de `[nexus.da]`, `[nexus.da.audit]` ו-`[nexus.da.recovery]`. Espera-se que os operadores mantenham os valores aprovados pelo conselho; ajuste apenas se uma politica atualizada foi ratificada.
4. רשום configuracao final no seu tracker de operacoes. O runbook de release de dupla via exige anexar o `config.toml` efetivo (com segredos redigidos) או כרטיס כניסה למטוס.## אטאפה 5 - ולידקאו לפני טיסה
1. בצע את validador de configuracao embutido antes de entrar na rede:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Iso imprime a configuracao resolvida e falha cedo se as entradas de catalogo/routing forem inconsistentes ou se genesis e config divergirem.
2. נסה לפרוס com contenedores, לבצע או mesmo comando dentro da imagem apos carregala com `docker load -i <profile>-<version>-<os>-image.tar` (lembre de incluir `--sora`).
3. בדוק יומנים לזיהוי זיהוי מיקום של נתיב/מרחב נתונים. לאחר מכן, חזור ל-Etapa 4 - פורס את ה-Producao Nao Devem תלוי dos IDs placeholder que acompanham OS תבניות.
4. בצע את ההליך המקומי של עשן (לדוגמה, שאילתת uma `FindNetworkStatus` com `iroha_cli`, אישור נקודות הקצה של טלמטריה תערוכת `nexus_lane_state_total`, ואימות הזרם עבור ייבוא קבצים או נתונים הכרחי).

## Etapa 6 - חתך מסירה
1. Guarde o `manifest.json` verificado e os artefatos de assinatura no ticket de release para que auditores possam reproduzir suas verificacoes.
2. Notifique Nexus Operations de que o no esta pronto para ser introduzido; כולל:
   - Identidade do no (זיהוי עמיתים, שמות מארח, נקודת קצה Torii).
   - Valores efetivos לעשות קטלוג מסלול/נתונים ופוליטיקה ניתוב.
   - Hashes dos binarios/imagens verificados.
3. Coordene a admissao final de peers (זרעי רכילות e atribuicao de lane) com `@nexus-core`. Nao entre na red אכל receber aprovacao; Sora Nexus aplica occupacao deterministica de lanes e exige um manifest de admissoes atualizado.
4. Depois que o no estiver ativo, atualize seus runbooks com quaisquer overrides que voce introduziu e anote o tag de release para que a proxima iteracao comecece a partir desta baseline.

## רשימת אזכור
- [ ] מסמך אישור שחרור como `iroha3`.
- [ ] Hashes e assinaturas do bundle/imagem verificados.
- [ ] Chaves, enderecos de peers e endpoints Torii atualizados para valores de producao.
- [ ] קטלוג נתיבים/מרחב נתונים ופוליטיקה של ניתוב לעשות Nexus מתכתבים עם אטריבו של קונסלו.
- [ ] Validador de configuracao (`irohad --sora --config ... --trace-config`) עובר סם אביזוס.
- [ ] Manifests/assinaturas arquivados no ticket de onboarding e Ops notificado.

Para contexto mais amplo sobre phases de migracao do Nexus e expectativas de telemetria, revize [Nexus הערות מעבר](./nexus-transition-notes).