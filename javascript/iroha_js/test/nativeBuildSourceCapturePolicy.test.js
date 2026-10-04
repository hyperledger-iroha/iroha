// Synthetic Git observations and temporary public file bodies exercise source
// resource guards only. These fixtures grant no build or release authority.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { syncBuiltinESMExports } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import * as proposal from '../scripts/native-build-provenance.mjs';

const MAX_FILE = 64 * 1024 ** 2;
const identityFields = ['dev', 'ino', 'mode', 'uid', 'gid', 'nlink', 'mtimeNs', 'ctimeNs'];
function withFixture(run) {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(tmpdir(), 'reviewed-source-policy-')));
  const source = path.join(root, 'source'), target = path.join(root, 'capture');
  fs.mkdirSync(source);
  fs.writeFileSync(path.join(source, 'Cargo.lock'), 'version = 4\n');
  fs.writeFileSync(path.join(source, '.gitignore'), 'ignored/\n');
  fs.writeFileSync(path.join(source, 'tracked.txt'), Buffer.alloc(128 * 1024, 0x74));
  fs.writeFileSync(path.join(source, 'empty.txt'), '');
  fs.writeFileSync(path.join(source, 'executable.sh'), '#!/bin/sh\nexit 0\n', { mode: 0o755 });
  fs.writeFileSync(path.join(source, 'untracked.txt'), 'untracked-public-source\n');
  fs.symlinkSync('tracked.txt', path.join(source, 'link.txt'));
  const tracked = [
    ['.gitignore', '100644'], ['tracked.txt', '100644'], ['empty.txt', '100644'],
    ['executable.sh', '100755'], ['link.txt', '120000'], ['absent.txt', '100644'],
    ['optional-docs', '160000'],
  ];
  const raw = Buffer.from(tracked.map(([name, mode]) => `${mode} ${'b'.repeat(40)} 0\t${name}\0`).join(''));
  const runner = (_command, args) => {
    if (args.includes('rev-parse')) return { status: 0, stdout: Buffer.from(`${'a'.repeat(40)}\n`) };
    if (args.includes('status')) return { status: 0, stdout: Buffer.from(' M tracked.txt\0') };
    if (args.includes('--stage')) return { status: 0, stdout: Buffer.from(raw) };
    if (args.includes('--others')) return { status: 0, stdout: Buffer.from('untracked.txt\0') };
    throw Error('Unexpected synthetic Git observation');
  };
  const entries = [...tracked.map(([name, mode]) => {
    if (mode === '160000') return { path: name, kind: 'gitlink', indexObject: 'b'.repeat(40) };
    if (name === 'absent.txt') return { path: name, kind: 'absent' };
    return claim(source, name);
  }), claim(source, 'untracked.txt'), claim(source, 'Cargo.lock')];
  const policy = {
    schema: 'iroha.js.reviewed-source-capture.v1', maximumFileBytes: MAX_FILE,
    maximumTotalBytes: entries.reduce((sum, entry) => sum + (entry.sizeBytes ?? 0), 0), entries,
  };
  try { run({ root, source, target, policy, runner }); }
  finally { fs.rmSync(root, { recursive: true, force: true }); }
}
function claim(source, name) {
  const stat = fs.lstatSync(path.join(source, name), { bigint: true });
  return { path: name, kind: stat.isSymbolicLink() ? 'symlink' : 'regular', sizeBytes: Number(stat.size),
    identity: Object.fromEntries(identityFields.map(field => [field, String(stat[field])])) };
}
function patchedFs(overrides, run) {
  const originals = Object.fromEntries(Object.keys(overrides).map(key => [key, fs[key]]));
  for (const [key, wrapper] of Object.entries(overrides)) fs[key] = wrapper(originals[key]);
  syncBuiltinESMExports();
  try { run(); }
  finally { Object.assign(fs, originals); syncBuiltinESMExports(); }
}
// Synthetic roots own their own lockfile and Git observations. Do not inherit
// the official build's repository-root lock selection into a different fixture.
const options = ({ policy, runner }) => ({ env: {}, run: runner, sourceCapturePolicy: policy });
const clone = value => structuredClone(value);

