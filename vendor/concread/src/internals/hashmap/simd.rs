#[cfg(feature = "simd_support")]
use core_simd::u64x8;
use std::borrow::Borrow;
use std::fmt::Debug;
use std::hash::Hash;

use super::node::{Branch, Leaf};

pub(crate) enum KeyLoc {
    Ok(usize, usize),
    Collision(usize),
    Missing(usize),
}

impl KeyLoc {
    pub(crate) fn ok(self) -> Option<(usize, usize)> {
        if let KeyLoc::Ok(a, b) = self {
            Some((a, b))
        } else {
            None
        }
    }
}

#[cfg(not(feature = "simd_support"))]
pub(crate) fn branch_simd_search<K, V>(branch: &Branch<K, V>, h: u64) -> Result<usize, usize>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    debug_assert!(h < u64::MAX);
    for i in 0..branch.slots() {
        if h == unsafe { branch.ctrl.a.1[i] } {
            return Ok(i);
        }
    }

    for i in 0..branch.slots() {
        if h < unsafe { branch.ctrl.a.1[i] } {
            return Err(i);
        }
    }
    Err(branch.slots())
}

#[cfg(feature = "simd_support")]
pub(crate) fn branch_simd_search<K, V>(branch: &Branch<K, V>, h: u64) -> Result<usize, usize>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    debug_assert!(h < u64::MAX);

    debug_assert!({
        let want = u64x8::splat(u64::MAX);
        let r1 = want.lanes_eq(unsafe { *branch.ctrl.simd });
        let mask = r1.to_bitmask()[0] & 0b1111_1110;

        match (mask, branch.slots()) {
            (0b1111_1110, 0)
            | (0b1111_1100, 1)
            | (0b1111_1000, 2)
            | (0b1111_0000, 3)
            | (0b1110_0000, 4)
            | (0b1100_0000, 5)
            | (0b1000_0000, 6)
            | (0b0000_0000, 7) => true,
            _ => {
                eprintln!("branch mask -> {:b}", mask);
                eprintln!("branch slots -> {:?}", branch.slots());
                false
            }
        }
    });

    let want = u64x8::splat(h);
    let r1 = want.lanes_eq(unsafe { *branch.ctrl.simd });

    let mask = r1.to_bitmask()[0] & 0b1111_1110;

    match mask {
        0b0000_0001 => unreachable!(),
        0b0000_0010 => return Ok(0),
        0b0000_0100 => return Ok(1),
        0b0000_1000 => return Ok(2),
        0b0001_0000 => return Ok(3),
        0b0010_0000 => return Ok(4),
        0b0100_0000 => return Ok(5),
        0b1000_0000 => return Ok(6),
        0b0000_0000 => {}
        _ => unreachable!(),
    };

    let r2 = want.lanes_lt(unsafe { *branch.ctrl.simd });
    let mask = r2.to_bitmask()[0] & 0b1111_1110;

    match mask {
        0b1111_1110 => Err(0),
        0b1111_1100 => Err(1),
        0b1111_1000 => Err(2),
        0b1111_0000 => Err(3),
        0b1110_0000 => Err(4),
        0b1100_0000 => Err(5),
        0b1000_0000 => Err(6),
        0b0000_0000 => Err(7),
        // Means something is out of order or invalid!
        _ => unreachable!(),
    }
}

#[cfg(not(feature = "simd_support"))]
pub(crate) fn leaf_simd_get_slot<K, V>(leaf: &Leaf<K, V>, h: u64) -> Option<usize>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    debug_assert!(h < u64::MAX);

    (0..leaf.slots()).find(|&cand_idx| h == unsafe { leaf.ctrl.a.1[cand_idx] })
}

#[cfg(feature = "simd_support")]
pub(crate) fn leaf_simd_get_slot<K, V>(leaf: &Leaf<K, V>, h: u64) -> Option<usize>
where
    K: Hash + Eq + Clone + Debug,
    V: Clone,
{
    // This is an important piece of logic!
    debug_assert!(h < u64::MAX);

    debug_assert!({
        let want = u64x8::splat(u64::MAX);
        let r1 = want.lanes_eq(unsafe { *leaf.ctrl.simd });
        let mask = r1.to_bitmask()[0] & 0b1111_1110;

        match (mask, leaf.slots()) {
            (0b1111_1110, 0)
            | (0b1111_1100, 1)
            | (0b1111_1000, 2)
            | (0b1111_0000, 3)
            | (0b1110_0000, 4)
            | (0b1100_0000, 5)
            | (0b1000_0000, 6)
            | (0b0000_0000, 7) => true,
            _ => false,
        }
    });

    let want = u64x8::splat(h);
    let r1 = want.lanes_eq(unsafe { *leaf.ctrl.simd });

    // println!("want: {:?}", want);
    // println!("ctrl: {:?}", unsafe { *leaf.ctrl.simd });

    // Always discard the meta field
    let mask = r1.to_bitmask()[0] & 0b1111_1110;
    // println!("res eq: 0b{:b}", mask);

    if mask != 0 {
        // Something was equal
        let cand_idx = match mask {
            // 0b0000_0001 => {},
            0b0000_0010 => 0,
            0b0000_0100 => 1,
            0b0000_1000 => 2,
            0b0001_0000 => 3,
            0b0010_0000 => 4,
            0b0100_0000 => 5,
            0b1000_0000 => 6,
            _ => unreachable!(),
        };
        return Some(cand_idx);
    }
    None
}

