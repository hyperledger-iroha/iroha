# Hyperledger Iroha Security Policy

## About This Document

This document defines how security vulnerability reporting is handled for
Hyperledger Iroha, an LF Decentralized Trust project. The process aligns with
the [LF Decentralized Trust Security Policy](https://lf-decentralized-trust.github.io/governance/governing-documents/security/).

If you are ready to report a vulnerability, use one of the channels in
[Report Intakes](#report-intakes).

## Security Team

The initial Hyperledger Iroha security triage team is:

| Name | Email ID | GitHub ID | Area/Specialty |
| ---- | -------- | --------- | -------------- |
| Makoto Takemiya | takemiya@soramitsu.co.jp | [@takemiyamakoto](https://github.com/takemiyamakoto) | Product ownership and project coordination |
| Marin Versic | versic@soramitsu.co.jp | [@mversic](https://github.com/mversic) | Technical leadership, Rust, Java/Kotlin |
| Vasily Zyabkin | zyabkin@soramitsu.co.jp | [@BAStos525](https://github.com/BAStos525) | DevOps and deployment operations |

Members are added and removed through approved pull requests to this
repository.

Responsibilities:

1. Acknowledge vulnerability reports within two business days.
2. Assess the report and ask the reporter for missing reproduction or impact
   details.
3. Open or continue a private GitHub Security Advisory when the report may be a
   security vulnerability.
4. Coordinate any embargo period with the reporter. Embargoes must not exceed
   90 days.
5. Request CVEs where applicable.
6. Prepare and publish fixed releases or mitigations.
7. Publicly disclose the issue through a GitHub Security Advisory within 48
   hours after a fixed release, unless LF Decentralized Trust security guidance
   requires a different schedule.

## Discussion Forums

Security issues should not be discussed in public GitHub issues, pull requests,
Telegram, Discord, mailing lists, or X Spaces before disclosure. Discussions
about each reported vulnerability should happen in the private GitHub Security
Advisory or a private LF Decentralized Trust coordination channel created for
the report.

## Report Intakes

Report suspected security vulnerabilities through one of these approved
channels:

- Email the [LF Decentralized Trust security list](mailto:security@lists.lfdecentralizedtrust.org).
  Include the project or repository name, a description of the issue,
  reproduction steps, affected versions, and known mitigations if available.
- Open a private [GitHub security vulnerability report](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability)
  from the Security tab of this repository.

Please do not open a public issue for a suspected vulnerability.

## CNA/CVE Reporting

Hyperledger Iroha uses GitHub Security Advisories for coordinated vulnerability
disclosure. The security team will request CVEs for qualifying vulnerabilities
through the appropriate CNA path.

## Embargo List

Hyperledger Iroha may maintain a private embargo list for downstream operators
or integrators that need advance notice of severe vulnerabilities. To request
inclusion, email the [LF Decentralized Trust security list](mailto:security@lists.lfdecentralizedtrust.org)
with the project name and the reason for the request. Requests are assessed by
the Hyperledger Iroha security team with LF Decentralized Trust staff.

## GitHub Security Advisories

Hyperledger Iroha uses GitHub Security Advisories to coordinate private
triage, patch preparation, release notes, and public vulnerability disclosure.

## Private Patch Deployment Infrastructure

When a vulnerability requires private patch development before disclosure, the
security team may use GitHub private security advisory forks or ask LF
Decentralized Trust community architects to provide private coordination
infrastructure.
