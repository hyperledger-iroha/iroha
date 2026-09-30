//! Private native journal ownership of the certified pre-tail World cut.
//!
//! A result capture retains only touched element hashes (including absent/no-op
//! touches). Publication compares the same complete frozen journal and retains
//! only tail differences. This is neither a decoded snapshot nor caller-selected
//! root authority; restoration must replay the original execution to obtain it.

use super::*;
use crate::state::{NativeExecutionTip, StateBlock};
use crate::execution_attempt::ExecutionDeferred;
use iroha_allocation::{AllocationBudget, AllocationCharge, AllocationRefusal, ChargedBuffer,
    ChargedBufferError, ChargedShared};
use std::alloc::Layout;

#[derive(Debug)]
pub(crate) enum CutError {
    Invalid(String),
    Deferred(ExecutionDeferred),
}
impl From<String> for CutError { fn from(value: String) -> Self { Self::Invalid(value) } }
impl From<AllocationRefusal> for CutError { fn from(value: AllocationRefusal) -> Self { Self::Deferred(value.into()) } }
impl From<ChargedBufferError> for CutError {
    fn from(value: ChargedBufferError) -> Self {
        match value {
            ChargedBufferError::Admission(refusal) => refusal.into(),
            ChargedBufferError::Allocator { .. } => Self::Deferred(ivm::error::ExecutionDeferral::ActiveMemoryCapacity.into()),
        }
    }
}
impl std::fmt::Display for CutError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { Self::Invalid(reason) => reason.fmt(out), Self::Deferred(reason) => reason.fmt(out) }
    }
}

#[derive(Clone, Copy)]
struct JournalRow {
    slot: usize,
    id: &'static str,
    kind: u8,
    key: Option<Hash>,
    before: Option<Hash>,
    after: Option<Hash>,
}
impl JournalRow {
    fn identity(&self) -> (usize, u8, Option<Hash>) { (self.slot, self.kind, self.key) }
}

/// No public constructor or serialization. Rows belong to one original overlay.
pub(in crate::state) struct JournalCapture {
    rows: ChargedBuffer<JournalRow>,
    root: Hash,
    entries: u64,
}

/// Exact native preimages of only post-result changes, bound to original publication.
pub(in crate::state) struct CutCapsule {
    pub(in crate::state) tip: NativeExecutionTip,
    pub(in crate::state) generation: u64,
    pub(in crate::state) root: Hash,
    pub(in crate::state) entries: u64,
    pub(in crate::state) applied_root: Hash,
    pub(in crate::state) applied_entries: u64,
    rows: ChargedBuffer<JournalRow>,
}

