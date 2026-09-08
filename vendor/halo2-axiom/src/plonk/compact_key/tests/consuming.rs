//! Consuming compact-key bytes, real-proof equivalence, sink failures and allocation lifetimes.
//!
//! The allocator wrapper exists only in the library test executable. It delegates every memory
//! operation to `System`; observation uses fixed atomic slots and cannot allocate, lock or panic.
//! A mutex serializes the tests that arm observation. Other tests and Rayon threads may allocate
//! concurrently: only exact addresses of still-live, distinct key buffers are watched. A free
//! clears its slot before forwarding to `System`, so later address reuse cannot count as another
//! release. Successful reallocations update the watched address instead of reporting a drop.
//! This measures selected buffer lifetimes, not process RSS or the complete allocator footprint.

use super::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
};

const WATCH_SLOTS: usize = 64;
static WATCH_LOCK: Mutex<()> = Mutex::new(());
static WATCH_ARMED: AtomicBool = AtomicBool::new(false);
static WATCH_POINTERS: [AtomicUsize; WATCH_SLOTS] = [const { AtomicUsize::new(0) }; WATCH_SLOTS];
static WATCH_RELEASED: [AtomicBool; WATCH_SLOTS] = [const { AtomicBool::new(false) }; WATCH_SLOTS];

struct ObservedSystem;

#[global_allocator]
static TEST_ALLOCATOR: ObservedSystem = ObservedSystem;

fn observe_free(pointer: *mut u8) {
    if WATCH_ARMED.load(SeqCst) {
        for (index, slot) in WATCH_POINTERS.iter().enumerate() {
            if slot
                .compare_exchange(pointer as usize, 0, SeqCst, SeqCst)
                .is_ok()
            {
                WATCH_RELEASED[index].store(true, SeqCst);
                break;
            }
        }
    }
}

// SAFETY: The wrapper uses exactly the allocation/deallocation contracts of `System`, with no
// pointer dereferences or layout changes. Observation touches only independent static atomics.
unsafe impl GlobalAlloc for ObservedSystem {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: GlobalAlloc forwards the caller's valid layout unchanged to System.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: GlobalAlloc forwards the caller's valid layout unchanged to System.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        observe_free(pointer);
        // SAFETY: All allocation methods delegate to System, so the unchanged pointer/layout
        // identify its allocation. Clear observation before System can reuse the address.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // Detach the slot BEFORE System may free/reuse the old address. Without this, an
        // unrelated thread could reuse and free the old address before we update the slot.
        // The observed buffers are exclusively owned by the test holding Observation's mutex;
        // that guard cannot be dropped or re-armed while its own realloc call is in flight.
        let tracked = if WATCH_ARMED.load(SeqCst) {
            WATCH_POINTERS.iter().position(|slot| {
                slot.compare_exchange(pointer as usize, 0, SeqCst, SeqCst)
                    .is_ok()
            })
        } else {
            None
        };
        // SAFETY: Forward the caller's existing allocation and valid new size unchanged.
        let result = unsafe { System.realloc(pointer, layout, new_size) };
        if let Some(index) = tracked {
            // A failed realloc leaves the original allocation alive; success transfers
            // observation to the returned allocation, including an in-place resize.
            let live = if result.is_null() { pointer } else { result };
            WATCH_POINTERS[index].store(live as usize, SeqCst);
        }
        result
    }
}

struct Observation {
    _lock: MutexGuard<'static, ()>,
    count: usize,
}

impl Observation {
    fn arm(pointers: &[usize]) -> Self {
        let lock = WATCH_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        assert!(!WATCH_ARMED.load(SeqCst));
        assert!(pointers.len() <= WATCH_SLOTS);
        for (index, pointer) in pointers.iter().copied().enumerate() {
            assert_ne!(pointer, 0);
            assert!(
                !pointers[..index].contains(&pointer),
                "distinct live buffers"
            );
            WATCH_POINTERS[index].store(pointer, SeqCst);
            WATCH_RELEASED[index].store(false, SeqCst);
        }
        WATCH_ARMED.store(true, SeqCst);
        Self {
            _lock: lock,
            count: pointers.len(),
        }
    }

