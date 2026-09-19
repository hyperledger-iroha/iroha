// The benchmarks aim to only measure times of the operations in their names.
// That's why all use Bencher::iter_batched which enables non-benchmarked
// preparation before running the measured function.
// Insert (which doesn't completely avoid updates, but makes them unlikely),
// remove and search have benchmarks with empty values and with custom structs
// of 42 64-bit integers.
// (as a sidenote, the performance really differs; in the case of remove, the
// remove function itself returns original value - the benchmark doesn't use
// this value, but performance is significantly worse - about twice on my
// machine - than the empty value remove; it might be interesting to see if a
// remove function returning void would perform better, ie. if the returns
// are optimized - omitted in this case).
// The counts of inserted/removed/searched elements are chosen at random from
// constant ranges in an attempt to avoid a single count performing better
// because of specific HW features of computers the code is benchmarked with.

extern crate concread;
extern crate criterion;
extern crate rand;

use concread::hashmap::*;
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use rand::{rng, Rng};
use std::hint::black_box;

// ranges of counts for different benchmarks (MINs are inclusive, MAXes exclusive):
const INSERT_COUNT_MIN: usize = 120;
const INSERT_COUNT_MAX: usize = 140;
const INSERT_COUNT_FOR_REMOVE_MIN: usize = 340;
const INSERT_COUNT_FOR_REMOVE_MAX: usize = 360;
const REMOVE_COUNT_MIN: usize = 120;
const REMOVE_COUNT_MAX: usize = 140;
const INSERT_COUNT_FOR_SEARCH_MIN: usize = 120;
const INSERT_COUNT_FOR_SEARCH_MAX: usize = 140;
const SEARCH_COUNT_MIN: usize = 120;
const SEARCH_COUNT_MAX: usize = 140;
// In the search benches, we randomly search for elements of a range of SEARCH_SIZE_NUMERATOR / SEARCH_SIZE_DENOMINATOR
// times the number of elements contained.
const SEARCH_SIZE_NUMERATOR: usize = 4;
const SEARCH_SIZE_DENOMINATOR: usize = 3;

pub fn insert_empty_value_rollback(c: &mut Criterion) {
    c.bench_function("insert_empty_value_rollback", |b| {
        b.iter_batched(
            || prepare_insert(()),
            |(mut map, list)| {
                insert_vec(&mut map, list);
            },
            BatchSize::SmallInput,
        )
    });
}

pub fn insert_empty_value_commit(c: &mut Criterion) {
    c.bench_function("insert_empty_value_commit", |b| {
        b.iter_batched(
            || prepare_insert(()),
            |(mut map, list)| insert_vec(&mut map, list).commit(),
            BatchSize::SmallInput,
        )
    });
}

pub fn insert_struct_value_rollback(c: &mut Criterion) {
    c.bench_function("insert_struct_value_rollback", |b| {
        b.iter_batched(
            || prepare_insert(Struct::default()),
            |(mut map, list)| {
                insert_vec(&mut map, list);
            },
            BatchSize::SmallInput,
        )
    });
}

pub fn insert_struct_value_commit(c: &mut Criterion) {
    c.bench_function("insert_struct_value_commit", |b| {
        b.iter_batched(
            || prepare_insert(Struct::default()),
            |(mut map, list)| insert_vec(&mut map, list).commit(),
            BatchSize::SmallInput,
        )
    });
}

pub fn remove_empty_value_rollback(c: &mut Criterion) {
    c.bench_function("remove_empty_value_rollback", |b| {
        b.iter_batched(
            || prepare_remove(()),
            |(ref mut map, ref list)| {
                remove_vec(map, list);
            },
            BatchSize::SmallInput,
        )
    });
}

pub fn remove_empty_value_commit(c: &mut Criterion) {
    c.bench_function("remove_empty_value_commit", |b| {
        b.iter_batched(
            || prepare_remove(()),
            |(ref mut map, ref list)| remove_vec(map, list).commit(),
            BatchSize::SmallInput,
        )
    });
}

pub fn remove_struct_value_no_read_rollback(c: &mut Criterion) {
    c.bench_function("remove_struct_value_no_read_rollback", |b| {
        b.iter_batched(
            || prepare_remove(Struct::default()),
            |(ref mut map, ref list)| {
                remove_vec(map, list);
            },
            BatchSize::SmallInput,
        )
    });
}

pub fn remove_struct_value_no_read_commit(c: &mut Criterion) {
    c.bench_function("remove_struct_value_no_read_commit", |b| {
        b.iter_batched(
            || prepare_remove(Struct::default()),
            |(ref mut map, ref list)| remove_vec(map, list).commit(),
            BatchSize::SmallInput,
        )
    });
}

pub fn search_empty_value(c: &mut Criterion) {
    c.bench_function("search_empty_value", |b| {
        b.iter_batched(
            || prepare_search(()),
            |(ref map, ref list)| search_vec(map, list),
            BatchSize::SmallInput,
        )
    });
}

