#![cfg_attr(not(feature = "cuda"), allow(dead_code))]

#[cfg(feature = "cuda")]
mod imp {
    use std::cell::Cell;
    use std::sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    };

    use cust::{
        context::CurrentContext,
        memory::{CopyDestination, DeviceCopy},
        prelude::*,
    };

    use crate::bn254_vec::FieldElem;

    static PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/add.ptx"));
    static VEC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/vector.ptx"));
    static SHA_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256.ptx"));
    static SHA_LEAVES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_leaves.ptx"));
    static POSEIDON_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/poseidon.ptx"));
    static SHA3_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha3.ptx"));
    static AES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/aes.ptx"));
    static BN254_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bn254.ptx"));
    static SIG_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/signature.ptx"));
    static SHA_PAIRS_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_pairs_reduce.ptx"));
    #[allow(dead_code)]
    static BITONIC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bitonic_sort.ptx"));
    static POSEIDON2_RC_FLAT: OnceLock<Vec<u64>> = OnceLock::new();
    static POSEIDON2_MDS_FLAT: OnceLock<Vec<u64>> = OnceLock::new();
    static POSEIDON6_RC_FLAT: OnceLock<Vec<u64>> = OnceLock::new();
    static POSEIDON6_MDS_FLAT: OnceLock<Vec<u64>> = OnceLock::new();

    static CUDA_DISABLED: AtomicBool = AtomicBool::new(false);
    static CUDA_FORCED_DISABLED: AtomicBool = AtomicBool::new(false);
    static CUDA_SELFTEST_OK: OnceLock<Mutex<Option<bool>>> = OnceLock::new();
    static CUDA_LAST_ERROR: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    thread_local! {
        static CUDA_SELFTEST_RUNNING: Cell<bool> = const { Cell::new(false) };
    }

    fn cuda_error_slot() -> &'static Mutex<Option<String>> {
        CUDA_LAST_ERROR.get_or_init(|| Mutex::new(None))
    }

    fn cuda_selftest_cache() -> &'static Mutex<Option<bool>> {
        CUDA_SELFTEST_OK.get_or_init(|| Mutex::new(None))
    }

    fn cuda_selftest_running() -> bool {
        CUDA_SELFTEST_RUNNING.with(Cell::get)
    }

    fn bind_cuda_context_for_current_thread() -> bool {
        let Some(mgr) = crate::GpuManager::shared() else {
            set_cuda_status_message(Some(
                "CUDA driver init or GPU manager setup failed".to_owned(),
            ));
            return false;
        };
        if mgr.device_count() == 0 {
            set_cuda_status_message(Some("no CUDA devices detected".to_owned()));
            return false;
        }
        let task_id = cuda_task_id(TASK_BIND_CONTEXT, &[mgr.device_count() as u64]);
        let rebound = with_cuda_task_scope(task_id, || {
            mgr.with_gpu_for_task(0, |gpu| CurrentContext::set_current(&gpu.context).ok())
                .flatten()
                .is_some()
        });
        if !rebound {
            set_cuda_status_message(Some(
                "failed to bind CUDA context on the current thread".to_owned(),
            ));
        }
        rebound
    }

    struct SelftestRunningGuard;

    impl SelftestRunningGuard {
        fn enter() -> Option<Self> {
            let already_running = CUDA_SELFTEST_RUNNING.with(|running| {
                let was_running = running.get();
                if !was_running {
                    running.set(true);
                }
                was_running
            });
            if already_running { None } else { Some(Self) }
        }
    }

    impl Drop for SelftestRunningGuard {
        fn drop(&mut self) {
            CUDA_SELFTEST_RUNNING.with(|running| running.set(false));
        }
    }

    fn set_cuda_status_message(message: Option<String>) {
        if let Ok(mut guard) = cuda_error_slot().lock() {
            *guard = message;
        }
    }

    fn record_cuda_disable(reason: impl Into<String>) {
        let message = reason.into();
        CUDA_DISABLED.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = cuda_error_slot().lock() {
            *guard = Some(message.clone());
        }
        eprintln!("ivm: cuda backend disabled: {message}");
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct KernelStatus {
        code: u32,
        detail: u32,
    }

    unsafe impl DeviceCopy for KernelStatus {}

    fn device_buffer_uninitialized<T: DeviceCopy>(len: usize) -> Option<DeviceBuffer<T>> {
        unsafe { DeviceBuffer::<T>::uninitialized(len).ok() }
    }

    const BN254_LIMBS: usize = 4;
    const POSEIDON2_WIDTH: usize = 3;
    const POSEIDON6_WIDTH: usize = 6;
    const POSEIDON2_STATE_WORDS: usize = POSEIDON2_WIDTH * BN254_LIMBS;
    const POSEIDON6_STATE_WORDS: usize = POSEIDON6_WIDTH * BN254_LIMBS;
    const POSEIDON_FULL_ROUNDS: u32 = 8;
    const POSEIDON_PARTIAL_ROUNDS: u32 = 56;
    #[cfg(test)]
    const POSEIDON_STATUS_ERR_STRIDE: u32 = 2; // keep in sync with poseidon.cu STATUS_ERR_STRIDE
    const POSEIDON_STATUS_ERR_ROUNDS: u32 = 3; // keep in sync with poseidon.cu STATUS_ERR_ROUNDS
    const TASK_BIND_CONTEXT: u64 = 0x0f0f_0f0f_0000_0001;
    const TASK_BITONIC: u64 = 0x0f0f_0f0f_0000_0002;
    const TASK_VECTOR_F32: u64 = 0x0f0f_0f0f_0000_0010;
    const TASK_VECTOR_U32: u64 = 0x0f0f_0f0f_0000_0011;
    const TASK_VECTOR_U64: u64 = 0x0f0f_0f0f_0000_0012;
    const TASK_SHA256_BLOCK: u64 = 0x0f0f_0f0f_0000_0020;
    const TASK_SHA256_LEAVES: u64 = 0x0f0f_0f0f_0000_0021;
    const TASK_SHA256_PAIRS: u64 = 0x0f0f_0f0f_0000_0022;
    const TASK_SELFTEST: u64 = 0x0f0f_0f0f_0000_002f;
    const TASK_POSEIDON2: u64 = 0x0f0f_0f0f_0000_0030;
    const TASK_POSEIDON6: u64 = 0x0f0f_0f0f_0000_0031;
    const TASK_KECCAK: u64 = 0x0f0f_0f0f_0000_0040;
    const TASK_AES_ROUND: u64 = 0x0f0f_0f0f_0000_0050;
    const TASK_AES_BATCH: u64 = 0x0f0f_0f0f_0000_0051;
    const TASK_AES_FUSED: u64 = 0x0f0f_0f0f_0000_0052;
    const TASK_BN254: u64 = 0x0f0f_0f0f_0000_0060;
    const TASK_ED25519_SINGLE: u64 = 0x0f0f_0f0f_0000_0070;
    const TASK_ED25519_BATCH: u64 = 0x0f0f_0f0f_0000_0071;
    const MODULE_BN254: &str = "bn254";
    const MODULE_BITONIC: &str = "bitonic";
    const MODULE_SHA256_BLOCK: &str = "sha256";
    const MODULE_SHA256_LEAVES: &str = "sha256_leaves";
    const MODULE_SHA256_PAIRS: &str = "sha256_pairs_reduce";
    const MODULE_SHA3: &str = "sha3";
    const MODULE_AES: &str = "aes";
    const MODULE_SIGNATURE: &str = "signature";
    const MODULE_VECTOR_SUM: &str = "vector_sum";
    const MODULE_VECTOR: &str = "vector";
    const MODULE_POSEIDON: &str = "poseidon";
    const BUFFER_POSEIDON2_RC: &str = "poseidon2_rc";
    const BUFFER_POSEIDON2_MDS: &str = "poseidon2_mds";
    const BUFFER_POSEIDON6_RC: &str = "poseidon6_rc";
    const BUFFER_POSEIDON6_MDS: &str = "poseidon6_mds";

    fn mix_task_id(mut state: u64, value: u64) -> u64 {
        state ^= value.wrapping_add(0x9e37_79b9_7f4a_7c15);
        state = state.rotate_left(27);
        state = state.wrapping_mul(0x94d0_49bb_1331_11eb);
        state ^ (state >> 31)
    }

    fn cuda_task_id(seed: u64, dims: &[u64]) -> u64 {
        dims.iter().copied().fold(seed, mix_task_id)
    }

    fn with_cuda_task_scope<T>(task_id: u64, func: impl FnOnce() -> T) -> T {
        crate::gpu_manager::with_task_scope(task_id, func)
    }

    fn kernel_name_tag(name: &str) -> u64 {
        let mut tag = 0u64;
        for &byte in name.as_bytes().iter().take(8) {
            tag = (tag << 8) | u64::from(byte);
        }
        tag
    }

    fn flatten_round_constants<const WIDTH: usize>(
        rc: &Vec<[[u64; BN254_LIMBS]; WIDTH]>,
    ) -> Vec<u64> {
        let mut flat = Vec::with_capacity(rc.len() * WIDTH * BN254_LIMBS);
        for round in rc.iter() {
            for lane in round.iter() {
                flat.extend_from_slice(lane);
            }
        }
        flat
    }

    fn flatten_mds<const WIDTH: usize>(mds: &[[[u64; BN254_LIMBS]; WIDTH]; WIDTH]) -> Vec<u64> {
        let mut flat = Vec::with_capacity(WIDTH * WIDTH * BN254_LIMBS);
        for row in mds.iter() {
            for elem in row.iter() {
                flat.extend_from_slice(elem);
            }
        }
        flat
    }

    #[allow(dead_code)]
    pub(super) fn bitonic_sort_pairs(hi: &mut [u64], lo: &mut [u64]) -> Option<()> {
        if hi.len() != lo.len() {
            return None;
        }
        if hi.is_empty() {
            return Some(());
        }
        if !ensure_cuda_selftest() {
            return None;
        }
        let len = hi.len();
        let pow2 = len.next_power_of_two();
        if pow2 > u32::MAX as usize {
            return None;
        }

        let mut hi_pad = Vec::with_capacity(pow2);
        hi_pad.extend_from_slice(hi);
        hi_pad.resize(pow2, u64::MAX);

        let mut lo_pad = Vec::with_capacity(pow2);
        lo_pad.extend_from_slice(lo);
        lo_pad.resize(pow2, u64::MAX);

        let task_id = cuda_task_id(TASK_BITONIC, &[len as u64, pow2 as u64]);
        with_cuda_task_scope(task_id, || {
            let mgr = crate::GpuManager::shared()?;
            mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = gpu.cached_module(MODULE_BITONIC, BITONIC_PTX)?;
                    let function = module.get_function("bitonic_step").ok()?;
                    let d_hi = DeviceBuffer::from_slice(&hi_pad).ok()?;
                    let d_lo = DeviceBuffer::from_slice(&lo_pad).ok()?;

                    let threads: u32 = 256;
                    let blocks: u32 = ((pow2 as u32) + threads - 1) / threads;

                    let mut k = 2usize;
                    while k <= pow2 {
                        let mut j = k >> 1;
                        while j > 0 {
                            unsafe {
                                launch!(function<<<blocks, threads, 0, stream>>>(
                                    d_hi.as_device_ptr(),
                                    d_lo.as_device_ptr(),
                                    pow2 as u32,
                                    j as u32,
                                    k as u32
                                ))
                                .ok()?;
                            }
                            j >>= 1;
                        }
                        k <<= 1;
                    }

                    stream.synchronize().ok()?;
                    let mut hi_out = vec![0u64; pow2];
                    let mut lo_out = vec![0u64; pow2];
                    d_hi.copy_to(&mut hi_out).ok()?;
                    d_lo.copy_to(&mut lo_out).ok()?;

                    for idx in 0..len {
                        hi[idx] = hi_out[idx];
                        lo[idx] = lo_out[idx];
                    }
                    Some(())
                })
            })?
        })
    }

    fn flatten_bn254_operands(elements: &[[u64; BN254_LIMBS]]) -> Vec<u64> {
        let mut flat = Vec::with_capacity(elements.len() * BN254_LIMBS);
        for element in elements {
            flat.extend_from_slice(element);
        }
        flat
    }

    fn collect_bn254_outputs(words: &[u64], elem_count: usize) -> Option<Vec<[u64; BN254_LIMBS]>> {
        if words.len() != elem_count.checked_mul(BN254_LIMBS)? {
            return None;
        }
        let mut out = Vec::with_capacity(elem_count);
        for chunk in words.chunks_exact(BN254_LIMBS) {
            let mut element = [0u64; BN254_LIMBS];
            element.copy_from_slice(chunk);
            out.push(element);
        }
        Some(out)
    }

    fn bn254_launch_kernel_words(
        kernel_name: &str,
        lhs_words: &[u64],
        rhs_words: &[u64],
        out_words: &mut [u64],
        elem_count: usize,
    ) -> Option<()> {
        let word_count = elem_count.checked_mul(BN254_LIMBS)?;
        if lhs_words.len() != word_count
            || rhs_words.len() != word_count
            || out_words.len() != word_count
        {
            return None;
        }
        if elem_count == 0 || elem_count > u32::MAX as usize {
            return None;
        }
        let task_id = cuda_task_id(
            TASK_BN254,
            &[
                kernel_name_tag(kernel_name),
                elem_count as u64,
                lhs_words[0],
                rhs_words[0],
            ],
        );
        with_cuda_task_scope(task_id, || {
            let mgr = crate::GpuManager::shared()?;
            mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = gpu.cached_module(MODULE_BN254, BN254_PTX)?;
                    let function = module.get_function(kernel_name).ok()?;
                    let d_lhs = DeviceBuffer::from_slice(lhs_words).ok()?;
                    let d_rhs = DeviceBuffer::from_slice(rhs_words).ok()?;
                    let d_out = device_buffer_uninitialized::<u64>(word_count)?;
                    let threads: u32 = 128;
                    let grid: u32 = (elem_count as u32).div_ceil(threads);
                    let launch_res = unsafe {
                        launch!(function<<<grid, threads, 0, stream>>>(
                            d_lhs.as_device_ptr(),
                            d_rhs.as_device_ptr(),
                            d_out.as_device_ptr(),
                            elem_count as u32,
                            BN254_LIMBS as u32
                        ))
                    };
                    if launch_res.is_err() {
                        record_cuda_disable(format!(
                            "kernel {kernel_name} launch failed; falling back to scalar backend"
                        ));
                        return None;
                    }
                    if stream.synchronize().is_err() {
                        record_cuda_disable(format!("stream sync failed for {kernel_name}"));
                        return None;
                    }
                    if d_out.copy_to(out_words).is_err() {
                        record_cuda_disable(format!("{kernel_name} copy failed"));
                        return None;
                    }
                    Some(())
                })
            })?
        })
    }

    fn bn254_launch_kernel(
        kernel_name: &str,
        lhs: &[u64; BN254_LIMBS],
        rhs: &[u64; BN254_LIMBS],
    ) -> Option<[u64; BN254_LIMBS]> {
        let mut out = [0u64; BN254_LIMBS];
        bn254_launch_kernel_words(kernel_name, lhs, rhs, &mut out, 1)?;
        Some(out)
    }

    fn bn254_launch_kernel_batch(
        kernel_name: &str,
        lhs: &[[u64; BN254_LIMBS]],
        rhs: &[[u64; BN254_LIMBS]],
    ) -> Option<Vec<[u64; BN254_LIMBS]>> {
        if lhs.len() != rhs.len() {
            return None;
        }
        if lhs.is_empty() {
            return Some(Vec::new());
        }
        let flat_lhs = flatten_bn254_operands(lhs);
        let flat_rhs = flatten_bn254_operands(rhs);
        let mut flat_out = vec![0u64; lhs.len() * BN254_LIMBS];
        bn254_launch_kernel_words(kernel_name, &flat_lhs, &flat_rhs, &mut flat_out, lhs.len())?;
        collect_bn254_outputs(&flat_out, lhs.len())
    }

    fn sha256_scalar_ref(state: &mut [u32; 8], block: &[u8; 64]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut w = [0u32; 64];
        for (t, chunk) in block.chunks(4).enumerate().take(16) {
            w[t] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        for t in 16..64 {
            let s0 = w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ (w[t - 15] >> 3);
            let s1 = w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ (w[t - 2] >> 10);
            w[t] = w[t - 16]
                .wrapping_add(s0)
                .wrapping_add(w[t - 7])
                .wrapping_add(s1);
        }
        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];
        for t in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[t])
                .wrapping_add(w[t]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    fn sha256_pairs_reduce_device_buffer(
        function: &Function,
        stream: &Stream,
        initial_digests: &DeviceBuffer<u8>,
        digest_count: usize,
    ) -> Option<[u8; 32]> {
        if digest_count == 0 || digest_count > u32::MAX as usize {
            return None;
        }
        let max_len = digest_count.checked_mul(32)?;
        let scratch = unsafe { DeviceBuffer::<u8>::uninitialized(max_len).ok()? };
        let mut current_count = digest_count as u32;
        let mut current_is_initial = true;
        while current_count > 1 {
            let next_count = current_count.div_ceil(2);
            let threads: u32 = 256;
            let grid: u32 = ((next_count + threads - 1) / threads).max(1);
            let (input, output) = if current_is_initial {
                (initial_digests, &scratch)
            } else {
                (&scratch, initial_digests)
            };
            unsafe {
                launch!(function<<<grid, threads, 0, stream>>>(
                    input.as_device_ptr(),
                    output.as_device_ptr(),
                    current_count
                ))
                .ok()?;
            }
            current_count = next_count;
            current_is_initial = !current_is_initial;
        }

        stream.synchronize().ok()?;
        let final_buf = if current_is_initial {
            initial_digests
        } else {
            &scratch
        };
        let mut root = [0u8; 32];
        final_buf.index(0..32).copy_to(&mut root).ok()?;
        Some(root)
    }

    fn sha256_pairs_reduce_on_gpu(
        function: &Function,
        gpu: &crate::gpu_manager::GpuContext,
        flat_digests: &[u8],
        digest_count: usize,
    ) -> Option<[u8; 32]> {
        if digest_count == 0 || flat_digests.len() != digest_count.checked_mul(32)? {
            return None;
        }
        gpu.with_stream(|stream| {
            let current = DeviceBuffer::from_slice(flat_digests).ok()?;
            sha256_pairs_reduce_device_buffer(function, stream, &current, digest_count)
        })
    }

    fn poseidon_cuda_selftest() -> bool {
        let sample2 = (1u64, 2u64);
        let expected2 = crate::poseidon::poseidon2_simd(sample2.0, sample2.1);
        let Some((outputs2, status2)) = poseidon2_cuda_many_impl(
            &[sample2],
            POSEIDON_FULL_ROUNDS,
            POSEIDON_PARTIAL_ROUNDS,
            true,
            true,
        ) else {
            record_cuda_disable("poseidon2 CUDA self-test launch failed");
            return false;
        };
        if status2.code != 0 || outputs2.first().copied() != Some(expected2) {
            record_cuda_disable("poseidon2 CUDA self-test mismatch");
            return false;
        }

        let sample6 = [1u64, 2, 3, 4, 5, 6];
        let expected6 = crate::poseidon::poseidon6_simd(sample6);
        let Some((outputs6, status6)) = poseidon6_cuda_many_impl(
            &[sample6],
            POSEIDON_FULL_ROUNDS,
            POSEIDON_PARTIAL_ROUNDS,
            true,
            true,
        ) else {
            record_cuda_disable("poseidon6 CUDA self-test launch failed");
            return false;
        };
        if status6.code != 0 || outputs6.first().copied() != Some(expected6) {
            record_cuda_disable("poseidon6 CUDA self-test mismatch");
            return false;
        }

        true
    }

    fn ed25519_cuda_selftest() -> bool {
        use ed25519_dalek::{Signer, SigningKey};

        let key = SigningKey::from_bytes(&[9u8; 32]);
        let pk = key.verifying_key();
        let msg = b"ivm-cuda-ed25519-selftest";
        let sig = key.sign(msg).to_bytes();
        let hram = crate::signature::ed25519_challenge_scalar_bytes(&sig, pk.as_bytes(), msg);

        let mgr = match crate::GpuManager::shared() {
            Some(mgr) => mgr,
            None => {
                record_cuda_disable("ed25519 CUDA self-test could not acquire GPU manager");
                return false;
            }
        };
        let single_task = cuda_task_id(
            TASK_ED25519_SINGLE,
            &[msg.len() as u64, u64::from(sig[0]), TASK_SELFTEST],
        );
        let single_ok = with_cuda_task_scope(single_task, || {
            mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = gpu.cached_module(MODULE_SIGNATURE, SIG_PTX)?;
                    let function = module.get_function("signature_kernel").ok()?;
                    let d_sig = DeviceBuffer::from_slice(sig.as_ref()).ok()?;
                    let d_pk = DeviceBuffer::from_slice(pk.as_bytes()).ok()?;
                    let d_hram = DeviceBuffer::from_slice(hram.as_ref()).ok()?;
                    let d_out = device_buffer_uninitialized::<u8>(1)?;
                    unsafe {
                        launch!(function<<<1, 32, 0, stream>>>(
                            d_sig.as_device_ptr(),
                            d_pk.as_device_ptr(),
                            d_hram.as_device_ptr(),
                            1u32,
                            d_out.as_device_ptr()
                        ))
                        .ok()?;
                    }
                    stream.synchronize().ok()?;
                    let mut out = [0u8; 1];
                    d_out.copy_to(&mut out).ok()?;
                    Some(out[0] == 1)
                })
            })
        });
        if single_ok != Some(Some(true)) {
            record_cuda_disable("golden self-test mismatch: ed25519 single");
            return false;
        }

        let mut bad_sig = sig;
        bad_sig[0] ^= 0x80;
        let sigs = [sig, bad_sig];
        let pks = [pk.to_bytes(), pk.to_bytes()];
        let hrams = [hram, hram];
        let flat_sigs: Vec<u8> = sigs
            .iter()
            .flat_map(|value| value.iter())
            .copied()
            .collect();
        let flat_pks: Vec<u8> = pks.iter().flat_map(|value| value.iter()).copied().collect();
        let flat_hrams: Vec<u8> = hrams
            .iter()
            .flat_map(|value| value.iter())
            .copied()
            .collect();
        let batch_task = cuda_task_id(TASK_ED25519_BATCH, &[2, TASK_SELFTEST]);
        let batch_ok = with_cuda_task_scope(batch_task, || {
            mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = gpu.cached_module(MODULE_SIGNATURE, SIG_PTX)?;
                    let function = module.get_function("signature_kernel").ok()?;
                    let d_sig = DeviceBuffer::from_slice(&flat_sigs).ok()?;
                    let d_pk = DeviceBuffer::from_slice(&flat_pks).ok()?;
                    let d_hram = DeviceBuffer::from_slice(&flat_hrams).ok()?;
                    let d_out = device_buffer_uninitialized::<u8>(2)?;
                    unsafe {
                        launch!(function<<<1, 128, 0, stream>>>(
                            d_sig.as_device_ptr(),
                            d_pk.as_device_ptr(),
                            d_hram.as_device_ptr(),
                            2u32,
                            d_out.as_device_ptr()
                        ))
                        .ok()?;
                    }
                    stream.synchronize().ok()?;
                    let mut out = [0u8; 2];
                    d_out.copy_to(&mut out).ok()?;
                    Some(out == [1u8, 0u8])
                })
            })
        });
        if batch_ok != Some(Some(true)) {
            record_cuda_disable("golden self-test mismatch: ed25519 batch");
            return false;
        }

        true
    }

    fn bn254_cuda_selftest() -> bool {
        let add_lhs = FieldElem::from_u64(3);
        let add_rhs = FieldElem::from_u64(4);
        let add_expected = crate::bn254_vec::add_scalar(add_lhs, add_rhs).0;
        let Some(add_out) = bn254_launch_kernel("bn254_add_kernel", &add_lhs.0, &add_rhs.0) else {
            record_cuda_disable("bn254 CUDA self-test launch failed: add");
            return false;
        };
        if add_out != add_expected {
            record_cuda_disable("golden self-test mismatch: bn254 add");
            return false;
        }

        let sub_lhs = FieldElem::from_u64(2);
        let sub_rhs = FieldElem::from_u64(5);
        let sub_expected = crate::bn254_vec::sub_scalar(sub_lhs, sub_rhs).0;
        let Some(sub_out) = bn254_launch_kernel("bn254_sub_kernel", &sub_lhs.0, &sub_rhs.0) else {
            record_cuda_disable("bn254 CUDA self-test launch failed: sub");
            return false;
        };
        if sub_out != sub_expected {
            record_cuda_disable("golden self-test mismatch: bn254 sub");
            return false;
        }

        let mul_lhs = FieldElem::from_u64(u32::MAX as u64 + 17);
        let mul_rhs = FieldElem::from_u64(11);
        let mul_expected = crate::bn254_vec::mul_scalar(mul_lhs, mul_rhs).0;
        let Some(mul_out) = bn254_launch_kernel("bn254_mul_kernel", &mul_lhs.0, &mul_rhs.0) else {
            record_cuda_disable("bn254 CUDA self-test launch failed: mul");
            return false;
        };
        if mul_out != mul_expected {
            record_cuda_disable("golden self-test mismatch: bn254 mul");
            return false;
        }

        true
    }

    fn ensure_cuda_selftest() -> bool {
        if CUDA_FORCED_DISABLED.load(Ordering::SeqCst) || CUDA_DISABLED.load(Ordering::SeqCst) {
            return false;
        }
        if cuda_selftest_running() {
            return false;
        }
        if let Ok(guard) = cuda_selftest_cache().lock()
            && let Some(cached) = *guard
        {
            return cached && bind_cuda_context_for_current_thread();
        }
        let Some(_selftest_guard) = SelftestRunningGuard::enter() else {
            return false;
        };
        let result = {
            if CUDA_FORCED_DISABLED.load(Ordering::SeqCst)
                || (crate::dev_env::dev_env_flag("IVM_DISABLE_CUDA")
                    && std::env::var("IVM_DISABLE_CUDA")
                        .map(|v| v == "1")
                        .unwrap_or(false))
            {
                CUDA_DISABLED.store(true, Ordering::SeqCst);
                set_cuda_status_message(Some(
                    "disabled by IVM_DISABLE_CUDA environment override".to_owned(),
                ));
                return false;
            }
            if crate::dev_env::dev_env_flag("IVM_FORCE_CUDA_SELFTEST_FAIL")
                && std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL")
                    .map(|v| v == "1")
                    .unwrap_or(false)
            {
                CUDA_DISABLED.store(true, Ordering::SeqCst);
                set_cuda_status_message(Some(
                    "self-test failure forced via IVM_FORCE_CUDA_SELFTEST_FAIL".to_owned(),
                ));
                return false;
            }
            let mgr = match crate::GpuManager::shared() {
                Some(mgr) => mgr,
                None => {
                    set_cuda_status_message(Some(
                        "CUDA driver init or GPU manager setup failed".to_owned(),
                    ));
                    return false;
                }
            };
            if mgr.device_count() == 0 {
                set_cuda_status_message(Some("no CUDA devices detected".to_owned()));
                return false;
            }
            // vadd32 parity
            let a = [1u32, 2, 3, 4];
            let b = [4u32, 3, 2, 1];
            let expect = [5u32, 5, 5, 5];
            let add_ok = match launch_u32_kernel("vadd32", &a, &b) {
                Some(out) if out.len() == 4 => out.as_slice() == expect,
                _ => false,
            };
            if !add_ok {
                record_cuda_disable("golden self-test mismatch: vadd32");
                return false;
            }
            if !vadd64_cuda_selftest() {
                record_cuda_disable("golden self-test mismatch: vadd64");
                return false;
            }
            if !bit_ops_cuda_selftest() {
                record_cuda_disable("golden self-test mismatch: vector bit kernels");
                return false;
            }
            // sha256 parity on single block ("abc")
            let mut st_scalar = [
                0x6a09e667u32,
                0xbb67ae85,
                0x3c6ef372,
                0xa54ff53a,
                0x510e527f,
                0x9b05688c,
                0x1f83d9ab,
                0x5be0cd19,
            ];
            let mut st_cuda = st_scalar;
            let mut block = [0u8; 64];
            block[0] = b'a';
            block[1] = b'b';
            block[2] = b'c';
            block[3] = 0x80;
            block[63] = 24;
            sha256_scalar_ref(&mut st_scalar, &block);
            let ok = if let Some(mgr) = crate::GpuManager::shared() {
                let task_id =
                    cuda_task_id(TASK_SHA256_BLOCK, &[u64::from(block[0]), TASK_SELFTEST]);
                let result = with_cuda_task_scope(task_id, || {
                    mgr.with_gpu_for_task(0, |gpu| {
                        gpu.with_stream(|stream| {
                            let module = gpu.cached_module(MODULE_SHA256_BLOCK, SHA_PTX)?;
                            let function = module.get_function("sha256_compress").ok()?;
                            let d_state = DeviceBuffer::from_slice(&st_cuda).ok()?;
                            let d_block = DeviceBuffer::from_slice(&block).ok()?;
                            unsafe {
                                launch!(function<<<1, 1, 0, stream>>>(
                                    d_state.as_device_ptr(), d_block.as_device_ptr()
                                ))
                                .ok()?;
                            }
                            stream.synchronize().ok()?;
                            d_state.copy_to(&mut st_cuda).ok()?;
                            Some(())
                        })
                    })
                });
                result.is_some()
            } else {
                false
            };
            if !ok || st_cuda != st_scalar {
                record_cuda_disable("golden self-test mismatch: sha256");
                return false;
            }
            if !sha256_leaves_cuda_selftest() {
                record_cuda_disable("golden self-test mismatch: sha256 leaves");
                return false;
            }
            if !sha256_pairs_reduce_cuda_selftest() {
                record_cuda_disable("golden self-test mismatch: sha256 pairs");
                return false;
            }
            // keccak_f1600 parity on a simple patterned state
            let mut k_scalar = [0u64; 25];
            for i in 0..25 {
                k_scalar[i] = (i as u64) * 0x0101_0101_0101_0101u64;
            }
            let mut k_cuda = k_scalar;
            crate::sha3::keccak_f1600_impl(&mut k_scalar);
            let ok = if let Some(mgr) = crate::GpuManager::shared() {
                let task_id = cuda_task_id(TASK_KECCAK, &[k_cuda[0], TASK_SELFTEST]);
                let result = with_cuda_task_scope(task_id, || {
                    mgr.with_gpu_for_task(0, |gpu| {
                        gpu.with_stream(|stream| {
                            let module = gpu.cached_module(MODULE_SHA3, SHA3_PTX)?;
                            let function = module.get_function("keccak_f1600_cuda").ok()?;
                            let d_state = DeviceBuffer::from_slice(&k_cuda).ok()?;
                            unsafe {
                                launch!(function<<<1, 1, 0, stream>>>(d_state.as_device_ptr()))
                                    .ok()?;
                            }
                            stream.synchronize().ok()?;
                            d_state.copy_to(&mut k_cuda).ok()?;
                            Some(())
                        })
                    })
                });
                result.is_some()
            } else {
                false
            };
            if !ok || k_cuda != k_scalar {
                record_cuda_disable("golden self-test mismatch: keccak");
                return false;
            }
            // AES round parity (ENC and DEC) using one block + rk
            let state = [
                0x00u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                0xdd, 0xee, 0xff,
            ];
            let rk = [
                0x0f, 0x15, 0x71, 0xc9, 0x47, 0xd9, 0xe8, 0x59, 0x0c, 0xb7, 0xad, 0xd6, 0xaf, 0x7f,
                0x67, 0x98,
            ];
            let cpu_enc = crate::aes::aesenc_impl(state, rk);
            let cpu_dec = crate::aes::aesdec_impl(cpu_enc, rk);
            let ok = if let Some(mgr) = crate::GpuManager::shared() {
                let task_id = cuda_task_id(TASK_AES_ROUND, &[u64::from(state[0]), TASK_SELFTEST]);
                let result = with_cuda_task_scope(task_id, || {
                    mgr.with_gpu_for_task(0, |gpu| {
                        gpu.with_stream(|stream| {
                            let module = gpu.cached_module(MODULE_AES, AES_PTX)?;
                            // AESENC
                            let enc_fn = module.get_function("aesenc_round").ok()?;
                            let d_state = DeviceBuffer::from_slice(&state).ok()?;
                            let d_rk = DeviceBuffer::from_slice(&rk).ok()?;
                            let d_out = device_buffer_uninitialized::<u8>(16)?;
                            unsafe {
                                launch!(enc_fn<<<1, 1, 0, stream>>>(
                                    d_state.as_device_ptr(),
                                    d_rk.as_device_ptr(),
                                    d_out.as_device_ptr()
                                ))
                                .ok()?;
                            }
                            stream.synchronize().ok()?;
                            let mut enc_out = [0u8; 16];
                            d_out.copy_to(&mut enc_out).ok()?;
                            if enc_out != cpu_enc {
                                return None;
                            }
                            // AESDEC on the encoded block
                            let dec_fn = module.get_function("aesdec_round").ok()?;
                            let d_state2 = DeviceBuffer::from_slice(&enc_out).ok()?;
                            let d_out2 = device_buffer_uninitialized::<u8>(16)?;
                            unsafe {
                                launch!(dec_fn<<<1, 1, 0, stream>>>(
                                    d_state2.as_device_ptr(),
                                    d_rk.as_device_ptr(),
                                    d_out2.as_device_ptr()
                                ))
                                .ok()?;
                            }
                            stream.synchronize().ok()?;
                            let mut dec_out = [0u8; 16];
                            d_out2.copy_to(&mut dec_out).ok()?;
                            if dec_out != cpu_dec {
                                return None;
                            }
                            Some(())
                        })
                    })
                });
                result.is_some()
            } else {
                false
            };
            if !ok {
                record_cuda_disable("golden self-test mismatch: aes round");
                return false;
            }
            if !aes_batch_cuda_selftest() {
                record_cuda_disable("golden self-test mismatch: aes batch");
                return false;
            }
            // AES fused two-round parity (ENC and DEC) to validate fused kernels
            let state = [
                0x00u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                0xdd, 0xee, 0xff,
            ];
            let rk1 = [
                0x0f, 0x15, 0x71, 0xc9, 0x47, 0xd9, 0xe8, 0x59, 0x0c, 0xb7, 0xad, 0xd6, 0xaf, 0x7f,
                0x67, 0x98,
            ];
            let mut rk2 = rk1; // derive a different key deterministically
            rk2[0] ^= 0xAA;
            rk2[1] ^= 0x55;
            let cpu_enc2 = {
                let r1 = crate::aes::aesenc_impl(state, rk1);
                crate::aes::aesenc_impl(r1, rk2)
            };
            let cpu_dec2 = {
                let r1 = crate::aes::aesdec_impl(cpu_enc2, rk1);
                crate::aes::aesdec_impl(r1, rk2)
            };
            let ok = if let Some(mgr) = crate::GpuManager::shared() {
                let task_id =
                    cuda_task_id(TASK_AES_FUSED, &[1, 2, TASK_SELFTEST, u64::from(state[0])]);
                let result = with_cuda_task_scope(task_id, || {
                    mgr.with_gpu_for_task(0, |gpu| {
                        gpu.with_stream(|stream| {
                            let module = gpu.cached_module(MODULE_AES, AES_PTX)?;
                            // Encrypt 2 rounds
                            let enc_fn = module.get_function("aesenc_rounds_batch").ok()?;
                            let d_states = DeviceBuffer::from_slice(&state).ok()?;
                            let rks: [u8; 32] = {
                                let mut buf = [0u8; 32];
                                buf[..16].copy_from_slice(&rk1);
                                buf[16..].copy_from_slice(&rk2);
                                buf
                            };
                            let d_rks = DeviceBuffer::from_slice(&rks).ok()?;
                            let d_out = device_buffer_uninitialized::<u8>(16)?;
                            unsafe {
                                launch!(enc_fn<<<1, 1, 0, stream>>>(
                                    d_states.as_device_ptr(),
                                    d_rks.as_device_ptr(),
                                    2u32,
                                    d_out.as_device_ptr(),
                                    1u32
                                ))
                                .ok()?;
                            }
                            stream.synchronize().ok()?;
                            let mut enc2 = [0u8; 16];
                            d_out.copy_to(&mut enc2).ok()?;
                            if enc2 != cpu_enc2 {
                                return None;
                            }
                            // Decrypt 2 rounds on enc2
                            let dec_fn = module.get_function("aesdec_rounds_batch").ok()?;
                            let d_states2 = DeviceBuffer::from_slice(&enc2).ok()?;
                            let d_out2 = device_buffer_uninitialized::<u8>(16)?;
                            unsafe {
                                launch!(dec_fn<<<1, 1, 0, stream>>>(
                                    d_states2.as_device_ptr(),
                                    d_rks.as_device_ptr(),
                                    2u32,
                                    d_out2.as_device_ptr(),
                                    1u32
                                ))
                                .ok()?;
                            }
                            stream.synchronize().ok()?;
                            let mut dec2 = [0u8; 16];
                            d_out2.copy_to(&mut dec2).ok()?;
                            if dec2 != cpu_dec2 {
                                return None;
                            }
                            Some(())
                        })
                    })
                });
                result.is_some()
            } else {
                false
            };
            if !ok {
                record_cuda_disable("golden self-test mismatch: aes fused-round");
                return false;
            }
            if !poseidon_cuda_selftest() {
                return false;
            }
            if !ed25519_cuda_selftest() {
                return false;
            }
            if !bn254_cuda_selftest() {
                return false;
            }
            set_cuda_status_message(None);
            true
        };
        if let Ok(mut guard) = cuda_selftest_cache().lock() {
            *guard = Some(result);
        }
        result
    }

    pub fn cuda_last_error_message() -> Option<String> {
        cuda_error_slot()
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    pub fn cuda_disabled() -> bool {
        CUDA_FORCED_DISABLED.load(Ordering::SeqCst) || CUDA_DISABLED.load(Ordering::SeqCst)
    }

    pub fn cuda_available() -> bool {
        if !ensure_cuda_selftest() {
            return false;
        }
        crate::GpuManager::shared()
            .map(|mgr| mgr.device_count() > 0)
            .unwrap_or(false)
            && !CUDA_FORCED_DISABLED.load(Ordering::SeqCst)
            && !CUDA_DISABLED.load(Ordering::SeqCst)
    }

    pub fn set_cuda_enabled(enabled: bool) {
        CUDA_FORCED_DISABLED.store(!enabled, Ordering::SeqCst);
        if enabled {
            CUDA_DISABLED.store(false, Ordering::SeqCst);
            if let Ok(mut guard) = cuda_selftest_cache().lock() {
                *guard = None;
            }
            set_cuda_status_message(None);
        } else {
            CUDA_DISABLED.store(true, Ordering::SeqCst);
            set_cuda_status_message(Some("disabled by configuration".to_owned()));
        }
        crate::gpu_manager::GpuManager::invalidate_cache();
    }

    #[doc(hidden)]
    pub fn reset_cuda_backend_for_tests() {
        CUDA_DISABLED.store(false, Ordering::SeqCst);
        CUDA_FORCED_DISABLED.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = cuda_selftest_cache().lock() {
            *guard = None;
        }
        set_cuda_status_message(None);
        crate::gpu_manager::GpuManager::invalidate_cache();
    }

    pub fn vector_add_f32(a: &[f32], b: &[f32]) -> Option<Vec<f32>> {
        if a.len() != b.len() {
            return None;
        }
        let len = a.len();
        let task_id = cuda_task_id(TASK_VECTOR_F32, &[len as u64]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            let mgr = crate::GpuManager::shared()?;
            mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = gpu.cached_module(MODULE_VECTOR_SUM, PTX)?;
                    let function = module.get_function("sum").ok()?;
                    let d_a = DeviceBuffer::from_slice(a).ok()?;
                    let d_b = DeviceBuffer::from_slice(b).ok()?;
                    let d_out = device_buffer_uninitialized::<f32>(len)?;
                    unsafe {
                        launch!(function<<<(len as u32 + 255) / 256, 256, 0, stream>>>(
                            d_a.as_device_ptr(),
                            d_b.as_device_ptr(),
                            d_out.as_device_ptr(),
                            len as u32
                        ))
                        .ok()?;
                    }
                    stream.synchronize().ok()?;
                    let mut out = vec![0f32; len];
                    d_out.copy_to(&mut out).ok()?;
                    Some(out)
                })
            })?
        })
    }

    fn launch_u32_kernel(name: &str, a: &[u32], b: &[u32]) -> Option<Vec<u32>> {
        if a.len() != b.len() {
            return None;
        }
        let len = a.len();
        let task_id = cuda_task_id(TASK_VECTOR_U32, &[kernel_name_tag(name), len as u64]);
        with_cuda_task_scope(task_id, || {
            let mgr = crate::GpuManager::shared()?;
            mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = gpu.cached_module(MODULE_VECTOR, VEC_PTX)?;
                    let function = module.get_function(name).ok()?;
                    let d_a = DeviceBuffer::from_slice(a).ok()?;
                    let d_b = DeviceBuffer::from_slice(b).ok()?;
                    let d_out = device_buffer_uninitialized::<u32>(len)?;
                    unsafe {
                        launch!(function<<<(len as u32 + 255) / 256, 256, 0, stream>>> (
                            d_a.as_device_ptr(),
                            d_b.as_device_ptr(),
                            d_out.as_device_ptr(),
                            len as u32
                        ))
                        .ok()?;
                    }
                    stream.synchronize().ok()?;
                    let mut out = vec![0u32; len];
                    d_out.copy_to(&mut out).ok()?;
                    Some(out)
                })
            })?
        })
    }

    fn launch_u64_kernel(name: &str, a: &[u64], b: &[u64]) -> Option<Vec<u64>> {
        if a.len() != b.len() {
            return None;
        }
        let len = a.len();
        let task_id = cuda_task_id(TASK_VECTOR_U64, &[kernel_name_tag(name), len as u64]);
        with_cuda_task_scope(task_id, || {
            let mgr = crate::GpuManager::shared()?;
            mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = gpu.cached_module(MODULE_VECTOR, VEC_PTX)?;
                    let function = module.get_function(name).ok()?;
                    let d_a = DeviceBuffer::from_slice(a).ok()?;
                    let d_b = DeviceBuffer::from_slice(b).ok()?;
                    let d_out = device_buffer_uninitialized::<u64>(len)?;
                    unsafe {
                        launch!(function<<<(len as u32 + 255) / 256, 256, 0, stream>>> (
                            d_a.as_device_ptr(),
                            d_b.as_device_ptr(),
                            d_out.as_device_ptr(),
                            len as u32
                        ))
                        .ok()?;
                    }
                    stream.synchronize().ok()?;
                    let mut out = vec![0u64; len];
                    d_out.copy_to(&mut out).ok()?;
                    Some(out)
                })
            })?
        })
    }

    fn vadd64_cuda_selftest() -> bool {
        let a = [
            (0x0000_0000u64 << 32) | 0xffff_ffff,
            (0x8000_0000u64 << 32) | 0x0000_0001,
        ];
        let b = [
            (0x0000_0001u64 << 32) | 0x0000_0001,
            (0x7fff_ffffu64 << 32) | 0xffff_ffff,
        ];
        let expected = vec![a[0].wrapping_add(b[0]), a[1].wrapping_add(b[1])];
        launch_u64_kernel("vadd64", &a, &b)
            .map(|actual| actual == expected)
            .unwrap_or(false)
    }

    fn bit_ops_cuda_selftest() -> bool {
        let lhs = [0xffff_0000u32, 0x1234_5678, 0x0f0f_0f0f, 0xaaaa_5555];
        let rhs = [0x00ff_ff00u32, 0xf0f0_f0f0, 0x3333_cccc, 0x5555_aaaa];
        let and_ok = launch_u32_kernel("vand", &lhs, &rhs)
            .map(|actual| {
                actual
                    == vec![
                        lhs[0] & rhs[0],
                        lhs[1] & rhs[1],
                        lhs[2] & rhs[2],
                        lhs[3] & rhs[3],
                    ]
            })
            .unwrap_or(false);
        let xor_ok = launch_u32_kernel("vxor", &lhs, &rhs)
            .map(|actual| {
                actual
                    == vec![
                        lhs[0] ^ rhs[0],
                        lhs[1] ^ rhs[1],
                        lhs[2] ^ rhs[2],
                        lhs[3] ^ rhs[3],
                    ]
            })
            .unwrap_or(false);
        let or_ok = launch_u32_kernel("vor", &lhs, &rhs)
            .map(|actual| {
                actual
                    == vec![
                        lhs[0] | rhs[0],
                        lhs[1] | rhs[1],
                        lhs[2] | rhs[2],
                        lhs[3] | rhs[3],
                    ]
            })
            .unwrap_or(false);
        and_ok && xor_ok && or_ok
    }

    fn aes_batch_cuda_selftest() -> bool {
        let states = [
            [
                0x00u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                0xdd, 0xee, 0xff,
            ],
            [
                0xffu8, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33,
                0x22, 0x11, 0x00,
            ],
        ];
        let rk = [
            0x0f, 0x15, 0x71, 0xc9, 0x47, 0xd9, 0xe8, 0x59, 0x0c, 0xb7, 0xad, 0xd6, 0xaf, 0x7f,
            0x67, 0x98,
        ];
        let expected_enc: Vec<[u8; 16]> = states
            .iter()
            .map(|&state| crate::aes::aesenc_impl(state, rk))
            .collect();
        let expected_dec: Vec<[u8; 16]> = states
            .iter()
            .map(|&state| crate::aes::aesdec_impl(state, rk))
            .collect();

        let mgr = match crate::GpuManager::shared() {
            Some(mgr) => mgr,
            None => return false,
        };
        let flat: Vec<u8> = states
            .iter()
            .flat_map(|state| state.iter())
            .copied()
            .collect();
        let count = states.len() as u32;
        let run_batch = |function_name: &str, expected: &[[u8; 16]]| -> Option<bool> {
            let mut out = vec![0u8; flat.len()];
            let task_id = cuda_task_id(
                TASK_AES_BATCH,
                &[count as u64, kernel_name_tag(function_name), TASK_SELFTEST],
            );
            let result = with_cuda_task_scope(task_id, || {
                mgr.with_gpu_for_task(0, |gpu| {
                    gpu.with_stream(|stream| {
                        let module = gpu.cached_module(MODULE_AES, AES_PTX)?;
                        let function = module.get_function(function_name).ok()?;
                        let d_states = DeviceBuffer::from_slice(&flat).ok()?;
                        let d_rk = DeviceBuffer::from_slice(&rk).ok()?;
                        let d_out = device_buffer_uninitialized::<u8>(out.len())?;
                        let threads: u32 = 256;
                        let grid: u32 = ((count + threads - 1) / threads).max(1);
                        unsafe {
                            launch!(function<<<grid, threads, 0, stream>>>(
                                d_states.as_device_ptr(),
                                d_rk.as_device_ptr(),
                                d_out.as_device_ptr(),
                                count
                            ))
                            .ok()?;
                        }
                        stream.synchronize().ok()?;
                        d_out.copy_to(&mut out).ok()?;
                        Some(())
                    })
                })
            });
            result??;
            let actual: Vec<[u8; 16]> = out
                .chunks_exact(16)
                .map(|chunk| {
                    let mut block = [0u8; 16];
                    block.copy_from_slice(chunk);
                    block
                })
                .collect();
            Some(actual == expected)
        };

        run_batch("aesenc_round_batch", &expected_enc).unwrap_or(false)
            && run_batch("aesdec_round_batch", &expected_dec).unwrap_or(false)
    }

    pub fn vadd32_cuda(a: &[u32], b: &[u32]) -> Option<Vec<u32>> {
        let task_id = cuda_task_id(TASK_VECTOR_U32, [a.len() as u64, 0].as_slice());
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            launch_u32_kernel("vadd32", a, b)
        })
    }

    pub fn vand_cuda(a: &[u32], b: &[u32]) -> Option<Vec<u32>> {
        let task_id = cuda_task_id(TASK_VECTOR_U32, [a.len() as u64, 1].as_slice());
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            launch_u32_kernel("vand", a, b)
        })
    }

    pub fn vxor_cuda(a: &[u32], b: &[u32]) -> Option<Vec<u32>> {
        let task_id = cuda_task_id(TASK_VECTOR_U32, [a.len() as u64, 2].as_slice());
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            launch_u32_kernel("vxor", a, b)
        })
    }

    pub fn vor_cuda(a: &[u32], b: &[u32]) -> Option<Vec<u32>> {
        let task_id = cuda_task_id(TASK_VECTOR_U32, [a.len() as u64, 3].as_slice());
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            launch_u32_kernel("vor", a, b)
        })
    }

    pub fn vadd64_cuda(a: &[u64], b: &[u64]) -> Option<Vec<u64>> {
        let task_id = cuda_task_id(TASK_VECTOR_U64, &[a.len() as u64]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            launch_u64_kernel("vadd64", a, b)
        })
    }

    /// Attempt to perform a SHA-256 compression round on the GPU.
    /// Returns true on success, false if the CUDA path failed.
    pub fn sha256_compress_cuda(state: &mut [u32; 8], block: &[u8; 64]) -> bool {
        let task_id = cuda_task_id(TASK_SHA256_BLOCK, &[u64::from(block[0])]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return false;
            }
            let mgr = match crate::GpuManager::shared() {
                Some(m) => m,
                None => return false,
            };
            let result = mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = match gpu.cached_module(MODULE_SHA256_BLOCK, SHA_PTX) {
                        Some(module) => module,
                        None => return Some(false),
                    };
                    let function = match module.get_function("sha256_compress") {
                        Ok(function) => function,
                        Err(_) => return Some(false),
                    };
                    let d_state = match DeviceBuffer::from_slice(state) {
                        Ok(b) => b,
                        Err(_) => return Some(false),
                    };
                    let d_block = match DeviceBuffer::from_slice(block) {
                        Ok(b) => b,
                        Err(_) => return Some(false),
                    };
                    unsafe {
                        if launch!(function<<<1, 1, 0, stream>>>(
                            d_state.as_device_ptr(),
                            d_block.as_device_ptr()
                        ))
                        .is_err()
                        {
                            return Some(false);
                        }
                    }
                    if stream.synchronize().is_err() {
                        return Some(false);
                    }
                    if d_state.copy_to(state).is_err() {
                        return Some(false);
                    }
                    Some(true)
                })
            });
            match result {
                Some(Some(r)) => r,
                None => false,
                Some(None) => false,
            }
        })
    }

    fn sha256_leaves_cuda_selftest() -> bool {
        let mut block_a = [0u8; 64];
        block_a[0] = b'a';
        block_a[1] = b'b';
        block_a[2] = b'c';
        block_a[3] = 0x80;
        block_a[63] = 24;

        let mut block_b = [0u8; 64];
        block_b[0] = b'n';
        block_b[1] = b'o';
        block_b[2] = b'r';
        block_b[3] = b'i';
        block_b[4] = b't';
        block_b[5] = b'o';
        block_b[6] = 0x80;
        block_b[63] = 48;

        let blocks = [block_a, block_b];
        let expected: Vec<[u8; 32]> = blocks
            .iter()
            .map(|block| {
                let mut state = [
                    0x6a09e667u32,
                    0xbb67ae85,
                    0x3c6ef372,
                    0xa54ff53a,
                    0x510e527f,
                    0x9b05688c,
                    0x1f83d9ab,
                    0x5be0cd19,
                ];
                sha256_scalar_ref(&mut state, block);
                let mut digest = [0u8; 32];
                for (index, word) in state.iter().enumerate() {
                    digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
                }
                digest
            })
            .collect();

        let mgr = match crate::GpuManager::shared() {
            Some(mgr) => mgr,
            None => return false,
        };

        let flat: Vec<u8> = blocks
            .iter()
            .flat_map(|block| block.iter())
            .copied()
            .collect();
        let mut out = vec![0u8; blocks.len() * 32];
        let count = blocks.len() as u32;
        let task_id = cuda_task_id(TASK_SHA256_LEAVES, &[count as u64, TASK_SELFTEST]);
        let result = with_cuda_task_scope(task_id, || {
            mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = gpu.cached_module(MODULE_SHA256_LEAVES, SHA_LEAVES_PTX)?;
                    let function = module.get_function("sha256_leaves").ok()?;
                    let d_blocks = DeviceBuffer::from_slice(&flat).ok()?;
                    let d_out = device_buffer_uninitialized::<u8>(out.len())?;
                    let threads: u32 = 256;
                    let grid: u32 = ((count + threads - 1) / threads).max(1);
                    unsafe {
                        launch!(function<<<grid, threads, 0, stream>>>(
                            d_blocks.as_device_ptr(),
                            d_out.as_device_ptr(),
                            count
                        ))
                        .ok()?;
                    }
                    stream.synchronize().ok()?;
                    d_out.copy_to(&mut out).ok()?;
                    Some(())
                })
            })
        });
        if result.is_none() || result.flatten().is_none() {
            return false;
        }

        let actual: Vec<[u8; 32]> = out
            .chunks_exact(32)
            .map(|chunk| {
                let mut digest = [0u8; 32];
                digest.copy_from_slice(chunk);
                digest
            })
            .collect();
        actual == expected
    }

    fn sha256_pairs_reduce_cuda_selftest() -> bool {
        fn cpu_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
            let mut state = [
                0x6a09e667u32,
                0xbb67ae85,
                0x3c6ef372,
                0xa54ff53a,
                0x510e527f,
                0x9b05688c,
                0x1f83d9ab,
                0x5be0cd19,
            ];
            let mut block = [0u8; 64];
            block[..32].copy_from_slice(left);
            block[32..].copy_from_slice(right);
            sha256_scalar_ref(&mut state, &block);
            let mut pad = [0u8; 64];
            pad[0] = 0x80;
            pad[62] = 0x02;
            pad[63] = 0x00;
            sha256_scalar_ref(&mut state, &pad);
            let mut out = [0u8; 32];
            for (index, word) in state.iter().enumerate() {
                out[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
            }
            out
        }

        let mut d0 = [0u8; 32];
        let mut d1 = [0u8; 32];
        let mut d2 = [0u8; 32];
        for (index, byte) in d0.iter_mut().enumerate() {
            *byte = index as u8;
        }
        for (index, byte) in d1.iter_mut().enumerate() {
            *byte = 0x40 + index as u8;
        }
        for (index, byte) in d2.iter_mut().enumerate() {
            *byte = 0x80 + index as u8;
        }
        let digests = [d0, d1, d2];
        let first = cpu_pair(&digests[0], &digests[1]);
        let expected = cpu_pair(&first, &digests[2]);

        let mgr = match crate::GpuManager::shared() {
            Some(mgr) => mgr,
            None => return false,
        };

        let selftest_task = cuda_task_id(TASK_SHA256_PAIRS, &[digests.len() as u64, TASK_SELFTEST]);
        let result = with_cuda_task_scope(selftest_task, || {
            let flat: Vec<u8> = digests
                .iter()
                .flat_map(|digest| digest.iter())
                .copied()
                .collect();
            mgr.with_gpu_for_task(0, |gpu| {
                let module = gpu.cached_module(MODULE_SHA256_PAIRS, SHA_PAIRS_PTX)?;
                let function = module.get_function("sha256_pairs_reduce").ok()?;
                sha256_pairs_reduce_on_gpu(&function, gpu, &flat, digests.len())
            })
            .flatten()
        });

        match result {
            Some(root) if root == expected => true,
            _ => false,
        }
    }

    /// Compute SHA-256 digests for many 64-byte blocks in parallel on the GPU.
    /// Each block must be a fully padded single-block message. Returns digest
    /// bytes (big-endian) per block on success.
    pub fn sha256_leaves_cuda(blocks: &[[u8; 64]]) -> Option<Vec<[u8; 32]>> {
        let count = blocks.len() as u32;
        if count == 0 {
            return Some(Vec::new());
        }
        let task_id = cuda_task_id(TASK_SHA256_LEAVES, &[count as u64]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            let mgr = crate::GpuManager::shared()?;
            // Flatten input blocks
            let flat: Vec<u8> = blocks.iter().flat_map(|b| b.iter()).copied().collect();
            let mut out = vec![0u8; (count as usize) * 32];
            let result = mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = gpu.cached_module(MODULE_SHA256_LEAVES, SHA_LEAVES_PTX)?;
                    let function = module.get_function("sha256_leaves").ok()?;
                    let d_blocks = DeviceBuffer::from_slice(&flat).ok()?;
                    let d_out = device_buffer_uninitialized::<u8>(out.len())?;
                    let threads: u32 = 256;
                    let grid: u32 = ((count + threads - 1) / threads).max(1);
                    unsafe {
                        launch!(function<<<grid, threads, 0, stream>>>(
                            d_blocks.as_device_ptr(),
                            d_out.as_device_ptr(),
                            count
                        ))
                        .ok()?;
                    }
                    stream.synchronize().ok()?;
                    d_out.copy_to(&mut out).ok()?;
                    Some(())
                })
            });
            result??;
            Some(
                out.chunks_exact(32)
                    .map(|chunk| {
                        let mut digest = [0u8; 32];
                        digest.copy_from_slice(chunk);
                        digest
                    })
                    .collect(),
            )
        })
    }

    /// Compute a SHA-256 Merkle root by hashing padded leaves and reducing
    /// parents entirely on the GPU before copying back the final digest.
    pub(crate) fn sha256_merkle_root_cuda(blocks: &[[u8; 64]]) -> Option<[u8; 32]> {
        let count = blocks.len();
        if count == 0 {
            return None;
        }
        let task_id = cuda_task_id(TASK_SHA256_PAIRS, &[count as u64, 0x6d65_726b_6c65_726f]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            let mgr = crate::GpuManager::shared()?;
            let flat_blocks: Vec<u8> = blocks
                .iter()
                .flat_map(|block| block.iter())
                .copied()
                .collect();
            mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let leaves_module = gpu.cached_module(MODULE_SHA256_LEAVES, SHA_LEAVES_PTX)?;
                    let leaves_function = leaves_module.get_function("sha256_leaves").ok()?;
                    let pairs_module = gpu.cached_module(MODULE_SHA256_PAIRS, SHA_PAIRS_PTX)?;
                    let pairs_function = pairs_module.get_function("sha256_pairs_reduce").ok()?;
                    let d_blocks = DeviceBuffer::from_slice(&flat_blocks).ok()?;
                    let d_digests = device_buffer_uninitialized::<u8>(count * 32)?;
                    let threads: u32 = 256;
                    let grid: u32 = (((count as u32) + threads - 1) / threads).max(1);
                    unsafe {
                        launch!(leaves_function<<<grid, threads, 0, stream>>>(
                            d_blocks.as_device_ptr(),
                            d_digests.as_device_ptr(),
                            count as u32
                        ))
                        .ok()?;
                    }
                    sha256_pairs_reduce_device_buffer(&pairs_function, stream, &d_digests, count)
                })
            })?
        })
    }

    /// Reduce a vector of digests by hashing pairs (left||right) using GPU until one remains.
    /// Left-promotion when right is absent. Returns the root digest.
    pub fn sha256_pairs_reduce_cuda(digests: &[[u8; 32]]) -> Option<[u8; 32]> {
        let n0 = digests.len();
        if n0 == 0 {
            return None;
        }
        if n0 == 1 {
            return Some(digests[0]);
        }
        let task_id = cuda_task_id(TASK_SHA256_PAIRS, &[n0 as u64]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            let mgr = crate::GpuManager::shared()?;

            let flat: Vec<u8> = digests.iter().flat_map(|d| d.iter()).copied().collect();
            mgr.with_gpu_for_task(0, |gpu| {
                let module = gpu.cached_module(MODULE_SHA256_PAIRS, SHA_PAIRS_PTX)?;
                let function = module.get_function("sha256_pairs_reduce").ok()?;
                sha256_pairs_reduce_on_gpu(&function, gpu, &flat, n0)
            })
            .flatten()
        })
    }

    #[derive(Clone, Copy)]
    enum PoseidonKernel {
        Poseidon2,
        Poseidon6,
    }

    fn launch_poseidon_kernel(
        kernel: PoseidonKernel,
        state_words: &mut [u64],
        state_stride_words: u32,
        batch_len: u32,
        rc_key: &'static str,
        rc_flat: &[u64],
        mds_key: &'static str,
        mds_flat: &[u64],
        full_rounds: u32,
        partial_rounds: u32,
        disable_on_error: bool,
        skip_selftest: bool,
    ) -> Option<KernelStatus> {
        if !skip_selftest && !ensure_cuda_selftest() {
            return None;
        }
        let kernel_name = match kernel {
            PoseidonKernel::Poseidon2 => "poseidon2_permute_kernel",
            PoseidonKernel::Poseidon6 => "poseidon6_permute_kernel",
        };
        let mut status = [KernelStatus::default(); 1];
        let task_id = cuda_task_id(
            match kernel {
                PoseidonKernel::Poseidon2 => TASK_POSEIDON2,
                PoseidonKernel::Poseidon6 => TASK_POSEIDON6,
            },
            &[batch_len as u64, state_stride_words as u64],
        );
        with_cuda_task_scope(task_id, || {
            let manager = crate::GpuManager::shared()?;
            manager.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = gpu.cached_module(MODULE_POSEIDON, POSEIDON_PTX)?;
                    let function = module.get_function(kernel_name).ok()?;
                    let d_state = DeviceBuffer::from_slice(state_words).ok()?;
                    let d_status = DeviceBuffer::from_slice(&status).ok()?;
                    gpu.with_cached_u64_buffer(rc_key, rc_flat, |d_rc| {
                        gpu.with_cached_u64_buffer(mds_key, mds_flat, |d_mds| {
                            let threads: u32 = 32;
                            let blocks = ((batch_len + threads - 1) / threads).max(1);
                            let grid = blocks.max(1);
                            unsafe {
                                launch!(function<<<grid, threads, 0, stream>>>(
                                    d_state.as_device_ptr(),
                                    state_stride_words,
                                    batch_len,
                                    0u32,
                                    d_rc.as_device_ptr(),
                                    d_mds.as_device_ptr(),
                                    full_rounds,
                                    partial_rounds,
                                    d_status.as_device_ptr()
                                ))
                                .ok()?;
                            }
                            stream.synchronize().ok()?;
                            d_state.copy_to(state_words).ok()?;
                            d_status.copy_to(&mut status).ok()?;
                            if status[0].code != 0 {
                                let message = if status[0].code == POSEIDON_STATUS_ERR_ROUNDS {
                                    format!(
                                        "{kernel_name} reported invalid round configuration (detail={})",
                                        status[0].detail
                                    )
                                } else {
                                    format!(
                                        "{kernel_name} reported error code {} (detail={})",
                                        status[0].code, status[0].detail
                                    )
                                };
                                if disable_on_error {
                                    record_cuda_disable(message);
                                } else {
                                    set_cuda_status_message(Some(message));
                                }
                            }
                            Some(())
                        })
                    })
                })
            })?;
            Some(())
        })?;
        Some(status[0])
    }

    fn poseidon2_cuda_many_impl(
        inputs: &[(u64, u64)],
        full_rounds: u32,
        partial_rounds: u32,
        skip_selftest: bool,
        disable_on_error: bool,
    ) -> Option<(Vec<u64>, KernelStatus)> {
        if inputs.is_empty() {
            return Some((Vec::new(), KernelStatus::default()));
        }
        if inputs.len() > u32::MAX as usize {
            return None;
        }
        if inputs.len().checked_mul(POSEIDON2_STATE_WORDS).is_none() {
            return None;
        }
        let rc = crate::poseidon::poseidon2_round_constants_words();
        let mds = crate::poseidon::poseidon2_mds_words();
        debug_assert_eq!(
            rc.len() as u32,
            POSEIDON_FULL_ROUNDS + POSEIDON_PARTIAL_ROUNDS
        );
        let rc_flat =
            POSEIDON2_RC_FLAT.get_or_init(|| flatten_round_constants::<POSEIDON2_WIDTH>(rc));
        let mds_flat = POSEIDON2_MDS_FLAT.get_or_init(|| flatten_mds::<POSEIDON2_WIDTH>(mds));

        let mut state_words = vec![0u64; inputs.len() * POSEIDON2_STATE_WORDS];
        for (idx, &(a, b)) in inputs.iter().enumerate() {
            let lanes = [
                FieldElem::from_u64(a),
                FieldElem::from_u64(b),
                FieldElem::from_u64(0),
            ];
            for (lane_idx, lane) in lanes.iter().enumerate() {
                let start = idx * POSEIDON2_STATE_WORDS + lane_idx * BN254_LIMBS;
                state_words[start..start + BN254_LIMBS].copy_from_slice(&lane.0);
            }
        }

        let status = launch_poseidon_kernel(
            PoseidonKernel::Poseidon2,
            &mut state_words,
            POSEIDON2_STATE_WORDS as u32,
            inputs.len() as u32,
            BUFFER_POSEIDON2_RC,
            rc_flat,
            BUFFER_POSEIDON2_MDS,
            mds_flat,
            full_rounds,
            partial_rounds,
            disable_on_error,
            skip_selftest,
        )?;
        let mut outputs = Vec::new();
        if status.code == 0 {
            outputs.reserve_exact(inputs.len());
            for idx in 0..inputs.len() {
                let start = idx * POSEIDON2_STATE_WORDS;
                let mut elem = FieldElem([0u64; BN254_LIMBS]);
                elem.0
                    .copy_from_slice(&state_words[start..start + BN254_LIMBS]);
                outputs.push(elem.to_u64());
            }
        }
        Some((outputs, status))
    }

    fn poseidon6_cuda_many_impl(
        inputs: &[[u64; 6]],
        full_rounds: u32,
        partial_rounds: u32,
        skip_selftest: bool,
        disable_on_error: bool,
    ) -> Option<(Vec<u64>, KernelStatus)> {
        if inputs.is_empty() {
            return Some((Vec::new(), KernelStatus::default()));
        }
        if inputs.len() > u32::MAX as usize {
            return None;
        }
        if inputs.len().checked_mul(POSEIDON6_STATE_WORDS).is_none() {
            return None;
        }
        let rc = crate::poseidon::poseidon6_round_constants_words();
        let mds = crate::poseidon::poseidon6_mds_words();
        debug_assert_eq!(
            rc.len() as u32,
            POSEIDON_FULL_ROUNDS + POSEIDON_PARTIAL_ROUNDS
        );
        let rc_flat =
            POSEIDON6_RC_FLAT.get_or_init(|| flatten_round_constants::<POSEIDON6_WIDTH>(rc));
        let mds_flat = POSEIDON6_MDS_FLAT.get_or_init(|| flatten_mds::<POSEIDON6_WIDTH>(mds));

        let mut state_words = vec![0u64; inputs.len() * POSEIDON6_STATE_WORDS];
        for (idx, values) in inputs.iter().enumerate() {
            for (lane_idx, value) in values.iter().enumerate() {
                let elem = FieldElem::from_u64(*value);
                let start = idx * POSEIDON6_STATE_WORDS + lane_idx * BN254_LIMBS;
                state_words[start..start + BN254_LIMBS].copy_from_slice(&elem.0);
            }
        }

        let status = launch_poseidon_kernel(
            PoseidonKernel::Poseidon6,
            &mut state_words,
            POSEIDON6_STATE_WORDS as u32,
            inputs.len() as u32,
            BUFFER_POSEIDON6_RC,
            rc_flat,
            BUFFER_POSEIDON6_MDS,
            mds_flat,
            full_rounds,
            partial_rounds,
            disable_on_error,
            skip_selftest,
        )?;
        let mut outputs = Vec::new();
        if status.code == 0 {
            outputs.reserve_exact(inputs.len());
            for idx in 0..inputs.len() {
                let start = idx * POSEIDON6_STATE_WORDS;
                let mut elem = FieldElem([0u64; BN254_LIMBS]);
                elem.0
                    .copy_from_slice(&state_words[start..start + BN254_LIMBS]);
                outputs.push(elem.to_u64());
            }
        }
        Some((outputs, status))
    }

    pub fn poseidon2_cuda(a: u64, b: u64) -> Option<u64> {
        poseidon2_cuda_many(&[(a, b)]).and_then(|mut outputs| outputs.pop())
    }

    pub fn poseidon2_cuda_many(inputs: &[(u64, u64)]) -> Option<Vec<u64>> {
        let task_id = cuda_task_id(TASK_POSEIDON2, &[inputs.len() as u64]);
        with_cuda_task_scope(task_id, || {
            let (outputs, status) = poseidon2_cuda_many_impl(
                inputs,
                POSEIDON_FULL_ROUNDS,
                POSEIDON_PARTIAL_ROUNDS,
                false,
                true,
            )?;
            if status.code != 0 {
                return None;
            }
            Some(outputs)
        })
    }

    pub fn poseidon6_cuda(inputs: [u64; 6]) -> Option<u64> {
        poseidon6_cuda_many(&[inputs]).and_then(|mut outputs| outputs.pop())
    }

    pub fn poseidon6_cuda_many(inputs: &[[u64; 6]]) -> Option<Vec<u64>> {
        let task_id = cuda_task_id(TASK_POSEIDON6, &[inputs.len() as u64]);
        with_cuda_task_scope(task_id, || {
            let (outputs, status) = poseidon6_cuda_many_impl(
                inputs,
                POSEIDON_FULL_ROUNDS,
                POSEIDON_PARTIAL_ROUNDS,
                false,
                true,
            )?;
            if status.code != 0 {
                return None;
            }
            Some(outputs)
        })
    }

    pub fn keccak_f1600_cuda(state: &mut [u64; 25]) -> bool {
        let task_id = cuda_task_id(TASK_KECCAK, &[state[0]]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return false;
            }
            let mgr = match crate::GpuManager::shared() {
                Some(m) => m,
                None => return false,
            };
            let result = mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = match gpu.cached_module(MODULE_SHA3, SHA3_PTX) {
                        Some(module) => module,
                        None => return Some(false),
                    };
                    let function = match module.get_function("keccak_f1600_cuda") {
                        Ok(function) => function,
                        Err(_) => return Some(false),
                    };
                    let d_state = match DeviceBuffer::from_slice(state) {
                        Ok(b) => b,
                        Err(_) => return Some(false),
                    };
                    unsafe {
                        if launch!(function<<<1, 1, 0, stream>>>(d_state.as_device_ptr())).is_err()
                        {
                            return Some(false);
                        }
                    }
                    if stream.synchronize().is_err() {
                        return Some(false);
                    }
                    if d_state.copy_to(state).is_err() {
                        return Some(false);
                    }
                    Some(true)
                })
            });
            match result {
                Some(Some(r)) => r,
                None => false,
                Some(None) => false,
            }
        })
    }

    pub fn aesenc_cuda(state: [u8; 16], rk: [u8; 16]) -> Option<[u8; 16]> {
        let task_id = cuda_task_id(TASK_AES_ROUND, &[u64::from(state[0]), 0]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return Some(crate::aes::aesenc_impl(state, rk));
            }
            aesenc_batch_cuda(&[state], rk)
                .and_then(|mut out| out.pop())
                .or_else(|| Some(crate::aes::aesenc_impl(state, rk)))
        })
    }

    pub fn aesdec_cuda(state: [u8; 16], rk: [u8; 16]) -> Option<[u8; 16]> {
        let task_id = cuda_task_id(TASK_AES_ROUND, &[u64::from(state[0]), 1]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return Some(crate::aes::aesdec_impl(state, rk));
            }
            aesdec_batch_cuda(&[state], rk)
                .and_then(|mut out| out.pop())
                .or_else(|| Some(crate::aes::aesdec_impl(state, rk)))
        })
    }

    /// Batch AESENC round: process N blocks with a single launch. Common round key for all.
    pub fn aesenc_batch_cuda(states: &[[u8; 16]], rk: [u8; 16]) -> Option<Vec<[u8; 16]>> {
        if states.is_empty() {
            return Some(Vec::new());
        }
        let count = states.len() as u32;
        let task_id = cuda_task_id(TASK_AES_BATCH, &[count as u64, 0]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return Some(
                    states
                        .iter()
                        .map(|&s| crate::aes::aesenc_impl(s, rk))
                        .collect(),
                );
            }
            let mgr = crate::GpuManager::shared()?;
            let flat: Vec<u8> = states.iter().flat_map(|b| b.iter()).copied().collect();
            let mut out = vec![0u8; states.len() * 16];
            let ok = mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = gpu.cached_module(MODULE_AES, AES_PTX)?;
                    let function = module.get_function("aesenc_round_batch").ok()?;
                    let d_states = DeviceBuffer::from_slice(&flat).ok()?;
                    let d_rk = DeviceBuffer::from_slice(&rk).ok()?;
                    let d_out = device_buffer_uninitialized::<u8>(out.len())?;
                    let threads: u32 = 256;
                    let grid: u32 = ((count + threads - 1) / threads).max(1);
                    unsafe {
                        launch!(function<<<grid, threads, 0, stream>>>(
                            d_states.as_device_ptr(),
                            d_rk.as_device_ptr(),
                            d_out.as_device_ptr(),
                            count
                        ))
                        .ok()?;
                    }
                    stream.synchronize().ok()?;
                    d_out.copy_to(&mut out).ok()?;
                    Some(())
                })
            });
            ok??;
            let mut vec_out = Vec::with_capacity(states.len());
            for i in 0..states.len() {
                let mut block = [0u8; 16];
                block.copy_from_slice(&out[i * 16..i * 16 + 16]);
                vec_out.push(block);
            }
            Some(vec_out)
        })
    }

    /// Batch AESDEC round.
    pub fn aesdec_batch_cuda(states: &[[u8; 16]], rk: [u8; 16]) -> Option<Vec<[u8; 16]>> {
        if states.is_empty() {
            return Some(Vec::new());
        }
        let count = states.len() as u32;
        let task_id = cuda_task_id(TASK_AES_BATCH, &[count as u64, 1]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return Some(
                    states
                        .iter()
                        .map(|&s| crate::aes::aesdec_impl(s, rk))
                        .collect(),
                );
            }
            let mgr = crate::GpuManager::shared()?;
            let flat: Vec<u8> = states.iter().flat_map(|b| b.iter()).copied().collect();
            let mut out = vec![0u8; states.len() * 16];
            let ok = mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = gpu.cached_module(MODULE_AES, AES_PTX)?;
                    let function = module.get_function("aesdec_round_batch").ok()?;
                    let d_states = DeviceBuffer::from_slice(&flat).ok()?;
                    let d_rk = DeviceBuffer::from_slice(&rk).ok()?;
                    let d_out = device_buffer_uninitialized::<u8>(out.len())?;
                    let threads: u32 = 256;
                    let grid: u32 = ((count + threads - 1) / threads).max(1);
                    unsafe {
                        launch!(function<<<grid, threads, 0, stream>>>(
                            d_states.as_device_ptr(),
                            d_rk.as_device_ptr(),
                            d_out.as_device_ptr(),
                            count
                        ))
                        .ok()?;
                    }
                    stream.synchronize().ok()?;
                    d_out.copy_to(&mut out).ok()?;
                    Some(())
                })
            });
            ok??;
            let mut vec_out = Vec::with_capacity(states.len());
            for i in 0..states.len() {
                let mut block = [0u8; 16];
                block.copy_from_slice(&out[i * 16..i * 16 + 16]);
                vec_out.push(block);
            }
            Some(vec_out)
        })
    }

    fn aesenc_rounds_batch_cpu(states: &[[u8; 16]], round_keys: &[[u8; 16]]) -> Vec<[u8; 16]> {
        let mut current = states.to_vec();
        for &round_key in round_keys {
            for block in &mut current {
                *block = crate::aes::aesenc_impl(*block, round_key);
            }
        }
        current
    }

    /// Fused N-round AESENC for many blocks with a single launch.
    pub fn aesenc_rounds_batch_cuda(
        states: &[[u8; 16]],
        round_keys: &[[u8; 16]],
    ) -> Option<Vec<[u8; 16]>> {
        if states.is_empty() {
            return Some(Vec::new());
        }
        if round_keys.is_empty() {
            return Some(states.to_vec());
        }
        let count = states.len() as u32;
        let nrounds = round_keys.len() as u32;
        let task_id = cuda_task_id(TASK_AES_FUSED, &[count as u64, nrounds as u64, 0]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return Some(aesenc_rounds_batch_cpu(states, round_keys));
            }
            let mgr = crate::GpuManager::shared()?;
            let flat_states: Vec<u8> = states.iter().flat_map(|b| b.iter()).copied().collect();
            let flat_rks: Vec<u8> = round_keys.iter().flat_map(|b| b.iter()).copied().collect();
            let mut out = vec![0u8; states.len() * 16];
            let ok = mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = gpu.cached_module(MODULE_AES, AES_PTX)?;
                    let function = module.get_function("aesenc_rounds_batch").ok()?;
                    let d_states = DeviceBuffer::from_slice(&flat_states).ok()?;
                    let d_rks = DeviceBuffer::from_slice(&flat_rks).ok()?;
                    let d_out = device_buffer_uninitialized::<u8>(out.len())?;
                    let threads: u32 = 256;
                    let grid: u32 = ((count + threads - 1) / threads).max(1);
                    unsafe {
                        launch!(function<<<grid, threads, 0, stream>>>(
                            d_states.as_device_ptr(),
                            d_rks.as_device_ptr(),
                            nrounds,
                            d_out.as_device_ptr(),
                            count
                        ))
                        .ok()?;
                    }
                    stream.synchronize().ok()?;
                    d_out.copy_to(&mut out).ok()?;
                    Some(())
                })
            });
            ok??;
            let mut vec_out = Vec::with_capacity(states.len());
            for i in 0..states.len() {
                let mut block = [0u8; 16];
                block.copy_from_slice(&out[i * 16..i * 16 + 16]);
                vec_out.push(block);
            }
            Some(vec_out)
        })
    }

    fn aesdec_rounds_batch_cpu(states: &[[u8; 16]], round_keys: &[[u8; 16]]) -> Vec<[u8; 16]> {
        let mut current = states.to_vec();
        for &round_key in round_keys {
            for block in &mut current {
                *block = crate::aes::aesdec_impl(*block, round_key);
            }
        }
        current
    }

    /// Fused N-round AESDEC for many blocks with a single launch.
    pub fn aesdec_rounds_batch_cuda(
        states: &[[u8; 16]],
        round_keys: &[[u8; 16]],
    ) -> Option<Vec<[u8; 16]>> {
        if states.is_empty() {
            return Some(Vec::new());
        }
        if round_keys.is_empty() {
            return Some(states.to_vec());
        }
        let count = states.len() as u32;
        let nrounds = round_keys.len() as u32;
        let task_id = cuda_task_id(TASK_AES_FUSED, &[count as u64, nrounds as u64, 1]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return Some(aesdec_rounds_batch_cpu(states, round_keys));
            }
            let mgr = crate::GpuManager::shared()?;
            let flat_states: Vec<u8> = states.iter().flat_map(|b| b.iter()).copied().collect();
            let flat_rks: Vec<u8> = round_keys.iter().flat_map(|b| b.iter()).copied().collect();
            let mut out = vec![0u8; states.len() * 16];
            let ok = mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = gpu.cached_module(MODULE_AES, AES_PTX)?;
                    let function = module.get_function("aesdec_rounds_batch").ok()?;
                    let d_states = DeviceBuffer::from_slice(&flat_states).ok()?;
                    let d_rks = DeviceBuffer::from_slice(&flat_rks).ok()?;
                    let d_out = device_buffer_uninitialized::<u8>(out.len())?;
                    let threads: u32 = 256;
                    let grid: u32 = ((count + threads - 1) / threads).max(1);
                    unsafe {
                        launch!(function<<<grid, threads, 0, stream>>>(
                            d_states.as_device_ptr(),
                            d_rks.as_device_ptr(),
                            nrounds,
                            d_out.as_device_ptr(),
                            count
                        ))
                        .ok()?;
                    }
                    stream.synchronize().ok()?;
                    d_out.copy_to(&mut out).ok()?;
                    Some(())
                })
            });
            ok??;
            let mut vec_out = Vec::with_capacity(states.len());
            for i in 0..states.len() {
                let mut block = [0u8; 16];
                block.copy_from_slice(&out[i * 16..i * 16 + 16]);
                vec_out.push(block);
            }
            Some(vec_out)
        })
    }

    /// Batch BN254 add: process many field-element pairs with one CUDA launch.
    ///
    /// Returns `None` when CUDA is unavailable, disabled, or the input slices
    /// differ in length.
    pub fn bn254_add_batch_cuda(
        lhs: &[[u64; BN254_LIMBS]],
        rhs: &[[u64; BN254_LIMBS]],
    ) -> Option<Vec<[u64; BN254_LIMBS]>> {
        let lhs_tag = lhs.first().map_or(0, |element| element[0]);
        let rhs_tag = rhs.first().map_or(0, |element| element[0]);
        let task_id = cuda_task_id(TASK_BN254, &[0, lhs.len() as u64, lhs_tag, rhs_tag]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            bn254_launch_kernel_batch("bn254_add_kernel", lhs, rhs)
        })
    }

    /// Batch BN254 subtract: process many field-element pairs with one CUDA launch.
    ///
    /// Returns `None` when CUDA is unavailable, disabled, or the input slices
    /// differ in length.
    pub fn bn254_sub_batch_cuda(
        lhs: &[[u64; BN254_LIMBS]],
        rhs: &[[u64; BN254_LIMBS]],
    ) -> Option<Vec<[u64; BN254_LIMBS]>> {
        let lhs_tag = lhs.first().map_or(0, |element| element[0]);
        let rhs_tag = rhs.first().map_or(0, |element| element[0]);
        let task_id = cuda_task_id(TASK_BN254, &[1, lhs.len() as u64, lhs_tag, rhs_tag]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            bn254_launch_kernel_batch("bn254_sub_kernel", lhs, rhs)
        })
    }

    /// Batch BN254 multiply: process many field-element pairs with one CUDA launch.
    ///
    /// Returns `None` when CUDA is unavailable, disabled, or the input slices
    /// differ in length.
    pub fn bn254_mul_batch_cuda(
        lhs: &[[u64; BN254_LIMBS]],
        rhs: &[[u64; BN254_LIMBS]],
    ) -> Option<Vec<[u64; BN254_LIMBS]>> {
        let lhs_tag = lhs.first().map_or(0, |element| element[0]);
        let rhs_tag = rhs.first().map_or(0, |element| element[0]);
        let task_id = cuda_task_id(TASK_BN254, &[2, lhs.len() as u64, lhs_tag, rhs_tag]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            bn254_launch_kernel_batch("bn254_mul_kernel", lhs, rhs)
        })
    }

    /// Attempt to add one BN254 field-element pair on the GPU.
    pub fn bn254_add_cuda(a: [u64; 4], b: [u64; 4]) -> Option<[u64; 4]> {
        let task_id = cuda_task_id(TASK_BN254, &[0, a[0], b[0]]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            // Language bindings query `ivm::cuda_available` / `ivm::cuda_disabled` at runtime so SDKs
            // can surface BN254 acceleration status without compile-time cfg guards.
            bn254_launch_kernel("bn254_add_kernel", &a, &b)
        })
    }

    /// Attempt to subtract one BN254 field-element pair on the GPU.
    pub fn bn254_sub_cuda(a: [u64; 4], b: [u64; 4]) -> Option<[u64; 4]> {
        let task_id = cuda_task_id(TASK_BN254, &[1, a[0], b[0]]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            bn254_launch_kernel("bn254_sub_kernel", &a, &b)
        })
    }

    /// Attempt to multiply one BN254 field-element pair on the GPU.
    pub fn bn254_mul_cuda(a: [u64; 4], b: [u64; 4]) -> Option<[u64; 4]> {
        let task_id = cuda_task_id(TASK_BN254, &[2, a[0], b[0]]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            bn254_launch_kernel("bn254_mul_kernel", &a, &b)
        })
    }

    pub fn ed25519_verify_cuda(msg: &[u8], sig: &[u8; 64], pk: &[u8; 32]) -> Option<bool> {
        let task_id = cuda_task_id(TASK_ED25519_SINGLE, &[msg.len() as u64, u64::from(sig[0])]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }
            use ed25519_dalek::{Signature, VerifyingKey};

            let signature = Signature::from_slice(sig).ok()?;
            let sig_bytes = signature.to_bytes();
            let verifying_key = VerifyingKey::from_bytes(pk).ok()?;
            let pk_bytes = verifying_key.to_bytes();
            let hram_bytes =
                crate::signature::ed25519_challenge_scalar_bytes(&sig_bytes, &pk_bytes, msg);
            let signatures = [sig_bytes];
            let public_keys = [pk_bytes];
            let hrams = [hram_bytes];
            match ed25519_verify_batch_cuda(&signatures, &public_keys, &hrams) {
                Some(mut result) => result.pop(),
                None => Some(verifying_key.verify_strict(msg, &signature).is_ok()),
            }
        })
    }

    pub fn ed25519_verify_batch_cuda(
        signatures: &[[u8; 64]],
        public_keys: &[[u8; 32]],
        hrams: &[[u8; 32]],
    ) -> Option<Vec<bool>> {
        if signatures.len() != public_keys.len() || signatures.len() != hrams.len() {
            return None;
        }
        if signatures.is_empty() {
            return Some(Vec::new());
        }
        if signatures.len() > u32::MAX as usize {
            return None;
        }
        let count = signatures.len();
        let task_id = cuda_task_id(TASK_ED25519_BATCH, &[count as u64]);
        with_cuda_task_scope(task_id, || {
            if !ensure_cuda_selftest() {
                return None;
            }

            let mut flat_sigs = Vec::with_capacity(count * 64);
            let mut flat_pks = Vec::with_capacity(count * 32);
            let mut flat_hrams = Vec::with_capacity(count * 32);
            for ((sig, pk), hram) in signatures.iter().zip(public_keys).zip(hrams) {
                flat_sigs.extend_from_slice(sig);
                flat_pks.extend_from_slice(pk);
                flat_hrams.extend_from_slice(hram);
            }

            let mgr = crate::GpuManager::shared()?;
            let gpu_result = mgr.with_gpu_for_task(0, |gpu| {
                gpu.with_stream(|stream| {
                    let module = gpu.cached_module(MODULE_SIGNATURE, SIG_PTX)?;
                    let function = module.get_function("signature_kernel").ok()?;
                    let d_sig = DeviceBuffer::from_slice(&flat_sigs).ok()?;
                    let d_pk = DeviceBuffer::from_slice(&flat_pks).ok()?;
                    let d_hram = DeviceBuffer::from_slice(&flat_hrams).ok()?;
                    let d_out = device_buffer_uninitialized::<u8>(count)?;
                    let threads: u32 = 128;
                    let blocks: u32 = ((count as u32) + threads - 1) / threads;
                    unsafe {
                        launch!(function<<<blocks.max(1), threads, 0, stream>>>(
                            d_sig.as_device_ptr(),
                            d_pk.as_device_ptr(),
                            d_hram.as_device_ptr(),
                            count as u32,
                            d_out.as_device_ptr()
                        ))
                        .ok()?;
                    }
                    stream.synchronize().ok()?;
                    let mut out = vec![0u8; count];
                    d_out.copy_to(&mut out).ok()?;
                    Some(out.into_iter().map(|b| b != 0).collect())
                })
            });

            match gpu_result {
                Some(Some(result)) => Some(result),
                _ => {
                    record_cuda_disable(
                        "ed25519 batch signature kernel unavailable; falling back to CPU path",
                    );
                    None
                }
            }
        })
    }

    #[cfg(all(test, feature = "cuda"))]
    mod tests {
        use std::sync::atomic::Ordering;

        use super::*;

        fn with_cuda_selftest_running_for_tests<T>(func: impl FnOnce() -> T) -> T {
            struct ResetGuard(bool);

            impl Drop for ResetGuard {
                fn drop(&mut self) {
                    CUDA_SELFTEST_RUNNING.with(|running| running.set(self.0));
                }
            }

            let previous = CUDA_SELFTEST_RUNNING.with(|running| {
                let old = running.get();
                running.set(true);
                old
            });
            let _reset = ResetGuard(previous);
            func()
        }

        #[test]
        fn poseidon_kernel_reports_round_errors_without_disabling_backend() {
            let disabled_before = CUDA_DISABLED.load(Ordering::SeqCst);
            if !ensure_cuda_selftest() {
                // On non-CUDA hosts the kernel helper should decline cleanly.
                assert!(
                    poseidon2_cuda_many_impl(&[(0u64, 1u64)], 0, 0, true, false).is_none(),
                    "cuda self-test must fail closed on unsupported hosts"
                );
                assert_eq!(
                    CUDA_DISABLED.load(Ordering::SeqCst),
                    disabled_before,
                    "probing unsupported hosts must not mutate disable flag"
                );
                return;
            }
            let Some((_, status)) = poseidon2_cuda_many_impl(&[(0u64, 1u64)], 0, 0, true, false)
            else {
                return;
            };
            assert_eq!(
                status.code, POSEIDON_STATUS_ERR_ROUNDS,
                "expected round count error from poseidon2 kernel"
            );
            assert_eq!(
                CUDA_DISABLED.load(Ordering::SeqCst),
                disabled_before,
                "fault injection must not disable CUDA backend"
            );
        }

        #[test]
        fn nested_cuda_selftest_requests_fail_closed() {
            with_cuda_selftest_running_for_tests(|| {
                assert!(!ensure_cuda_selftest());
                let mut state = [0u64; 25];
                assert!(
                    !keccak_f1600_cuda(&mut state),
                    "nested keccak probe should fail closed during self-test",
                );
                assert_eq!(
                    poseidon2_cuda_many(&[(0u64, 1u64)]),
                    None,
                    "nested poseidon probe should fail closed during self-test",
                );
            });
        }

        #[test]
        fn poseidon_kernel_reports_stride_errors_without_disabling_backend() {
            let rc = crate::poseidon::poseidon2_round_constants_words();
            let mds = crate::poseidon::poseidon2_mds_words();
            let rc_flat =
                POSEIDON2_RC_FLAT.get_or_init(|| flatten_round_constants::<POSEIDON2_WIDTH>(rc));
            let mds_flat = POSEIDON2_MDS_FLAT.get_or_init(|| flatten_mds::<POSEIDON2_WIDTH>(mds));
            let mut state = vec![0u64; POSEIDON2_STATE_WORDS];
            let disabled_before = CUDA_DISABLED.load(Ordering::SeqCst);
            if !ensure_cuda_selftest() {
                assert!(
                    launch_poseidon_kernel(
                        PoseidonKernel::Poseidon2,
                        &mut state,
                        1,
                        1,
                        BUFFER_POSEIDON2_RC,
                        rc_flat,
                        BUFFER_POSEIDON2_MDS,
                        mds_flat,
                        POSEIDON_FULL_ROUNDS,
                        POSEIDON_PARTIAL_ROUNDS,
                        false,
                        true,
                    )
                    .is_none(),
                    "cuda kernel launch must fail closed on unsupported hosts"
                );
                assert_eq!(
                    CUDA_DISABLED.load(Ordering::SeqCst),
                    disabled_before,
                    "probing unsupported hosts must not mutate disable flag"
                );
                return;
            }
            let Some(status) = launch_poseidon_kernel(
                PoseidonKernel::Poseidon2,
                &mut state,
                1,
                1,
                BUFFER_POSEIDON2_RC,
                rc_flat,
                BUFFER_POSEIDON2_MDS,
                mds_flat,
                POSEIDON_FULL_ROUNDS,
                POSEIDON_PARTIAL_ROUNDS,
                false,
                true,
            ) else {
                return;
            };
            assert_eq!(
                status.code, POSEIDON_STATUS_ERR_STRIDE,
                "expected stride error from poseidon2 kernel"
            );
            assert_eq!(
                status.detail, 1,
                "stride error detail should surface the provided stride"
            );
            assert_eq!(
                CUDA_DISABLED.load(Ordering::SeqCst),
                disabled_before,
                "fault injection must not disable CUDA backend"
            );
        }

        #[test]
        fn ed25519_selftest_covers_signature_kernel() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping ed25519 self-test regression");
                return;
            }
            assert!(
                ed25519_cuda_selftest(),
                "ed25519 CUDA self-test must accept the golden truth set",
            );
        }

        #[test]
        fn sha256_merkle_selftest_covers_cuda_kernels() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping SHA-256 merkle self-test regression");
                return;
            }
            assert!(
                sha256_leaves_cuda_selftest(),
                "sha256 leaves CUDA self-test must accept the golden truth set",
            );
            assert!(
                sha256_pairs_reduce_cuda_selftest(),
                "sha256 pairs-reduce CUDA self-test must accept the golden truth set",
            );
        }

        #[test]
        fn vector_selftest_covers_cuda_kernels() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping vector self-test regression");
                return;
            }
            assert!(
                vadd64_cuda_selftest(),
                "vadd64 CUDA self-test must accept the golden truth set",
            );
            assert!(
                bit_ops_cuda_selftest(),
                "bitwise CUDA self-test must accept the golden truth set",
            );
        }

        #[test]
        fn aes_batch_selftest_covers_cuda_kernels() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping AES batch self-test regression");
                return;
            }
            assert!(
                aes_batch_cuda_selftest(),
                "AES batch CUDA self-test must accept the golden truth set",
            );
        }

        #[test]
        fn sha256_merkle_selftest_survives_prior_cuda_truth_sets() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping SHA-256 post-order regression");
                return;
            }
            assert!(
                aes_batch_cuda_selftest(),
                "AES batch CUDA self-test must accept the golden truth set before SHA-256",
            );
            assert!(
                bn254_cuda_selftest(),
                "BN254 CUDA self-test must accept the golden truth set before SHA-256",
            );
            assert!(
                ed25519_cuda_selftest(),
                "Ed25519 CUDA self-test must accept the golden truth set before SHA-256",
            );
            assert!(
                sha256_leaves_cuda_selftest(),
                "sha256 leaves CUDA self-test must remain green after prior truth sets",
            );
            assert!(
                sha256_pairs_reduce_cuda_selftest(),
                "sha256 pairs-reduce CUDA self-test must remain green after prior truth sets",
            );
        }

        #[test]
        fn cached_cuda_selftest_rebinds_context_on_new_thread() {
            reset_cuda_backend_for_tests();
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping cached self-test thread rebind regression");
                return;
            }

            let blocks = std::thread::spawn(|| {
                let mut block_a = [0u8; 64];
                block_a[0] = b'a';
                block_a[1] = b'b';
                block_a[2] = b'c';
                block_a[3] = 0x80;
                block_a[63] = 24;

                let mut block_b = [0u8; 64];
                block_b[0] = b'n';
                block_b[1] = b'o';
                block_b[2] = b'r';
                block_b[3] = b'i';
                block_b[4] = b't';
                block_b[5] = b'o';
                block_b[6] = 0x80;
                block_b[63] = 48;

                sha256_leaves_cuda(&[block_a, block_b])
            })
            .join()
            .expect("worker thread must complete");

            assert!(
                blocks.is_some(),
                "cached self-test should rebind a CUDA context on fresh worker threads",
            );
        }

        #[test]
        fn public_bitonic_sort_pairs_match_scalar_when_cuda_available() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping public bitonic-sort parity regression");
                return;
            }

            let mut hi = [5u64, 3, 5, 3, 3];
            let mut lo = [7u64, 9, 1, 2, 1];
            let mut expected: Vec<(u64, u64)> =
                hi.iter().copied().zip(lo.iter().copied()).collect();
            expected.sort_unstable();

            assert_eq!(bitonic_sort_pairs(&mut hi, &mut lo), Some(()));
            assert_eq!(
                hi.into_iter().zip(lo).collect::<Vec<_>>(),
                expected,
                "bitonic_sort_pairs should match scalar lexicographic ordering",
            );
        }

        #[test]
        fn public_vector_helpers_match_scalar_when_cuda_available() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping public vector parity regression");
                return;
            }

            let a32 = [0xffff_0000u32, 0x1234_5678, 0x0f0f_0f0f, 0xaaaa_5555];
            let b32 = [0x00ff_ff00u32, 0xf0f0_f0f0, 0x3333_cccc, 0x5555_aaaa];
            let expected_add32: Vec<u32> = a32
                .iter()
                .zip(b32.iter())
                .map(|(&lhs, &rhs)| lhs.wrapping_add(rhs))
                .collect();
            let expected_and: Vec<u32> = a32
                .iter()
                .zip(b32.iter())
                .map(|(&lhs, &rhs)| lhs & rhs)
                .collect();
            let expected_xor: Vec<u32> = a32
                .iter()
                .zip(b32.iter())
                .map(|(&lhs, &rhs)| lhs ^ rhs)
                .collect();
            let expected_or: Vec<u32> = a32
                .iter()
                .zip(b32.iter())
                .map(|(&lhs, &rhs)| lhs | rhs)
                .collect();

            assert_eq!(vadd32_cuda(&a32, &b32), Some(expected_add32));
            assert_eq!(vand_cuda(&a32, &b32), Some(expected_and));
            assert_eq!(vxor_cuda(&a32, &b32), Some(expected_xor));
            assert_eq!(vor_cuda(&a32, &b32), Some(expected_or));

            let a64 = [0xffff_ffff_ffff_ff00u64, 0x1234_5678_9abc_def0];
            let b64 = [0x0000_0000_0000_0201u64, 0x0fed_cba9_8765_4321];
            let expected_add64: Vec<u64> = a64
                .iter()
                .zip(b64.iter())
                .map(|(&lhs, &rhs)| lhs.wrapping_add(rhs))
                .collect();
            assert_eq!(vadd64_cuda(&a64, &b64), Some(expected_add64));

            let fa = [1.0f32, -2.5, 3.25, 4.5];
            let fb = [2.0f32, 0.5, -1.25, 3.5];
            let expected_f32: Vec<f32> = fa
                .iter()
                .zip(fb.iter())
                .map(|(&lhs, &rhs)| lhs + rhs)
                .collect();
            assert_eq!(vector_add_f32(&fa, &fb), Some(expected_f32));
        }

        #[test]
        fn public_sha256_compress_matches_scalar_when_cuda_available() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping public sha256-compress parity regression");
                return;
            }

            let mut block = [0u8; 64];
            block[0] = b'a';
            block[1] = b'b';
            block[2] = b'c';
            block[3] = 0x80;
            block[63] = 24;

            let mut scalar = [
                0x6a09e667u32,
                0xbb67ae85,
                0x3c6ef372,
                0xa54ff53a,
                0x510e527f,
                0x9b05688c,
                0x1f83d9ab,
                0x5be0cd19,
            ];
            let mut cuda = scalar;
            sha256_scalar_ref(&mut scalar, &block);

            assert!(sha256_compress_cuda(&mut cuda, &block));
            assert_eq!(cuda, scalar);
        }

        #[test]
        fn public_keccak_matches_scalar_when_cuda_available() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping public keccak parity regression");
                return;
            }

            let mut scalar = [0u64; 25];
            for (index, lane) in scalar.iter_mut().enumerate() {
                *lane = index as u64;
            }
            let mut cuda = scalar;
            crate::sha3::keccak_f1600(&mut scalar);

            assert!(keccak_f1600_cuda(&mut cuda));
            assert_eq!(cuda, scalar);
        }

        #[test]
        fn public_poseidon_helpers_match_scalar_when_cuda_available() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping public Poseidon parity regression");
                return;
            }

            let single2 = (1u64, 2u64);
            assert_eq!(
                poseidon2_cuda(single2.0, single2.1),
                Some(crate::poseidon::poseidon2_simd(single2.0, single2.1))
            );

            let single6 = [1u64, 2, 3, 4, 5, 6];
            assert_eq!(
                poseidon6_cuda(single6),
                Some(crate::poseidon::poseidon6_simd(single6))
            );

            let many2 = [(0u64, 1u64), (7, 9), (11, 13), (21, 34)];
            let expected_many2: Vec<u64> = many2
                .iter()
                .map(|&(lhs, rhs)| crate::poseidon::poseidon2_simd(lhs, rhs))
                .collect();
            assert_eq!(poseidon2_cuda_many(&many2), Some(expected_many2));

            let many6 = [
                [1u64, 2, 3, 4, 5, 6],
                [7u64, 8, 9, 10, 11, 12],
                [13u64, 21, 34, 55, 89, 144],
            ];
            let expected_many6: Vec<u64> = many6
                .iter()
                .copied()
                .map(crate::poseidon::poseidon6_simd)
                .collect();
            assert_eq!(poseidon6_cuda_many(&many6), Some(expected_many6));
        }

        #[test]
        fn public_aes_round_helpers_match_scalar_when_cuda_available() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping public AES round parity regression");
                return;
            }

            let state = [
                0x00u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                0xdd, 0xee, 0xff,
            ];
            let rk = [
                0x0f, 0x15, 0x71, 0xc9, 0x47, 0xd9, 0xe8, 0x59, 0x0c, 0xb7, 0xad, 0xd6, 0xaf, 0x7f,
                0x67, 0x98,
            ];
            let expected_enc = crate::aes::aesenc_impl(state, rk);
            let expected_dec = crate::aes::aesdec_impl(expected_enc, rk);

            assert_eq!(aesenc_cuda(state, rk), Some(expected_enc));
            assert_eq!(aesdec_cuda(expected_enc, rk), Some(expected_dec));
        }

        #[test]
        fn public_sha256_leaves_matches_scalar_when_cuda_available() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping public sha256-leaves parity regression");
                return;
            }

            let mut block_a = [0u8; 64];
            block_a[0] = b'a';
            block_a[1] = b'b';
            block_a[2] = b'c';
            block_a[3] = 0x80;
            block_a[63] = 24;

            let mut block_b = [0u8; 64];
            block_b[0] = b'n';
            block_b[1] = b'o';
            block_b[2] = b'r';
            block_b[3] = b'i';
            block_b[4] = b't';
            block_b[5] = b'o';
            block_b[6] = 0x80;
            block_b[63] = 48;

            let blocks = [block_a, block_b];
            let expected: Vec<[u8; 32]> = blocks
                .iter()
                .map(|block| {
                    let mut state = [
                        0x6a09e667u32,
                        0xbb67ae85,
                        0x3c6ef372,
                        0xa54ff53a,
                        0x510e527f,
                        0x9b05688c,
                        0x1f83d9ab,
                        0x5be0cd19,
                    ];
                    sha256_scalar_ref(&mut state, block);
                    let mut digest = [0u8; 32];
                    for (index, word) in state.iter().enumerate() {
                        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
                    }
                    digest
                })
                .collect();

            assert_eq!(sha256_leaves_cuda(&blocks), Some(expected));
        }

        #[test]
        fn public_sha256_pairs_reduce_matches_scalar_when_cuda_available() {
            fn cpu_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
                let mut state = [
                    0x6a09e667u32,
                    0xbb67ae85,
                    0x3c6ef372,
                    0xa54ff53a,
                    0x510e527f,
                    0x9b05688c,
                    0x1f83d9ab,
                    0x5be0cd19,
                ];
                let mut block = [0u8; 64];
                block[..32].copy_from_slice(left);
                block[32..].copy_from_slice(right);
                sha256_scalar_ref(&mut state, &block);
                let mut pad = [0u8; 64];
                pad[0] = 0x80;
                pad[62] = 0x02;
                pad[63] = 0x00;
                sha256_scalar_ref(&mut state, &pad);
                let mut out = [0u8; 32];
                for (index, word) in state.iter().enumerate() {
                    out[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
                }
                out
            }

            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping public sha256-pairs parity regression");
                return;
            }

            let mut d0 = [0u8; 32];
            let mut d1 = [0u8; 32];
            let mut d2 = [0u8; 32];
            for (index, byte) in d0.iter_mut().enumerate() {
                *byte = index as u8;
            }
            for (index, byte) in d1.iter_mut().enumerate() {
                *byte = 0x40 + index as u8;
            }
            for (index, byte) in d2.iter_mut().enumerate() {
                *byte = 0x80 + index as u8;
            }
            let digests = [d0, d1, d2];
            let first = cpu_pair(&digests[0], &digests[1]);
            let expected = cpu_pair(&first, &digests[2]);

            assert_eq!(sha256_pairs_reduce_cuda(&digests), Some(expected));
        }

        #[test]
        fn public_aes_batch_matches_scalar_when_cuda_available() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping public AES batch regression");
                return;
            }

            let states = [
                [
                    0x00u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                    0xdd, 0xee, 0xff,
                ],
                [
                    0xffu8, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33,
                    0x22, 0x11, 0x00,
                ],
            ];
            let rk = [
                0x0f, 0x15, 0x71, 0xc9, 0x47, 0xd9, 0xe8, 0x59, 0x0c, 0xb7, 0xad, 0xd6, 0xaf, 0x7f,
                0x67, 0x98,
            ];
            let expected_enc: Vec<[u8; 16]> = states
                .iter()
                .map(|&state| crate::aes::aesenc_impl(state, rk))
                .collect();
            let expected_dec: Vec<[u8; 16]> = states
                .iter()
                .map(|&state| crate::aes::aesdec_impl(state, rk))
                .collect();

            assert_eq!(aesenc_batch_cuda(&states, rk), Some(expected_enc));
            assert_eq!(aesdec_batch_cuda(&states, rk), Some(expected_dec));
        }

        #[test]
        fn public_aes_rounds_batch_matches_scalar_when_cuda_available() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping public AES rounds batch regression");
                return;
            }

            let states = [
                [
                    0x00u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                    0xdd, 0xee, 0xff,
                ],
                [
                    0xffu8, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33,
                    0x22, 0x11, 0x00,
                ],
            ];
            let round_keys = [
                [
                    0x0f, 0x15, 0x71, 0xc9, 0x47, 0xd9, 0xe8, 0x59, 0x0c, 0xb7, 0xad, 0xd6, 0xaf,
                    0x7f, 0x67, 0x98,
                ],
                [
                    0xa5, 0x40, 0x76, 0x28, 0x10, 0x4f, 0xdc, 0xe6, 0x43, 0xdd, 0x27, 0x0f, 0x6c,
                    0xa7, 0x63, 0x6f,
                ],
                [
                    0x2c, 0x5e, 0x2f, 0x88, 0x6a, 0x84, 0xd2, 0x57, 0x8b, 0x3f, 0x8c, 0x9c, 0x4f,
                    0x11, 0x64, 0x15,
                ],
            ];
            let expected_enc: Vec<[u8; 16]> = states
                .iter()
                .copied()
                .map(|state| {
                    round_keys
                        .iter()
                        .copied()
                        .fold(state, |block, rk| crate::aes::aesenc_impl(block, rk))
                })
                .collect();
            let expected_dec: Vec<[u8; 16]> = states
                .iter()
                .copied()
                .map(|state| {
                    round_keys
                        .iter()
                        .copied()
                        .fold(state, |block, rk| crate::aes::aesdec_impl(block, rk))
                })
                .collect();

            assert_eq!(
                aesenc_rounds_batch_cuda(&states, &round_keys),
                Some(expected_enc)
            );
            assert_eq!(
                aesdec_rounds_batch_cuda(&states, &round_keys),
                Some(expected_dec)
            );
        }

        #[test]
        fn public_bn254_helpers_match_scalar_when_cuda_available() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping public BN254 parity regression");
                return;
            }

            let add_lhs = crate::bn254_vec::FieldElem::from_u64(0x1234_5678_9abc_def0);
            let add_rhs = crate::bn254_vec::FieldElem::from_u64(0x0fed_cba9_8765_4321);
            let sub_lhs = crate::bn254_vec::FieldElem::from_u64(0x0fff_ffff_ffff_fffb);
            let sub_rhs = crate::bn254_vec::FieldElem::from_u64(0x0000_0000_0000_0011);
            let mul_lhs = crate::bn254_vec::FieldElem::from_u64(0x0102_0304_0506_0708);
            let mul_rhs = crate::bn254_vec::FieldElem::from_u64(0x1112_1314_1516_1718);

            assert_eq!(
                bn254_add_cuda(add_lhs.0, add_rhs.0),
                Some(crate::bn254_vec::add_scalar(add_lhs, add_rhs).0)
            );
            assert_eq!(
                bn254_sub_cuda(sub_lhs.0, sub_rhs.0),
                Some(crate::bn254_vec::sub_scalar(sub_lhs, sub_rhs).0)
            );
            assert_eq!(
                bn254_mul_cuda(mul_lhs.0, mul_rhs.0),
                Some(crate::bn254_vec::mul_scalar(mul_lhs, mul_rhs).0)
            );
        }

        #[test]
        fn public_bn254_batch_helpers_match_scalar_when_cuda_available() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping public BN254 batch parity regression");
                return;
            }

            let add_lhs = vec![
                crate::bn254_vec::FieldElem::from_u64(0x1234_5678_9abc_def0),
                crate::bn254_vec::FieldElem::from_u64(0x2222_3333_4444_5555),
                crate::bn254_vec::FieldElem::from_u64(0x0fff_eeee_dddd_cccc),
            ];
            let add_rhs = vec![
                crate::bn254_vec::FieldElem::from_u64(0x0fed_cba9_8765_4321),
                crate::bn254_vec::FieldElem::from_u64(0x0101_0101_0101_0101),
                crate::bn254_vec::FieldElem::from_u64(0x1111_0000_ffff_eeee),
            ];
            let sub_lhs = vec![
                crate::bn254_vec::FieldElem::from_u64(0x0fff_ffff_ffff_fffb),
                crate::bn254_vec::FieldElem::from_u64(0x9999_8888_7777_6666),
                crate::bn254_vec::FieldElem::from_u64(0x1212_1212_1212_1212),
            ];
            let sub_rhs = vec![
                crate::bn254_vec::FieldElem::from_u64(0x0000_0000_0000_0011),
                crate::bn254_vec::FieldElem::from_u64(0x1111_2222_3333_4444),
                crate::bn254_vec::FieldElem::from_u64(0x0101_0101_0101_0101),
            ];
            let mul_lhs = vec![
                crate::bn254_vec::FieldElem::from_u64(0x0102_0304_0506_0708),
                crate::bn254_vec::FieldElem::from_u64(0x1112_1314_1516_1718),
                crate::bn254_vec::FieldElem::from_u64(0x2122_2324_2526_2728),
            ];
            let mul_rhs = vec![
                crate::bn254_vec::FieldElem::from_u64(0x1112_1314_1516_1718),
                crate::bn254_vec::FieldElem::from_u64(0x0102_0304_0506_0708),
                crate::bn254_vec::FieldElem::from_u64(0x3334_3536_3738_393a),
            ];

            let add_lhs_words: Vec<[u64; BN254_LIMBS]> =
                add_lhs.iter().map(|elem| elem.0).collect();
            let add_rhs_words: Vec<[u64; BN254_LIMBS]> =
                add_rhs.iter().map(|elem| elem.0).collect();
            let sub_lhs_words: Vec<[u64; BN254_LIMBS]> =
                sub_lhs.iter().map(|elem| elem.0).collect();
            let sub_rhs_words: Vec<[u64; BN254_LIMBS]> =
                sub_rhs.iter().map(|elem| elem.0).collect();
            let mul_lhs_words: Vec<[u64; BN254_LIMBS]> =
                mul_lhs.iter().map(|elem| elem.0).collect();
            let mul_rhs_words: Vec<[u64; BN254_LIMBS]> =
                mul_rhs.iter().map(|elem| elem.0).collect();

            let expected_add: Vec<[u64; BN254_LIMBS]> = add_lhs
                .iter()
                .copied()
                .zip(add_rhs.iter().copied())
                .map(|(lhs, rhs)| crate::bn254_vec::add_scalar(lhs, rhs).0)
                .collect();
            let expected_sub: Vec<[u64; BN254_LIMBS]> = sub_lhs
                .iter()
                .copied()
                .zip(sub_rhs.iter().copied())
                .map(|(lhs, rhs)| crate::bn254_vec::sub_scalar(lhs, rhs).0)
                .collect();
            let expected_mul: Vec<[u64; BN254_LIMBS]> = mul_lhs
                .iter()
                .copied()
                .zip(mul_rhs.iter().copied())
                .map(|(lhs, rhs)| crate::bn254_vec::mul_scalar(lhs, rhs).0)
                .collect();

            assert_eq!(
                bn254_add_batch_cuda(&add_lhs_words, &add_rhs_words),
                Some(expected_add)
            );
            assert_eq!(
                bn254_sub_batch_cuda(&sub_lhs_words, &sub_rhs_words),
                Some(expected_sub)
            );
            assert_eq!(
                bn254_mul_batch_cuda(&mul_lhs_words, &mul_rhs_words),
                Some(expected_mul)
            );
            assert_eq!(bn254_add_batch_cuda(&[], &[]), Some(Vec::new()));
            assert_eq!(
                bn254_add_batch_cuda(&add_lhs_words, &add_rhs_words[..2]),
                None
            );
        }

        #[test]
        fn public_ed25519_verify_helpers_match_cpu_when_cuda_available() {
            use ed25519_dalek::{Signature, Signer, SigningKey};

            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping public ed25519 parity regression");
                return;
            }

            let signing_key = SigningKey::from_bytes(&[0x11; 32]);
            let msg = b"cuda ed25519 public parity";
            let sig = signing_key.sign(msg).to_bytes();
            let pk_bytes = signing_key.verifying_key().to_bytes();
            let expected_good = signing_key
                .verifying_key()
                .verify_strict(msg, &Signature::from_bytes(&sig))
                .is_ok();

            let mut bad_sig = sig;
            bad_sig[0] ^= 0x42;
            let expected_bad = signing_key
                .verifying_key()
                .verify_strict(msg, &Signature::from_bytes(&bad_sig))
                .is_ok();

            assert_eq!(
                ed25519_verify_cuda(msg, &sig, &pk_bytes),
                Some(expected_good)
            );
            assert_eq!(
                ed25519_verify_cuda(msg, &bad_sig, &pk_bytes),
                Some(expected_bad)
            );
            let singleton_hram =
                crate::signature::ed25519_challenge_scalar_bytes(&sig, &pk_bytes, msg);
            assert_eq!(
                ed25519_verify_batch_cuda(&[sig], &[pk_bytes], &[singleton_hram]),
                Some(vec![expected_good])
            );

            let key1 = SigningKey::from_bytes(&[0x22; 32]);
            let key2 = SigningKey::from_bytes(&[0x33; 32]);
            let msg1 = b"cuda batch one";
            let msg2 = b"cuda batch two";
            let sig1 = key1.sign(msg1).to_bytes();
            let sig2_good = key2.sign(msg2).to_bytes();
            let mut sig2_bad = sig2_good;
            sig2_bad[0] ^= 0x11;

            let pks = vec![
                key1.verifying_key().to_bytes(),
                key2.verifying_key().to_bytes(),
            ];
            let sigs = vec![sig1, sig2_bad];
            let hrams = vec![
                crate::signature::ed25519_challenge_scalar_bytes(&sigs[0], &pks[0], msg1),
                crate::signature::ed25519_challenge_scalar_bytes(&sigs[1], &pks[1], msg2),
            ];
            let expected_batch = vec![
                key1.verifying_key()
                    .verify_strict(msg1, &Signature::from_bytes(&sigs[0]))
                    .is_ok(),
                key2.verifying_key()
                    .verify_strict(msg2, &Signature::from_bytes(&sigs[1]))
                    .is_ok(),
            ];

            assert_eq!(
                ed25519_verify_batch_cuda(&sigs, &pks, &hrams),
                Some(expected_batch)
            );
        }

        #[test]
        fn public_ed25519_single_helper_matches_singleton_batch_when_cuda_available() {
            use ed25519_dalek::{Signer, SigningKey};

            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping single-vs-batch ed25519 regression");
                return;
            }

            let signing_key = SigningKey::from_bytes(&[0x44; 32]);
            let msg = b"cuda singletons should follow batch path";
            let sig = signing_key.sign(msg).to_bytes();
            let pk_bytes = signing_key.verifying_key().to_bytes();
            let hram = crate::signature::ed25519_challenge_scalar_bytes(&sig, &pk_bytes, msg);
            let single = ed25519_verify_cuda(msg, &sig, &pk_bytes);
            let batch = ed25519_verify_batch_cuda(&[sig], &[pk_bytes], &[hram])
                .and_then(|mut out| out.pop());
            assert_eq!(single, batch);
        }

        #[test]
        fn bn254_selftest_covers_cuda_kernels() {
            if !ensure_cuda_selftest() {
                eprintln!("CUDA unavailable; skipping BN254 self-test regression");
                return;
            }
            assert!(
                bn254_cuda_selftest(),
                "bn254 CUDA self-test must accept the golden truth set",
            );
        }
    }
}