test('bounded capture has identical ordinary fingerprint bytes, including root lock and optional paths', () => {
  withFixture(f => {
    const old = proposal.readNativeBuildSourceState(f.source, { env: {}, run: f.runner });
    assert.deepEqual(proposal.readNativeBuildSourceState(f.source, options(f)), old);
    const snapshot = proposal.createNativeBuildSourceSnapshot(f.source, f.target, options(f));
    try {
      assert.deepEqual(snapshot.sourceState, old);
      assert.deepEqual(proposal.verifyNativeBuildSourceSnapshot(snapshot), old);
      assert.equal(fs.lstatSync(path.join(snapshot.snapshotRoot, 'empty.txt')).size, 0);
      assert.equal(fs.lstatSync(path.join(snapshot.snapshotRoot, 'executable.sh')).mode & 0o777, 0o500);
      assert.equal(fs.readlinkSync(path.join(snapshot.snapshotRoot, 'link.txt')), 'tracked.txt');
      assert.equal(fs.existsSync(path.join(snapshot.snapshotRoot, 'optional-docs')), false);
    } finally { proposal.cleanupNativeBuildSourceSnapshot(snapshot); }
  });
});

for (const [name, mutate, error] of [
  ['oversize per-file allowance', p => { p.maximumFileBytes = MAX_FILE + 1; }, /Malformed bounded/],
  ['oversize total allowance', p => { p.maximumTotalBytes = 1024 ** 3 + 1; }, /Malformed bounded/],
  ['exhausted aggregate budget', p => { p.maximumTotalBytes--; }, /aggregate byte budget/],
  ['missing mandatory root lock', p => { p.entries = p.entries.filter(e => e.path !== 'Cargo.lock'); }, /regular root Cargo.lock/],
  ['duplicate path', p => { p.entries.push(clone(p.entries[0])); }, /Duplicate/],
  ['unsafe path', p => { p.entries[0].path = '../private'; }, /unsafe source path/],
  ['quoted unsafe identity', p => { p.entries[0].identity.ino = '01'; }, /Malformed reviewed/],
  ['overlong identity', p => { p.entries[0].identity.ino = '1'.repeat(40); }, /Malformed reviewed/],
  ['identity type substitution', p => { p.entries[0].identity.mode = String(0o120644); }, /unsafe type/],
  ['multiply linked identity', p => { p.entries[0].identity.nlink = '2'; }, /unsafe type or link/],
  ['unknown policy field', p => { p.skipIdentity = true; }, /Malformed bounded/],
]) test(`policy rejects ${name} before any file body read`, () => {
  withFixture(f => {
    const policy = clone(f.policy); mutate(policy); let reads = 0;
    patchedFs({ readSync: old => (...args) => { reads++; return old(...args); } }, () => {
      assert.throws(() => proposal.readNativeBuildSourceState(f.source, { env: {}, run: f.runner, sourceCapturePolicy: policy }), error);
    });
    assert.equal(reads, 0);
  });
});

test('same-size inode replacement rejects before reading the substituted body', () => {
  withFixture(f => {
    const file = path.join(f.source, 'tracked.txt'), old = fs.lstatSync(file, { bigint: true });
    fs.renameSync(file, `${file}.old`); fs.writeFileSync(file, Buffer.alloc(Number(old.size), 0x74));
    const replacement = fs.lstatSync(file, { bigint: true }); let replacementReads = 0;
    patchedFs({ readSync: read => (fd, ...args) => {
      if (fs.fstatSync(fd, { bigint: true }).ino === replacement.ino) replacementReads++;
      return read(fd, ...args);
    } }, () => assert.throws(() => proposal.readNativeBuildSourceState(f.source, options(f)), /identity or exact size changed/));
    assert.equal(replacementReads, 0);
  });
});

for (const name of ['tracked.txt', 'untracked.txt', 'Cargo.lock']) test(`${name} growth during held read consumes at most fixed length plus one EOF byte`, () => {
  withFixture(f => {
    const file = path.join(f.source, name), stat = fs.lstatSync(file, { bigint: true });
    let changed = false, bytesRead = 0;
    patchedFs({ readSync: read => (fd, ...args) => {
      const same = fs.fstatSync(fd, { bigint: true }).ino === stat.ino;
      if (same && !changed) { changed = true; fs.appendFileSync(file, Buffer.alloc(1024 * 1024, 0x78)); }
      const count = read(fd, ...args); if (same) bytesRead += count; return count;
    } }, () => assert.throws(() => proposal.readNativeBuildSourceState(f.source, options(f)), /grew beyond its fixed byte budget/));
    assert.equal(changed, true); assert.equal(bytesRead, Number(stat.size) + 1);
  });
});

