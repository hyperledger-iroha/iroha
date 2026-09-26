//! Ordered persistence (`specs/sumeragi.md` §12.3 O2, §7.4): safety records and block bodies
//! are written one at a time in the order the core emitted them, a failed write is retried with
//! backoff and never skipped (the instance is silent meanwhile), and the durable watermark
//! releases the O2 barrier. Also the §7.4 record-provenance steps at start: the store-id check
//! and the installation event of every `(instance, key)`.

use std::collections::VecDeque;

use iroha_sumeragi::{
    crypto::Crypto,
    message::Block,
    safety::{RecordState, SafetyRecord},
    types::{Hash32, Millis, PublicKey},
};

use super::traits::{BodyStore, LogEntry, RecordStore};

/// One durable write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Write {
    /// A safety record (`PersistSafety`).
    Record(Box<SafetyRecord>),
    /// A block body (`StoreBody`).
    Body(Box<Block>),
    /// Drop the stored bodies at or below a height after its apply.
    Prune(u64),
}

/// Retry delays after consecutive failures: `initial`, doubling, at most `max`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Backoff {
    /// Delay after the first failure.
    pub initial: Millis,
    /// Largest delay.
    pub max: Millis,
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            initial: 10,
            max: 1_000,
        }
    }
}

impl Backoff {
    /// The delay after `failures ≥ 1` consecutive failures.
    pub fn delay(&self, failures: u32) -> Millis {
        let shift = failures.saturating_sub(1).min(20);
        self.initial
            .max(1)
            .saturating_mul(1 << shift)
            .min(self.max.max(1))
    }
}

/// The ordered write queue of one instance: at most one write in flight, first in first out.
#[derive(Debug)]
pub struct PersistQueue {
    last_seq: u64,
    queue: VecDeque<(u64, Write)>,
    /// Sequence number of the write handed to the writer.
    in_flight: Option<u64>,
    failures: u32,
    retry_at: Option<Millis>,
    durable: u64,
    backoff: Backoff,
}

impl PersistQueue {
    /// An empty queue.
    pub fn new(backoff: Backoff) -> Self {
        Self {
            last_seq: 0,
            queue: VecDeque::new(),
            in_flight: None,
            failures: 0,
            retry_at: None,
            durable: 0,
            backoff,
        }
    }

    /// Queue a write; returns its sequence number.
    pub fn push(&mut self, write: Write) -> u64 {
        self.last_seq += 1;
        self.queue.push_back((self.last_seq, write));
        self.last_seq
    }

    /// The oldest queued write, handed to the writer now: none while another is in flight or
    /// before the backoff of a failed one ended.
    pub fn next(&mut self, now: Millis) -> Option<(u64, Write)> {
        if self.in_flight.is_some() || self.retry_at.is_some_and(|at| at > now) {
            return None;
        }
        let (seq, write) = self.queue.pop_front()?;
        self.in_flight = Some(seq);
        Some((seq, write))
    }

    /// Write `seq` is durable; returns the new durable watermark.
    pub fn done(&mut self, seq: u64) -> u64 {
        if self.in_flight == Some(seq) {
            self.in_flight = None;
            self.failures = 0;
            self.retry_at = None;
            self.durable = seq;
        }
        self.durable
    }

    /// Write `seq` failed: it goes back to the head and is retried after the backoff.
    pub fn failed(&mut self, seq: u64, write: Write, now: Millis) {
        if self.in_flight == Some(seq) {
            self.in_flight = None;
            self.failures = self.failures.saturating_add(1);
            self.retry_at = Some(now.saturating_add(self.backoff.delay(self.failures)));
            self.queue.push_front((seq, write));
        }
    }

    /// Everything up to this sequence number is durable.
    pub fn durable(&self) -> u64 {
        self.durable
    }

