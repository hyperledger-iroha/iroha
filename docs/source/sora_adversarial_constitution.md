# SORA Adversarial Constitution

Status: design draft

This document defines a self-contained game-theoretic frame for SORA as an
opt-in network state. It treats SORA as a set of games among self-interested
actors, not as a community that depends on moral appeals. Its purpose is to
make production safer and more profitable than capture.

The core thesis is:

> SORA makes building cheaper than capture.

SORA should not be designed for honest citizens only. It should be designed so
that selfish citizens, producers, auditors, experts, and capital providers find
it safer and more profitable to build within the system than to capture it.

## 1. Constitutional Axioms

### 1.1 Cost Curves Over Purity

SORA does not assume corruption, collusion, bribery, Sybil farming, or rent
seeking can be eliminated. It assumes they will be attempted constantly.

The goal is to make every capture path:

- slower;
- more expensive;
- more visible;
- easier to challenge;
- easier to reverse;
- less profitable than production.

The target is not a pure system. The target is a hostile environment for
capture.

### 1.2 Productive Self-Interest

SORA does not ask people to be altruistic. It routes ambition, greed, status
seeking, security seeking, and capital accumulation into useful work:

- production;
- infrastructure;
- trade;
- auditing;
- expert analysis;
- liquidity and reserves;
- public works;
- temporary governance service.

The system should make the dominant strategy for ambitious actors to become
producers, builders, auditors, and temporary public servants, not permanent
rentiers.

### 1.3 No One Owns the Game Board

People can win games inside SORA. They can become rich, respected, influential,
and useful. They cannot turn wealth, expertise, tenure, family, early entry, or
temporary office into permanent sovereignty.

SORA should permit temporary earned advantage. It should reject permanent power.

## 2. Mandatory Design Test

Every grant, protocol upgrade, governance reform, treasury program, Phoenix
issuance rule, public-works proposal, or monetary-policy change must answer
three questions:

```text
1. Does this make production more attractive than extraction?
2. Does this make capture more expensive, slower, or easier to reverse?
3. Does this create permanent power, or only temporary earned advantage?
```

If the answers are weak, the proposal should be treated as a likely rent path.

A useful cultural warning label:

> A future rent path wearing nice clothes.

## 3. Core Assets and Power Separation

SORA separates safety, upside, and governance.

### 3.1 XOR

XOR is the senior stable-money layer.

Design intent:

- stable purchasing power against a basket of goods or other governed target;
- useful unit of account for wages, contracts, public works, imports, exports,
  savings, and credit;
- senior to Phoenix in stress;
- not marketed as a moonshot asset;
- not used to buy extra Parliament votes.

XOR holders should expect stability and utility, not unlimited appreciation.

### 3.2 Phoenix

Phoenix is the junior growth-claim layer.

Design intent:

- risky upside claim on SORA surplus;
- first-loss or junior buffer before XOR stability is threatened;
- reward for risk capital, productive expansion, liquidity support, and useful
  service;
- no Parliament votes;
- no permanent claim on SORA sovereignty;
- no promise of continuous yield.

Phoenix may be implemented as a separate token, but the preferred design is a
locked-XOR position or receipt:

```text
lock XOR -> receive Phoenix position
Phoenix position -> earns surplus share when reserve rules permit
Phoenix position -> has maturity, exit queue, haircut, and dilution rules
Phoenix position -> has no governance vote
```

Phoenix should not be a simple farming token. If Phoenix value depends on
continuous payout, then payout pauses can become a death-spiral signal. Phoenix
must be understood as junior risk capital: it receives upside only after XOR
stability and reserves are protected.

### 3.3 Parliament

Parliament is the governance-service layer.

Design intent:

- equal-citizen sortition;
- temporary service;
- no permanent seats;
- no extra votes from extra XOR or Phoenix;
- private and receipt-free ballots where feasible;
- challenge windows and delayed enactment for high-risk actions;
- accountability through bonds, clawbacks, public records, and later review.

Parliament power is borrowed. It is not owned.

## 4. Reserve Regimes