test('held descriptor truncation cannot become a shorter admitted body', () => {
  withFixture(f => {
    const file = path.join(f.source, 'tracked.txt'), ino = fs.lstatSync(file, { bigint: true }).ino;
    let changed = false;
    patchedFs({ readSync: read => (fd, ...args) => {
      if (!changed && fs.fstatSync(fd, { bigint: true }).ino === ino) { changed = true; fs.truncateSync(file, 0); }
      return read(fd, ...args);
    } }, () => assert.throws(() => proposal.readNativeBuildSourceState(f.source, options(f)), /shortened/));
    assert.equal(changed, true);
  });
});

test('held original bytes do not admit a replacement named path after opening', () => {
  withFixture(f => {
    const file = path.join(f.source, 'tracked.txt'), ino = fs.lstatSync(file, { bigint: true }).ino;
    let changed = false;
    patchedFs({ readSync: read => (fd, ...args) => {
      if (!changed && fs.fstatSync(fd, { bigint: true }).ino === ino) {
        changed = true; fs.renameSync(file, `${file}.old`); fs.writeFileSync(file, Buffer.alloc(128 * 1024, 0x74));
      }
      return read(fd, ...args);
    } }, () => assert.throws(() => proposal.readNativeBuildSourceState(f.source, options(f)), /changed while reading held bytes/));
  });
});

test('growth during bounded snapshot copy never writes more than the held source budget', () => {
  withFixture(f => {
    const file = path.join(f.source, 'tracked.txt'), selected = f.policy.entries.find(e => e.path === 'tracked.txt');
    let changed = false, written = 0, trackedWritten = 0;
    patchedFs({ writeSync: write => (fd, buffer, offset, length, ...rest) => {
      const destination = fs.fstatSync(fd, { bigint: true }).ino !== BigInt(selected.identity.ino);
      const tracked = destination && buffer[offset] === 0x74;
      const count = write(fd, buffer, offset, length, ...rest); if (destination) written += count; if (tracked) trackedWritten += count;
      if (tracked && !changed) { changed = true; fs.appendFileSync(file, Buffer.alloc(1024 * 1024, 0x78)); }
      return count;
    } }, () => assert.throws(() => proposal.createNativeBuildSourceSnapshot(f.source, f.target, options(f)), /grew beyond its fixed byte budget/));
    assert.equal(changed, true); assert.equal(trackedWritten, selected.sizeBytes);
    assert.ok(written <= f.policy.maximumTotalBytes);
    assert.deepEqual(fs.readdirSync(f.target), []);
  });
});

test('growth between fingerprint and copy rejects before any bytes of the changed file are copied', () => {
  withFixture(f => {
    const file = path.join(f.source, 'tracked.txt'); let changed = false, copiedTracked = 0;
    patchedFs({ writeSync: write => (fd, buffer, offset, length, ...rest) => {
      const count = write(fd, buffer, offset, length, ...rest);
      if (buffer[offset] === 0x74) copiedTracked += count;
      if (!changed) { changed = true; fs.appendFileSync(file, Buffer.alloc(1024 * 1024, 0x78)); }
      return count;
    } }, () => assert.throws(() => proposal.createNativeBuildSourceSnapshot(f.source, f.target, options(f)), /identity or exact size changed/));
    assert.equal(copiedTracked, 0); assert.deepEqual(fs.readdirSync(f.target), []);
  });
});

test('partial writes are completed without exceeding the fixed body budget', () => {
  withFixture(f => {
    let written = 0;
    patchedFs({ writeSync: write => (fd, buffer, offset, length, ...rest) => {
      const count = write(fd, buffer, offset, Math.max(1, Math.floor(length / 2)), ...rest); written += count; return count;
    } }, () => {
      const snapshot = proposal.createNativeBuildSourceSnapshot(f.source, f.target, options(f));
      try { assert.deepEqual(proposal.verifyNativeBuildSourceSnapshot(snapshot), snapshot.sourceState); }
      finally { proposal.cleanupNativeBuildSourceSnapshot(snapshot); }
    });
    const regularBytes = f.policy.entries.filter(e => e.kind === 'regular').reduce((sum, e) => sum + e.sizeBytes, 0);
    assert.equal(written, regularBytes);
  });
});

test('caller mutation after normalization cannot widen held source size or identity', () => {
  withFixture(f => {
    let changed = false;
    const run = (command, args, opts) => {
      if (!changed) { changed = true; f.policy.maximumTotalBytes = 1024 ** 3; f.policy.entries[0].identity.ino = '0'; }
      return f.runner(command, args, opts);
    };
    assert.doesNotThrow(() => proposal.readNativeBuildSourceState(f.source, { env: {}, run, sourceCapturePolicy: f.policy }));
  });
});

