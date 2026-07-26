import assert from 'node:assert/strict';
import test from 'node:test';

import {buildPortalSummary} from '../volunteer-brief-validator-lib.mjs';

test('buildPortalSummary groups proposals and normalizes entries', () => {
  const cliReport = {
    total_entries: 2,
    entries_with_errors: 1,
    total_errors: 3,
    total_warnings: 1,
    generated_at_unix_ms: 1_706_000_000_000,
    inputs: ['/repo/docs/examples/ministry/volunteer_a.json'],
    entries: [
      {
        input_path: '/repo/docs/examples/ministry/volunteer_a.json',
        errors: [],
        warnings: ['abstract too long'],
        metadata: {
          brief_id: 'VB-1',
          proposal_id: 'BLACKLIST-1',
          stance: 'support',
          language: 'en',
          submitted_at: '2026-02-01T00:00:00Z'
        }
      },
      {
        input_path: '/repo/docs/examples/ministry/volunteer_b.json',
        errors: ['missing fact rows'],
        warnings: [],
        metadata: {
          brief_id: 'VB-2',
          proposal_id: 'BLACKLIST-2',
          stance: 'oppose',
          language: 'ja',
          submitted_at: '2026-02-02T00:00:00Z',
          moderation_off_topic: true
        }
      }
    ]
  };

  const summary = buildPortalSummary(cliReport, {
    repoRoot: '/repo',
    now: new Date('2026-02-15T12:00:00Z')
  });

  assert.equal(summary.version, 1);
  assert.equal(summary.summary.totalBriefs, 2);
  assert.equal(summary.summary.failingBriefs, 1);
  assert.equal(summary.summary.warningBriefs, 1);
  assert.equal(summary.summary.totalErrors, 3);
  assert.equal(summary.summary.totalWarnings, 1);
  assert.equal(summary.entries.length, 2);
  assert.equal(summary.proposals.length, 2);

  const first = summary.proposals.find((proposal) => proposal.proposalId === 'BLACKLIST-1');
  assert.ok(first);
  assert.equal(first.totalBriefs, 1);
  assert.equal(first.support, 1);
  assert.equal(first.warning, 1);
  assert.equal(first.blocked, 0);
  assert.equal(first.entries[0].inputPath, 'docs/examples/ministry/volunteer_a.json');
  assert.equal(first.entries[0].status, 'warning');

  const second = summary.proposals.find((proposal) => proposal.proposalId === 'BLACKLIST-2');
  assert.ok(second);
  assert.equal(second.totalBriefs, 1);
  assert.equal(second.blocked, 1);
  assert.equal(second.oppose, 1);
  assert.equal(second.entries[0].status, 'blocked');
  assert.equal(second.entries[0].moderationOffTopic, true);
});
