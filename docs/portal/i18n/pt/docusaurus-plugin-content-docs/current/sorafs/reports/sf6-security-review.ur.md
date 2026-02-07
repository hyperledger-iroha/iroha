---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: SF-6 سیکیورٹی ریویو
resumo: assinatura sem chave, streaming de prova, manifestos e pipelines کی آزادانہ جانچ کے نتائج e acompanhamento آئٹمز۔
---

# SF-6 سیکیورٹی ریویو

**Janela de avaliação:** 10/02/2026 → 18/02/2026  
**Líderes de análise:** Associação de Engenharia de Segurança (`@sec-eng`), Grupo de Trabalho de Ferramentas (`@tooling-wg`)  
**Escopo:** CLI/SDK SoraFS (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), APIs de streaming de prova, manipulação de manifesto Torii, Integração Sigstore/OIDC, ganchos de liberação CI۔  
**Artefatos:**  
- Fonte CLI e testes (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Manipuladores de manifesto/prova Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Automação de liberação (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Chicote de paridade determinística (`crates/sorafs_car/tests/sorafs_cli.rs`, [SoraFS Orchestrator GA Parity Report](./orchestrator-ga-parity.md))

## Metodologia

1. **Workshops de modelagem de ameaças** em estações de trabalho de desenvolvedor, sistemas CI e nós Torii e mapa de capacidades do invasor.  
2. **Revisão de código** superfícies de credenciais (troca de token OIDC, assinatura sem chave), validação de manifesto Norito e pressão de retorno de streaming de prova پر فوکس کیا۔  
3. **Testes dinâmicos** Replay de manifestos de dispositivo elétrico e modos de falha simulam کیے (repetição de token, adulteração de manifesto, fluxos de prova truncados) chicote de paridade e fuzz drives sob medida کے ساتھ۔  
4. **Inspeção de configuração** نے `iroha_config` padrões, manipulação de sinalizadores CLI e scripts de liberação validados کیے تاکہ determinísticos, execuções auditáveis ​​یقینی ہوں۔  
5. **Entrevista de processo** Fluxo de remediação, caminhos de escalonamento اور captura de evidências de auditoria کو Tooling WG کے proprietários de liberação کے ساتھ confirmar کیا۔

## Resumo das descobertas| ID | Gravidade | Área | Encontrando | Resolução |
|----|----------|------|---------|------------|
| SF6-SR-01 | Alto | Assinatura sem chave | Modelos de CI padrão de público de token OIDC میں implícito تھے، جس سے reprodução entre locatários کا خطرہ تھا۔ | ganchos de liberação e modelos de CI میں `--identity-token-audience` کی aplicação explícita شامل کی گئی ([processo de liberação](../developer-releases.md), `docs/examples/sorafs_ci.md`). audiência omitir ہونے پر CI اب falhar ہوتا ہے۔ |
| SF6-SR-02 | Médio | Transmissão de prova | Caminhos de contrapressão نے buffers de assinantes ilimitados قبول کیے, جس سے esgotamento de memória ممکن تھی۔ | `sorafs_cli proof stream` tamanhos de canal limitados impõem کرتا ہے, truncamento determinístico کے ساتھ Norito log de resumos کرتا ہے اور fluxo abort کرتا ہے؛ Espelho Torii کو blocos de resposta محدود کرنے کے لیے اپڈیٹ کیا گیا (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Médio | Envio de manifesto | CLI نے manifestos قبول کیے بغیر planos de blocos incorporados verificam کیے جب `--plan` موجود نہ تھا۔ | `sorafs_cli manifest submit` اب CAR digests دوبارہ computar اور comparar کرتا ہے جب تک `--expect-plan-digest` دیا نہ جائے, incompatibilidades rejeitar کرتا ہے اور dicas de remediação دکھاتا ہے۔ os casos de sucesso/falha dos testes cobrem کرتے ہیں (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Baixo | Trilha de auditoria | Lista de verificação de liberação میں سیکیورٹی ریویو کے لیے log de aprovação assinado شامل نہیں تھا۔ | [processo de liberação](../developer-releases.md) میں سیکشن شامل کیا گیا جو revisar hashes de memorando اور URL do ticket de assinatura کو GA سے پہلے anexar کرنے کا تقاضا کرتا ہے۔ |

تمام janela de revisão de resultados altos/médios کے دوران correção ہوئیں اور موجود chicote de paridade سے validar ہوئیں۔ کوئی problemas críticos latentes باقی نہیں۔

## Validação de controle

- **Escopo da credencial:** Modelos de CI padrão اب público explícito اور afirmações do emissor لازم کرتے ہیں؛ CLI para auxiliar de liberação `--identity-token-audience` کے بغیر `--identity-token-provider` کے falhar rápido ہوتے ہیں۔  
- **Repetição determinística:** Testes atualizados fluxos de envio de manifesto positivos/negativos cobrem کرتے ہیں، یہ یقینی بناتے ہیں کہ resumos incompatíveis, falhas não determinísticas رہیں اور rede کو touch کرنے سے پہلے superfície ہوں۔  
- **Prova de contrapressão de streaming:** Torii com itens PoR/PoTR کو canais limitados پر stream کرتا ہے, اور CLI صرف amostras de latência truncadas + پانچ exemplares de falha رکھتا ہے, crescimento ilimitado de assinantes روکتا ہے جبکہ resumos determinísticos برقرار رکھتا ہے۔  
- **Observabilidade:** Contadores de streaming de prova (`torii_sorafs_proof_stream_*`) اور Resumos CLI abortar motivos de captura کرتے ہیں, جس سے operadores کے لیے auditar breadcrumbs ملتے ہیں۔  
- **Documentação:** Guias do desenvolvedor ([índice do desenvolvedor](../developer-index.md), [referência CLI](../developer-cli.md)) sinalizadores sensíveis à segurança اور fluxos de trabalho de escalonamento کو واضح کرتے ہیں۔

## Adições à lista de verificação de lançamento

Gerentes de lançamento e candidato GA promovem کرتے وقت درج ذیل evidências **لازمی** anexar کرنا ہوگا:1. تازہ ترین سیکیورٹی ریویو memo کا hash (یہ دستاویز)۔  
2. link do ticket de remediação rastreado (exemplo: `governance/tickets/SF6-SR-2026.md`)۔  
3. `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` کا saída جس میں argumentos explícitos de público/emissor دکھیں۔  
4. Chicote de paridade کے logs capturados (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)۔  
5. تصدیق کہ Torii notas de versão میں contadores de telemetria de streaming de prova limitada شامل ہیں۔

اوپر دیے گئے artefatos جمع نہ کرنا Assinatura GA کو روکتا ہے۔

**Hashes de artefato de referência (aprovação em 20/02/2026):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Acompanhamentos excelentes

- **Atualização do modelo de ameaça:** یہ ریویو ہر سہ ماہی یا بڑے Adições de sinalizadores CLI سے پہلے دوبارہ کریں۔  
- **Cobertura difusa:** Codificações de transporte de streaming de prova کو `fuzz/proof_stream_transport` کے ذریعے fuzz کیا جاتا ہے، جو identidade، gzip، deflate اور zstd payloads کو cover کرتا ہے۔  
- **Ensaio de incidente:** comprometimento do token اور reversão do manifesto کو simular کرنے والی exercício do operador شیڈول کریں، تاکہ documentos میں procedimentos praticados refletem ہوں۔

## Aprovação

- Representante da Guilda de Engenharia de Segurança: @sec-eng (2026-02-20)  
- Representante do Grupo de Trabalho de Ferramentas: @tooling-wg (2026-02-20)

Aprovações assinadas کو liberar pacote de artefatos کے ساتھ محفوظ کریں۔