    fn all_released(&self) {
        for index in 0..self.count {
            assert!(
                WATCH_RELEASED[index].load(SeqCst),
                "buffer {index} retained"
            );
            assert_eq!(WATCH_POINTERS[index].load(SeqCst), 0);
        }
    }
}

impl Drop for Observation {
    fn drop(&mut self) {
        WATCH_ARMED.store(false, SeqCst);
        for index in 0..WATCH_SLOTS {
            WATCH_POINTERS[index].store(0, SeqCst);
            WATCH_RELEASED[index].store(false, SeqCst);
        }
    }
}

fn watch_vec<T>(
    value: &Vec<T>,
    released_by: usize,
    pointers: &mut Vec<usize>,
    deadlines: &mut Vec<usize>,
) {
    if value.capacity() != 0 && std::mem::size_of::<T>() != 0 {
        pointers.push(value.as_ptr() as usize);
        deadlines.push(released_by);
    }
}

fn observe_key<C: SerdeCurveAffine>(pk: &ProvingKey<C>) -> (Observation, Vec<usize>)
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let mut pointers = Vec::new();
    let mut deadlines = Vec::new();
    // These omitted bases and the evaluator must be gone before the first output write.
    watch_vec(&pk.fixed_polys, 0, &mut pointers, &mut deadlines);
    watch_vec(&pk.permutation.polys, 0, &mut pointers, &mut deadlines);
    for poly in pk.fixed_polys.iter().chain(&pk.permutation.polys) {
        watch_vec(&poly.values, 0, &mut pointers, &mut deadlines);
    }
    watch_vec(
        &pk.ev.custom_gates.constants,
        0,
        &mut pointers,
        &mut deadlines,
    );
    watch_vec(
        &pk.ev.custom_gates.rotations,
        0,
        &mut pointers,
        &mut deadlines,
    );
    watch_vec(
        &pk.ev.custom_gates.calculations,
        0,
        &mut pointers,
        &mut deadlines,
    );

    let vk_end = HEADER_BYTES as usize + pk.vk.to_bytes(SerdeFormat::Processed).len();
    watch_vec(
        &pk.vk.fixed_commitments,
        vk_end,
        &mut pointers,
        &mut deadlines,
    );
    watch_vec(
        pk.vk.permutation.commitments(),
        vk_end,
        &mut pointers,
        &mut deadlines,
    );
    watch_vec(&pk.vk.selectors, vk_end, &mut pointers, &mut deadlines);
    for selector in &pk.vk.selectors {
        watch_vec(selector, vk_end, &mut pointers, &mut deadlines);
    }

    let scalar_bytes = C::Scalar::ZERO.to_repr().as_ref().len();
    let mut end = vk_end;
    for poly in [&pk.l0, &pk.l_last, &pk.l_active_row] {
        end += 4 + scalar_bytes * poly.len();
        watch_vec(&poly.values, end, &mut pointers, &mut deadlines);
    }
    for vector in [&pk.fixed_values, &pk.permutation.permutations] {
        end += 4;
        for poly in vector {
            end += 4 + scalar_bytes * poly.len();
            watch_vec(&poly.values, end, &mut pointers, &mut deadlines);
        }
        watch_vec(vector, end, &mut pointers, &mut deadlines);
    }
    (Observation::arm(&pointers), deadlines)
}

#[derive(Clone, Copy)]
enum SinkEnd {
    Complete,
    ErrorAt(usize),
    ZeroAt(usize),
    PanicAt(usize),
}

struct LifecycleSink<'a> {
    bytes: Vec<u8>,
    deadlines: &'a [usize],
    end: SinkEnd,
}