SORA should avoid cliff behavior by defining visible reserve regimes. Thresholds
are parameters, but the state machine should be simple.

| Regime | Meaning | Phoenix | XOR |
|---|---|---|---|
| Green | Reserves exceed target and stress metrics are normal | Surplus payouts allowed | Stable |
| Yellow | Reserves or liquidity are weakening | Payouts reduced, more surplus retained | Stable |
| Red | Reserve defense is active | Payouts stop, exits queue, haircuts may apply | Defended |
| Black | Recapitalization required | Phoenix diluted or auctioned, junior losses realized | Senior claim protected first |

Surplus distribution must be reserve-gated:

```text
distributable_surplus =
  max(0, liquid_reserves + haircut_assets - required_reserve_buffer)
```

No Phoenix payout should occur from unbacked issuance. If a reward is not backed
by real surplus, it is inflation.

## 5. Actor Model

SORA assumes actors are self-interested and often collusive. A rough heuristic
for group effectiveness is:

```text
effectiveness = people + 2 * energy + 4 * coordination
```

Coordination dominates. A small, disciplined group can beat a large passive
group.

| Actor | Wants | Likely Strategy | Capture Risk | Productive Path |
|---|---|---|---|---|
| Early capital | upside, liquidity, status, exit options | lock capital if Phoenix upside is credible | convert capital into influence | fund reserves, liquidity, and production |
| Producers | credit, customers, imports, profit | seek subsidies and demand | fake output or lobby for protection | create goods, services, jobs, exports |
| Builders | mission, status, architecture influence | build core systems and narratives | become founder priesthood | create durable public infrastructure |
| Citizens | security, belonging, status, opportunity | hold XOR, serve when drawn | passive dividend rent seeking | temporary governance and public service |
| Auditors | fees, reputation, power | inspect grants, proposals, and fraud | extortion or shirking | adversarial truth discovery |
| Experts | status, accuracy rewards, influence | write briefs and forecasts | capture the information supply chain | bonded prediction and analysis |
| Phoenix holders | surplus, buybacks, upside | support growth if payouts credible | pressure short-term extraction | junior risk capital |
| Market makers | spreads, rules, liquidity | stabilize if rules are predictable | attack weak reserve rules | provide liquidity around target |
| Rent seekers | grants, titles, permanent claims | optimize metrics and committees | turn programs into entitlements | should face negative expected value |
| External states | compliance, control, taxes | tolerate, regulate, or attack | choke points and legal pressure | trade, lawful integration, public utility |

## 6. Core Games

### 6.1 Money Game

Players want stable savings, liquidity, and upside.

Desired equilibrium:

- XOR remains stable and useful;
- Phoenix absorbs risk and receives upside only from surplus;
- people who want safety hold XOR;
- people who want upside accept Phoenix lockup and loss risk.

Failure mode:

```text
XOR marketed as moonshot -> stabilization feels like betrayal -> holders revolt
```

Countermeasure:

```text
XOR = stable money
Phoenix = risky growth claim
```

### 6.2 Production Game

Players want capital, customers, and profit.

Desired equilibrium:

- producers earn XOR revenue and Phoenix upside by creating real goods and
  services;
- public funding follows delivered outcomes, not vanity metrics;
- fake activity has negative expected value.

Failure mode:

```text
subsidy metric becomes target -> wash activity and fake productivity
```

Countermeasure:

- sublinear subsidies;
- delayed vesting;
- clawbacks;
- diverse demand requirements;
- randomized audits;
- real third-party payments;
- export or import-substitution proofs where relevant.

### 6.3 Governance Game

Players want influence, status, and protection.

Desired equilibrium:

- governance is temporary service;
- citizens can rise into governance through sortition;
- wealth cannot buy sovereignty;
- jurors can vote without giving bribers proof.

Failure mode:

```text
capital -> governance capture -> extractive rents -> collapse of trust
```

Countermeasure:

- Phoenix has no votes;
- equal-citizen sortition;
- no permanent seats;
- receipt-free ballots;
- challenge windows;
- bonded minority escalations;
- defection bounties.

### 6.4 Expert Game

Players want influence and reputation.

