---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo de taxa de nexo
título: Nexus فیس ماڈل اپ ڈیٹس
description: `docs/source/nexus_fee_model.md` کا آئینہ، جو recibos de liquidação de pista اور superfícies de reconciliação کی دستاویز کرتا ہے۔
---

:::nota کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_fee_model.md` کی عکاسی کرتا ہے۔ جاپانی, عبرانی, ہسپانوی, پرتگالی, فرانسیسی, روسی, عربی اور اردو ترجمے migrar ہونے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Nexus é um arquivo de código aberto

یکساں roteador de liquidação اب ہر lane کے لئے recibos determinísticos محفوظ کرتا ہے تاکہ آپریٹرز débitos de gás کو Nexus فیس ماڈل کے مطابق reconciliar کر سکیں۔

- roteador کی مکمل arquitetura, política de buffer, matriz de telemetria e sequenciamento de implementação کے لئے `docs/settlement-router.md` دیکھیں۔ یہ گائیڈ وضاحت کرتا ہے کہ یہاں درج parâmetros کیسے roteiro NX-3 entregável سے جڑتے ہیں اور SREs کو produção میں roteador کی نگرانی کیسے کرنی چاہئے۔
- Configuração de ativos de gás (`pipeline.gas.units_per_gas`) میں `twap_local_per_xor` decimal, `liquidity_profile` (`tier1`, `tier2`, یا `tier3`) ou `volatility_class` (`stable`, `elevated`, `dislocated`) شامل ہیں۔ یہ roteador de liquidação de bandeiras کو feed ہوتے ہیں تاکہ حاصل ہونے والی cotação XOR, TWAP canônico اور lane کے camada de corte de cabelo سے میل کھائے۔
- ہر pagamento de gás e transação ایک `LaneSettlementReceipt` ریکارڈ کرتی ہے۔ ہر chamador de recibo کی فراہم کردہ identificador de origem, micro-quantidade local, فوری واجب الادا XOR, corte de cabelo کے بعد esperado XOR, حاصل شدہ variância (`xor_variance_micro`) اور bloco timestamp (milissegundos) محفوظ کرتا ہے۔
- Bloquear recibos de execução کو pista/espaço de dados کے حساب سے agregado کرتا ہے اور انہیں `/v1/sumeragi/status` میں `lane_settlement_commitments` کے ذریعے شائع کرتا ہے۔ totais میں `total_local_micro`, `total_xor_due_micro`, اور `total_xor_after_haircut_micro` شامل ہوتے ہیں جو bloco پر جمع کر کے exportações de reconciliação noturna کے لئے فراہم ہوتے ہیں۔
- ایک نیا `total_xor_variance_micro` contador یہ faixa کرتا ہے کہ کتنا margem de segurança استعمال ہوا (devido XOR اور expectativa pós-corte de cabelo کے درمیان فرق), اور Parâmetros de conversão determinísticos `swap_metadata` (TWAP, épsilon, perfil de liquidez, e volatilidade_class) کو دستاویز کرتا ہے تاکہ configuração de tempo de execução de auditores سے الگ entradas de cotação کی تصدیق کر سکیں۔

Consumidores `lane_settlement_commitments` کو موجودہ lane اور instantâneos de compromisso de espaço de dados کے ساتھ دیکھ سکتے ہیں تاکہ یہ تصدیق ہو کہ buffers de taxa, corte de cabelo níveis, e execução de swap configurada modelo de taxa Nexus سے میل کھاتے ہیں۔