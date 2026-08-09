fn recompute_canonical_len<T>(value: &T) -> Result<usize, Error>
where
    T: crate::NoritoSerialize,
{
    struct LenWriter {
        len: usize,
    }

    impl std::io::Write for LenWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.len = self
                .len
                .checked_add(buf.len())
                .ok_or_else(|| std::io::Error::other("norito length overflow"))?;
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }

        fn write_vectored(&mut self, bufs: &[std::io::IoSlice<'_>]) -> std::io::Result<usize> {
            let mut total = 0usize;
            for b in bufs {
                total = total
                    .checked_add(b.len())
                    .ok_or_else(|| std::io::Error::other("norito length overflow"))?;
            }
            self.len = self
                .len
                .checked_add(total)
                .ok_or_else(|| std::io::Error::other("norito length overflow"))?;
            Ok(total)
        }
    }

    let mut sink = LenWriter { len: 0 };
    let mut encoder = Encoder::new(&mut sink);
    value.serialize(&mut encoder)?;
    Ok(sink.len)
}
