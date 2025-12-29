use crate::{
    IVM, Memory, VMError,
    memory::{AccessRange, WriteLogEntry},
};

/// Stage of execution for post-run callbacks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostRunPhase {
    /// Speculative run used for conflict detection.
    Speculative,
    /// Final run whose effects will be committed; avoid mutating VM memory here,
    /// as write logs have already been captured.
    Final,
}

pub struct Transaction {
    pub id: usize,
    pub ivm: IVM,
    pub base: IVM,
    pub read_set: Vec<AccessRange>,
    pub write_set: Vec<WriteLogEntry>,
    pub post_run: Option<Box<dyn Fn(&mut IVM, PostRunPhase) + Send + Sync + 'static>>,
    pub result: Result<(), VMError>,
}

// `Transaction` inherits `Send` from its fields but intentionally remains `!Sync`
// because it owns an `IVM`, which relies on interior mutability that is not
// thread-safe when shared.

pub fn execute_transactions_parallel(transactions: &mut [Transaction]) -> Result<Memory, VMError> {
    if transactions.is_empty() {
        return Ok(Memory::new(0));
    }

    let mut global_mem = transactions[0].base.memory.clone();

    std::thread::scope(|s| {
        for tx in transactions.iter_mut() {
            s.spawn(move || {
                tx.ivm.memory.clear_tracking();
                tx.result = tx.ivm.run_simple();
                if tx.result.is_ok() {
                    if let Some(callback) = tx.post_run.as_ref() {
                        callback(&mut tx.ivm, PostRunPhase::Speculative);
                    }
                    tx.read_set = tx.ivm.memory.read_set();
                    tx.write_set = tx.ivm.memory.write_log();
                }
            });
        }
    });

    for tx in transactions.iter() {
        if let Err(ref e) = tx.result {
            return Err(e.clone());
        }
    }

    let mut order: Vec<usize> = (0..transactions.len()).collect();
    order.sort_by_key(|&i| transactions[i].id);
    let mut committed: Vec<usize> = Vec::new();

    for &idx in &order {
        let mut conflict = false;
        for &c in &committed {
            for write in &transactions[c].write_set {
                let written_len = write.bytes.len() as u64;
                if transactions[idx]
                    .read_set
                    .iter()
                    .any(|r| ranges_overlap(r.addr, r.len, write.addr, written_len))
                    || transactions[idx].write_set.iter().any(|w| {
                        ranges_overlap(w.addr, w.bytes.len() as u64, write.addr, written_len)
                    })
                {
                    conflict = true;
                    break;
                }
            }
            if conflict {
                break;
            }
        }

        if conflict {
            transactions[idx].ivm = transactions[idx].base.clone();
            transactions[idx].ivm.memory = global_mem.clone();
            transactions[idx]
                .ivm
                .memory
                .overlay_code(&transactions[idx].base.memory);
            transactions[idx].ivm.memory.clear_tracking();
            let res = transactions[idx].ivm.run_simple();
            if let Err(ref e) = res {
                return Err(e.clone());
            }
            transactions[idx].result = res;
            if let Some(callback) = transactions[idx].post_run.as_ref() {
                callback(&mut transactions[idx].ivm, PostRunPhase::Speculative);
            }
            transactions[idx].read_set = transactions[idx].ivm.memory.read_set();
            transactions[idx].write_set = transactions[idx].ivm.memory.write_log();
        }

        if let Some(callback) = transactions[idx].post_run.as_ref() {
            callback(&mut transactions[idx].ivm, PostRunPhase::Final);
        }
        for entry in &transactions[idx].write_set {
            global_mem.store_bytes(entry.addr, &entry.bytes)?;
        }
        committed.push(idx);
    }

    Ok(global_mem)
}

fn ranges_overlap(a_start: u64, a_len: u64, b_start: u64, b_len: u64) -> bool {
    if a_len == 0 || b_len == 0 {
        return false;
    }
    let a_end = a_start.saturating_add(a_len);
    let b_end = b_start.saturating_add(b_len);
    a_start < b_end && b_start < a_end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Transaction>();
    }
}
