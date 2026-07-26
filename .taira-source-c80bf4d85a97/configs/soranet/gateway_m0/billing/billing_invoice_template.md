# SoraGlobal Gateway Invoice

Period: 2026-11
Tenant: <tenant>
Prepared: <generated UTC timestamp>

| Meter | Units | Rate (XOR) | Amount (XOR) |
| ---- | ----- | --------- | ------------ |
| http.request | <count> | 0.000005 | <calc> |
| http.egress_gib | <gib> | 0.25 | <calc> |
| dns.doh_query | <count> | 0.000003 | <calc> |
| waf.decision | <count> | 0.00002 | <calc> |
| car.verification_ms | <ms> | 0.00000001 | <calc> |
| storage.gib_month | <gib-month> | 0.18 | <calc> |

Subtotal: <xor>
Discounts/credits: <xor>
Taxes: <xor>
Total due (XOR): <xor>

Dispute window: 7 days from invoice delivery. Attach `billing_dispute_template.md` with meter identifiers, timestamps, and evidence paths. Credit memos apply to the next period after dispute closure.
