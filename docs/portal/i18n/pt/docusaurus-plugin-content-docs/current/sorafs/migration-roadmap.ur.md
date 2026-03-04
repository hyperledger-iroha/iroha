---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "SoraFS مائیگریشن روڈ میپ"
---

> [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md) سے ماخوذ۔

# SoraFS Arquivo de configuração padrão (SF-1)

یہ دستاویز `docs/source/sorafs_architecture_rfc.md` میں بیان کردہ مائیگریشن رہنمائی کو
عملی بناتی ہے۔ یہ SF-1 کے entregas کو ایسے marcos, critérios de portão e proprietário
listas de verificação
ہم آہنگ کر سکیں۔

یہ روڈ میپ جان بوجھ کر determinístico ہے: ہر marco مطلوبہ artefatos, comando
invocações e etapas de atestado کو نام دیتا ہے تاکہ pipelines downstream ایک جیسے
resultados بنائیں اور governança کے پاس trilha capaz de auditar برقرار رہے۔

## Visão geral do marco

| Marco | Janela | Objetivos primários | Deve enviar | Proprietários |
|-----------|--------|---------------|-----------|--------|
| **M1 - Aplicação Determinística** | Semanas 7 a 12 | luminárias assinadas نافذ کرنا اور alias estágio de provas کرنا جب sinalizadores de expectativa de pipelines اپنائیں۔ | verificação noturna de luminárias, manifestos assinados pelo conselho, entradas de teste de registro de alias. | Armazenamento, governança, SDKs |

Status do marco `docs/source/sorafs/migration_ledger.md` میں ٹریک ہوتا ہے۔ اس روڈ میپ میں
ہر تبدیلی لازمی طور پر ledger کو اپڈیٹ کرے تاکہ governança e engenharia de liberação ہم آہنگ رہیں۔

## Fluxos de trabalho

### 2. Adoção de fixação determinística

| Etapa | Marco | Descrição | Proprietário(s) | Saída |
|------|-----------|-------------|----------|--------|
| Ensaios de fixação | M0 | ہفتہ وار simulações جو resumos de pedaços locais کو `fixtures/sorafs_chunker` کے ساتھ comparar کریں۔ `docs/source/sorafs/reports/` میں رپورٹ شائع کریں۔ | Provedores de armazenamento | Matriz de aprovação/reprovação `determinism-<date>.md` |
| Aplicar assinaturas | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` اس وقت falha ہوں جب assinaturas یا manifestos desvio کریں۔ Dev substitui کے لیے renúncia de governança PR کے ساتھ منسلک ہونا چاہیے۔ | GT Ferramentaria | Registro CI, link de tíquete de isenção (اگر لاگو ہو). |
| Sinalizadores de expectativa | M1 | Pipelines `sorafs_manifest_stub` کو expectativas explícitas کے ساتھ کال کرتے ہیں تاکہ pino de saída ہوں: | Documentos CI | Scripts atualizados referenciando sinalizadores de expectativa (bloco de comando دیکھیں). |
| Fixação primeiro do registro | M2 | `sorafs pin propose` e `sorafs pin approve` envios de manifesto e wrap کرتے ہیں؛ CLI padrão `--require-registry` ہے۔ | Operações de Governança | Log de auditoria CLI do registro, propostas com falha e telemetria. |
| Paridade de observabilidade | M3 | Alerta de painéis Prometheus/Grafana کرتے ہیں جب manifestos de registro de inventários de blocos سے divergem ہوں؛ operações de alerta de plantão سے جڑے ہوں۔ | Observabilidade | Link do painel, IDs de regras de alerta, resultados do GameDay. |

#### Comando de publicação canônica

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

resumo, tamanho اور CID کو انہی متوقع حوالوں سے بدلیں جو entrada do razão de migração میں ریکارڈ ہیں۔

### 3. Transição de alias para comunicações| Etapa | Marco | Descrição | Proprietário(s) | Saída |
|------|-----------|-------------|----------|--------|
| Provas de alias na preparação | M1 | Teste de registro de pinos میں reivindicações de alias رجسٹر کریں اور manifestos کے ساتھ Provas Merkle لگائیں (`--alias`). | Governança, Documentos | Manifesto do pacote de prova کے ساتھ محفوظ + comentário do razão میں alias نام۔ |
| Aplicação da prova | M2 | Gateways ایسے manifestos کو rejeitar کریں جن میں تازہ Cabeçalhos `Sora-Proof` نہ ہوں؛ CI میں `sorafs alias verify` passo شامل کریں۔ | Rede | Patch de configuração do gateway + saída CI جس میں sucesso de verificação ہو۔ |

### 4. Comunicação e auditoria

- **Disciplina de razão:** mudança de estado de ہر (desvio de fixação, envio de registro, ativação de alias)
  کو `docs/source/sorafs/migration_ledger.md` میں nota datada کے طور پر anexar کرنا لازمی ہے۔
- **Atas de governança:** sessões do conselho e alterações no registro de pinos یا políticas de alias aprovadas کریں
  Roteiro para livro-razão
- **Comunicações externas:** DevRel ہر marco پر atualizações de status شائع کرتا ہے (blog + trecho do changelog)
  جو garantias determinísticas اور cronogramas de alias کو نمایاں کریں۔

## Dependências e riscos

| Dependência | Impacto | Mitigação |
|------------|--------|-----------|
| Disponibilidade do contrato de registro de pinos | Implementação do primeiro pino M2 | M2 سے پہلے testes de repetição کے ساتھ estágio de contrato کریں؛ regressões ختم ہونے تک envelope fallback برقرار رکھیں۔ |
| Chaves de assinatura do Conselho | Envelopes de manifesto e aprovações de registro کے لیے ضروری۔ | Cerimônia de assinatura `docs/source/sorafs/signing_ceremony.md` میں دستاویزی ہے؛ sobreposição کے ساتھ teclas giram کریں اور nota contábil رکھیں۔ |
| Cadência de lançamento do SDK | Clientes کو M3 سے پہلے provas de alias پر عمل کرنا ہوگا۔ | Janelas de lançamento do SDK e portas de marco کے ساتھ alinhar کریں؛ listas de verificação de migração e modelos de lançamento |

Riscos residuais e mitigações `docs/source/sorafs_architecture_rfc.md` میں بھی درج ہیں
اور ajustes کے وقت referência cruzada کرنا چاہیے۔

## Lista de verificação de critérios de saída

| Marco | Critérios |
|-----------|----------|
| M1 | - Trabalho noturno سات دن مسلسل verde۔  - Preparação de provas de alias CI میں verify۔  - Política de sinalização de expectativa de governança کی توثیق کرے۔ |

## Gerenciamento de mudanças

1. PR کے ذریعے تبدیلیاں تجویز کریں اور یہ فائل **اور**
   `docs/source/sorafs/migration_ledger.md` دونوں اپڈیٹ کریں۔
2. Descrição do PR میں minutos de governança اور evidências CI کے links شامل کریں۔
3. mesclar armazenamento + lista de discussão DevRel resumo e ações esperadas do operador بھیجیں۔

یہ طریقہ کار یقینی بناتا ہے کہ Implementação SoraFS determinística, auditável, e transparente رہے
اور Nexus لانچ میں شریک ٹیموں کے درمیان ہم آہنگی برقرار رہے۔