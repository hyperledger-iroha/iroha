import React from 'react';
import ComponentCreator from '@docusaurus/ComponentCreator';

export default [
  {
    path: '/reference/torii-openapi',
    component: ComponentCreator('/reference/torii-openapi', '8d6'),
    exact: true
  },
  {
    path: '/',
    component: ComponentCreator('/', '070'),
    exact: true
  },
  {
    path: '/',
    component: ComponentCreator('/', 'c41'),
    routes: [
      {
        path: '/next',
        component: ComponentCreator('/next', 'ac6'),
        routes: [
          {
            path: '/next/tags',
            component: ComponentCreator('/next/tags', '609'),
            exact: true
          },
          {
            path: '/next/tags/acceptance',
            component: ComponentCreator('/next/tags/acceptance', '0a8'),
            exact: true
          },
          {
            path: '/next/tags/checklist',
            component: ComponentCreator('/next/tags/checklist', '4d6'),
            exact: true
          },
          {
            path: '/next/tags/sf-2-c',
            component: ComponentCreator('/next/tags/sf-2-c', 'c5d'),
            exact: true
          },
          {
            path: '/next',
            component: ComponentCreator('/next', '067'),
            routes: [
              {
                path: '/next/api/overview',
                component: ComponentCreator('/next/api/overview', '386'),
                exact: true
              },
              {
                path: '/next/da/commitments-plan',
                component: ComponentCreator('/next/da/commitments-plan', '01c'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/da/ingest-plan',
                component: ComponentCreator('/next/da/ingest-plan', '30f'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/da/replication-policy',
                component: ComponentCreator('/next/da/replication-policy', 'dfc'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/da/threat-model',
                component: ComponentCreator('/next/da/threat-model', '38c'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/deploy-guide',
                component: ComponentCreator('/next/devportal/deploy-guide', '458'),
                exact: true
              },
              {
                path: '/next/devportal/incident-runbooks',
                component: ComponentCreator('/next/devportal/incident-runbooks', '2f1'),
                exact: true
              },
              {
                path: '/next/devportal/norito-rpc-adoption',
                component: ComponentCreator('/next/devportal/norito-rpc-adoption', '906'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/observability',
                component: ComponentCreator('/next/devportal/observability', 'ce3'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/preview-feedback-log',
                component: ComponentCreator('/next/devportal/preview-feedback-log', 'c72'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/preview-feedback/w0/preview-feedback-w0-summary',
                component: ComponentCreator('/next/devportal/preview-feedback/w0/preview-feedback-w0-summary', 'f67'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/preview-feedback/w1/preview-feedback-w1-log',
                component: ComponentCreator('/next/devportal/preview-feedback/w1/preview-feedback-w1-log', '144'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/preview-feedback/w1/preview-feedback-w1-plan',
                component: ComponentCreator('/next/devportal/preview-feedback/w1/preview-feedback-w1-plan', 'f71'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/preview-feedback/w1/preview-feedback-w1-summary',
                component: ComponentCreator('/next/devportal/preview-feedback/w1/preview-feedback-w1-summary', 'd16'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/preview-feedback/w2/preview-feedback-w2-plan',
                component: ComponentCreator('/next/devportal/preview-feedback/w2/preview-feedback-w2-plan', '626'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/preview-feedback/w2/preview-feedback-w2-summary',
                component: ComponentCreator('/next/devportal/preview-feedback/w2/preview-feedback-w2-summary', 'bea'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/preview-feedback/w3/preview-feedback-w3-log',
                component: ComponentCreator('/next/devportal/preview-feedback/w3/preview-feedback-w3-log', 'ca7'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/preview-feedback/w3/preview-feedback-w3-summary',
                component: ComponentCreator('/next/devportal/preview-feedback/w3/preview-feedback-w3-summary', 'a9f'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/preview-host-exposure',
                component: ComponentCreator('/next/devportal/preview-host-exposure', '7c2'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/preview-integrity-plan',
                component: ComponentCreator('/next/devportal/preview-integrity-plan', '88b'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/preview-invite-flow',
                component: ComponentCreator('/next/devportal/preview-invite-flow', 'ca9'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/preview-invite-tracker',
                component: ComponentCreator('/next/devportal/preview-invite-tracker', 'cb7'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/public-preview-invite',
                component: ComponentCreator('/next/devportal/public-preview-invite', '3e6'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/publishing-monitoring',
                component: ComponentCreator('/next/devportal/publishing-monitoring', 'dee'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/reviewer-onboarding',
                component: ComponentCreator('/next/devportal/reviewer-onboarding', '0d9'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/security-hardening',
                component: ComponentCreator('/next/devportal/security-hardening', '176'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/torii-rpc-overview',
                component: ComponentCreator('/next/devportal/torii-rpc-overview', '137'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/devportal/try-it',
                component: ComponentCreator('/next/devportal/try-it', '543'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/finance/settlement-iso-mapping',
                component: ComponentCreator('/next/finance/settlement-iso-mapping', 'c5b'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/governance/api',
                component: ComponentCreator('/next/governance/api', '297'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/intro',
                component: ComponentCreator('/next/intro', 'fb7'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/ministry/agenda-workflow',
                component: ComponentCreator('/next/ministry/agenda-workflow', '66a'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/ministry/ai-moderation-runner',
                component: ComponentCreator('/next/ministry/ai-moderation-runner', '161'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/ministry/volunteer-briefs',
                component: ComponentCreator('/next/ministry/volunteer-briefs', 'fe3'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/confidential-assets',
                component: ComponentCreator('/next/nexus/confidential-assets', '2a0'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/confidential-gas-calibration',
                component: ComponentCreator('/next/nexus/confidential-gas-calibration', 'bca'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/nexus-bootstrap-plan',
                component: ComponentCreator('/next/nexus/nexus-bootstrap-plan', '4ac'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/nexus-default-lane-quickstart',
                component: ComponentCreator('/next/nexus/nexus-default-lane-quickstart', '988'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/nexus-elastic-lane',
                component: ComponentCreator('/next/nexus/nexus-elastic-lane', '90d'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/nexus-fee-model',
                component: ComponentCreator('/next/nexus/nexus-fee-model', 'd35'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/nexus-lane-model',
                component: ComponentCreator('/next/nexus/nexus-lane-model', 'a69'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/nexus-operations',
                component: ComponentCreator('/next/nexus/nexus-operations', 'd6b'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/nexus-operator-onboarding',
                component: ComponentCreator('/next/nexus/nexus-operator-onboarding', '6f1'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/nexus-overview',
                component: ComponentCreator('/next/nexus/nexus-overview', '185'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/nexus-refactor-plan',
                component: ComponentCreator('/next/nexus/nexus-refactor-plan', '091'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/nexus-routed-trace-audit-2026q1',
                component: ComponentCreator('/next/nexus/nexus-routed-trace-audit-2026q1', '1a3'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/nexus-settlement-faq',
                component: ComponentCreator('/next/nexus/nexus-settlement-faq', 'a96'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/nexus-spec',
                component: ComponentCreator('/next/nexus/nexus-spec', '127'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/nexus-telemetry-remediation',
                component: ComponentCreator('/next/nexus/nexus-telemetry-remediation', '7de'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/nexus/nexus-transition-notes',
                component: ComponentCreator('/next/nexus/nexus-transition-notes', 'c65'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/norito-streaming-roadmap',
                component: ComponentCreator('/next/norito-streaming-roadmap', '370'),
                exact: true
              },
              {
                path: '/next/norito/examples/',
                component: ComponentCreator('/next/norito/examples/', 'b93'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/norito/examples/call-transfer-asset',
                component: ComponentCreator('/next/norito/examples/call-transfer-asset', '44c'),
                exact: true
              },
              {
                path: '/next/norito/examples/hajimari-entrypoint',
                component: ComponentCreator('/next/norito/examples/hajimari-entrypoint', '2df'),
                exact: true
              },
              {
                path: '/next/norito/examples/nft-flow',
                component: ComponentCreator('/next/norito/examples/nft-flow', '455'),
                exact: true
              },
              {
                path: '/next/norito/examples/register-and-mint',
                component: ComponentCreator('/next/norito/examples/register-and-mint', 'fe2'),
                exact: true
              },
              {
                path: '/next/norito/examples/threshold-escrow',
                component: ComponentCreator('/next/norito/examples/threshold-escrow', '67a'),
                exact: true
              },
              {
                path: '/next/norito/examples/transfer-asset',
                component: ComponentCreator('/next/norito/examples/transfer-asset', 'dc7'),
                exact: true
              },
              {
                path: '/next/norito/getting-started',
                component: ComponentCreator('/next/norito/getting-started', 'c9a'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/norito/ledger-walkthrough',
                component: ComponentCreator('/next/norito/ledger-walkthrough', 'b8c'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/norito/overview',
                component: ComponentCreator('/next/norito/overview', '4c3'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/norito/quickstart',
                component: ComponentCreator('/next/norito/quickstart', 'd25'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/norito/streaming',
                component: ComponentCreator('/next/norito/streaming', '22b'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/norito/try-it-console',
                component: ComponentCreator('/next/norito/try-it-console', '949'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/reference',
                component: ComponentCreator('/next/reference', 'ec6'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/reference/account-address-status',
                component: ComponentCreator('/next/reference/account-address-status', '59b'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/reference/address-safety',
                component: ComponentCreator('/next/reference/address-safety', '74b'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/reference/norito-codec',
                component: ComponentCreator('/next/reference/norito-codec', '5c1'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/reference/publishing-checklist',
                component: ComponentCreator('/next/reference/publishing-checklist', 'f4d'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/reference/torii-app-api-parity',
                component: ComponentCreator('/next/reference/torii-app-api-parity', '775'),
                exact: true
              },
              {
                path: '/next/reference/torii-mcp',
                component: ComponentCreator('/next/reference/torii-mcp', '144'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/reference/torii-rapidoc',
                component: ComponentCreator('/next/reference/torii-rapidoc', '193'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/reference/torii-swagger',
                component: ComponentCreator('/next/reference/torii-swagger', '189'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sdks/android-telemetry',
                component: ComponentCreator('/next/sdks/android-telemetry', '3f1'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sdks/javascript',
                component: ComponentCreator('/next/sdks/javascript', '27a'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sdks/javascript/governance-iso-examples',
                component: ComponentCreator('/next/sdks/javascript/governance-iso-examples', '300'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sdks/nexus-app-facade',
                component: ComponentCreator('/next/sdks/nexus-app-facade', 'fc5'),
                exact: true
              },
              {
                path: '/next/sdks/nexus-quickstarts',
                component: ComponentCreator('/next/sdks/nexus-quickstarts', 'a48'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sdks/python',
                component: ComponentCreator('/next/sdks/python', 'f2d'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sdks/recipes/java-ledger-flow',
                component: ComponentCreator('/next/sdks/recipes/java-ledger-flow', '358'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sdks/recipes/javascript-connect-preview',
                component: ComponentCreator('/next/sdks/recipes/javascript-connect-preview', '814'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sdks/recipes/javascript-governance-iso',
                component: ComponentCreator('/next/sdks/recipes/javascript-governance-iso', 'b11'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sdks/recipes/javascript-ledger-flow',
                component: ComponentCreator('/next/sdks/recipes/javascript-ledger-flow', 'e57'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sdks/recipes/python-ledger-flow',
                component: ComponentCreator('/next/sdks/recipes/python-ledger-flow', '599'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sdks/recipes/rust-ledger-flow',
                component: ComponentCreator('/next/sdks/recipes/rust-ledger-flow', 'cea'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sdks/recipes/swift-ledger-flow',
                component: ComponentCreator('/next/sdks/recipes/swift-ledger-flow', 'b92'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sdks/rust',
                component: ComponentCreator('/next/sdks/rust', 'ded'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sns/address-checksum-runbook',
                component: ComponentCreator('/next/sns/address-checksum-runbook', '17f'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sns/address-display-guidelines',
                component: ComponentCreator('/next/sns/address-display-guidelines', 'fba'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sns/bulk-onboarding-toolkit',
                component: ComponentCreator('/next/sns/bulk-onboarding-toolkit', '228'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sns/governance-playbook',
                component: ComponentCreator('/next/sns/governance-playbook', 'a38'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sns/kpi-dashboard',
                component: ComponentCreator('/next/sns/kpi-dashboard', '064'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sns/local-to-global-toolkit',
                component: ComponentCreator('/next/sns/local-to-global-toolkit', 'a4c'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sns/onboarding-kit',
                component: ComponentCreator('/next/sns/onboarding-kit', '133'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sns/payment-settlement-plan',
                component: ComponentCreator('/next/sns/payment-settlement-plan', '199'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sns/registrar-api',
                component: ComponentCreator('/next/sns/registrar-api', 'a68'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sns/registry-schema',
                component: ComponentCreator('/next/sns/registry-schema', '982'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sns/regulatory/regulatory-eu-dsa-2026-03',
                component: ComponentCreator('/next/sns/regulatory/regulatory-eu-dsa-2026-03', '04b'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sns/regulatory/regulatory-eu-dsa-2027-01',
                component: ComponentCreator('/next/sns/regulatory/regulatory-eu-dsa-2027-01', '625'),
                exact: true
              },
              {
                path: '/next/sns/suffix-catalog',
                component: ComponentCreator('/next/sns/suffix-catalog', 'ee5'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sns/training-collateral',
                component: ComponentCreator('/next/sns/training-collateral', 'b90'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/capacity-reconciliation',
                component: ComponentCreator('/next/sorafs/capacity-reconciliation', 'caa'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/capacity-simulation',
                component: ComponentCreator('/next/sorafs/capacity-simulation', '957'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/chunker-conformance',
                component: ComponentCreator('/next/sorafs/chunker-conformance', 'bd3'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/chunker-profile-authoring',
                component: ComponentCreator('/next/sorafs/chunker-profile-authoring', 'd21'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/chunker-registry',
                component: ComponentCreator('/next/sorafs/chunker-registry', '590'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/chunker-registry-charter',
                component: ComponentCreator('/next/sorafs/chunker-registry-charter', 'a08'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/chunker-registry-rollout-checklist',
                component: ComponentCreator('/next/sorafs/chunker-registry-rollout-checklist', 'ea0'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/deal-engine',
                component: ComponentCreator('/next/sorafs/deal-engine', 'e7e'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/developer-ci',
                component: ComponentCreator('/next/sorafs/developer-ci', '0e9'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/developer-cli',
                component: ComponentCreator('/next/sorafs/developer-cli', '301'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/developer-deployment',
                component: ComponentCreator('/next/sorafs/developer-deployment', '82d'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/developer-index',
                component: ComponentCreator('/next/sorafs/developer-index', '6b1'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/developer-releases',
                component: ComponentCreator('/next/sorafs/developer-releases', '983'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/developer-sdk-index',
                component: ComponentCreator('/next/sorafs/developer-sdk-index', 'c9e'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/developer-sdk-rust',
                component: ComponentCreator('/next/sorafs/developer-sdk-rust', '30a'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/direct-mode-pack',
                component: ComponentCreator('/next/sorafs/direct-mode-pack', '4a3'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/dispute-revocation-runbook',
                component: ComponentCreator('/next/sorafs/dispute-revocation-runbook', '9da'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/gateway-dns-runbook',
                component: ComponentCreator('/next/sorafs/gateway-dns-runbook', '383'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/manifest-pipeline',
                component: ComponentCreator('/next/sorafs/manifest-pipeline', '316'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/migration-ledger',
                component: ComponentCreator('/next/sorafs/migration-ledger', '4d6'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/migration-roadmap',
                component: ComponentCreator('/next/sorafs/migration-roadmap', '6d1'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/multi-source-rollout',
                component: ComponentCreator('/next/sorafs/multi-source-rollout', 'e91'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/node-client-protocol',
                component: ComponentCreator('/next/sorafs/node-client-protocol', '28f'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/node-operations',
                component: ComponentCreator('/next/sorafs/node-operations', '73f'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/node-plan',
                component: ComponentCreator('/next/sorafs/node-plan', 'ecc'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/node-storage',
                component: ComponentCreator('/next/sorafs/node-storage', '942'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/observability-plan',
                component: ComponentCreator('/next/sorafs/observability-plan', '206'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/operations-playbook',
                component: ComponentCreator('/next/sorafs/operations-playbook', 'ad3'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/orchestrator-config',
                component: ComponentCreator('/next/sorafs/orchestrator-config', 'ed7'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/orchestrator-ops',
                component: ComponentCreator('/next/sorafs/orchestrator-ops', 'ccf'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/orchestrator-tuning',
                component: ComponentCreator('/next/sorafs/orchestrator-tuning', '8fe'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/pin-registry-ops',
                component: ComponentCreator('/next/sorafs/pin-registry-ops', '993'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/pin-registry-plan',
                component: ComponentCreator('/next/sorafs/pin-registry-plan', '6a5'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/pin-registry-validation-plan',
                component: ComponentCreator('/next/sorafs/pin-registry-validation-plan', '9bb'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/portal-publish-plan',
                component: ComponentCreator('/next/sorafs/portal-publish-plan', '208'),
                exact: true
              },
              {
                path: '/next/sorafs/priority-snapshot-2025-03',
                component: ComponentCreator('/next/sorafs/priority-snapshot-2025-03', '271'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/provider-admission-policy',
                component: ComponentCreator('/next/sorafs/provider-admission-policy', '039'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/provider-advert-multisource',
                component: ComponentCreator('/next/sorafs/provider-advert-multisource', '7c5'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/provider-advert-rollout',
                component: ComponentCreator('/next/sorafs/provider-advert-rollout', '020'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/quickstart',
                component: ComponentCreator('/next/sorafs/quickstart', '829'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/reference-sdk/errors',
                component: ComponentCreator('/next/sorafs/reference-sdk/errors', '120'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/repair-plan',
                component: ComponentCreator('/next/sorafs/repair-plan', '018'),
                exact: true
              },
              {
                path: '/next/sorafs/reports/ai-moderation-calibration-202602',
                component: ComponentCreator('/next/sorafs/reports/ai-moderation-calibration-202602', '433'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/reports/capacity-marketplace-validation',
                component: ComponentCreator('/next/sorafs/reports/capacity-marketplace-validation', 'ce8'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/reports/orchestrator-ga-parity',
                component: ComponentCreator('/next/sorafs/reports/orchestrator-ga-parity', '82e'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/reports/sf1-determinism',
                component: ComponentCreator('/next/sorafs/reports/sf1-determinism', '579'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/reports/sf2c-capacity-soak',
                component: ComponentCreator('/next/sorafs/reports/sf2c-capacity-soak', '904'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/reports/sf6-security-review',
                component: ComponentCreator('/next/sorafs/reports/sf6-security-review', '4da'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/reserve-ledger-digest',
                component: ComponentCreator('/next/sorafs/reserve-ledger-digest', '9f3'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/runbooks-index',
                component: ComponentCreator('/next/sorafs/runbooks-index', '9c0'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/signing-ceremony',
                component: ComponentCreator('/next/sorafs/signing-ceremony', '7f7'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/staging-manifest-playbook',
                component: ComponentCreator('/next/sorafs/staging-manifest-playbook', '379'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/storage-capacity-marketplace',
                component: ComponentCreator('/next/sorafs/storage-capacity-marketplace', '411'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/taikai-anchor-runbook',
                component: ComponentCreator('/next/sorafs/taikai-anchor-runbook', '532'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/sorafs/taikai-monitoring-dashboards',
                component: ComponentCreator('/next/sorafs/taikai-monitoring-dashboards', '3e1'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/soranet/constant-rate-profiles',
                component: ComponentCreator('/next/soranet/constant-rate-profiles', 'b4b'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/soranet/gar-jurisdictional-review',
                component: ComponentCreator('/next/soranet/gar-jurisdictional-review', '705'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/soranet/gar-operator-onboarding',
                component: ComponentCreator('/next/soranet/gar-operator-onboarding', '815'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/soranet/pq-primitives',
                component: ComponentCreator('/next/soranet/pq-primitives', '4ef'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/soranet/pq-ratchet-runbook',
                component: ComponentCreator('/next/soranet/pq-ratchet-runbook', 'd25'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/soranet/pq-rollout-plan',
                component: ComponentCreator('/next/soranet/pq-rollout-plan', '589'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/soranet/privacy-metrics-pipeline',
                component: ComponentCreator('/next/soranet/privacy-metrics-pipeline', 'ef4'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/soranet/puzzle-service-operations',
                component: ComponentCreator('/next/soranet/puzzle-service-operations', '959'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/soranet/testnet-rollout',
                component: ComponentCreator('/next/soranet/testnet-rollout', 'd45'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/next/soranet/transport',
                component: ComponentCreator('/next/soranet/transport', '93e'),
                exact: true,
                sidebar: "docs"
              }
            ]
          }
        ]
      },
      {
        path: '/',
        component: ComponentCreator('/', '98f'),
        routes: [
          {
            path: '/',
            component: ComponentCreator('/', '11f'),
            routes: [
              {
                path: '/api/overview',
                component: ComponentCreator('/api/overview', '390'),
                exact: true
              },
              {
                path: '/devportal/try-it',
                component: ComponentCreator('/devportal/try-it', 'ea1'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/intro',
                component: ComponentCreator('/intro', '98c'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/norito-streaming-roadmap',
                component: ComponentCreator('/norito-streaming-roadmap', '419'),
                exact: true
              },
              {
                path: '/norito/getting-started',
                component: ComponentCreator('/norito/getting-started', 'a95'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/norito/overview',
                component: ComponentCreator('/norito/overview', '874'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/reference',
                component: ComponentCreator('/reference', '5a9'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/reference/norito-codec',
                component: ComponentCreator('/reference/norito-codec', 'a06'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/reference/publishing-checklist',
                component: ComponentCreator('/reference/publishing-checklist', '6fd'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/reference/torii-rapidoc',
                component: ComponentCreator('/reference/torii-rapidoc', 'b72'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/reference/torii-swagger',
                component: ComponentCreator('/reference/torii-swagger', '699'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/sdks/javascript',
                component: ComponentCreator('/sdks/javascript', '261'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/sdks/python',
                component: ComponentCreator('/sdks/python', '14d'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/sdks/rust',
                component: ComponentCreator('/sdks/rust', '5b7'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/sorafs/manifest-pipeline',
                component: ComponentCreator('/sorafs/manifest-pipeline', 'e62'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/sorafs/node-operations',
                component: ComponentCreator('/sorafs/node-operations', '25d'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/sorafs/pin-registry-ops',
                component: ComponentCreator('/sorafs/pin-registry-ops', '262'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/sorafs/quickstart',
                component: ComponentCreator('/sorafs/quickstart', '707'),
                exact: true,
                sidebar: "docs"
              },
              {
                path: '/sorafs/staging-manifest-playbook',
                component: ComponentCreator('/sorafs/staging-manifest-playbook', '408'),
                exact: true,
                sidebar: "docs"
              }
            ]
          }
        ]
      }
    ]
  },
  {
    path: '*',
    component: ComponentCreator('*'),
  },
];
