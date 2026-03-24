[//]: # (SPDX-License-Identifier: CC-BY-4.0)

---
layout: default
title: 2026 Annual Review Hyperledger Iroha
parent: 2026
grand_parent: Project Updates
---

# Hyperledger Iroha Annual Report

## Governance Model Changes

There were no material governance-model changes for Hyperledger Iroha in 2025.
The project continues to operate under its existing maintainer model, with the
active maintainer roster published in the main repository.

## Project Lifecycle Status Recommendation

Graduated status still makes sense for Hyperledger Iroha.

## Project Health

2025 was primarily a year of release-candidate delivery, stabilization, and
continued architecture work across the shared Iroha codebase.

The repository currently ships two deployment tracks from one codebase:

- **Hyperledger Iroha 2** for permissioned and consortium deployments.
- **Hyperledger Iroha 3 (SORA Nexus)** for the Nexus-oriented deployment track built on the
  same core crates.

Throughout 2025, the main repository delivered eight published GitHub releases
on the `v2.0.0` release-candidate line, from `v2.0.0-rc.1.0` through
`v2.0.0-rc.2.1`. The codebase continues to invest in deterministic core
infrastructure, including Sumeragi consensus, Norito serialization, the Iroha
Virtual Machine (IVM), SDKs, operator tooling, and documentation.

The public documentation portal remains available at
[`docs.iroha.tech`](https://docs.iroha.tech/), and the repository now includes
versioned portal content in addition to the main documentation tree. Community
support channels remain available through the LF Decentralized Trust mailing
list, the public Telegram community chat, and Discord. In local git history,
the main repository recorded 119 commits in calendar year 2025 from 12 unique
email identities.

**Public activity snapshot (main repository / public signals, as of March 24,
2026 unless otherwise stated):**

| Signal | Value |
|--------|-------|
| GitHub stars | 483 |
| GitHub forks | 283 |
| GitHub watchers | 33 |
| Open GitHub issues | 463 |
| Published GitHub releases in 2025 | 8 |
| Local git commits in calendar year 2025 | 119 |
| Unique email identities in 2025 git history | 12 |
| Current listed maintainers | 19 |

The picture this gives is a project with real ongoing engineering output,
visible release activity, and a growing public architecture story, while still
being in an important transition period between the established Iroha 2 line
and the newer Hyperledger Iroha 3 / Sora Nexus direction.

## Hyperledger Iroha 2 / Iroha 3 (Sora Nexus) Transition

Work on the Hyperledger Iroha 2 / Iroha 3 split and the Sora Nexus direction began in July
2025. In the main repository, that work became explicitly visible later in
2025 as the shared codebase, documentation, configuration, and roadmap were
reorganized around two release lines built from the same workspace.

- **Hyperledger Iroha 2** remains the self-hosted track for consortium and private-network
  deployments.
- **Hyperledger Iroha 3 / Sora Nexus** extends the same core platform with multi-lane
  execution, governance-scoped data spaces, shared settlement and
  observability, and a broader public-network architecture.

Both versions share the same core building blocks, including the Iroha Virtual
Machine (IVM), Kotodama smart contract toolchain, Norito serialization, and the SDK surface.

As of **March 24, 2026**, Hyperledger Iroha 3 / SORA Nexus should be described as an active
pre-release engineering program rather than a finished public rollout. The
current repository already contains substantial Nexus-specific functionality,
including:

- a live public **Taira** testnet track, announced on **March 3, 2026**, as
  the current SORA Nexus integration and pre-production hardening environment;
- explicit separation between lane-free Iroha 2 mode and Nexus-enabled mode;
- dynamic lane creation and retirement tied to dataspace manifests;
- per-lane proofs and merge-ledger handling;
- data-availability policy, receipt, and manifest plumbing;
- cross-dataspace atomic-transaction support; and
- dedicated operator and onboarding documentation.

At the same time, the current roadmap makes clear that the project is still
working through launch-readiness items for the first public release, especially
runtime validation, soak and stability work, security hardening, and broader
operational polish.

The Taira announcement is important because it marks the point where Hyperledger Iroha 3 / SORA Nexus moved from an internal architecture and documentation effort into a public testnet phase. The announcement describes Taira as the first public
testnet for SORA Nexus, aimed at validating transaction flow, routing behavior,
lane-aware execution, operational observability, and overall readiness before a
mainnet rollout. It also describes Taira as the first public showcase of the
multilane, multidataspace, IVM, and zk-oriented SORA Nexus model, giving the
ecosystem a concrete public environment for integration testing rather than only
architecture drafts and roadmap items.

## Releases

Hyperledger Iroha published eight GitHub releases in 2025:

- `v2.0.0-rc.1.0` — January 27, 2025
- `v2.0.0-rc.1.1` — February 13, 2025
- `v2.0.0-rc.1.2` — March 7, 2025
- `v2.0.0-rc.1.3` — March 7, 2025
- `v2.0.0-rc.1.4` — March 13, 2025
- `v2.0.0-rc.1.5` — March 18, 2025
- `v2.0.0-rc.2.0` — May 8, 2025
- `v2.0.0-rc.2.1` — June 5, 2025

An important release-story point is that, in **July 2025**, the project
pivoted toward building a **combined Iroha 2 / Iroha 3 codebase** around the
**Iroha Virtual Machine (IVM)** rather than treating the next-generation Nexus
work as a fully separate product line. That direction was driven by
requirements from our **central banking users**, who wanted one coherent
programmable platform that could support both tightly governed permissioned
deployments and the emerging public-network / SORA Nexus direction without
splitting the virtual-machine, smart-contract, and SDK story.

This is also why the public GitHub release list for 2025 is concentrated on the
`v2.0.0` release-candidate line in the first half of the year: after the July
pivot, more engineering effort moved into the shared Iroha 2 / Iroha 3
architecture, IVM-centered portability, and the Nexus transition path that is
now visible in the current repository and testnet work.

This release history reflects steady work on the 2.0 line while the broader
shared codebase continued to evolve toward the next operational and architectural
milestones.

## Maintainer Diversity

The current `MAINTAINERS.md` in the main repository lists 19 maintainers across
product ownership, project management, technical leadership, Rust development,
SDK/client development, QA, DevOps, and documentation.

This is a healthy cross-functional base for a project of Iroha's scope, but the
organizational concentration is still high: 18 of the 19 listed maintainers use
`soramitsu.co.jp` email addresses. However, the contributor set is much broader than the maintainer roster, as visible in the 2025 git history.

The diversity challenge for Iroha is therefore not whether outside
contributions exist, but whether more outside contributors can be grown into
repeat reviewers, subsystem owners, and eventually maintainers.

## Project Adoption

Hyperledger Iroha continues to see its clearest public adoption in regulated
financial infrastructure and digital-asset platforms.

Publicly disclosed examples include:

- **Digital Kina**, the Bank of Papua New Guinea CBDC proof-of-concept,
  publicly announced as completed on January 28, 2025 in collaboration with
  Soramitsu and Japan's METI.
- **Palau Invest**, a blockchain-based savings bond system developed with the
  Government of Palau.
- **Bokolo Cash**, the Central Bank of Solomon Islands CBDC proof-of-concept,
  publicly described as running on a Hyperledger Iroha 2-based permissioned
  blockchain network.

In addition, public reporting in **Nikkei Asia** and **Dawn** in August 2025
stated that Soramitsu was working with the **State Bank of Pakistan** on a CBDC
pilot programme. That should be understood as a public signal of ongoing market
interest and pilot activity, not as a completed production deployment.

Public Soramitsu materials also describe additional Iroha-based digital-currency
and financial-infrastructure work across the Asia-Pacific region. The project's
main strength continues to be deployments that need deterministic behavior,
permissioned governance, programmable assets, and strong operator control.

The public adoption pattern is therefore fairly coherent: Iroha is strongest in
CBDC, regulated financial infrastructure, and government-adjacent digital asset
systems where policy control and operational determinism matter more than
permissionless-network branding.

## Community Engagement

The main public community touchpoints remain:

- LF Decentralized Trust mailing list
- Telegram community chat (`https://t.me/hyperledgeriroha`)
- Discord
- X account (`https://x.com/hl_iroha`)
- GitHub issues and pull requests
- The public documentation portal at `docs.iroha.tech`

The Telegram community chat remains the clearest real-time public coordination
channel for users and builders. The Taira public testnet should improve this
story materially in 2026 because it gives integrators, operators, and SDK teams
an actual public environment against which to test wallets, services, routing,
and operational assumptions.

The project also held a public **Hyperledger Iroha X Space**, which is useful
evidence that community engagement is starting to move beyond static channels
and into more active public discussion formats.

This is still an area where Iroha should become stronger. The next step is to
turn these channels and the public testnet into a more regular cadence of
release notes, public demos, operator onboarding, and contributor-facing
updates.

## Goals

### Performance Against Prior Goals

The prior annual report emphasized transaction fees, hub-chain / public-network
evolution, and broader documentation/ecosystem growth. Our assessment is:

- **Transaction-fee support:** `IN PROGRESS`
  - The current codebase contains meaningful fee-related groundwork,
    fee-aware runtime/configuration work, and telemetry surfaces, but a general
    and clearly packaged configurable transaction-fee module should still be
    treated as in progress rather than fully complete.
- **Hub-chain / public-network direction:** `SUBSTANTIAL PROGRESS`
  - This is the clearest area of forward movement. The repository now
    explicitly carries the Hyperledger Iroha 2 and Iroha 3 / SORA Nexus lines, the Nexus architecture is reflected in docs and config, and the public Taira testnet
    marks a transition from private architecture work into public validation.
- **Documentation and ecosystem contribution:** `IN PROGRESS`
  - The docs portal, versioned documentation surface, operator runbooks, and
    overview material are materially stronger than a year ago. This
    remains ongoing work rather than a finished milestone.
- **Release stabilization of the 2.0 line:** `STRONG PROGRESS`
  - In 2025 the project delivered a sustained `v2.0.0` release-candidate
    cadence and kept the core codebase, SDKs, and docs moving forward.

### Next Year's Goals

**Hyperledger Iroha 2**

- Continue the path from the `v2.0.0` release-candidate line toward a broadly
  consumable general-availability release with strong SDK and documentation
  parity.
- Continue supporting the digital-currency and regulated-finance use cases
  where Iroha already has the strongest public adoption signals.

**Hyperledger Iroha 3 / SORA Nexus**

- Use the Taira public testnet to harden consensus, routing, data availability,
  operator workflows, and SDK integration before mainnet rollout.
- Main net rollout of Hyperledger Iroha 3 / SORA Nexus.
- Continue refining the multilane, multidataspace, and governance-scoped
  execution model so the public-network architecture is operationally simple as
  well as technically ambitious.
- Complete the remaining security-hardening, runtime validation, and end-to-end
  integration work needed for external-facing services and SDK flows.

**Project-wide**

- Improve contributor and maintainer diversity beyond the current core team. Already work ot increase maintainer diversity is yielding results.
- Increase the visibility and regularity of public community engagement.
- Convert public interest around CBDC and regulated-finance pilots into deeper
  ecosystem adoption and repeat integrator activity.

### Help Required

LF Decentralized Trust can help Hyperledger Iroha by:

- Increasing visibility for the project within the broader LFDT ecosystem.
- Helping connect the project with additional adopters in regulated-finance and
  digital-asset infrastructure use cases.
- Helping attract more external contributors, reviewers, and future maintainers.

## Useful Links

- [Hyperledger Iroha repository](https://github.com/hyperledger-iroha/iroha)
- [Documentation portal](https://docs.iroha.tech/)
- [Releases](https://github.com/hyperledger-iroha/iroha/releases)
- [Maintainers](https://github.com/hyperledger-iroha/iroha/blob/main/MAINTAINERS.md)
- [Mailing list](https://lists.lfdecentralizedtrust.org/g/iroha)
- [Telegram community chat](https://t.me/hyperledgeriroha)
- [Discord](https://discord.com/channels/905194001349627914/905205848547155968)
- [Hyperledger Iroha X account](https://x.com/hl_iroha)
- [Public Hyperledger Iroha X Space](https://x.com/hl_iroha/status/1994319855672070147)
- [SORA Taira testnet announcement](https://medium.com/sora-xor/sora-taira-testnet-be8cfc924b58)
- [Taira explorer](https://taira-explorer.sora.org)
- [BPNG Digital Kina video](https://www.youtube.com/watch?v=Pmrhj_I2eEY)
- [Palau Invest announcement](https://soramitsu.co.jp/palauinvest)
- [Bokolo Cash announcement](https://soramitsu.co.jp/bokolo-cash)
- [Nikkei Asia: Pakistan's digital currency effort gets support from Japan](https://asia.nikkei.com/business/markets/currencies/pakistan-s-digital-currency-effort-gets-support-from-japan)
- [Dawn: Pakistan picks Japanese company for digital currency rollout](https://www.dawn.com/news/1930568)