#[cfg(feature = "cuda")]
pub(crate) use imp::sha256_merkle_root_cuda;
#[cfg(feature = "cuda")]
pub use imp::*;

#[cfg(feature = "cuda")]
/// Sort `(hi, lo)` key pairs lexicographically with the CUDA bitonic kernel.
///
/// Returns `None` when CUDA is unavailable, disabled, or the input slices have
/// different lengths.
#[allow(dead_code)]
pub fn bitonic_sort_pairs(hi: &mut [u64], lo: &mut [u64]) -> Option<()> {
    imp::bitonic_sort_pairs(hi, lo)
}

#[cfg(not(feature = "cuda"))]
pub fn cuda_available() -> bool {
    false
}

#[cfg(not(feature = "cuda"))]
pub fn cuda_disabled() -> bool {
    false
}

#[cfg(not(feature = "cuda"))]
pub fn cuda_last_error_message() -> Option<String> {
    None
}

#[cfg(not(feature = "cuda"))]
#[doc(hidden)]
pub fn reset_cuda_backend_for_tests() {}

#[cfg(not(feature = "cuda"))]
/// Sort `(hi, lo)` key pairs lexicographically with the CUDA bitonic kernel.
///
/// Returns `None` when the crate is built without CUDA support.
pub fn bitonic_sort_pairs(_hi: &mut [u64], _lo: &mut [u64]) -> Option<()> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn vector_add_f32(_a: &[f32], _b: &[f32]) -> Option<Vec<f32>> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn vadd32_cuda(_a: &[u32], _b: &[u32]) -> Option<Vec<u32>> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn vadd64_cuda(_a: &[u64], _b: &[u64]) -> Option<Vec<u64>> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn vand_cuda(_a: &[u32], _b: &[u32]) -> Option<Vec<u32>> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn vxor_cuda(_a: &[u32], _b: &[u32]) -> Option<Vec<u32>> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn vor_cuda(_a: &[u32], _b: &[u32]) -> Option<Vec<u32>> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn sha256_compress_cuda(_state: &mut [u32; 8], _block: &[u8; 64]) -> bool {
    false
}