test('missing, extra, type-swapped and changed gitlink claims reject exact roster admission', () => {
  withFixture(f => {
    for (const mutate of [
      p => { p.entries = p.entries.filter(e => e.path !== 'tracked.txt'); },
      p => { p.entries.push({ path: 'unadmitted.txt', kind: 'absent' }); },
      p => { p.entries.find(e => e.path === 'optional-docs').indexObject = 'c'.repeat(40); },
      p => { p.entries[p.entries.findIndex(e => e.path === 'tracked.txt')] = { path: 'tracked.txt', kind: 'absent' }; },
    ]) {
      const policy = clone(f.policy); mutate(policy);
      assert.throws(() => proposal.readNativeBuildSourceState(f.source, { env: {}, run: f.runner, sourceCapturePolicy: policy }), /Reviewed (?:source|absent)/);
    }
  });
});

for (const phase of ['fingerprint', 'copy']) for (const payload of ['missing.txt', 'untracked.txt']) {
  test(`${phase} rejects ${payload.length === 11 ? 'same-size' : 'grown'} symlink replacement during read`, () => {
    withFixture(f => {
      const link = path.join(f.source, 'link.txt'); let reads = 0, changed = false;
      const replaceAt = phase === 'fingerprint' ? 1 : 3;
      patchedFs({ readlinkSync: read => (file, ...args) => {
        if (String(file) === link && ++reads === replaceAt) {
          changed = true; fs.unlinkSync(link); fs.symlinkSync(payload, link);
        }
        return read(file, ...args);
      } }, () => assert.throws(() => phase === 'fingerprint'
        ? proposal.readNativeBuildSourceState(f.source, options(f))
        : proposal.createNativeBuildSourceSnapshot(f.source, f.target, options(f)),
      /Reviewed source identity or exact size changed|Reviewed source symlink changed/));
      assert.equal(changed, true);
      if (phase === 'copy') assert.deepEqual(fs.readdirSync(f.target), []);
    });
  });
}

// Renaming the parent preserves the selected leaf identity while the named
// path is genuinely absent for the second syscall. No stat result is forged.
function temporarilyHideParent(parent, read) {
  const hidden = `${parent}.temporarily-hidden`;
  fs.renameSync(parent, hidden);
  try { return read(); }
  finally { fs.renameSync(hidden, parent); }
}

for (const name of ['tracked.txt', 'link.txt']) {
  test(`fingerprint rejects ${name} disappearing after selected metadata`, () => {
    withFixture(f => {
      const file = path.join(f.source, name); let calls = 0, changed = false;
      let bodiesRead = 0;
      const selected = f.policy.entries.find(entry => entry.path === name);
      patchedFs({
        lstatSync: read => (value, ...args) => {
          if (String(value) === file && ++calls === 2) {
            changed = true;
            return temporarilyHideParent(f.source, () => read(value, ...args));
          }
          return read(value, ...args);
        },
        readSync: read => (fd, ...args) => {
          if (fs.fstatSync(fd, { bigint: true }).ino === BigInt(selected.identity.ino)) bodiesRead++;
          return read(fd, ...args);
        },
        readlinkSync: read => (value, ...args) => {
          if (String(value) === file) bodiesRead++;
          return read(value, ...args);
        },
      }, () => assert.throws(() => proposal.readNativeBuildSourceState(f.source, options(f)),
        /Reviewed source identity or exact size changed/));
      assert.equal(changed, true);
      assert.equal(bodiesRead, 0);
    });
  });
}

test('fingerprint rejects selected absent entry appearing before branch metadata', () => {
  withFixture(f => {
    const file = path.join(f.source, 'absent.txt'); let calls = 0, changed = false;
    let substitutedBodiesRead = 0, substitutedInode;
    patchedFs({
      lstatSync: read => (value, ...args) => {
        if (String(value) === file && ++calls === 2) {
          fs.writeFileSync(file, 'newly appeared unreviewed body');
          substitutedInode = read(file, { bigint: true }).ino;
          changed = true;
        }
        return read(value, ...args);
      },
      readSync: read => (fd, ...args) => {
        if (substitutedInode !== undefined && fs.fstatSync(fd, { bigint: true }).ino === substitutedInode) {
          substitutedBodiesRead++;
        }
        return read(fd, ...args);
      },
    }, () => assert.throws(() => proposal.readNativeBuildSourceState(f.source, options(f)),
      /Reviewed absent source changed/));
    assert.equal(changed, true);
    assert.equal(substitutedBodiesRead, 0);
  });
});

