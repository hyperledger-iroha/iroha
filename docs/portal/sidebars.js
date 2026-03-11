/**
 * @type {import('@docusaurus/plugin-content-docs').SidebarsConfig}
 */
const sidebars = {
  docs: [
    'intro',
    {
      type: 'category',
      label: 'Norito',
      collapsed: false,
      items: [
        'norito/overview',
        'norito/quickstart',
        'norito/ledger-walkthrough',
        'norito/getting-started',
        'norito/streaming',
        'norito/try-it-console',
        'norito/examples/index'
      ]
    },
    {
      type: 'category',
      label: 'SoraFS',
      collapsed: false,
      items: [
        'sorafs/quickstart',
        'sorafs/developer-index',
        'sorafs/manifest-pipeline',
        'sorafs/signing-ceremony',
        'sorafs/priority-snapshot-2025-03',
        'sorafs/chunker-registry',
        'sorafs/chunker-registry-charter',
        'sorafs/chunker-profile-authoring',
        'sorafs/chunker-conformance',
        'sorafs/chunker-registry-rollout-checklist',
        'sorafs/observability-plan',
        'sorafs/operations-playbook',
        'sorafs/runbooks-index',
        'sorafs/reserve-ledger-digest',
        'sorafs/capacity-reconciliation',
        'sorafs/taikai-anchor-runbook',
        'sorafs/taikai-monitoring-dashboards',
        'sorafs/gateway-dns-runbook',
        'sorafs/migration-roadmap',
        'sorafs/migration-ledger',
        'sorafs/multi-source-rollout',
        'sorafs/provider-admission-policy',
        'sorafs/provider-advert-multisource',
        'sorafs/provider-advert-rollout',
        'sorafs/storage-capacity-marketplace',
        'sorafs/capacity-simulation',
        'sorafs/deal-engine',
        'sorafs/direct-mode-pack',
        'sorafs/developer-ci',
        'sorafs/developer-deployment',
        'sorafs/orchestrator-config',
        'sorafs/orchestrator-tuning',
        'sorafs/developer-cli',
        'sorafs/developer-releases',
        'sorafs/developer-sdk-index',
        'sorafs/developer-sdk-rust',
        'sorafs/orchestrator-ops',
        'sorafs/staging-manifest-playbook',
        'sorafs/dispute-revocation-runbook',
        'sorafs/node-client-protocol',
        'sorafs/node-plan',
        'sorafs/node-operations',
        'sorafs/node-storage',
        'sorafs/pin-registry-plan',
        'sorafs/pin-registry-validation-plan',
        'sorafs/pin-registry-ops',
        {
          type: 'category',
          label: 'Reports',
          items: [
            'sorafs/reports/ai-moderation-calibration-202602',
            'sorafs/reports/capacity-marketplace-validation',
            'sorafs/reports/orchestrator-ga-parity',
            'sorafs/reports/sf1-determinism',
            'sorafs/reports/sf2c-capacity-soak',
            'sorafs/reports/sf6-security-review'
          ]
        }
      ]
    },
    {
      type: 'category',
      label: 'Data Availability',
      collapsed: false,
      items: [
        'da/ingest-plan',
        'da/replication-policy',
        'da/commitments-plan',
        'da/threat-model'
      ]
    },
    {
      type: 'category',
      label: 'SoraNet',
      collapsed: false,
      items: [
        'soranet/transport',
        'soranet/constant-rate-profiles',
        'soranet/testnet-rollout',
        'soranet/gar-jurisdictional-review',
        'soranet/gar-operator-onboarding',
        'soranet/puzzle-service-operations',
        'soranet/privacy-metrics-pipeline',
        'soranet/pq-primitives',
        'soranet/pq-ratchet-runbook',
        'soranet/pq-rollout-plan'
      ]
    },
    {
      type: 'category',
      label: 'Sora Nexus',
      collapsed: false,
      items: [
        'nexus/nexus-overview',
        'nexus/nexus-spec',
        'nexus/nexus-lane-model',
        'nexus/nexus-default-lane-quickstart',
        'nexus/nexus-elastic-lane',
        'nexus/nexus-fee-model',
        'nexus/nexus-operations',
        'nexus/nexus-operator-onboarding',
        'nexus/nexus-transition-notes',
        'nexus/nexus-bootstrap-plan',
        'nexus/nexus-telemetry-remediation',
        'nexus/nexus-refactor-plan',
        'nexus/nexus-routed-trace-audit-2026q1',
        'nexus/confidential-assets',
        'nexus/confidential-gas-calibration',
        'nexus/nexus-settlement-faq'
      ]
    },
    {
      type: 'category',
      label: 'Governance',
      collapsed: false,
      items: [
        'governance/api'
      ]
    },
    {
      type: 'category',
      label: 'Finance',
      collapsed: false,
      items: [
        'finance/settlement-iso-mapping'
      ]
    },
    {
      type: 'category',
      label: 'Ministry of Information',
      collapsed: false,
      items: [
        'ministry/ai-moderation-runner',
        'ministry/agenda-workflow',
        'ministry/volunteer-briefs'
      ]
    },
    {
      type: 'category',
      label: 'Sora Name Service',
      collapsed: false,
      items: [
        'sns/registry-schema',
        'sns/suffix-catalog',
        'sns/registrar-api',
        'sns/governance-playbook',
        'sns/payment-settlement-plan',
        'sns/kpi-dashboard',
        'sns/onboarding-kit',
        'sns/training-collateral',
        'sns/address-display-guidelines',
        'sns/address-checksum-runbook',
        'sns/bulk-onboarding-toolkit',
        'sns/local-to-global-toolkit',
        {
          type: 'category',
          label: 'Regulatory notes',
          items: [
            'sns/regulatory/eu-dsa-2026-03'
          ]
        }
      ]
    },
    {
      type: 'category',
      label: 'SDK guides',
      collapsed: false,
      items: [
        'sdks/nexus-quickstarts',
        'sdks/rust',
        'sdks/python',
        'sdks/android-telemetry-redaction',
        'sdks/javascript',
        'sdks/javascript-governance-iso',
        'sdks/recipes/rust-ledger-flow',
        'sdks/recipes/python-ledger-flow',
        'sdks/recipes/javascript-ledger-flow',
        'sdks/recipes/javascript-connect-preview',
        'sdks/recipes/javascript-governance-iso',
        'sdks/recipes/swift-ledger-flow',
        'sdks/recipes/java-ledger-flow'
      ]
    },
    {
      type: 'category',
      label: 'Developer portal',
      collapsed: false,
      items: [
        'devportal/try-it',
        'devportal/torii-rpc-overview',
        'devportal/norito-rpc-adoption',
        'devportal/preview-integrity-plan',
        'devportal/security-hardening',
        'devportal/observability',
        'devportal/publishing-monitoring',
        'devportal/preview-host-exposure',
        'devportal/preview-invite-flow',
        'devportal/preview-invite-tracker',
        'devportal/preview-feedback-log',
        {
          type: 'category',
          label: 'Preview feedback',
          items: [
            'devportal/preview-feedback/w0/summary',
            'devportal/preview-feedback/w1/plan',
            'devportal/preview-feedback/w1/log',
            'devportal/preview-feedback/w1/summary',
            'devportal/preview-feedback/w2/plan',
            'devportal/preview-feedback/w2/summary',
            'devportal/preview-feedback/w3/log',
            'devportal/preview-feedback/w3/summary'
          ]
        },
        'devportal/reviewer-onboarding',
        'devportal/public-preview-invite'
      ]
    },
    {
      type: 'category',
      label: 'Reference',
      collapsed: false,
      items: [
        'reference/README',
        'reference/norito-codec',
        'reference/address-safety',
        'reference/account-address-status',
        'reference/publishing-checklist',
        'reference/torii-swagger',
        'reference/torii-mcp',
        'reference/torii-rapidoc'
      ]
    }
  ]
};

module.exports = sidebars;