#[cfg(not(feature = "simd_support"))]
pub(crate) fn leaf_simd_search<K, V, Q>(leaf: &Leaf<K, V>, h: u64, k: &Q) -> KeyLoc
where
    K: Hash + Eq + Clone + Debug + Borrow<Q>,
    V: Clone,
    Q: Eq + ?Sized,
{
    debug_assert!(h < u64::MAX);

    for cand_idx in 0..leaf.slots() {
        if h == unsafe { leaf.ctrl.a.1[cand_idx] } {
            let bucket = unsafe { (*leaf.values[cand_idx].as_ptr()).as_slice() };
            for (i, d) in bucket.iter().enumerate() {
                if k.eq(d.k.borrow()) {
                    return KeyLoc::Ok(cand_idx, i);
                }
            }
            // Wasn't found despite all the collisions, err.
            return KeyLoc::Collision(cand_idx);
        }
    }

    for i in 0..leaf.slots() {
        if h < unsafe { leaf.ctrl.a.1[i] } {
            return KeyLoc::Missing(i);
        }
    }
    KeyLoc::Missing(leaf.slots())
}

#[cfg(feature = "simd_support")]
pub(crate) fn leaf_simd_search<K, V, Q>(leaf: &Leaf<K, V>, h: u64, k: &Q) -> KeyLoc
where
    K: Hash + Eq + Clone + Debug + Borrow<Q>,
    V: Clone,
    Q: Eq + ?Sized,
{
    // This is an important piece of logic!
    debug_assert!(h < u64::MAX);

    debug_assert!({
        let want = u64x8::splat(u64::MAX);
        let r1 = want.lanes_eq(unsafe { *leaf.ctrl.simd });
        let mask = r1.to_bitmask()[0] & 0b1111_1110;

        match (mask, leaf.slots()) {
            (0b1111_1110, 0)
            | (0b1111_1100, 1)
            | (0b1111_1000, 2)
            | (0b1111_0000, 3)
            | (0b1110_0000, 4)
            | (0b1100_0000, 5)
            | (0b1000_0000, 6)
            | (0b0000_0000, 7) => true,
            _ => false,
        }
    });

    let want = u64x8::splat(h);
    let r1 = want.lanes_eq(unsafe { *leaf.ctrl.simd });

    // println!("want: {:?}", want);
    // println!("ctrl: {:?}", unsafe { *leaf.ctrl.simd });

    // Always discard the meta field
    let mask = r1.to_bitmask()[0] & 0b1111_1110;
    // println!("res eq: 0b{:b}", mask);

    if mask != 0 {
        // Something was equal
        let cand_idx = match mask {
            // 0b0000_0001 => {},
            0b0000_0010 => 0,
            0b0000_0100 => 1,
            0b0000_1000 => 2,
            0b0001_0000 => 3,
            0b0010_0000 => 4,
            0b0100_0000 => 5,
            0b1000_0000 => 6,
            _ => unreachable!(),
        };

        // Search in the bucket. Generally this is inlined and one element.
        let bucket = unsafe { (*leaf.values[cand_idx].as_ptr()).as_slice() };
        for (i, d) in bucket.iter().enumerate() {
            if k.eq(d.k.borrow()) {
                return KeyLoc::Ok(cand_idx, i);
            }
        }
        // Wasn't found despite all the collisions, err.
        return KeyLoc::Collision(cand_idx);
    }

    let r2 = want.lanes_lt(unsafe { *leaf.ctrl.simd });
    // Always discard the meta field
    let mask = r2.to_bitmask()[0] & 0b1111_1110;
    // println!("res lt: 0b{:b}", mask);
    let r = match mask {
        0b1111_1110 => 0,
        0b1111_1100 => 1,
        0b1111_1000 => 2,
        0b1111_0000 => 3,
        0b1110_0000 => 4,
        0b1100_0000 => 5,
        0b1000_0000 => 6,
        0b0000_0000 => 7,
        // Means something is out of order or invalid!
        _ => unreachable!(),
    };
    KeyLoc::Missing(r)
}