#[cfg(not(feature = "cuda"))]
pub fn sha256_leaves_cuda(_blocks: &[[u8; 64]]) -> Option<Vec<[u8; 32]>> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn sha256_pairs_reduce_cuda(_digests: &[[u8; 32]]) -> Option<[u8; 32]> {
    None
}

#[cfg(not(feature = "cuda"))]
pub(crate) fn sha256_merkle_root_cuda(_blocks: &[[u8; 64]]) -> Option<[u8; 32]> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn poseidon2_cuda(_a: u64, _b: u64) -> Option<u64> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn poseidon2_cuda_many(_inputs: &[(u64, u64)]) -> Option<Vec<u64>> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn poseidon6_cuda(_inputs: [u64; 6]) -> Option<u64> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn poseidon6_cuda_many(_inputs: &[[u64; 6]]) -> Option<Vec<u64>> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn keccak_f1600_cuda(_state: &mut [u64; 25]) -> bool {
    false
}

#[cfg(not(feature = "cuda"))]
pub fn aesenc_cuda(_state: [u8; 16], _rk: [u8; 16]) -> Option<[u8; 16]> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn aesdec_cuda(_state: [u8; 16], _rk: [u8; 16]) -> Option<[u8; 16]> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn aesenc_batch_cuda(_states: &[[u8; 16]], _rk: [u8; 16]) -> Option<Vec<[u8; 16]>> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn aesdec_batch_cuda(_states: &[[u8; 16]], _rk: [u8; 16]) -> Option<Vec<[u8; 16]>> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn aesenc_rounds_batch_cuda(
    _states: &[[u8; 16]],
    _round_keys: &[[u8; 16]],
) -> Option<Vec<[u8; 16]>> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn aesdec_rounds_batch_cuda(
    _states: &[[u8; 16]],
    _round_keys: &[[u8; 16]],
) -> Option<Vec<[u8; 16]>> {
    None
}

