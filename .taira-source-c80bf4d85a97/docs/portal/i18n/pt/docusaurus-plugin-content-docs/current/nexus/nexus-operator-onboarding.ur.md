---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: integração do operador nexus
título: Sora Nexus espaço de dados
description: `docs/source/sora_nexus_operator_onboarding.md` کا آئینہ, جو Nexus آپریٹرز کے لئے end-to-end ریلیز چیک لسٹ کو ٹریک کرتا ہے۔
---

:::nota کینونیکل ماخذ
یہ صفحہ `docs/source/sora_nexus_operator_onboarding.md` کی عکاسی کرتا ہے۔ لوکلائزڈ ایڈیشنز پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Integração do operador de espaço de dados Sora Nexus

یہ گائیڈ ponta a ponta فلو کو محفوظ کرتی ہے جس پر Sora Nexus espaço de dados آپریٹرز کو ریلیز کے اعلان کے بعد عمل کرنا ہوتا ہے۔ یہ runbook de trilha dupla (`docs/source/release_dual_track_runbook.md`) e nota de seleção de artefato (`docs/source/release_artifact_selection.md`) Você pode usar pacotes/imagens, manifestos e modelos de configuração, além de expectativas de faixa, expectativas de faixa, configurações de configuração e expectativas de faixa. ہے۔

## سامعین اور پیشگی شرائط
- آپ کو Nexus Program نے منظور کیا ہے اور آپ کو atribuição de espaço de dados مل چکی ہے (índice de faixa, ID/alias de espaço de dados, e requisitos de política de roteamento).
- آپ Engenharia de Liberação کی شائع کردہ artefatos de liberação assinados تک رسائی رکھتے ہیں (tarballs, imagens, manifestos, assinaturas, chaves públicas).
- آپ نے اپنے validador/observador رول کے لئے پروڈکشن material chave تیار یا حاصل کیا ہے (identidade do nó Ed25519, validadores کے لئے chave de consenso BLS + PoP؛ اور کوئی بھی alterna recursos confidenciais).
- آپ ان موجودہ Sora Nexus peers تک رسائی کر سکتے ہیں جو آپ کے نوڈ کا bootstrap کریں گے۔

## مرحلہ 1 - ریلیز پروفائل کی تصدیق
1. وہ alias de rede یا ID de cadeia شناخت کریں جو آپ کو دیا گیا ہے۔
2. Faça o checkout em `scripts/select_release_profile.py --network <alias>` (یا `--chain-id <id>`) چلائیں۔ helper `release/network_profiles.toml` دیکھ کر implantar ہونے والا پروفائل پرنٹ کرتا ہے۔ Sora Nexus کے لئے جواب `iroha3` ہونا چاہئے۔ کسی بھی دوسرے ویلیو پر رک جائیں اور Engenharia de Liberação سے رابطہ کریں۔
3. ریلیز اعلان میں دیا گیا tag de versão نوٹ کریں (مثلاً `iroha3-v3.2.0`); اسی سے آپ artefatos اور manifestos حاصل کریں گے۔

## مرحلہ 2 - artefatos حاصل کریں اور ویریفائی کریں
1. Pacote `iroha3` (`<profile>-<version>-<os>.tar.zst`) e seus arquivos complementares ڈاؤن لوڈ کریں (`.sha256`, اختیاری `.sig/.pub`, `<profile>-<version>-manifest.json`, اور `<profile>-<version>-image.json` اگر آپ contêineres ڈپلائے کر رہے ہیں)۔
2. O que você precisa saber sobre integridade:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   اگر آپ KMS apoiado por hardware استعمال کرتے ہیں تو `openssl` کو ادارہ منظور شدہ verificador سے بدل دیں۔
3. tarball com `PROFILE.toml` e manifestos JSON دیکھ کر تصدیق کریں:
   -`profile = "iroha3"`
   - `version`, `commit`, اور `built_at` فیلڈز ریلیز اعلان سے ملتے ہیں۔
   - SO/arquitetura آپ کے destino de implantação سے correspondência کرتی ہے۔
4. Crie uma imagem de contêiner استعمال کرتے ہیں تو `<profile>-<version>-<os>-image.tar` کے لئے hash/assinatura دوبارہ verifique کریں اور `<profile>-<version>-image.json` میں ID da imagem کنفرم کریں۔

## مرحلہ 3 - modelos سے configuração تیار کریں
1. extrato de pacote کریں اور `config/` کو اس جگہ کاپی کریں جہاں نوڈ اپنی configuração پڑھے گا۔
2. `config/` کے تحت فائلوں کو modelos سمجھیں:
   - `public_key`/`private_key` کو اپنے پروڈکشن Chaves Ed25519 سے بدلیں۔ اگر نوڈ chaves HSM سے لے گا تو chaves privadas کو disco سے ہٹا دیں؛ config e conector HSM کی طرف پوائنٹ کریں۔
   - `trusted_peers`, `network.address` e `torii.address` کو آپ کے قابل رسائی interfaces اور مقررہ bootstrap peers کے مطابق ایڈجسٹ کریں۔
   - `client.toml` é um endpoint Torii voltado para o operador (configuração TLS سمیت، اگر لاگو ہو) اور آپ کی provisionamento کردہ credenciais کے ساتھ اپ ڈیٹ کریں۔