Desired equilibrium:

- experts compete to be accurate;
- jurors see adversarial briefs, not a single official narrative;
- experts gain reputation by being right under adversarial review.

Failure mode:

```text
experts become priesthood -> citizens rubber-stamp captured framing
```

Countermeasure:

- anyone may submit a bonded brief;
- briefs include falsifiable predictions;
- red-team briefs are required for high-risk proposals;
- bad-faith briefs can be challenged and slashed;
- brief authors gain or lose reputation after outcomes are observed.

### 6.5 Identity Game

Players want eligibility and influence while remaining pseudonymous.

Desired equilibrium:

- anonymous citizens can enter and rise;
- instant Sybil capture is expensive;
- high-risk governance requires durable pseudonymous history and social risk.

Failure mode:

```text
AI agents and capital farm aged accounts until sortition capture becomes likely
```

Countermeasure:

- persistent pseudonyms;
- aged bonds;
- service history;
- reputation decay;
- no consecutive high-risk service;
- random duty timing;
- slashed vouching;
- trust-cluster rate limits;
- optional privacy-preserving uniqueness credentials.

## 7. Anti-Collusion Mechanisms

### 7.1 Receipt-Free Voting

Private voting is not enough. Bribers must be unable to verify compliance.

SORA should use MACI-like properties where feasible:

- voters can change votes;
- later valid messages override earlier coerced messages;
- voters can rotate keys;
- voters can present decoy keys or fake credentials;
- final effective vote remains hidden from coercers;
- vote windows allow recovery from coercion.

The goal:

> A juror can safely lie to a briber.

If a briber cannot verify the vote, bribery becomes harder to enforce.

### 7.2 Whistleblower and Defection Bounties

Collusion is strongest when cartel members can trust each other. SORA should
make cartel trust unstable.

For bribery, fake productivity, auditor capture, expert capture, and Sybil
clusters, the first credible defector should receive a large reward.

One possible rule:

```text
whistleblower_bounty = max(base_bounty, alpha * protected_value_at_risk)
```

Funding sources may include:

- slashed briber bonds;
- slashed colluder bonds;
- Phoenix haircut pool;
- fraud insurance fund;
- treasury security budget.

The bounty must be large enough that betrayal is an attractive strategy from
inside the cartel.

### 7.3 Dark DAO Resistance

A Dark DAO is a trustless collusion contract that pays bribes and enforces
commitments through collateral. SORA cannot assume bribers are informal.

Countermeasures:

- receipt-free voting;
- vote override and key rotation;
- dynamic whistleblower bounties;
- delayed enactment;
- rollback and challenge paths;
- high penalties for exposed coordination.

The goal is to make the expected return on trustless collusion negative.

## 8. Production Funding Rules

Production funding should not reward easily faked metrics.

### 8.1 Negative Expected Value for Fake Activity

Every subsidy formula should ask:

```text
If a cartel fakes this metric with circular activity, does it lose money?
```

A base rule:

```text
subsidy <= value_signal * haircut
```

The haircut should be severe until a producer demonstrates durable,
diversified, non-circular demand.

### 8.2 Prefer Retrospective Funding

Where possible, SORA should pay for observed outcomes, not promises.

Preferred mechanisms:

- retroactive public goods funding;
- reverse auctions for public works;
- milestone escrow;
- matching real third-party payments;
- import-credit loans tied to delivered output;
- export-revenue recognition;
- clawbacks for fraud;
- randomized audits.

### 8.3 Avoid Direct Volume Subsidies

Do not subsidize raw transactions, signups, page views, or self-reported
activity. These are easy to farm.

Prefer signals with friction:

- repeat buyers;
- buyer diversity;
- external revenue;
- delivery proofs;
- inventory or infrastructure inspection;
- repayment history;
- independent customer attestations with slashing risk.

## 9. Auditing Game

Auditors are also self-interested actors. They may shirk, extort, collude, or
be captured.

Audit design should include:

- randomly assigned audit panels;
- second-opinion audits for large grants;
- auditor bonds;
- slashing for provable fraud;
- rewards for finding auditor corruption;
- public audit trails;
- producer appeal rights;
- rotation of audit pools;
- separation between approving auditors and investigating auditors.

