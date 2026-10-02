//! Separate hardware-evidence frame grammar. No financial coordinator method is admitted.
pub(super) const VERSION: i32 = 1;
pub(super) const PURPOSE: i32 = 1;
pub(crate) const MAX_FIELD: usize = 192 * 1024;
pub(crate) const MAX_FIELDS: usize = 7;
pub(crate) const CONTRACT: [i32; 5] = [VERSION, PURPOSE, 18, MAX_FIELDS as i32, MAX_FIELD as i32];
/// Validate all shape and byte bounds before acquiring or mutating an owner.
pub(crate) fn valid_request(method: i32, fields: &[Vec<u8>]) -> bool {
    let count = match method {
        1 | 5 | 6 | 8 | 10 | 12 | 14 | 16 | 17 | 18 => 0,
        2 | 3 | 4 | 9 | 11 | 13 | 15 => 1,
        7 => 2,
        _ => return false,
    };
    if fields.len() != count || fields.iter().any(|b| b.len() > MAX_FIELD) {
        return false;
    }
    match method {
        2 => fields[0].len() == 1 && (1..=6).contains(&fields[0][0]),
        3 => {
            !fields[0].is_empty()
                && fields[0].len() <= 64 * 1024
                && fields[0].iter().all(u8::is_ascii_graphic)
        }
        4 | 9 | 15 => !fields[0].is_empty(),
        7 => fields[0].len() == 65 && fields[0][0] == 4 && !fields[1].is_empty(),
        11 => (8..=80).contains(&fields[0].len()),
        13 => {
            !fields[0].is_empty()
                && fields[0].len() <= 64 * 1024
                && fields[0].iter().all(u8::is_ascii_graphic)
        }
        _ => true,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_methods_and_shapes() {
        assert!(valid_request(1, &[]));
        assert!(!valid_request(19, &[]));
        assert!(!valid_request(0, &[]));
        assert!(!valid_request(1, &[vec![1]]));
        assert!(!valid_request(2, &[vec![0]]));
        assert!(!valid_request(2, &[vec![7]]));
        assert!(valid_request(2, &[vec![6]]));
    }
    #[test]
    fn token_bounds_before_intake() {
        assert!(valid_request(3, &[vec![b'a'; 64 * 1024]]));
        assert!(!valid_request(3, &[vec![b'a'; 64 * 1024 + 1]]));
        assert!(!valid_request(3, &[b"token\n".to_vec()]));
        assert!(!valid_request(13, &[vec![b'a'; 64 * 1024 + 1]]));
        assert!(!valid_request(13, &[vec![0xff]]));
    }
    #[test]
    fn original_shapes_and_caps() {
        let mut point = vec![1; 65];
        point[0] = 4;
        assert!(valid_request(7, &[point.clone(), vec![1; MAX_FIELD]]));
        assert!(!valid_request(7, &[point, vec![1; MAX_FIELD + 1]]));
        assert!(!valid_request(11, &[vec![1; 81]]));
        assert!(!valid_request(15, &[vec![]]));
    }
}