impl CutCapsule {
    pub(in crate::state) fn change_for(&self, id:&str, kind:WorldStateElementKindV1,key:Option<Hash>) -> Option<(usize,Option<Hash>,Option<Hash>)> {
        let kind=if kind==WorldStateElementKindV1::Table {TABLE} else {CELL};
        self.rows.as_slice().binary_search_by(|row| (row.id,row.kind,row.key).cmp(&(id,kind,key)))
            .ok().map(|index| (index,self.rows.as_slice()[index].before,self.rows.as_slice()[index].after))
    }
    pub(in crate::state) fn changes(&self) -> impl ExactSizeIterator<Item=(&'static str, WorldStateElementKindV1, Option<Hash>, Option<Hash>, Option<Hash>)> + '_ {
        self.rows.as_slice().iter().map(move |row| (row.id,
            if row.kind == TABLE { WorldStateElementKindV1::Table } else { WorldStateElementKindV1::Cell },
            row.key, row.before, row.after))
    }
}

struct JournalVisitor<'a> {
    index: &'a FieldIndex,
    visited: ChargedBuffer<bool>,
    count: usize,
    rows: Option<ChargedBuffer<JournalRow>>,
}
impl<'a> JournalVisitor<'a> {
    fn new(index: &'a FieldIndex, budget: &AllocationBudget, rows: Option<ChargedBuffer<JournalRow>>) -> Result<Self, CutError> {
        let mut visited = ChargedBuffer::new(index.canonical, budget)?;
        for _ in 0..index.canonical { visited.push_reserved(false); }
        Ok(Self { index, visited, count: 0, rows })
    }
    fn field(&mut self, name: &str, kind: u8) -> Result<Option<usize>, CutError> {
        match self.index.by_name.get(name) {
            Some(Classified::Excluded) => Ok(None),
            Some(Classified::Canonical { kind: declared, slot, .. }) if *declared == kind => {
                if self.visited.as_slice()[*slot] { return Err(format!("World cut repeats field {name}").into()); }
                self.visited.as_mut_slice()[*slot] = true;
                Ok(Some(*slot))
            }
            _ => Err(format!("World cut has unclassified or mistyped field {name}").into()),
        }
    }
    fn record(&mut self, row: JournalRow) -> Result<(), CutError> {
        self.count = self.count.checked_add(1).ok_or_else(|| CutError::from(AllocationRefusal::DemandOverflow))?;
        if let Some(rows) = self.rows.as_mut() {
            if rows.as_slice().len() == rows.capacity() { return Err("World cut journal count changed between borrowed passes".to_owned().into()); }
            rows.push_reserved(row);
        }
        Ok(())
    }
    fn finish(self) -> Result<(usize, Option<ChargedBuffer<JournalRow>>), CutError> {
        if self.visited.as_slice().iter().any(|value| !value) { return Err("World cut omits a canonical registry field".to_owned().into()); }
        Ok((self.count, self.rows))
    }
}
impl WorldProjection for JournalVisitor<'_> {
    type Error = CutError;
    // The exact original execution seal / frozen publication validation performs
    // the full trigger contract-row check. These passes must be changes-proportional.
    fn validates_trigger_contract_rows(&self) -> bool { false }
    fn append_musubi_archive_availability(&mut self, storage: &StorageBlock<'_, ArchiveId, MusubiArchiveAvailabilityV1>) -> Result<(), Self::Error> {
        self.append_storage_with("musubi_archive_availability", storage, |row| {
            hash_value(&crate::state::authority_registry::world::musubi_availability_policy::MusubiAvailabilityAuthorityV1::from_record(row))
        })
    }
    fn append_musubi_resolver_index(&mut self, storage: &StorageBlock<'_, MusubiReleaseIdV1, MusubiResolverReleaseRowV1>) -> Result<(), Self::Error> {
        self.append_storage_with("musubi_resolver_index", storage, |row| {
            hash_value(&crate::state::authority_registry::world::musubi_universal_policy::MusubiResolverAuthorityV1::from_record(row))
        })
    }
    fn append_musubi_public_directory(&mut self, storage: &StorageBlock<'_, MusubiPackageSelectorV1, MusubiOrderedPackageEntryV1>) -> Result<(), Self::Error> {
        self.append_storage_with("musubi_public_directory", storage, |row| {
            hash_value(&crate::state::authority_registry::world::musubi_universal_policy::MusubiDirectoryAuthorityV1::from_record(row))
        })
    }
    fn append_storage_with<K: Key + Encode, V: Value, M: mv::storage::StorageMode<K,V>>(&mut self, name: &'static str, storage: &StorageBlock<'_, K,V,M>, encode: impl Fn(&V)->Result<Hash,String>) -> Result<(), Self::Error> {
        let Some(slot) = self.field(name, TABLE)? else { return Ok(()); };
        for entry in storage.touched_entries() {
            let hashing = self.rows.is_some();
            self.record(JournalRow { slot, id:self.index.ids[slot],kind: TABLE,
                key: if hashing { Some(hash_value(entry.key)?) } else { None },
                before: if hashing { entry.before.map(&encode).transpose()? } else { None },
                after: if hashing { entry.after.map(&encode).transpose()? } else { None } })?;
        }
        Ok(())
    }
    fn append_cell_with<V: Value,C:Send+Sync+'static>(&mut self, name: &'static str, cell: &CellBlock<'_, V,C>, encode:impl Fn(&V)->Result<Hash,String>) -> Result<(),Self::Error> {
        let Some(slot) = self.field(name,CELL)? else { return Ok(()); };
        if let Some(value) = cell.touched_value() {
            let hashing = self.rows.is_some();
            self.record(JournalRow { slot,id:self.index.ids[slot],kind:CELL,key:None,
                before: if hashing {Some(encode(value.before)?)} else {None},
                after: if hashing {Some(encode(value.after)?)} else {None} })?;
        }
        Ok(())
    }
}
fn journal(world:&WorldBlock<'_>,budget:&AllocationBudget)->Result<ChargedBuffer<JournalRow>,CutError> {
    let index = field_index().as_ref().map_err(Clone::clone)?;
    let count = {
        let mut visitor=JournalVisitor::new(index,budget,None)?;
        world.project_world(&mut visitor)?; visitor.finish()?.0
    };
    let rows=ChargedBuffer::new(count,budget)?;
    let mut visitor=JournalVisitor::new(index,budget,Some(rows))?;
    world.project_world(&mut visitor)?;
    let (actual,rows)=visitor.finish()?;
    if actual!=count {return Err("World cut original journal count changed".to_owned().into());}
    let mut rows=rows.ok_or_else(||CutError::Invalid("World cut journal absent".into()))?;
    rows.as_mut_slice().sort_unstable_by_key(JournalRow::identity);
    if rows.as_slice().windows(2).any(|pair|pair[0].identity()==pair[1].identity()) {return Err("World cut repeats a canonical touched identity".to_owned().into());}
    Ok(rows)
}

