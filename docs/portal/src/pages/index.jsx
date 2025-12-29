import React from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';

const highlights = [
  {
    title: 'Ship deterministic artifacts',
    description:
      'Use the SoraFS chunker CLI and signed fixtures to generate reproducible manifests for cross-language toolchains.',
    href: '/sorafs/quickstart',
    cta: 'SoraFS quickstart'
  },
  {
    title: 'Author Norito smart contracts',
    description:
      'Compile Kotodama `.ko` sources into IVM bytecode, validate manifests, and deploy to Iroha or Nexus lanes with confidence.',
    href: '/norito/getting-started',
    cta: 'Norito guide'
  },
  {
    title: 'Operate production clusters',
    description:
      'Follow governance updates, telemetry guides, and operations runbooks drawn from the Nexus and Sumeragi programmes.',
    href: '/reference',
    cta: 'Reference hub'
  }
];

const quickStarts = [
  {
    title: 'Norito pipeline',
    steps: [
      'Install Kotodama and Norito toolchains',
      'Compile contracts to `.to` bytecode',
      'Attach manifests and publish through Torii'
    ],
    href: '/norito/getting-started'
  },
  {
    title: 'SoraFS publishing',
    steps: [
      'Chunk assets with the `sorafs.sf1@1.0.0` profile',
      'Sign `manifest_blake3.json` with council keys',
      'Advertise availability through provider manifests'
    ],
    href: '/sorafs/quickstart'
  },
  {
    title: 'Client SDK integration',
    steps: [
      'Select Rust, Python, or TypeScript bindings',
      'Authenticate with Torii and ingest OpenAPI specs',
      'Stream blocks, events, and query responses'
    ],
    href: '/sdks/rust'
  }
];

const resourceLinks = [
  {
    label: 'Torii API overview',
    href: '/api/overview',
    description: 'REST surface, pagination, and OpenAPI explorer.'
  },
  {
    label: 'Norito codec reference',
    href: '/reference/norito-codec',
    description: 'Binary and JSON layout guarantees for Norito payloads.'
  },
  {
    label: 'Norito streaming roadmap',
    href: '/norito-streaming-roadmap',
    description: 'Planned streaming upgrades for contract authors.'
  },
  {
    label: 'Norito streaming codec',
    href: '/norito/streaming',
    description: 'Control-plane, HPKE suites, and runtime knobs for live media.'
  },
  {
    label: 'Rust SDK quickstart',
    href: '/sdks/rust',
    description: 'Build end-to-end clients with the Rust bindings.'
  },
  {
    label: 'Python SDK quickstart',
    href: '/sdks/python',
    description: 'Interact with Torii using the typed Python SDK.'
  },
  {
    label: 'JavaScript SDK overview',
    href: '/sdks/javascript',
    description: 'Use Connect helpers in browsers and Node.js runtimes.'
  }
];

export default function Home() {
  const midpoint = Math.ceil(resourceLinks.length / 2);
  const resourceColumns = [
    resourceLinks.slice(0, midpoint),
    resourceLinks.slice(midpoint)
  ];

  return (
    <Layout
      title="SORA Nexus Developer Portal"
      description="Interactive documentation for SORA Nexus and Hyperledger Iroha">
      <header className="homeHero">
        <div className="container homeHero__content">
          <span className="homeHero__badge">Roadmap focus · SORA Nexus launch readiness</span>
          <h1 className="homeHero__title">Build on SORA Nexus with confidence</h1>
          <p className="homeHero__subtitle">
            Unified documentation for Norito contracts, Torii APIs, SDKs, and operational playbooks
            spanning SORA Nexus and Hyperledger Iroha 2.
          </p>
          <div className="homeHero__actions">
            <Link className="button button--secondary button--lg" to="/intro">
              Start building
            </Link>
            <Link
              className="button button--outline button--lg homeHero__action"
              to="/api/overview">
              Explore Torii API
            </Link>
            <Link
              className="button button--outline button--lg homeHero__action"
              href="https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md">
              View roadmap
            </Link>
          </div>
        </div>
      </header>
      <main className="homeMain">
        <section className="homeSection">
          <div className="container">
            <h2 className="homeSection__title">Build with confidence</h2>
            <div className="homeCardGrid">
              {highlights.map(({title, description, href, cta}) => (
                <article className="homeCard" key={title}>
                  <h3 className="homeCard__title">{title}</h3>
                  <p className="homeCard__body">{description}</p>
                  <Link className="homeCard__cta" to={href}>
                    {cta} →
                  </Link>
                </article>
              ))}
            </div>
          </div>
        </section>

        <section className="homeSection homeSection--alt">
          <div className="container">
            <h2 className="homeSection__title">Production-ready workflows</h2>
            <div className="homeWorkflowGrid">
              {quickStarts.map(({title, steps, href}) => (
                <article className="homeWorkflow" key={title}>
                  <h3 className="homeWorkflow__title">{title}</h3>
                  <ol className="homeWorkflow__steps">
                    {steps.map(step => (
                      <li key={step}>{step}</li>
                    ))}
                  </ol>
                  <Link className="homeWorkflow__cta" to={href}>
                    View guide →
                  </Link>
                </article>
              ))}
            </div>
          </div>
        </section>

        <section className="homeSection">
          <div className="container">
            <h2 className="homeSection__title">Featured resources</h2>
            <div className="homeResourceGrid">
              {resourceColumns.map((column, idx) => (
                <ul className="homeResourceList" key={idx}>
                  {column.map(({label, description, href}) => (
                    <li key={href} className="homeResourceList__item">
                      <Link className="homeResourceLink" to={href}>
                        {label}
                      </Link>
                      <p className="homeResourceDescription">{description}</p>
                    </li>
                  ))}
                </ul>
              ))}
            </div>
          </div>
        </section>

        <section className="homeSection homeSection--cta">
          <div className="container">
            <div className="homeCtaCard">
              <div className="homeCtaCard__body">
                <h3>Need a guided tour?</h3>
                <p>
                  Walk through the Torii API explorer, review signed fixture bundles, and join the
                  community for roadmap discussions.
                </p>
              </div>
              <div className="homeCtaCard__actions">
                <Link className="button button--secondary button--lg" to="/api/overview">
                  OpenAPI explorer
                </Link>
                <Link
                  className="button button--outline button--lg homeHero__action"
                  href="https://discord.gg/hyperledger">
                  Join the community
                </Link>
              </div>
            </div>
          </div>
        </section>
      </main>
    </Layout>
  );
}
