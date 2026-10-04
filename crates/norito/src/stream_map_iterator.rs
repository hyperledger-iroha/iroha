impl<K, V> Iterator for StreamMapIter<K, V>
where
    K: for<'de> NoritoDeserialize<'de>,
    V: for<'de> NoritoDeserialize<'de>,
{
    type Item = Result<(K, V), Error>;
    fn next(&mut self) -> Option<Self::Item> {
        let decode_budget = self.decode_budget.clone();
        core::with_optional_decode_budget(decode_budget.as_ref(), || {
            let _ = &self.flags_guard;
            if self.idx >= self.entries {
                return None;
            }
            let klen = match self.read_len() {
                Ok(len) => len,
                Err(e) => return Some(Err(e)),
            };
            if klen > self.payload_remaining {
                return Some(Err(Error::LengthMismatch));
            }
            if let Err(error) = try_resize_decode_buffer(&mut self.kbuf, klen) {
                return Some(Err(error));
            }
            if let Err(e) = self.read_exact_update_kbuf() {
                return Some(Err(e));
            }
            let _gk = core::PayloadCtxGuard::enter(&self.kbuf);
            let _key_depth = match core::DecodeDepthGuard::enter() {
                Ok(guard) => guard,
                Err(error) => return Some(Err(error)),
            };
            let ak = unsafe { &*(self.kbuf.as_ptr() as *const Archived<K>) };
            let key = match guarded_try_deserialize(|| K::try_deserialize(ak)) {
                Ok(k) => k,
                Err(e) => return Some(Err(e)),
            };
            drop(_key_depth);
            drop(_gk);
            let vlen = match self.read_len() {
                Ok(len) => len,
                Err(e) => return Some(Err(e)),
            };
            if vlen > self.payload_remaining {
                return Some(Err(Error::LengthMismatch));
            }
            if let Err(error) = try_resize_decode_buffer(&mut self.vbuf, vlen) {
                return Some(Err(error));
            }
            if let Err(e) = self.read_exact_update_vbuf() {
                return Some(Err(e));
            }
            let _gv = core::PayloadCtxGuard::enter(&self.vbuf);
            let _value_depth = match core::DecodeDepthGuard::enter() {
                Ok(guard) => guard,
                Err(error) => return Some(Err(error)),
            };
            let av = unsafe { &*(self.vbuf.as_ptr() as *const Archived<V>) };
            let val = match guarded_try_deserialize(|| V::try_deserialize(av)) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            self.idx += 1;
            if self.idx == self.entries {
                if self.payload_remaining != 0 {
                    return Some(Err(Error::LengthMismatch));
                }
                if self.digest.sum64() != self.checksum {
                    return Some(Err(Error::ChecksumMismatch));
                }
            }
            Some(Ok((key, val)))
        })
    }
}
