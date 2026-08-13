// Zeroizing scratch-vector helpers included at parent-module scope.

#[cfg(test)]
std::thread_local! {
    static CKS_STREAM_ZEROIZING_DROP_AUDIT_V1: core::cell::Cell<(usize, usize)> =
        const { core::cell::Cell::new((0, 0)) };
}

#[cfg(test)]
fn record_cks_stream_zeroizing_drop_v1(all_zero: bool) {
    CKS_STREAM_ZEROIZING_DROP_AUDIT_V1.with(|audit| {
        let (drops, failed) = audit.get();
        audit.set((drops + 1, failed + usize::from(!all_zero)));
    });
}

struct ZeroizingU64VectorV1(Vec<u64>);

impl ZeroizingU64VectorV1 {
    fn with_capacity_exact(capacity: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(Self(values))
    }

    fn zeroed(length: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut values = Self::with_capacity_exact(length)?;
        values.0.resize(length, 0);
        Ok(values)
    }
}

impl core::ops::Deref for ZeroizingU64VectorV1 {
    type Target = Vec<u64>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for ZeroizingU64VectorV1 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for ZeroizingU64VectorV1 {
    fn drop(&mut self) {
        let values = core::hint::black_box(self.0.as_mut_slice());
        values.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *values);
        #[cfg(test)]
        record_cks_stream_zeroizing_drop_v1(self.0.iter().all(|value| *value == 0));
    }
}

struct ZeroizingByteVectorV1(Vec<u8>);

impl ZeroizingByteVectorV1 {
    fn read_exact<R: std::io::Read>(
        reader: &mut R,
        length: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        bytes.resize(length, 0);
        read_canonical_raw_exact(reader, &mut bytes)?;
        Ok(Self(bytes))
    }
}

impl core::ops::Deref for ZeroizingByteVectorV1 {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for ZeroizingByteVectorV1 {
    fn drop(&mut self) {
        let values = core::hint::black_box(self.0.as_mut_slice());
        values.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *values);
        #[cfg(test)]
        record_cks_stream_zeroizing_drop_v1(self.0.iter().all(|value| *value == 0));
    }
}