Auditing should be an adversarial market, not a priesthood.

## 10. Parliament and Proposal Lifecycle

High-stakes governance should be delayed, staged, challengeable, and reversible
where possible.

An ideal lifecycle:

```text
proposal submitted
-> bonded expert briefs and red-team briefs
-> public comment and prediction market window
-> sortition roster commitment
-> private receipt-free voting
-> aggregate result
-> challenge window
-> canary or staged rollout
-> full enactment
-> retrospective review
-> clawback or rollback if needed
```

For high-risk proposals, require:

- larger juries;
- multiple independent bodies;
- longer challenge windows;
- stronger expert red-team requirements;
- explicit rollback plan;
- Phoenix first-loss exposure if the action damages reserves.

## 11. Minority Delay Without Extortion

Minority escalation can protect against capture, but free delay becomes a
rent-seeking weapon.

Rules:

- minority delay requires a challenge bond;
- bond is refunded or rewarded if the challenge finds a real defect;
- bond is partially burned if frivolous;
- repeated bad-faith challengers lose escalation privileges for a period;
- critical emergency paths require higher thresholds and post-hoc review.

The goal is to make delay a truth-discovery tool, not a veto market.

## 12. Phoenix Design Constraints

Phoenix must attract early capital without creating a permanent rentier class.

Recommended constraints:

- no Parliament votes;
- no continuous yield promise;
- reserve-gated payouts;
- maturity or exit queue;
- withdrawal cooldowns;
- stress haircuts;
- dilution before XOR impairment;
- smooth decay rather than cliff expiry;
- maximum return multiples or buyout rights where appropriate;
- program-specific vintages for high-risk projects;
- transparent reserve regime and payout formula.

Avoid a hard cliff sunset. Cliff sunsets create short-term extraction pressure.
Use smooth decay and vintage-based maturities:

```text
early Phoenix share high at launch
-> gradually declines as SORA matures
-> matures into lower-yield reserve participation or expires by rule
```

## 13. Identity and Vouching

Because SORA permits pseudonymity, identity cannot rely on real-world names.
The problem is Sybil resistance, not "false identity" in a legal-name sense.

SORA should combine:

- persistent pseudonymous citizen keys;
- aged citizenship bonds;
- service history;
- no-show and misconduct records;
- cooldowns after powerful service;
- slashed vouching;
- cluster-level introduction limits;
- privacy-preserving uniqueness credentials where acceptable.

Vouching must be costly:

```text
vouch for citizen -> bond at risk
Sybil cluster exposed -> vouching bonds slash backward through introducers
```

This makes identity farming require time, capital, and social-graph risk.

## 14. Capture Path Register

| Capture Path | Attack | Countermeasure |
|---|---|---|
| Capital to governance | Phoenix whales buy policy | Phoenix has no votes, Parliament sortition, receipt-free voting |
| Expert capture | Captured experts frame choices | Bonded adversarial briefs and red teams |
| Vote bribery | Briber buys vote | Receipt-free voting, decoy keys, vote override |
| Key sale | Briber buys private key | Key rotation, master key recovery, decoy credentials |
| Dark DAO | Smart contract enforces collusion | Dynamic bounties, receipt-freeness, delayed enactment |
| Sybil farming | AI agents farm citizenship | Aged bonds, slashed vouching, service history |
| Fake demand | Cartel circulates payments | Sublinear subsidies, haircuts, audits, clawbacks |
| Auditor capture | Auditors extort or shirk | Auditor bonds, second opinions, anti-auditor bounties |
| Minority extortion | Delays used as ransom | Bonded challenges and penalties for frivolous delay |
| Founder priesthood | Builders become permanent rulers | Term limits, open expert markets, no permanent seats |
| Hereditary rent | Early claims become tribute | Smooth decay, caps, maturity, buyout rights |

## 15. Proposal Template

Every formal SORA proposal should include this header.