impl Write for LifecycleSink<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        for (index, deadline) in self.deadlines.iter().copied().enumerate() {
            assert_eq!(
                WATCH_RELEASED[index].load(SeqCst),
                self.bytes.len() >= deadline,
                "buffer {index}, byte {}, release at {deadline}",
                self.bytes.len()
            );
        }
        let remaining = match self.end {
            SinkEnd::Complete => usize::MAX,
            SinkEnd::ErrorAt(at) | SinkEnd::ZeroAt(at) | SinkEnd::PanicAt(at) => {
                at.saturating_sub(self.bytes.len())
            }
        };
        if remaining == 0 {
            match self.end {
                SinkEnd::ErrorAt(_) => return Err(io::Error::from(io::ErrorKind::StorageFull)),
                SinkEnd::ZeroAt(_) => return Ok(0),
                SinkEnd::PanicAt(_) => panic!("synthetic consuming sink unwind"),
                SinkEnd::Complete => unreachable!(),
            }
        }
        let count = bytes.len().min(7).min(remaining);
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("the caller owns sink flushing")
    }
}

fn consuming_roundtrip<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(6);
    for compressed in [false, true] {
        let pk = keygen_pk2(
            &params,
            &KeyCircuit {
                value: Value::unknown(),
            },
            compressed,
        )
        .unwrap();
        let legacy = pk.to_bytes(SerdeFormat::Processed);
        let mut borrowed = Vec::new();
        pk.write_compact_v1(&mut borrowed).unwrap();
        let witness = C::Scalar::from(23);
        let expected_proof = proof(&params, &pk, witness);
        let mut consuming = Vec::new();
        pk.write_compact_v1_consuming(&mut consuming).unwrap();
        assert_eq!(consuming, borrowed);
        let restored = read_key::<C>(&consuming, 6, consuming.len() as u64).unwrap();
        assert_eq!(restored.to_bytes(SerdeFormat::Processed), legacy);
        assert_eq!(proof(&params, &restored, witness), expected_proof);
        assert!(verify(&params, restored.get_vk(), witness, &expected_proof));
        assert!(!verify(
            &params,
            restored.get_vk(),
            witness + C::Scalar::ONE,
            &expected_proof
        ));
    }
}

#[test]
fn consuming_compact_keys_preserve_eq_ep_bytes_and_real_proofs() {
    consuming_roundtrip::<EqAffine>();
    consuming_roundtrip::<EpAffine>();
}

#[test]
fn consuming_compact_writer_releases_buffers_at_serialization_boundaries() {
    let params = ParamsIPA::<EqAffine>::new(6);
    for compressed in [false, true] {
        let pk = keygen_pk2(
            &params,
            &KeyCircuit {
                value: Value::unknown(),
            },
            compressed,
        )
        .unwrap();
        let mut expected = Vec::new();
        pk.write_compact_v1(&mut expected).unwrap();
        let (observation, deadlines) = observe_key(&pk);
        let mut sink = LifecycleSink {
            bytes: Vec::new(),
            deadlines: &deadlines,
            end: SinkEnd::Complete,
        };
        pk.write_compact_v1_consuming(&mut sink).unwrap();
        observation.all_released();
        assert_eq!(sink.bytes, expected);
    }
}