#[cfg(not(feature = "cuda"))]
/// Batch BN254 add: unavailable when the crate is built without CUDA support.
pub fn bn254_add_batch_cuda(_lhs: &[[u64; 4]], _rhs: &[[u64; 4]]) -> Option<Vec<[u64; 4]>> {
    None
}

#[cfg(not(feature = "cuda"))]
/// Batch BN254 subtract: unavailable when the crate is built without CUDA support.
pub fn bn254_sub_batch_cuda(_lhs: &[[u64; 4]], _rhs: &[[u64; 4]]) -> Option<Vec<[u64; 4]>> {
    None
}

#[cfg(not(feature = "cuda"))]
/// Batch BN254 multiply: unavailable when the crate is built without CUDA support.
pub fn bn254_mul_batch_cuda(_lhs: &[[u64; 4]], _rhs: &[[u64; 4]]) -> Option<Vec<[u64; 4]>> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn bn254_add_cuda(_a: [u64; 4], _b: [u64; 4]) -> Option<[u64; 4]> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn bn254_sub_cuda(_a: [u64; 4], _b: [u64; 4]) -> Option<[u64; 4]> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn bn254_mul_cuda(_a: [u64; 4], _b: [u64; 4]) -> Option<[u64; 4]> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn ed25519_verify_cuda(_msg: &[u8], _sig: &[u8; 64], _pk: &[u8; 32]) -> Option<bool> {
    None
}

#[cfg(not(feature = "cuda"))]
pub fn ed25519_verify_batch_cuda(
    _signatures: &[[u8; 64]],
    _public_keys: &[[u8; 32]],
    _hrams: &[[u8; 32]],
) -> Option<Vec<bool>> {
    None
}
