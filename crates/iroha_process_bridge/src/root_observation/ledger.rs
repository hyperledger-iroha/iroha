//! Bounded lifecycle parsing and terminal-only ordering; no process lookup.

use super::{Failure, Result};

pub(super) const MAX_RECORDS: usize = 4096;
pub(super) const MAX_TASKS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Record {
    pub kind: u32,
    pub pid: u32,
    pub tid: u32,
    pub ppid: u32,
    pub ptid: u32,
    pub time: u64,
    pub cpu: u32,
    pub exec: bool,
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or(Failure::new("record_extent"))?;
    Ok(u32::from_ne_bytes(
        raw.try_into().map_err(|_| Failure::new("record_extent"))?,
    ))
}

fn u64_at(bytes: &[u8], offset: usize) -> Result<u64> {
    let raw = bytes
        .get(offset..offset + 8)
        .ok_or(Failure::new("record_extent"))?;
    Ok(u64::from_ne_bytes(
        raw.try_into().map_err(|_| Failure::new("record_extent"))?,
    ))
}

impl Record {
    pub fn parse(bytes: &[u8], ring_cpu: u32) -> Result<Self> {
        if bytes.len() < 32 || bytes.len() > 128 || !bytes.len().is_multiple_of(8) {
            return Err(Failure::new("record_extent"));
        }
        let kind = u32_at(bytes, 0)?;
        let misc = u16::from_ne_bytes([bytes[4], bytes[5]]);
        let size = usize::from(u16::from_ne_bytes([bytes[6], bytes[7]]));
        if size != bytes.len() {
            return Err(Failure::new("record_extent"));
        }
        let sample = bytes.len() - 24;
        let cpu = u32_at(bytes, sample + 16)?;
        if cpu != ring_cpu || u32_at(bytes, sample + 20)? != 0 {
            return Err(Failure::new("record_cpu"));
        }
        if kind == 2 {
            return Err(Failure::new("event_loss"));
        }
        let (process, thread, parent_process, parent_thread, exec) = match kind {
            3 => {
                if !(24..=32).contains(&sample) || !bytes[16..sample].contains(&0) {
                    return Err(Failure::new("comm_extent"));
                }
                (
                    u32_at(bytes, 8)?,
                    u32_at(bytes, 12)?,
                    0,
                    0,
                    misc & (1 << 13) != 0,
                )
            }
            4 | 7 => {
                if sample != 32 {
                    return Err(Failure::new("task_extent"));
                }
                (
                    u32_at(bytes, 8)?,
                    u32_at(bytes, 16)?,
                    u32_at(bytes, 12)?,
                    u32_at(bytes, 20)?,
                    false,
                )
            }
            _ => return Err(Failure::new("unknown_event")),
        };
        let time = u64_at(bytes, sample + 8)?;
        if process == 0 || thread == 0 || time == 0 {
            return Err(Failure::new("record_identity"));
        }
        Ok(Self {
            kind,
            pid: process,
            tid: thread,
            ppid: parent_process,
            ptid: parent_thread,
            time,
            cpu,
            exec,
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct Task {
    pid: u32,
    tid: u32,
    live: bool,
    born: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Facts {
    pub target_tasks: usize,
    pub initial_exec_ns: Option<u64>,
}

/// Evaluate only after all original target tasks are kernel-terminal and drained.
/// During live per-CPU drains, an earlier FORK may arrive after a later COMM;
/// therefore incomplete live ledgers never acquire a global-order certificate.
pub(super) fn terminal(
    records: &[Record],
    supervisor: u32,
    root: Option<u32>,
    admitted_ns: u64,
) -> Result<Facts> {
    if records.len() > MAX_RECORDS {
        return Err(Failure::new("record_bound"));
    }
    let mut ordered: Vec<&Record> = records.iter().collect();
    ordered.sort_unstable_by_key(|record| record.time);
    if ordered.windows(2).any(|pair| pair[0].time == pair[1].time) {
        return Err(Failure::new("ambiguous_event_time"));
    }
    let mut tasks = Vec::<Task>::with_capacity(MAX_TASKS);
    let mut outside = vec![Task {
        pid: supervisor,
        tid: supervisor,
        live: true,
        born: 0,
    }];
    let mut facts = Facts::default();
    let mut roots = 0;
    for event in ordered {
        if event.time < admitted_ns {
            return Err(Failure::new("event_before_admission"));
        }
        let task = tasks.iter().position(|task| task.tid == event.tid);
        let observer = outside.iter().position(|task| task.tid == event.tid);
        match event.kind {
            7 => {
                if task.is_some() || observer.is_some() {
                    return Err(Failure::new("reused_task_identifier"));
                }
                if tasks.len() + outside.len() >= MAX_TASKS {
                    return Err(Failure::new("task_bound"));
                }
                let parent = tasks.iter().find(|task| task.tid == event.ptid);
                let parent_outside = outside.iter().find(|task| task.tid == event.ptid);
                if let Some(parent) = parent {
                    if !parent.live || event.ppid != parent.pid || event.pid != parent.pid {
                        return Err(Failure::new("unexpected_process_fork"));
                    }
                    tasks.push(Task {
                        pid: event.pid,
                        tid: event.tid,
                        live: true,
                        born: event.time,
                    });
                } else if let Some(parent) = parent_outside {
                    if !parent.live || event.ppid != supervisor {
                        return Err(Failure::new("unknown_parent"));
                    }
                    if event.pid == supervisor {
                        outside.push(Task {
                            pid: supervisor,
                            tid: event.tid,
                            live: true,
                            born: event.time,
                        });
                    } else {
                        if event.ptid != supervisor
                            || event.pid != event.tid
                            || roots != 0
                            || root.is_some_and(|root| root != event.pid)
                        {
                            return Err(Failure::new("unbound_process_fork"));
                        }
                        roots += 1;
                        tasks.push(Task {
                            pid: event.pid,
                            tid: event.tid,
                            live: true,
                            born: event.time,
                        });
                    }
                } else {
                    return Err(Failure::new("unknown_parent"));
                }
            }
            3 => {
                if let Some(index) = observer {
                    let task = outside[index];
                    if !task.live || event.pid != supervisor || event.exec {
                        return Err(Failure::new("supervisor_transition"));
                    }
                } else if let Some(index) = task {
                    let task = tasks[index];
                    if !task.live || task.pid != event.pid {
                        return Err(Failure::new("comm_identity"));
                    }
                    if event.exec {
                        if root != Some(event.pid)
                            || event.tid != event.pid
                            || facts.initial_exec_ns.is_some()
                            || event.time <= task.born
                        {
                            return Err(Failure::new("unexpected_exec"));
                        }
                        facts.initial_exec_ns = Some(event.time);
                    }
                } else {
                    return Err(Failure::new("unknown_task"));
                }
            }
            4 => {
                if let Some(index) = observer {
                    if event.tid == supervisor || !outside[index].live || event.pid != supervisor {
                        return Err(Failure::new("supervisor_exit"));
                    }
                    outside[index].live = false;
                } else if let Some(index) = task {
                    if !tasks[index].live || tasks[index].pid != event.pid {
                        return Err(Failure::new("exit_identity"));
                    }
                    tasks[index].live = false;
                } else {
                    return Err(Failure::new("unknown_exit"));
                }
            }
            _ => return Err(Failure::new("unknown_event")),
        }
    }
    if tasks.iter().any(|task| task.live) {
        return Err(Failure::new("live_target_task"));
    }
    // Output-reader threads may remain alive until pipe EOF. Their descendants
    // are outside target scope, but no unobserved process fork is accepted.
    if root.is_some() && (roots != 1 || facts.initial_exec_ns.is_none()) {
        return Err(Failure::new("incomplete_initial_image_transition"));
    }
    if root.is_none() && facts.initial_exec_ns.is_some() {
        return Err(Failure::new("unbound_exec"));
    }
    facts.target_tasks = tasks.len();
    Ok(facts)
}

/// Join only an already captured direct creation; the number grants no lookup.
pub(super) fn birth(
    records: &[Record],
    supervisor: u32,
    root: u32,
    admitted_ns: u64,
) -> Result<bool> {
    let mut matches = 0;
    for event in records
        .iter()
        .filter(|record| record.kind == 7 && record.pid != supervisor && record.ptid == supervisor)
    {
        if event.pid != root
            || event.tid != root
            || event.ppid != supervisor
            || event.time < admitted_ns
        {
            return Err(Failure::new("unobserved_child_selection"));
        }
        matches += 1;
    }
    if matches > 1 {
        return Err(Failure::new("ambiguous_root_birth"));
    }
    Ok(matches == 1)
}

#[cfg(test)]
mod tests;