#[test]
fn consuming_compact_writer_drops_remaining_buffers_on_error_zero_write_and_unwind() {
    let params = ParamsIPA::<EqAffine>::new(6);
    let template = keygen_pk2(
        &params,
        &KeyCircuit {
            value: Value::unknown(),
        },
        true,
    )
    .unwrap();
    let mut expected = Vec::new();
    template.write_compact_v1(&mut expected).unwrap();
    let vk_end = HEADER_BYTES as usize + template.vk.to_bytes(SerdeFormat::Processed).len();
    let polynomial_bytes = 4 + 64 * 32;
    let fixed_start = vk_end + 3 * polynomial_bytes;
    let permutation_start = fixed_start + 4 + template.fixed_values.len() * polynomial_bytes;
    for offset in [
        0,
        15,
        56,
        vk_end,
        vk_end + polynomial_bytes,
        fixed_start,
        fixed_start + 4 + polynomial_bytes,
        permutation_start,
        expected.len() - 1,
    ] {
        for end in [
            SinkEnd::ErrorAt(offset),
            SinkEnd::ZeroAt(offset),
            SinkEnd::PanicAt(offset),
        ] {
            let pk = template.clone();
            let (observation, deadlines) = observe_key(&pk);
            let mut sink = LifecycleSink {
                bytes: Vec::new(),
                deadlines: &deadlines,
                end,
            };
            let result = catch_unwind(AssertUnwindSafe(|| {
                pk.write_compact_v1_consuming(&mut sink)
            }));
            match end {
                SinkEnd::ErrorAt(_) => assert_eq!(
                    result.unwrap().unwrap_err().kind(),
                    io::ErrorKind::StorageFull
                ),
                SinkEnd::ZeroAt(_) => assert_eq!(
                    result.unwrap().unwrap_err().kind(),
                    io::ErrorKind::WriteZero
                ),
                SinkEnd::PanicAt(_) => assert_eq!(
                    result.unwrap_err().downcast_ref::<&'static str>(),
                    Some(&"synthetic consuming sink unwind"),
                    "the intended sink panic, not a failed lifecycle assertion"
                ),
                SinkEnd::Complete => unreachable!(),
            }
            observation.all_released();
            assert_eq!(sink.bytes, expected[..offset]);
        }
    }
}

#[test]
fn consuming_compact_writer_rejects_invalid_keys_before_output_and_drops_them() {
    let params = ParamsIPA::<EqAffine>::new(6);
    let template = keygen_pk2(
        &params,
        &KeyCircuit {
            value: Value::unknown(),
        },
        true,
    )
    .unwrap();
    for corruption in 0..11 {
        let mut pk = template.clone();
        match corruption {
            0 => pk.fixed_polys[0][0] += Fp::ONE,
            1 => pk.fixed_values[0][0] += Fp::ONE,
            2 => pk.permutation.polys[0][0] += Fp::ONE,
            3 => pk.permutation.permutations[0][0] += Fp::ONE,
            4 => {
                pk.fixed_polys.pop();
            }
            5 => {
                pk.permutation.permutations.pop();
            }
            6 => {
                pk.fixed_values[0].values.pop();
            }
            7 => {
                pk.l0.values.pop();
            }
            8 => {
                pk.l_last.values.pop();
            }
            9 => {
                pk.l_active_row.values.pop();
            }
            10 => {
                pk.permutation.polys[0].values.pop();
            }
            _ => unreachable!(),
        }
        let (observation, _) = observe_key(&pk);
        let mut output = Vec::new();
        assert_eq!(
            pk.write_compact_v1_consuming(&mut output)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        observation.all_released();
        assert!(output.is_empty(), "corruption {corruption} emitted bytes");
    }
}

#[test]
fn allocator_observation_disarms_on_unwind_and_tracks_reallocation_without_a_false_release() {
    let mut bytes = vec![1_u8; 1];
    let observation = Observation::arm(&[bytes.as_ptr() as usize]);
    bytes.reserve_exact(128);
    assert!(!WATCH_RELEASED[0].load(SeqCst));
    assert_eq!(WATCH_POINTERS[0].load(SeqCst), bytes.as_ptr() as usize);
    drop(bytes);
    observation.all_released();
    drop(observation);

    let result = catch_unwind(|| {
        let bytes = vec![2_u8; 32];
        let _observation = Observation::arm(&[bytes.as_ptr() as usize]);
        panic!("synthetic observation unwind");
    });
    assert!(result.is_err());
    // Re-arming obtains the same mutex and verifies that the previous guard disarmed tracking.
    let bytes = vec![3_u8; 32];
    let observation = Observation::arm(&[bytes.as_ptr() as usize]);
    drop(bytes);
    observation.all_released();
}