pub fn search_struct_value(c: &mut Criterion) {
    c.bench_function("search_struct_value", |b| {
        b.iter_batched(
            || prepare_search(Struct::default()),
            |(ref map, ref list)| search_vec(map, list),
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    insert,
    insert_empty_value_rollback,
    insert_empty_value_commit,
    insert_struct_value_rollback,
    insert_struct_value_commit
);
criterion_group!(
    remove,
    remove_empty_value_rollback,
    remove_empty_value_commit,
    remove_struct_value_no_read_rollback,
    remove_struct_value_no_read_commit
);
criterion_group!(search, search_empty_value, search_struct_value);
criterion_main!(insert, remove, search);

// Utility functions:

fn insert_vec<V: Clone + Sync + Send + 'static>(
    map: &mut HashMap<u32, V>,
    list: Vec<(u32, V)>,
) -> HashMapWriteTxn<'_, u32, V> {
    let mut write_txn = map.write();
    for (key, val) in list.into_iter() {
        write_txn.insert(key, val);
    }
    write_txn
}

fn remove_vec<'a, V: Clone + Sync + Send + 'static>(
    map: &'a mut HashMap<u32, V>,
    list: &[u32],
) -> HashMapWriteTxn<'a, u32, V> {
    let mut write_txn = map.write();
    for i in list.iter() {
        write_txn.remove(i);
    }
    write_txn
}

fn search_vec<V: Clone + Sync + Send + 'static>(map: &HashMap<u32, V>, list: &[u32]) {
    let read_txn = map.read();
    for i in list.iter() {
        read_txn.get(black_box(i));
    }
}

#[derive(Default, Clone)]
#[allow(dead_code)]
struct Struct {
    var1: i64,
    var2: i64,
    var3: i64,
    var4: i64,
    var5: i64,
    var6: i64,
    var7: i64,
    var8: i64,
    var9: i64,
    var10: i64,
    var11: i64,
    var12: i64,
    var13: i64,
    var14: i64,
    var15: i64,
    var16: i64,
    var17: i64,
    var18: i64,
    var19: i64,
    var20: i64,
    var21: i64,
    var22: i64,
    var23: i64,
    var24: i64,
    var25: i64,
    var26: i64,
    var27: i64,
    var28: i64,
    var29: i64,
    var30: i64,
    var31: i64,
    var32: i64,
    var33: i64,
    var34: i64,
    var35: i64,
    var36: i64,
    var37: i64,
    var38: i64,
    var39: i64,
    var40: i64,
    var41: i64,
    var42: i64,
}

fn prepare_insert<V: Clone + Sync + Send + 'static>(value: V) -> (HashMap<u32, V>, Vec<(u32, V)>) {
    let mut rng = rng();
    let count = rng.random_range(INSERT_COUNT_MIN..INSERT_COUNT_MAX);
    let mut list = Vec::with_capacity(count);
    for _ in 0..count {
        list.push((
            rng.random_range(0..INSERT_COUNT_MAX << 8) as u32,
            value.to_owned(),
        ));
    }
    (HashMap::new(), list)
}

/// Prepares a remove benchmark with values in the HashMap being clones of the 'value' parameter
fn prepare_remove<V: Clone + Sync + Send + 'static>(value: V) -> (HashMap<u32, V>, Vec<u32>) {
    let mut rng = rng();
    let insert_count = rng.random_range(INSERT_COUNT_FOR_REMOVE_MIN..INSERT_COUNT_FOR_REMOVE_MAX);
    let remove_count = rng.random_range(REMOVE_COUNT_MIN..REMOVE_COUNT_MAX);
    let map = HashMap::new();
    let mut write_txn = map.write();
    for i in random_order(insert_count, insert_count).iter() {
        // We could count on the hash function alone to make the order random, but it seems
        // better to count on every possible implementation.
        write_txn.insert(*i, value.clone());
    }
    write_txn.commit();
    (map, random_order(insert_count, remove_count))
}

fn prepare_search<V: Clone + Sync + Send + 'static>(value: V) -> (HashMap<u32, V>, Vec<u32>) {
    let mut rng = rng();
    let insert_count = rng.random_range(INSERT_COUNT_FOR_SEARCH_MIN..INSERT_COUNT_FOR_SEARCH_MAX);
    let search_limit = insert_count * SEARCH_SIZE_NUMERATOR / SEARCH_SIZE_DENOMINATOR;
    let search_count = rng.random_range(SEARCH_COUNT_MIN..SEARCH_COUNT_MAX);

    // Create a HashMap with elements 0 through insert_count(-1)
    let map = HashMap::new();
    let mut write_txn = map.write();
    for k in 0..insert_count {
        write_txn.insert(k as u32, value.clone());
    }
    write_txn.commit();

    // Choose 'search_count' numbers from [0,search_limit) randomly to be searched in the created map.
    let mut list = Vec::with_capacity(search_count);
    for _ in 0..search_count {
        list.push(rng.random_range(0..search_limit as u32));
    }
    (map, list)
}

/// Returns a Vec of n elements from the range [0,up_to) in random order without repetition
fn random_order(up_to: usize, n: usize) -> Vec<u32> {
    let mut rng = rng();
    let mut order = Vec::with_capacity(n);
    let mut generated = vec![false; up_to];
    let mut remaining = n;
    let mut remaining_elems = up_to;
    while remaining > 0 {
        let mut r = rng.random_range(0..remaining_elems);
        // find the r-th yet nongenerated number:
        for (i, value) in generated.iter_mut().enumerate().take(up_to) {
            if *value {
                continue;
            }
            if r == 0 {
                order.push(i as u32);
                *value = true;
                break;
            }
            r -= 1;
        }
        remaining -= 1;
        remaining_elems -= 1;
    }
    order
}
