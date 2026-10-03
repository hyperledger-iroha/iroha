//! Independent KAT boundaries, paired CPU parity and exact admission.
use super::*;
use zeroize::Zeroizing;
fn bytes(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 73 + len).to_le_bytes()[0]).collect()
}
#[test]
fn paired_cpu_matches_scalar_for_different_partial_rates_lengths_and_worker_counts() {
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                for prefix_len in [0, 1, 7, 135, 136, 137, 271] {
                    let mut prefix = Sha3_256V1::new();
                    prefix.update(&bytes(prefix_len));
                    let bodies = [0, 1, 7, 8, 135, 136, 137, 271, 272, 273, 8192].map(bytes);
                    let jobs = bodies
                        .iter()
                        .flat_map(|b| [Job::new(&prefix, b), Job::new(&prefix, b)])
                        .collect::<Vec<_>>();
                    let mut output = Zeroizing::new(vec![[0; 32]; jobs.len()]);
                    hash_cpu(&jobs, &mut output).unwrap();
                    for (job, actual) in jobs.iter().zip(output.iter()) {
                        assert_eq!(*actual, job.scalar().into_bytes());
                    }
                    let heterogeneous = [jobs[0], jobs[jobs.len() - 1], jobs[3]];
                    hash_cpu(&heterogeneous, &mut output[..3]).unwrap();
                    for (job, actual) in heterogeneous.iter().zip(output.iter()) {
                        assert_eq!(*actual, job.scalar().into_bytes());
                    }
                }
            });
    }
}
#[test]
fn fixed_independent_hashlib_vectors_match_batched_paths() {
    for line in include_str!("../../fastpq_isi/src/assets/keccak256_reference_v1.tsv").lines() {
        let fields = line.split('\t').collect::<Vec<_>>();
        let length = fields[0].parse::<usize>().unwrap();
        // The shared independent hashlib fixture uses this exact byte pattern;
        // the heterogeneous paired-parity fixture above has a different one.
        let message = (0..length)
            .map(|index| (index * 37 + 11).to_le_bytes()[0])
            .collect::<Vec<_>>();
        let expected = hex::decode(fields[1]).unwrap();
        for split in [0, 1, 135, 136, 137, length].map(|n| n.min(length)) {
            if length - split > MAX_BODY_BYTES {
                continue;
            }
            let mut prefix = Sha3_256V1::new();
            prefix.update(&message[..split]);
            let jobs = [Job::new(&prefix, &message[split..]); 2];
            let mut actual = [[0; 32]; 2];
            hash_cpu(&jobs, &mut actual).unwrap();
            for digest in actual {
                assert_eq!(digest.as_slice(), expected);
            }
        }
    }
}
#[test]
fn bounded_jobs_reject_malformed_shapes_before_touching_output() {
    let prefix = Sha3_256V1::new();
    let job = Job::new(&prefix, b"body");
    let mut output = [[0xA7; 32]; 1];
    assert!(hash_cpu(&[], &mut output).is_err());
    assert!(hash_cpu(&[job, job], &mut output).is_err());
    assert!(hash_cpu(&[Job::new(&prefix, &[0; 8193])], &mut output).is_err());
    assert_eq!(output, [[0xA7; 32]; 1]);
    let jobs = vec![job; MAX_JOBS];
    let mut output = vec![[0; 32]; MAX_JOBS];
    hash_cpu(&jobs, &mut output).unwrap();
    assert!(output.iter().all(|v| *v == job.scalar().into_bytes()));
    assert!(device_payload_bytes(0, 0).is_err());
    assert!(device_payload_bytes(1025, 0).is_err());
    assert!(device_payload_bytes(1, 8193).is_err());
    assert!(device_payload_bytes(1024, 1024 * 8192).unwrap() < 10 * 1024 * 1024);
}
#[test]
fn work_counts_rate_crossing_and_final_padding_without_double_charging_prefix() {
    for prefix_len in [0, 1, 135, 136, 137, 4096] {
        let mut prefix = Sha3_256V1::new();
        prefix.update(&bytes(prefix_len));
        for body_len in [0, 1, 135, 136, 137, 8192] {
            let body = bytes(body_len);
            let job = Job::new(&prefix, &body);
            assert_eq!(
                prefix_len / 136 + job.permutations().unwrap(),
                (prefix_len + body_len) / 136 + 1
            );
        }
    }
}
