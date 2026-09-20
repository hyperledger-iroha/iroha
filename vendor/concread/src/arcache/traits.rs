//! Traits for allowing customised behaviours for ARCache

/// A trait that allows custom weighting of items in the arc.
pub trait ArcWeight {
    /// Return the weight of this item. This value MAY be dynamic
    /// as the cache copies this for it's internal tracking purposes
    fn arc_weight(&self) -> usize;
}

impl<T> ArcWeight for T {
    #[inline]
    default fn arc_weight(&self) -> usize {
        1
    }
}
