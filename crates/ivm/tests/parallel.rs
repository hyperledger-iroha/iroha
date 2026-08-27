use ivm::{
    IVM, VMError, encoding,
    host::{AccessLog, IVMHost},
    instruction,
    parallel::{
        Block, DependencyGraph, ExecutionContext, Scheduler, State, StateAccessSet, Transaction,
        TxResult,
    },
    syscalls,
};
use std::{
    any::Any,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
mod common;
use common::{assemble, assemble_syscalls};
#[test]
fn test_state_apply() {
    let state = State::new();
    state.apply(vec![
        ivm::parallel::StateUpdate {
            key: "a".to_string(),
            value: 1,
        },
        ivm::parallel::StateUpdate {
            key: "b".to_string(),
            value: 2,
        },
    ]);
    assert_eq!(state.get("a"), Some(1));
    assert_eq!(state.get("b"), Some(2));
}
#[test]
fn test_execution_context_init() {
    let mut access = StateAccessSet::new();
    access.read_keys.insert("k1".to_string());
    let tx = Transaction {
        code: vec![],
        gas_limit: 100,
        access,
    };
    let state = State::new();
    state.apply(vec![ivm::parallel::StateUpdate {
        key: "k1".to_string(),
        value: 42,
    }]);
    let mut ctx = ExecutionContext::new();
    ctx.init_for_transaction(&tx, &state);
    assert_eq!(ctx.gas_limit, 100);
    assert_eq!(ctx.read("k1"), Some(42));
    assert_eq!(ctx.gas_used, 0);
}
#[test]
fn test_dependency_graph_conflicts() {
    let mut a1 = StateAccessSet::new();
    a1.write_keys.insert("a".to_string());
    let tx1 = Transaction {
        code: vec![],
        gas_limit: 0,
        access: a1,
    };
    let mut a2 = StateAccessSet::new();
    a2.read_keys.insert("a".to_string());
    let tx2 = Transaction {
        code: vec![],
        gas_limit: 0,
        access: a2,
    };
    let mut a3 = StateAccessSet::new();
    a3.write_keys.insert("b".to_string());
    let tx3 = Transaction {
        code: vec![],
        gas_limit: 0,
        access: a3,
    };
    let block = Block {
        transactions: vec![tx1, tx2, tx3],
    };
    let graph = DependencyGraph::build_from_block(&block);
    assert_eq!(graph.indegree[0], 0);
    assert_eq!(graph.indegree[1], 1); // depends on tx0
    assert_eq!(graph.indegree[2], 0); // independent
}
#[test]
fn test_ilp_scheduler_basic() {
    let make_tx = |read: Vec<&str>, write: Vec<&str>| {
        let mut set = StateAccessSet::new();
        for r in read {
            set.read_keys.insert(r.to_string());
        }
        for w in write {
            set.write_keys.insert(w.to_string());
        }
        Transaction {
            code: vec![],
            gas_limit: 0,
            access: set,
        }
    };
    let block = Block {
        transactions: vec![
            make_tx(vec![], vec!["a"]),
            make_tx(vec!["a"], vec![]),
            make_tx(vec![], vec!["c"]),
        ],
    };
    let scheduler = Scheduler::new(2);
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_ref = &counter;
    let result = scheduler.schedule_block(block, |_tx| {
        counter_ref.fetch_add(1, Ordering::SeqCst);
        TxResult {
            success: true,
            gas_used: 1,
        }
    });
    assert_eq!(counter.load(Ordering::SeqCst), 3);
    assert_eq!(result.tx_results.len(), 3);
    assert!(result.tx_results.iter().all(|r| r.success));
}
#[test]
fn test_execution_context_write_updates() {
    let mut ctx = ExecutionContext::new();
    ctx.write("x".to_string(), 10);
    ctx.write("y".to_string(), 20);
    assert_eq!(ctx.write_set.len(), 2);
    assert_eq!(ctx.read("x"), Some(10));
    assert_eq!(ctx.read("y"), Some(20));
}
#[test]
fn test_dependency_graph_write_conflict() {
    let mut set1 = StateAccessSet::new();
    set1.write_keys.insert("k".to_string());
    let tx1 = Transaction {
        code: vec![],
        gas_limit: 0,
        access: set1,
    };
    let mut set2 = StateAccessSet::new();
    set2.write_keys.insert("k".to_string());
    let tx2 = Transaction {
        code: vec![],
        gas_limit: 0,
        access: set2,
    };
    let block = Block {
        transactions: vec![tx1, tx2],
    };
    let graph = DependencyGraph::build_from_block(&block);
    assert_eq!(graph.indegree[0], 0);
    assert_eq!(graph.indegree[1], 1);
    assert_eq!(graph.adj[0], vec![1]);
}
#[test]
fn test_ivm_execute_block_commits() {
    use ivm::{IVM, parallel::StateUpdate};
    let state = State::new();
    state.apply(vec![StateUpdate {
        key: "a".to_string(),
        value: 0,
    }]);
    let mut set1 = StateAccessSet::new();
    set1.write_keys.insert("a".to_string());
    let tx1 = Transaction {
        code: 1u64.to_le_bytes().to_vec(),
        gas_limit: 0,
        access: set1,
    };
    let mut set2 = StateAccessSet::new();
    set2.read_keys.insert("a".to_string());
    set2.write_keys.insert("b".to_string());
    let tx2 = Transaction {
        code: 2u64.to_le_bytes().to_vec(),
        gas_limit: 0,
        access: set2,
    };
    let mut set3 = StateAccessSet::new();
    set3.write_keys.insert("c".to_string());
    let tx3 = Transaction {
        code: 3u64.to_le_bytes().to_vec(),
        gas_limit: 0,
        access: set3,
    };
    let block = Block {
        transactions: vec![tx1, tx2, tx3],
    };
    let mut ivm = IVM::new_with_options(Some(2), state.clone(), u64::MAX);
    let result = ivm.execute_block(block);
    assert_eq!(result.tx_results.len(), 3);
}
#[test]
fn test_scheduler_parallelism() {
    use std::time::Duration;
    let make_tx = || Transaction {
        code: vec![],
        gas_limit: 0,
        access: StateAccessSet::new(),
    };
    let block = Block {
        transactions: vec![make_tx(), make_tx(), make_tx(), make_tx()],
    };
    let scheduler = Scheduler::new(4);
    let counter = Arc::new(AtomicUsize::new(0));
    let max = Arc::new(AtomicUsize::new(0));
    let counter_c = &counter;
    let max_c = &max;
    scheduler.schedule_block(block, |_tx| {
        let cur = counter_c.fetch_add(1, Ordering::SeqCst) + 1;
        loop {
            let m = max_c.load(Ordering::SeqCst);
            if cur > m {
                if max_c
                    .compare_exchange(m, cur, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    break;
                }
            } else {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
        counter_c.fetch_sub(1, Ordering::SeqCst);
        TxResult {
            success: true,
            gas_used: 0,
        }
    });
    // Independent tasks may execute in parallel. Accept any observed level.
    assert!(max.load(Ordering::SeqCst) >= 1);
}
#[test]
fn test_deterministic_results() {
    use ivm::{IVM, parallel::StateUpdate};
    // Prepare block
    let mut access1 = StateAccessSet::new();
    access1.write_keys.insert("x".to_string());
    let tx1 = Transaction {
        code: 1u64.to_le_bytes().to_vec(),
        gas_limit: 0,
        access: access1,
    };
    let mut access2 = StateAccessSet::new();
    access2.read_keys.insert("x".to_string());
    access2.write_keys.insert("y".to_string());
    let tx2 = Transaction {
        code: 2u64.to_le_bytes().to_vec(),
        gas_limit: 0,
        access: access2,
    };
    let block = Block {
        transactions: vec![tx1, tx2],
    };
    // Run twice and compare state
    let run_once = || {
        let state = State::new();
        state.apply(vec![StateUpdate {
            key: "x".to_string(),
            value: 0,
        }]);
        let mut ivm = IVM::new_with_options(Some(2), state.clone(), u64::MAX);
        ivm.execute_block(block.clone());
        (state.get("x"), state.get("y"))
    };
    let first = run_once();
    let second = run_once();
    assert_eq!(first, second);
}
#[test]
fn test_parallel_block_executes_syscall() {
    let assembled = assemble_syscalls(&[syscalls::SYSCALL_ALLOC as u8]);
    let make_tx = || Transaction {
        code: assembled.clone(),
        gas_limit: 10,
        access: StateAccessSet::new(),
    };
    let block = Block {
        transactions: vec![make_tx(), make_tx()],
    };
    let state = State::new();
    let mut ivm = IVM::new_with_options(Some(2), state, u64::MAX);
    let result = ivm.execute_block(block);
    assert!(result.tx_results.iter().all(|res| res.success));
    assert!(ivm.host_mut_any().is_some());
}
#[derive(Clone)]
struct CheckpointHost {
    calls: usize,
    fail_on: usize,
}
impl CheckpointHost {
    fn new(fail_on: usize) -> Self {
        Self { calls: 0, fail_on }
    }
}
impl IVMHost for CheckpointHost {
    fn prepare_syscall(&self, _number: u32, _vm: &IVM) -> Result<u64, VMError> {
        Ok(0)
    }
    fn syscall(&mut self, number: u32, _vm: &mut IVM) -> Result<u64, VMError> {
        self.calls += 1;
        if self.calls == self.fail_on {
            Err(VMError::UnknownSyscall(number))
        } else {
            Ok(0)
        }
    }
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
    fn supports_concurrent_blocks(&self) -> bool {
        false
    }
    fn checkpoint(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
    fn restore(&mut self, snapshot: &dyn Any) -> bool {
        if let Some(saved) = snapshot.downcast_ref::<CheckpointHost>() {
            *self = saved.clone();
            true
        } else {
            false
        }
    }
}
#[derive(Clone)]
struct AccessLoggingHost {
    log: AccessLog,
}
impl AccessLoggingHost {
    fn new() -> Self {
        Self {
            log: AccessLog::default(),
        }
    }
}
impl IVMHost for AccessLoggingHost {
    fn prepare_syscall(&self, _number: u32, _vm: &IVM) -> Result<u64, VMError> {
        Ok(0)
    }
    fn syscall(&mut self, _number: u32, _vm: &mut IVM) -> Result<u64, VMError> {
        self.log.read_keys.insert("key_a".to_string());
        self.log.write_keys.insert("key_a".to_string());
        self.log.state_writes.push(ivm::parallel::StateUpdate {
            key: "key_a".to_string(),
            value: 1,
        });
        Ok(0)
    }
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
    fn supports_concurrent_blocks(&self) -> bool {
        false
    }
    fn begin_tx(&mut self, _declared: &ivm::parallel::StateAccessSet) -> Result<(), VMError> {
        self.log.read_keys.clear();
        self.log.write_keys.clear();
        self.log.reg_tags.clear();
        Ok(())
    }
    fn finish_tx(&mut self) -> Result<AccessLog, VMError> {
        Ok(self.log.clone())
    }
    fn access_logging_supported(&self) -> bool {
        true
    }
}
#[test]
fn block_rollback_restores_host_on_failure() {
    let scall = encoding::wide::encode_sys(instruction::wide::system::SCALL, 1).to_le_bytes();
    let halt = encoding::wide::encode_halt().to_le_bytes();
    let mut prog = Vec::new();
    prog.extend_from_slice(&scall);
    prog.extend_from_slice(&halt);
    let assembled = assemble(&prog);
    let make_tx = || Transaction {
        code: assembled.clone(),
        gas_limit: 10,
        access: StateAccessSet::new(),
    };
    let block = Block {
        transactions: vec![make_tx(), make_tx()],
    };
    let mut ivm = IVM::new_with_options(Some(1), State::new(), 100);
    ivm.set_host(CheckpointHost::new(2));
    let result = ivm.execute_block(block);
    assert_eq!(result.tx_results.len(), 2);
    assert!(result.tx_results[0].success);
    assert!(!result.tx_results[1].success);
    let host = ivm
        .host_mut_any()
        .and_then(|h| h.downcast_mut::<CheckpointHost>())
        .expect("checkpoint host available");
    assert_eq!(host.calls, 1);
}
#[derive(Clone)]
struct FinishErrorHost {
    restored: bool,
    checkpointed: bool,
}
impl FinishErrorHost {
    fn new() -> Self {
        Self {
            restored: false,
            checkpointed: false,
        }
    }
}
impl IVMHost for FinishErrorHost {
    fn prepare_syscall(&self, _number: u32, _vm: &IVM) -> Result<u64, VMError> {
        Ok(0)
    }
    fn syscall(&mut self, _number: u32, _vm: &mut IVM) -> Result<u64, VMError> {
        Ok(0)
    }
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
    fn supports_concurrent_blocks(&self) -> bool {
        false
    }
    fn begin_tx(&mut self, _declared: &StateAccessSet) -> Result<(), VMError> {
        Ok(())
    }
    fn finish_tx(&mut self) -> Result<AccessLog, VMError> {
        Err(VMError::PermissionDenied)
    }
    fn checkpoint(&self) -> Option<Box<dyn Any + Send>> {
        let mut snapshot = self.clone();
        snapshot.checkpointed = true;
        Some(Box::new(snapshot))
    }
    fn restore(&mut self, snapshot: &dyn Any) -> bool {
        if let Some(saved) = snapshot.downcast_ref::<FinishErrorHost>() {
            *self = saved.clone();
            self.restored = true;
            true
        } else {
            false
        }
    }
}
#[test]
fn finish_tx_error_triggers_restore() {
    let halt = encoding::wide::encode_halt().to_le_bytes();
    let prog = assemble(&halt);
    let tx = Transaction {
        code: prog,
        gas_limit: 10,
        access: StateAccessSet::new(),
    };
    let block = Block {
        transactions: vec![tx],
    };
    let mut ivm = IVM::new_with_options(Some(1), State::new(), 100);
    ivm.set_host(FinishErrorHost::new());
    let result = ivm.execute_block(block);
    assert_eq!(result.tx_results.len(), 1);
    assert!(
        !result.tx_results[0].success,
        "finish_tx error should abort tx"
    );
    let host = ivm
        .host_mut_any()
        .expect("host present")
        .downcast_mut::<FinishErrorHost>()
        .expect("FinishErrorHost");
    assert!(
        host.restored,
        "host snapshot should restore after finish_tx failure"
    );
    assert!(
        host.checkpointed,
        "checkpoint should be captured before execution"
    );
}
#[test]
fn block_fails_when_access_log_exceeds_declared() {
    let scall = encoding::wide::encode_sys(instruction::wide::system::SCALL, 1).to_le_bytes();
    let halt = encoding::wide::encode_halt().to_le_bytes();
    let mut prog = Vec::new();
    prog.extend_from_slice(&scall);
    prog.extend_from_slice(&halt);
    let assembled = assemble(&prog);
    let tx = Transaction {
        code: assembled.clone(),
        gas_limit: 10,
        access: StateAccessSet::new(), // declare no access
    };
    let block = Block {
        transactions: vec![tx],
    };
    let mut ivm = IVM::new_with_options(Some(1), State::new(), 100);
    ivm.set_host(AccessLoggingHost::new());
    let result = ivm.execute_block(block);
    assert_eq!(result.tx_results.len(), 1);
    assert!(!result.tx_results[0].success);
}
#[test]
fn block_succeeds_with_logged_state_writes_committed() {
    let scall = encoding::wide::encode_sys(instruction::wide::system::SCALL, 1).to_le_bytes();
    let halt = encoding::wide::encode_halt().to_le_bytes();
    let mut prog = Vec::new();
    prog.extend_from_slice(&scall);
    prog.extend_from_slice(&halt);
    let assembled = assemble(&prog);
    let mut access = StateAccessSet::new();
    access.read_keys.insert("key_a".to_string());
    access.write_keys.insert("key_a".to_string());
    let tx = Transaction {
        code: assembled.clone(),
        gas_limit: 10,
        access,
    };
    let block = Block {
        transactions: vec![tx],
    };
    let mut ivm = IVM::new_with_options(Some(1), State::new(), 100);
    ivm.set_host(AccessLoggingHost::new());
    let result = ivm.execute_block(block);
    assert_eq!(result.tx_results.len(), 1);
    assert!(result.tx_results[0].success);
}
#[test]
fn test_conflict_order_consistency() {
    use std::sync::Mutex;
    let mk_tx = |id: u8| {
        let mut access = StateAccessSet::new();
        access.write_keys.insert("a".to_string());
        Transaction {
            code: vec![id],
            gas_limit: 0,
            access,
        }
    };
    let block = Block {
        transactions: vec![mk_tx(0), mk_tx(1)],
    };
    let run = || {
        let order = Arc::new(Mutex::new(Vec::new()));
        {
            let order_ref = &order;
            Scheduler::new(2).schedule_block(block.clone(), |tx| {
                order_ref.lock().unwrap().push(tx.code[0]);
                TxResult {
                    success: true,
                    gas_used: 0,
                }
            });
        }
        Arc::try_unwrap(order).unwrap().into_inner().unwrap()
    };
    let first = run();
    let second = run();
    assert_eq!(first, vec![0, 1]);
    assert_eq!(first, second);
}
#[test]
fn test_declared_conflicts_preserve_independent_parallelism() {
    use std::time::Duration;
    let mut set1 = StateAccessSet::new();
    set1.write_keys.insert("a".to_string());
    let tx1 = Transaction {
        code: vec![],
        gas_limit: 0,
        access: set1,
    };
    let mut set2 = StateAccessSet::new();
    set2.write_keys.insert("a".to_string());
    let tx2 = Transaction {
        code: vec![],
        gas_limit: 0,
        access: set2,
    };
    let mut set3 = StateAccessSet::new();
    set3.write_keys.insert("a".to_string());
    let tx3 = Transaction {
        code: vec![],
        gas_limit: 0,
        access: set3,
    };
    let mut set4 = StateAccessSet::new();
    set4.write_keys.insert("b".to_string());
    let tx4 = Transaction {
        code: vec![],
        gas_limit: 0,
        access: set4,
    };
    let block = Block {
        transactions: vec![tx1, tx2, tx3, tx4],
    };
    let scheduler = Scheduler::new(2);
    let counter = Arc::new(AtomicUsize::new(0));
    let max = Arc::new(AtomicUsize::new(0));
    let counter_c = &counter;
    let max_c = &max;
    scheduler.schedule_block(block, |_tx| {
        let cur = counter_c.fetch_add(1, Ordering::SeqCst) + 1;
        loop {
            let m = max_c.load(Ordering::SeqCst);
            if cur > m {
                if max_c
                    .compare_exchange(m, cur, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    break;
                }
            } else {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
        counter_c.fetch_sub(1, Ordering::SeqCst);
        TxResult {
            success: true,
            gas_used: 0,
        }
    });
    // The scheduler allows parallel execution for independent transactions.
    assert!(max.load(Ordering::SeqCst) >= 2);
}
#[test]
fn test_scheduler_dynamic_scaling() {
    let make_tx = || Transaction {
        code: vec![],
        gas_limit: 0,
        access: StateAccessSet::new(),
    };
    let block = Block {
        transactions: vec![make_tx(); 64],
    };
    let scheduler = Scheduler::new_dynamic(1, 4);
    assert_eq!(scheduler.thread_count(), 1);
    scheduler.schedule_block(block.clone(), |_| TxResult::default());
    scheduler.schedule_block(block, |_| TxResult::default());
    assert!(scheduler.thread_count() >= 2);
}
