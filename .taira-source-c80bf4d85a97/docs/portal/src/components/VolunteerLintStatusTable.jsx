import React from 'react';

import lintData from '@site/src/data/volunteerLintStatus.json';

function formatIso(value) {
  if (!value) {
    return '—';
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toISOString();
}

function proposalStatus(proposal) {
  if (!proposal) {
    return 'unknown';
  }
  if (proposal.blocked > 0) {
    return 'blocked';
  }
  if (proposal.warning > 0) {
    return 'warning';
  }
  return 'pass';
}

function statusLabel(proposal) {
  const status = proposalStatus(proposal);
  switch (status) {
    case 'blocked':
      return 'Blocked (errors present)';
    case 'warning':
      return 'Warnings';
    case 'pass':
      return 'Pass';
    default:
      return 'Unknown';
  }
}

function entryStatus(entry) {
  if (!entry) {
    return 'unknown';
  }
  if (entry.errorCount > 0) {
    return 'blocked';
  }
  if (entry.warningCount > 0) {
    return 'warning';
  }
  return 'pass';
}

export default function VolunteerLintStatusTable() {
  const data = lintData ?? {};
  const proposals = Array.isArray(data.proposals) ? data.proposals : [];
  const summary = data.summary ?? {};

  if (proposals.length === 0) {
    return (
      <div className="volunteerLintStatus">
        <p>No volunteer briefs have been processed yet. Run <code>npm run generate:volunteer-lint</code> to refresh the status table.</p>
      </div>
    );
  }

  return (
    <div className="volunteerLintStatus">
      <p>
        <strong>Lint summary:</strong> {summary.totalBriefs ?? proposals.reduce((sum, p) => sum + (p.totalBriefs ?? 0), 0)} brief(s),
        &nbsp;{summary.failingBriefs ?? 0} blocked,&nbsp;
        {summary.warningBriefs ?? 0} warning-only.&nbsp;Generated{' '}
        {formatIso(data.generatedAtIso)} (CLI capture {formatIso(data.cliGeneratedAtIso)}).
      </p>
      {Array.isArray(summary.inputs) && summary.inputs.length > 0 ? (
        <p>
          <strong>Validated files:</strong>{' '}
          {summary.inputs.map((input) => (
            <code key={input} style={{marginRight: '0.5rem'}}>
              {input}
            </code>
          ))}
        </p>
      ) : null}

      <table>
        <thead>
          <tr>
            <th>Proposal</th>
            <th>Total briefs</th>
            <th>Status</th>
            <th>Support</th>
            <th>Oppose</th>
            <th>Context</th>
            <th>Other</th>
          </tr>
        </thead>
        <tbody>
          {proposals.map((proposal) => (
            <tr key={proposal.proposalId}>
              <td>{proposal.proposalId}</td>
              <td>{proposal.totalBriefs}</td>
              <td>{statusLabel(proposal)}</td>
              <td>{proposal.support}</td>
              <td>{proposal.oppose}</td>
              <td>{proposal.context}</td>
              <td>{proposal.other}</td>
            </tr>
          ))}
        </tbody>
      </table>

      {proposals.map((proposal) => (
        <details key={`details-${proposal.proposalId}`} style={{marginTop: '1rem'}}>
          <summary>
            {proposal.proposalId} ({proposal.totalBriefs} brief
            {proposal.totalBriefs === 1 ? '' : 's'})
          </summary>
          <ul>
            {proposal.entries.map((entry) => (
              <li key={`${proposal.proposalId}-${entry.briefId ?? entry.inputPath}`}>
                <strong>{entry.briefId ?? 'Unlabelled brief'}</strong> — {entry.stance} —{' '}
                {entryStatus(entry)} ({entry.errorCount} error(s), {entry.warningCount} warning(s)).
                {entry.language ? ` Language: ${entry.language}.` : ' '}
                {entry.submittedAt ? ` Submitted: ${formatIso(entry.submittedAt)}.` : ' '}
                {entry.inputPath ? (
                  <>
                    {' '}
                    Source: <code>{entry.inputPath}</code>
                  </>
                ) : null}
              </li>
            ))}
          </ul>
        </details>
      ))}
    </div>
  );
}
