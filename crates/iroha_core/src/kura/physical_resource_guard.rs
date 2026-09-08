// Included at Kura module scope; replaces the superseded private index-only guard.

/// In-flight filesystem mutation registered before its first physical write.
#[must_use]
pub(crate) struct TotalDiskUsageMutation<'a> {
    kura: &'a Kura,
    published: bool,
    physical_resources: Option<PhysicalResourceMutation<'a>>,
    physical_scope_classified: bool,
    physical_children_remaining: Option<usize>,
}

impl TotalDiskUsageMutation<'_> {
    /// Complete the existing cache publication and this owner's exact resources.
    pub(crate) fn finish(mut self) {
        self.published = true;
        self.publish_physical_resources();
        // Unclassified or incomplete resource tokens deliberately drop unfinished.
        // The original disk-cache completion semantics remain independent.
    }
}

impl Drop for TotalDiskUsageMutation<'_> {
    fn drop(&mut self) {
        self.kura.finish_total_disk_usage_mutation(self.published);
    }
}