```markdown
# Proposal Title

## Summary

What changes, who benefits, and what state transitions occur?

## Production Over Extraction

Does this make production more attractive than extraction?

- Productive behavior rewarded:
- Extractive behavior made less profitable:
- Metrics that could be Goodharted:
- Fake-activity cost analysis:

## Capture Cost

Does this make capture more expensive, slower, or easier to reverse?

- Capture paths introduced:
- Capture paths closed:
- Challenge window:
- Reversal or rollback path:
- Minority escalation rules:

## Temporary Advantage

Does this create permanent power, or only temporary earned advantage?

- New privileges created:
- Expiry, decay, or review:
- Governance rights affected:
- Wealth-to-power conversion risk:

## Actor Incentives

| Actor | Expected action | Benefit | Abuse path | Countermeasure |
|---|---|---|---|---|

## Expert Briefs and Forecasts

- Required expert briefs:
- Required red-team briefs:
- Falsifiable predictions:
- Bond amount:
- Slashing conditions:

## Reserve and Phoenix Impact

- XOR reserve impact:
- Phoenix payout or dilution impact:
- Reserve regime transition risk:
- Stress behavior:

## Audit and Clawback

- Audit plan:
- Randomized audit rate:
- Clawback conditions:
- Whistleblower bounty:

## Rollout

- Canary stage:
- Full activation condition:
- Rollback plan:
- Sunset or review date:
```

## 16. Implementation Phases

### Phase 0: Doctrine

- Publish this constitution.
- Require the three-question design test for governance proposals.
- Document XOR as stable money and Phoenix as junior risk capital.

### Phase 1: Minimal Mechanisms

- Equal-citizen Parliament sortition.
- Reserve-gated Phoenix payout rules.
- Bonded challenge windows.
- Basic expert brief format.
- Basic producer milestone escrow and clawback rules.

### Phase 2: Anti-Collusion Hardening

- MACI-like private and receipt-free ballots.
- Vote override and key rotation.
- Whistleblower bounties.
- Slashed vouching.
- Auditor bonds and second-opinion audits.

### Phase 3: Market-Based Governance Inputs

- Bonded expert market.
- Prediction markets for proposal outcomes.
- Retroactive public goods funding.
- Sublinear matching and anti-circularity scoring.
- Public capture-risk dashboards.

### Phase 4: Full Adversarial Network State

- Mature reserve regimes.
- Phoenix vintages and smooth decay.
- High-risk proposal canaries.
- Automated challenge and rollback paths.
- Continuous adversarial simulation.

## 17. Non-Goals

SORA should not promise:

- perfect trustlessness;
- zero corruption;
- identity certainty under full anonymity;
- permanent passive income;
- a moonshot XOR price;
- technocratic rule by experts;
- permanent governance by founders, whales, or early citizens.

SORA should promise a harder thing:

> Capture is never impossible, but production should usually be cheaper.

## 18. Glossary

**Capture**: Any strategy that converts temporary advantage, capital, expertise,
or office into persistent control over SORA rules or surplus.

**Phoenix**: Junior growth claim or locked-XOR position that receives upside
from surplus and absorbs stress before XOR holders.

**XOR**: Senior stable-money unit intended for savings, wages, trade, credit,
and contracts.

**Sortition**: Random selection of eligible citizens for temporary governance
service.

**Receipt-free voting**: Voting design where a voter cannot prove to a briber
how they voted.

**Dark DAO**: A trustless collusion contract that attempts to enforce bribery
or cartel commitments.

**Negative-EV subsidy**: A funding rule where fake activity costs more to
produce than the subsidy it can extract.

**Slashed vouching**: A web-of-trust mechanism where endorsers risk bonds when
endorsed accounts are later proven to be part of a Sybil or fraud cluster.

**Temporary earned advantage**: A benefit earned through useful contribution
that expires, decays, vests, or remains subject to review.

**Permanent power**: A durable control right over governance, issuance, public
funding, or rule changes that persists without continuous productive
justification.

## 19. Final Principle

The constitutional posture is:

> Not a trustless utopia, but a hostile environment for capture.

Every implementation detail should be judged by whether it preserves that
posture.