impl JournalCapture {
    fn capture(world:&WorldBlock<'_>,genesis:bool,budget:&AllocationBudget)->Result<Self,CutError> {
        // Only normal genesis initialization cold-captures the complete World;
        // every later block derives R from its original complete predecessor.
        let index=field_index().as_ref().map_err(Clone::clone)?;
        let mut scratch=budget.try_reserve_layouts([
            Layout::new::<[u16;LANES]>(),Layout::new::<[u16;LANES]>(),
            Layout::array::<bool>(index.canonical).map_err(|_|AllocationRefusal::DemandOverflow)?])?;
        // Credits precede the existing accumulator pass's fixed heap lanes and
        // exhaustive visited-field scratch; none of those buffers escapes here.
        let (_,post)=world.state_transition(genesis)?;
        let root=post.root()?; let entries=post.entries();
        drop(post); drop(scratch);
        Ok(Self {rows:journal(world,budget)?,root,entries})
    }
    pub(in crate::state) fn prepare(&self,world:&WorldBlock<'_>,tip:NativeExecutionTip,generation:u64,budget:&AllocationBudget)->Result<ChargedShared<CutCapsule>,CutError> {
        let mut final_rows=journal(world,budget)?;
        let mut old=0;
        let mut changed=0;
        for row in final_rows.as_mut_slice() {
            if let Some(original)=self.rows.as_slice().get(old) {
                if original.identity()<row.identity() {return Err("World cut lost an original touched identity".to_owned().into());}
                if original.identity()==row.identity() {
                    row.before=original.after; old+=1;
                }
                // Otherwise this identity was first touched after R, so native
                // journal.before is exactly its at-R preimage (including absence).
            }
            if row.before!=row.after {changed+=1;}
        }
        if old!=self.rows.as_slice().len() {return Err("World cut omits original execution touches".to_owned().into());}
        let mut rows=ChargedBuffer::new(changed,budget)?;
        for row in final_rows.as_slice() {if row.before!=row.after {rows.push_reserved(*row);}}
        drop(final_rows);
        // Admit the accumulator's sole heap allocation before cloning its lanes.
        let layout=Layout::new::<[u16;LANES]>();
        let _scratch=budget.try_reserve(layout)?.try_split(layout).map_err(|e|CutError::Invalid(e.to_string()))?;
        let applied=world.state_accumulator.get();
        let mut reconstructed=applied.clone();
        let index=field_index().as_ref().map_err(Clone::clone)?;
        for row in rows.as_slice() {
            let path=match index.by_name.get(index.ids[row.slot].strip_prefix("world.").unwrap_or(index.ids[row.slot])) {
                Some(Classified::Canonical {path,..})=>path,
                _=>return Err("World cut canonical path missing".to_owned().into()),
            };
            if let Some(after)=row.after {reconstructed.remove(&element(path,row.key.as_ref(),&after));}
            if let Some(before)=row.before {reconstructed.add(&element(path,row.key.as_ref(),&before));}
        }
        if reconstructed.entries()!=self.entries || reconstructed.root()?!=self.root {
            return Err("World cut exact frozen tail does not reconstruct original R/count".to_owned().into());
        }
        let capsule=CutCapsule {tip,generation,root:self.root,entries:self.entries,
            applied_root:applied.root()?,applied_entries:applied.entries(),rows};
        let mut reservation=budget.try_reserve(ChargedShared::<CutCapsule>::allocation_layout())?;
        ChargedShared::from_reservation(capsule,&mut reservation).map_err(|(_owner,error)|
            CutError::Deferred(match error {
                iroha_allocation::PrepaidSharedError::Allocator{..}=>ivm::error::ExecutionDeferral::ActiveMemoryCapacity.into(),
                _=>ivm::error::ExecutionDeferral::ActiveMemoryCapacity.into(),
            }))
    }
}

impl StateBlock<'_> {
    /// Only the original completed executor calls this before constructing R.
    pub(crate) fn capture_original_world_cut(&mut self,expected:Hash)->Result<(),CutError> {
        if let Some(capture)=&self.world_cut_capture {
            if capture.root!=expected {return Err("World cut original result changed during retry".to_owned().into());}
            return Ok(());
        }
        let capture=JournalCapture::capture(&self.world,self._curr_block.is_genesis(),&self.state_ref.ivm_execution_budget())?;
        if capture.root!=expected {return Err("World cut original execution differs from R".to_owned().into());}
        self.world_cut_capture=Some(capture); Ok(())
    }
}

#[cfg(test)]
#[path="world_state_cut_tests.rs"]
mod tests;