3. pacote میں فراہم کردہ ID da cadeia برقرار رکھیں, الا یہ کہ Governança واضح طور پر ہدایت دے - via global ایک واحد identificador de cadeia canônica چاہتا ہے۔
4. نوڈ کو Sora پروفائل فلیگ کے ساتھ اسٹارٹ کرنے کا ارادہ رکھیں: `irohad --sora --config <path>`. اگر فلیگ نہ ہو تو carregador de configuração SoraFS یا multi-lane سیٹنگز کو rejeitar کر دے گا۔## مرحلہ 4 - metadados de espaço de dados e roteamento ہم آہنگ کریں
1. `config/config.toml` ایڈٹ کریں تاکہ `[nexus]` سیکشن Nexus Council کے فراہم کردہ catálogo de espaço de dados سے correspondência کرے:
   - `lane_count` موجودہ época میں فعال pistas کی مجموعی تعداد کے برابر ہونا چاہئے۔
   - `[[nexus.lane_catalog]]` ou `[[nexus.dataspace_catalog]]` کی ہر انٹری میں منفرد `index`/`id` e outros aliases ہونے چاہئیں۔ موجودہ entradas globais نہ ہٹائیں؛ اگر conselho نے اضافی espaços de dados دیئے ہیں تو اپنے aliases delegados شامل کریں۔
   - ہر dataspace انٹری میں `fault_tolerance (f)` شامل ہونا یقینی بنائیں؛ comitês de revezamento de pista کا سائز `3f+1` ہوتا ہے۔
2. `[[nexus.routing_policy.rules]]` کو اپنی دی گئی پالیسی کے مطابق اپ ڈیٹ کریں۔ instruções de governança do modelo padrão کو pista `1` e implantações de contrato کو pista `2` پر rota کرتا ہے؛ قواعد شامل یا تبدیل کریں تاکہ آپ کے espaço de dados کی ٹریفک درست pista اور alias پر جائے۔ قواعد کی ترتیب بدلنے سے پہلے Engenharia de Liberação کے ساتھ ہم آہنگی کریں۔
3. `[nexus.da]`, `[nexus.da.audit]`, e `[nexus.da.recovery]` limites ریویو کریں۔ آپریٹرز سے توقع ہے کہ وہ ویلیوز رکھیں؛ صرف اسی وقت بدلیں جب نئی پالیسی منظور ہو۔
4. Configuração حتمی کو اپنے rastreador de operações میں ریکارڈ کریں۔ ticket de integração do runbook de liberação de trilha dupla کے ساتھ موثر `config.toml` (segredos redigidos)

## مرحلہ 5 - پری فلائٹ ویلیڈیشن
1. نیٹ ورک میں شامل ہونے سے پہلے validador de configuração integrado چلائیں:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   یہ configuração resolvida پرنٹ کرتا ہے اور اگر entradas de catálogo/roteamento میں تضاد ہو یا genesis اور config نہ ملیں تو جلدی fail ہو جاتا ہے۔
2. Como implantar contêineres کرتے ہیں تو `docker load -i <profile>-<version>-<os>-image.tar` کے بعد وہی کمانڈ imagem کے اندر چلائیں ( `--sora` شامل کرنا نہ بھولیں)۔
3. logs میں marcadores de posição/identificadores de espaço de dados کے avisos دیکھیں۔ اگر ملیں تو مرحلہ 4 پر واپس جائیں - پروڈکشن implantações کو modelos کے IDs de espaço reservado پر انحصار نہیں کرنا چاہئے۔
4. Procedimento de fumaça local چلائیں (مثلاً `iroha_cli` ou `FindNetworkStatus` consulta بھیجیں, تصدیق کریں کہ terminais de telemetria `nexus_lane_state_total` expor کرتے ہیں, اور chaves de streaming کی rotação/importação کی تصدیق کریں)۔

## Passo 6 - Corte e transferência
1. تصدیق شدہ `manifest.json` اور artefatos de assinatura کو ticket de liberação میں محفوظ کریں تاکہ auditores آپ کی cheques دوبارہ کر سکیں۔
2. Operações Nexus کو اطلاع دیں کہ نوڈ متعارف کرنے کے لئے تیار ہے؛ O que fazer:
   - Identidade do nó (ID de peer, nomes de host, endpoint Torii).
   - Catálogo de faixa/espaço de dados e política de roteamento ویلیوز۔
   - Binários/imagens verificados کے hashes۔
3. Admissão por pares (sementes de fofoca e atribuição de pista) کو `@nexus-core` کے ساتھ کوآرڈینیٹ کریں۔ منظوری ملنے سے پہلے نیٹ ورک junte-se a نہ کریں؛ Sora Nexus ocupação determinística da pista نافذ کرتا ہے اور manifesto de admissão atualizado چاہتا ہے۔
4. نوڈ live ہونے کے بعد اپنے runbooks میں کی گئی substitui اپ ڈیٹ کریں اور tag de lançamento نوٹ کریں تاکہ اگلی iteração A linha de base é a linha de base

## ریفرنس چیک لسٹ
- [] Liberar perfil `iroha3` کے طور پر validar ہو چکا ہے۔
- [] Pacote/imagem کے hashes اور verificação de assinaturas ہو چکے ہیں۔
- [ ] Chaves, endereços de pares e terminais Torii پروڈکشن ویلیوز پر اپ ڈیٹ ہیں۔
- [ ] Nexus catálogo de pista/espaço de dados اور atribuição de conselho de política de roteamento سے correspondência کرتی ہے۔
- [ ] Validador de configuração (`irohad --sora --config ... --trace-config`) بغیر avisos کے پاس کرتا ہے۔
- [ ] Manifestos/assinaturas bilhete de embarque میں آرکائیو اور Ops کو اطلاع دے دی گئی ہے۔

Fases de migração Nexus e expectativas de telemetria