    /// When a failed write is due again (`Millis::MAX`: nothing to wait for).
    pub fn wakeup(&self) -> Millis {
        match (self.in_flight, self.retry_at) {
            (None, Some(at)) if !self.queue.is_empty() => at,
            _ => Millis::MAX,
        }
    }

    /// Writes queued or in flight.
    pub fn len(&self) -> usize {
        self.queue.len() + usize::from(self.in_flight.is_some())
    }

    /// Whether nothing is queued or in flight.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Perform one write on the stores (the persistence thread); the write comes back on failure.
///
/// # Errors
/// The write, when it did not become durable.
pub fn perform<R, B>(
    records: &R,
    bodies: &B,
    crypto: &dyn Crypto,
    write: Write,
) -> Result<(), Write>
where
    R: RecordStore + ?Sized,
    B: BodyStore + ?Sized,
{
    let result = match &write {
        Write::Record(record) => record
            .encode(crypto)
            .map_err(|e| std::io::Error::other(e.to_string()))
            .and_then(|bytes| records.write(&record.instance, &record.key, &bytes)),
        Write::Body(block) => bodies.put(&block.hash(crypto), block),
        Write::Prune(height) => bodies.prune_through(*height),
    };
    result.map_err(|error| {
        iroha_logger::warn!(%error, "sumeragi persistence failed; retrying");
        write
    })
}

/// Append a log entry with a fresh store id: the id file first, then the entry (§7.4 rule 3).
fn append<R: RecordStore + ?Sized>(
    store: &R,
    entry: impl FnOnce(u128) -> LogEntry,
    fresh_id: &mut dyn FnMut() -> u128,
    log: &mut Vec<LogEntry>,
) -> std::io::Result<()> {
    let id = fresh_id();
    store.set_store_id(id)?;
    let entry = entry(id);
    store.append_log(&entry)?;
    log.push(entry);
    Ok(())
}

/// The §7.4 record-provenance steps when instance `instance` starts with `keys` (configured and
/// retired, `(key, retired)`), then what is on disk for each key:
///
/// 1. store-id check (rule 3): if the id next to the record files differs from the newest log
///    entry's, or one of them is missing while the log has entries, every key of the log is
///    durably marked imported;
/// 2. a key without a key entry is recorded as imported;
/// 3. installation event (rule 2) of every `(instance, key)` the log lacks: the initial record
///    `{instance, key, height: genesis_height}` is written first — only for a key generated on
///    this node or when the operator asserts it never signed for the instance
///    (`assert_fresh`), and never over an existing record file — then the entry.
///
/// # Errors
/// A store failure; the driver does not start.
pub fn install_records<R: RecordStore + ?Sized>(
    store: &R,
    crypto: &dyn Crypto,
    instance: &Hash32,
    keys: &[(PublicKey, bool)],
    genesis_height: u64,
    assert_fresh: bool,
    fresh_id: &mut dyn FnMut() -> u128,
) -> std::io::Result<Vec<(PublicKey, RecordState, bool)>> {
    let mut log = store.log()?;
    let newest = log.last().map(LogEntry::store_id);
    if !log.is_empty() && newest != store.store_id()? {
        let mut seen: Vec<PublicKey> = Vec::new();
        for entry in &log {
            if !seen.contains(entry.key()) {
                seen.push(entry.key().clone());
            }
        }
        for key in seen {
            append(
                store,
                |store_id| LogEntry::Key {
                    key,
                    generated: false,
                    store_id,
                },
                fresh_id,
                &mut log,
            )?;
        }
    }
    let mut out = Vec::with_capacity(keys.len());
    for (key, retired) in keys {
        let known = log
            .iter()
            .any(|e| matches!(e, LogEntry::Key { key: k, .. } if k == key));
        if !known {
            let key = key.clone();
            append(
                store,
                |store_id| LogEntry::Key {
                    key,
                    generated: false,
                    store_id,
                },
                fresh_id,
                &mut log,
            )?;
        }
        let generated = log
            .iter()
            .rev()
            .find_map(|e| match e {
                LogEntry::Key {
                    key: k, generated, ..
                } if k == key => Some(*generated),
                _ => None,
            })
            .unwrap_or(false);
        let started = log.iter().any(
            |e| matches!(e, LogEntry::Instance { instance: i, key: k, .. } if i == instance && k == key),
        );
        if !started {
            let absent = store.load(instance, key)? == RecordState::Absent;
            if absent && (generated || assert_fresh) {
                let record = SafetyRecord::fresh(*instance, key.clone(), genesis_height, None);
                let bytes = record
                    .encode(crypto)
                    .map_err(|e| std::io::Error::other(e.to_string()))?;
                store.write(instance, key, &bytes)?;
            }
            let key = key.clone();
            append(
                store,
                |store_id| LogEntry::Instance {
                    instance: *instance,
                    key,
                    store_id,
                },
                fresh_id,
                &mut log,
            )?;
        }
        out.push((key.clone(), store.load(instance, key)?, *retired));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use iroha_sumeragi::testing::FakeCrypto;

    use super::{super::tests::fakes::FakeRecords, *};

    fn record(height: u64) -> Write {
        let key = PublicKey::new(vec![1; 32]).unwrap();
        Write::Record(Box::new(SafetyRecord::fresh(
            Hash32::ZERO,
            key,
            height,
            None,
        )))
    }

    #[test]
    fn backoff_doubles_to_its_cap() {
        let backoff = Backoff::default();
        let delays: Vec<Millis> = (1..=9).map(|n| backoff.delay(n)).collect();
        assert_eq!(delays, vec![10, 20, 40, 80, 160, 320, 640, 1_000, 1_000]);
        assert_eq!(backoff.delay(u32::MAX), 1_000);
    }

    /// One write in flight, first in first out; the watermark follows completions.
    #[test]
    fn queue_order_and_watermark() {
        let mut queue = PersistQueue::new(Backoff::default());
        assert!(queue.is_empty());
        let a = queue.push(Write::Prune(1));
        let b = queue.push(record(2));
        assert_eq!((a, b), (1, 2));
        let (seq, write) = queue.next(0).unwrap();
        assert_eq!((seq, write), (1, Write::Prune(1)));
        assert!(queue.next(0).is_none(), "one write in flight");
        assert_eq!(queue.done(1), 1);
        assert_eq!(queue.next(0).map(|(s, _)| s), Some(2));
        assert_eq!(queue.done(2), 2);
        assert_eq!(queue.durable(), 2);
        assert!(queue.is_empty() && queue.next(0).is_none());
        assert_eq!(queue.done(7), 2, "an unknown completion changes nothing");
    }

    /// A failed write is retried after its backoff and never skipped: nothing behind it
    /// becomes durable first.
    #[test]
    fn retry_without_skipping() {
        let mut queue = PersistQueue::new(Backoff::default());
        queue.push(record(1));
        queue.push(Write::Prune(1));
        let (seq, write) = queue.next(100).unwrap();
        queue.failed(seq, write, 100);
        assert_eq!(queue.wakeup(), 110);
        assert!(queue.next(109).is_none(), "backing off");
        let (again, write) = queue.next(110).unwrap();
        assert_eq!(again, 1, "the failed write is retried first");
        queue.failed(again, write, 110);
        assert_eq!(queue.wakeup(), 130, "the delay doubles");
        let (again, _) = queue.next(130).unwrap();
        assert_eq!(queue.done(again), 1);
        assert_eq!(queue.wakeup(), Millis::MAX);
        assert_eq!(queue.next(130).map(|(s, _)| s), Some(2));
        assert_eq!(queue.len(), 1);
    }

    /// `perform` writes records and bodies to their stores and hands a failed write back.
    #[test]
    fn perform_writes_or_returns_the_write() {
        let crypto = FakeCrypto::new();
        let records = FakeRecords::default();
        let bodies = super::super::tests::fakes::FakeBodies::default();
        let Write::Record(r) = record(4) else {
            unreachable!()
        };
        assert!(perform(&records, &bodies, &crypto, Write::Record(r.clone())).is_ok());
        assert!(matches!(
            records.load(&r.instance, &r.key).unwrap(),
            RecordState::Present(_)
        ));
        records.fail_next(1);
        let back = perform(&records, &bodies, &crypto, Write::Record(r.clone()));
        assert_eq!(back, Err(Write::Record(r)));
    }

    /// §7.4 record provenance: the initial record only for a generated key (or with the
    /// operator assertion) and never over an existing record; a store-id mismatch makes every
    /// key imported; a started instance is never re-initialised.
    #[test]
    fn installation_events_and_store_id() {
        let crypto = FakeCrypto::new();
        let store = FakeRecords::default();
        let (i0, i1) = (Hash32([1; 32]), Hash32([2; 32]));
        let k = PublicKey::new(vec![7; 32]).unwrap();
        let mut next = 0u128;
        let mut fresh = || {
            next += 1;
            next
        };
        store.install_key(&k, true, 100);
        let got = install_records(
            &store,
            &crypto,
            &i0,
            &[(k.clone(), false)],
            0,
            false,
            &mut fresh,
        )
        .unwrap();
        assert!(matches!(got[0].1, RecordState::Present(_)), "generated key");
        // A dataspace instance created later: initial record too; the log is consistent.
        let backup = store.snapshot_log();
        install_records(
            &store,
            &crypto,
            &i1,
            &[(k.clone(), false)],
            0,
            false,
            &mut fresh,
        )
        .unwrap();
        // The record store is replaced by an empty one: every key imported, records Absent.
        store.replace_records();
        let got = install_records(
            &store,
            &crypto,
            &i1,
            &[(k.clone(), true)],
            0,
            false,
            &mut fresh,
        )
        .unwrap();
        assert_eq!(got, vec![(k.clone(), RecordState::Absent, true)]);
        // The key store rolled back to before `i1`: the id differs → imported; no initial
        // record for `i1` without the assertion, one with it (the file being absent).
        store.restore_log(backup.clone());
        let got = install_records(
            &store,
            &crypto,
            &i1,
            &[(k.clone(), false)],
            0,
            false,
            &mut fresh,
        )
        .unwrap();
        assert_eq!(got[0].1, RecordState::Absent);
        store.restore_log(backup);
        store.replace_records();
        let got = install_records(
            &store,
            &crypto,
            &i1,
            &[(k.clone(), false)],
            0,
            true,
            &mut fresh,
        )
        .unwrap();
        assert!(
            matches!(got[0].1, RecordState::Present(_)),
            "operator assertion"
        );
        // Never over an existing record file.
        let bytes = SafetyRecord::fresh(i0, k.clone(), 9, None)
            .encode(&crypto)
            .unwrap();
        store.replace_records();
        store.write(&i0, &k, &bytes).unwrap();
        store.restore_log(Vec::new());
        store.install_key(&k, true, 500);
        let got = install_records(
            &store,
            &crypto,
            &i0,
            &[(k.clone(), false)],
            0,
            false,
            &mut fresh,
        )
        .unwrap();
        assert_eq!(got[0].1, RecordState::Present(bytes));
        // An unknown key is recorded as imported.
        let other = PublicKey::new(vec![8; 32]).unwrap();
        let got = install_records(
            &store,
            &crypto,
            &i0,
            &[(other.clone(), false)],
            0,
            false,
            &mut fresh,
        )
        .unwrap();
        assert_eq!(got[0].1, RecordState::Absent);
        assert!(
            store
                .snapshot_log()
                .iter()
                .any(|e| matches!(e, LogEntry::Key { key, generated: false, .. } if *key == other))
        );
    }
}
