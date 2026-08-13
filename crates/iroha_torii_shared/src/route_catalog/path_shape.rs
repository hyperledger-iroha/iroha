//! Canonical route-shape normalization used by catalog validation.
pub(super) fn normalized_route_shape(path: &str) -> String {
    let mut shape = String::with_capacity(path.len());
    for (index, segment) in path.split('/').enumerate() {
        if index > 0 {
            shape.push('/');
        }
        if segment.starts_with("{*") || segment.ends_with("..}") {
            shape.push_str("{*}");
        } else if segment.starts_with('{') && segment.ends_with('}') {
            shape.push_str("{}");
        } else {
            shape.push_str(segment);
        }
    }
    shape
}