for (const name of ['tracked.txt', 'link.txt']) {
  test(`copy rejects ${name} disappearing after selected metadata`, () => {
    withFixture(f => {
      const file = path.join(f.source, name); let calls = 0, changed = false;
      let phaseBodiesRead = 0, laterBodiesRead = 0;
      const selected = f.policy.entries.find(entry => entry.path === name);
      // Each complete fingerprint pass makes four lstat observations:
      // selection, branch, and the two held-body/readlink identity joins.
      const secondCopyStat = 10;
      patchedFs({
        lstatSync: read => (value, ...args) => {
          if (String(value) === file && ++calls === secondCopyStat) {
            changed = true;
            return temporarilyHideParent(f.source, () => read(value, ...args));
          }
          return read(value, ...args);
        },
        readSync: read => (fd, ...args) => {
          if (changed) laterBodiesRead++;
          if (changed && fs.fstatSync(fd, { bigint: true }).ino === BigInt(selected.identity.ino)) phaseBodiesRead++;
          return read(fd, ...args);
        },
        readlinkSync: read => (value, ...args) => {
          if (changed && String(value).startsWith(`${f.source}/`)) laterBodiesRead++;
          if (changed && String(value) === file) phaseBodiesRead++;
          return read(value, ...args);
        },
      }, () => assert.throws(() => proposal.createNativeBuildSourceSnapshot(f.source, f.target, options(f)),
        /Reviewed source identity or exact size changed/));
      assert.equal(changed, true);
      assert.equal(phaseBodiesRead, 0);
      assert.equal(laterBodiesRead, 0);
      assert.deepEqual(fs.readdirSync(f.target), []);
    });
  });
}

test('snapshot verifier retains reviewed fixed length when snapshot grows during held read', () => {
  withFixture(f => {
    const snapshot = proposal.createNativeBuildSourceSnapshot(f.source, f.target, options(f));
    const file = path.join(snapshot.snapshotRoot, 'tracked.txt');
    const selected = fs.lstatSync(file, { bigint: true });
    let changed = false, bytesRead = 0;
    try {
      patchedFs({ readSync: read => (fd, ...args) => {
        const same = fs.fstatSync(fd, { bigint: true }).ino === selected.ino;
        if (same && !changed) {
          changed = true;
          fs.chmodSync(file, 0o600);
          fs.appendFileSync(file, Buffer.alloc(1024 * 1024, 0x78));
          fs.chmodSync(file, 0o400);
        }
        const count = read(fd, ...args); if (same) bytesRead += count; return count;
      } }, () => assert.throws(() => proposal.verifyNativeBuildSourceSnapshot(snapshot),
        /grew beyond its fixed byte budget/));
      assert.equal(changed, true);
      assert.equal(bytesRead, Number(selected.size) + 1);
    } finally { proposal.cleanupNativeBuildSourceSnapshot(snapshot); }
  });
});

for (const name of ['tracked.txt', 'link.txt']) {
  test(`snapshot verifier rejects ${name} disappearing after selected metadata`, () => {
    withFixture(f => {
      const snapshot = proposal.createNativeBuildSourceSnapshot(f.source, f.target, options(f));
      const file = path.join(snapshot.snapshotRoot, name);
      const selected = fs.lstatSync(file, { bigint: true });
      let calls = 0, changed = false, bodiesRead = 0;
      // Expected inventory, actual inventory and symlink inventory each read
      // the path once. Containment also stats tracked.txt as link.txt's target.
      // The next two calls are fingerprint selection and branch observation.
      const secondFingerprintStat = name === 'tracked.txt' ? 6 : 5;
      try {
        patchedFs({
          lstatSync: read => (value, ...args) => {
            if (String(value) === file && ++calls === secondFingerprintStat) {
              changed = true;
              return temporarilyHideParent(snapshot.snapshotRoot, () => read(value, ...args));
            }
            return read(value, ...args);
          },
          readSync: read => (fd, ...args) => {
            if (fs.fstatSync(fd, { bigint: true }).ino === selected.ino) bodiesRead++;
            return read(fd, ...args);
          },
        }, () => assert.throws(() => proposal.verifyNativeBuildSourceSnapshot(snapshot),
          /Reviewed source identity or exact size changed/));
        assert.equal(changed, true);
        assert.equal(bodiesRead, 0);
      } finally { proposal.cleanupNativeBuildSourceSnapshot(snapshot); }
    });
  });
}
