//! Shared-piece storage form of canonical native context records.
//!
//! Consecutive records repeat large unchanged regions: the lane records, and the open SCCP anchor
//! chunk, which every SCCP delta rewrites in full while it grows by one anchor per carrier (up to
//! 4,096 anchors, about 270 KiB). A packed record stores the canonical bytes as an exact
//! partition into inline runs and content-addressed pieces shared between records. Reads rebuild
//! the original bytes and check their length and hash before returning them, so the partition
//! decides only how much space is saved, never which bytes a reader receives. Canonical records
//! written by earlier releases stay readable, and publication accepts either form when a record
//! already exists.

use super::{RecordName, open_named_record, verify_bytes};
use crate::query::native_context_archive::NativeContextArchiveError;
use iroha_allocation::ChargedBuffer;
use iroha_crypto::Hash;
use std::{
    fs::File,
    io::{self, Read, Seek, Write},
};

/// Leading bytes of a packed record; canonical Norito records start with `NRT0` instead.
pub(super) const MAGIC: [u8; 4] = *b"NCP1";
/// Magic, canonical length, canonical hash and piece count.
const HEADER_LEN: usize = 4 + 8 + 32 + 4;
const INLINE: u8 = 0;
const SHARED: u8 = 1;
const INLINE_HEADER_LEN: usize = 1 + 4;
const SHARED_HEADER_LEN: usize = 1 + 4 + 32;
/// One filesystem block. A complete anchor piece never changes while its chunk keeps growing.
const ANCHOR_PIECE_LEN: usize = 4096;
/// Lane records smaller than one filesystem block stay inline.
const LANES_SHARED_MIN: usize = 4096;
/// The SCCP delta names each component by a little-endian `u64` length and the World field name.
const ANCHOR_COMPONENT: &[u8] = b"\x10\0\0\0\0\0\0\0sccp_anchor_open";
/// Bytes copied per read poll, matching canonical record acquisition.
const STEP: usize = 4096;

/// Canonical ranges shared with neighbouring records. Any ranges reproduce the same bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Layout {
    lanes: Option<(usize, usize)>,
    anchors: Option<(usize, usize)>,
}

impl Layout {
    /// Locate the lane records and the complete open-anchor pieces of a canonical projection.
    /// Anything unexpected leaves that region inline.
    pub(super) fn of(canonical: &[u8], height: u64, carrier: &[u8; 32]) -> Self {
        let Some((fields, flags)) = projection_fields(canonical, height, carrier) else {
            return Self::default();
        };
        // The lane state's first two fields hold the lane records and their signer custody, which
        // change only with the lanes; the autoscale load samples after them change every carrier
        // and stay inline.
        let lanes = leading_fields(canonical, fields[2], flags, 2)
            .filter(|(start, end)| end - start >= LANES_SHARED_MIN);
        let anchors = anchor_pieces(canonical, fields[5])
            .filter(|&(start, _)| lanes.is_none_or(|(_, end)| end <= start));
        Self { lanes, anchors }
    }

    /// Whether the record has nothing to share and is stored in canonical form.
    pub(super) const fn is_inline(&self) -> bool {
        self.lanes.is_none() && self.anchors.is_none()
    }

    fn pieces(self, len: usize) -> Pieces {
        Pieces {
            layout: self,
            len,
            cursor: 0,
        }
    }
}

/// The six length-prefixed fields of a canonical projection frame, checked against its carrier,
/// and the frame's layout flags.
fn projection_fields(
    canonical: &[u8],
    height: u64,
    carrier: &[u8; 32],
) -> Option<([(usize, usize); 6], u8)> {
    let header = norito::core::Header::read(canonical).ok()?;
    if header.compression != norito::core::Compression::None {
        return None;
    }
    let mut position = canonical
        .len()
        .checked_sub(usize::try_from(header.length).ok()?)
        .filter(|&start| start >= norito::core::Header::SIZE)?;
    let mut fields = [(0, 0); 6];
    for field in &mut fields {
        *field = next_field(canonical, position, canonical.len(), header.flags)?;
        position = field.1;
    }
    (position == canonical.len()
        && canonical[fields[0].0..fields[0].1] == height.to_le_bytes()
        && canonical[fields[1].0..fields[1].1] == carrier[..])
        .then_some((fields, header.flags))
}

/// The length-prefixed field at `position`, ending no later than `end`.
fn next_field(canonical: &[u8], position: usize, end: usize, flags: u8) -> Option<(usize, usize)> {
    let (length, used) =
        norito::core::inspect_raw_len_from_slice_with_flags(&canonical[position..end], flags)
            .ok()?;
    let start = position + used;
    Some((
        start,
        start.checked_add(length).filter(|&last| last <= end)?,
    ))
}

/// The span from the first field's payload to the end of field `count` (or the last field), of
/// the length-prefixed fields that exactly fill `canonical[start..end]`.
fn leading_fields(
    canonical: &[u8],
    (start, end): (usize, usize),
    flags: u8,
    count: usize,
) -> Option<(usize, usize)> {
    let first = next_field(canonical, start, end, flags)?;
    let mut span = first;
    let mut seen = 1;
    while span.1 < end && seen < count {
        span.1 = next_field(canonical, span.1, end, flags)?.1;
        seen += 1;
    }
    let mut position = span.1;
    while position < end {
        position = next_field(canonical, position, end, flags)?.1;
    }
    Some(span)
}

/// Complete fixed-size pieces of the open anchor chunk inside the SCCP delta field.
/// The chunk only grows at its end, so these pieces repeat unchanged in later records.
fn anchor_pieces(canonical: &[u8], (start, end): (usize, usize)) -> Option<(usize, usize)> {
    let name = canonical[start..end]
        .windows(ANCHOR_COMPONENT.len())
        .position(|window| window == ANCHOR_COMPONENT)?;
    let length_at = start + name + ANCHOR_COMPONENT.len();
    let length = canonical.get(length_at..length_at.checked_add(8)?)?;
    let length = usize::try_from(u64::from_le_bytes(length.try_into().ok()?)).ok()?;
    let value_end = (length_at + 8)
        .checked_add(length)
        .filter(|&value_end| value_end <= end)?;
    // The value starts with its anchor count; the anchors after it never move.
    let anchors = length_at + 8 + 8;
    let pieces = value_end.checked_sub(anchors)? / ANCHOR_PIECE_LEN;
    (pieces > 0).then(|| (anchors, anchors + pieces * ANCHOR_PIECE_LEN))
}

/// One canonical range, stored inline or as a shared piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Piece {
    start: usize,
    end: usize,
    shared: bool,
}

/// Pieces of a layout in canonical order; their ranges partition the record.
struct Pieces {
    layout: Layout,
    len: usize,
    cursor: usize,
}

impl Iterator for Pieces {
    type Item = Piece;

    fn next(&mut self) -> Option<Piece> {
        let start = self.cursor;
        if start >= self.len {
            return None;
        }
        let piece = if let Some((lanes, end)) = self.layout.lanes.filter(|&(_, end)| start < end) {
            Piece {
                start,
                end: if start < lanes { lanes } else { end },
                shared: start >= lanes,
            }
        } else if let Some((anchors, end)) = self.layout.anchors.filter(|&(_, end)| start < end) {
            Piece {
                start,
                end: if start < anchors {
                    anchors
                } else {
                    (start + ANCHOR_PIECE_LEN).min(end)
                },
                shared: start >= anchors,
            }
        } else {
            Piece {
                start,
                end: self.len,
                shared: false,
            }
        };
        self.cursor = piece.end;
        Some(piece)
    }
}

/// Shared pieces a packed record references, in record order.
pub(super) fn shared_pieces(canonical: &[u8], layout: Layout) -> impl Iterator<Item = &[u8]> {
    layout
        .pieces(canonical.len())
        .filter(|piece| piece.shared)
        .map(|piece| &canonical[piece.start..piece.end])
}

/// Content-addressed name of a shared piece.
pub(super) fn piece_name(bytes: &[u8], staged: bool) -> RecordName {
    RecordName::shared(&Hash::new(bytes).into(), staged)
}

/// Stored bytes of one packed record, written and compared without assembling a copy.
pub(super) struct Stored<'a> {
    canonical: &'a [u8],
    layout: Layout,
}

impl<'a> Stored<'a> {
    pub(super) const fn new(canonical: &'a [u8], layout: Layout) -> Self {
        Self { canonical, layout }
    }

    /// Length of the stored record, excluding its shared pieces.
    pub(super) fn len(&self) -> usize {
        self.layout
            .pieces(self.canonical.len())
            .map(|piece| {
                if piece.shared {
                    SHARED_HEADER_LEN
                } else {
                    INLINE_HEADER_LEN + piece.end - piece.start
                }
            })
            .sum::<usize>()
            + HEADER_LEN
    }

    /// Visit the stored bytes in order.
    fn chunks(&self, mut visit: impl FnMut(&[u8]) -> io::Result<()>) -> io::Result<()> {
        let pieces = self.layout.pieces(self.canonical.len()).count();
        let mut header = [0; HEADER_LEN];
        header[..4].copy_from_slice(&MAGIC);
        header[4..12].copy_from_slice(&(self.canonical.len() as u64).to_le_bytes());
        header[12..44].copy_from_slice(Hash::new(self.canonical).as_ref());
        header[44..].copy_from_slice(
            &u32::try_from(pieces)
                .map_err(io::Error::other)?
                .to_le_bytes(),
        );
        visit(&header)?;
        for piece in self.layout.pieces(self.canonical.len()) {
            let bytes = &self.canonical[piece.start..piece.end];
            let length = u32::try_from(bytes.len()).map_err(io::Error::other)?;
            let mut prefix = [0; SHARED_HEADER_LEN];
            prefix[1..5].copy_from_slice(&length.to_le_bytes());
            if piece.shared {
                prefix[0] = SHARED;
                prefix[5..].copy_from_slice(Hash::new(bytes).as_ref());
                visit(&prefix)?;
            } else {
                prefix[0] = INLINE;
                visit(&prefix[..INLINE_HEADER_LEN])?;
                visit(bytes)?;
            }
        }
        Ok(())
    }
}

/// Bytes a record file holds, written and compared without assembling a copy.
pub(super) trait StoredContent {
    /// Write the complete stored bytes to a freshly truncated file.
    fn write_to(&self, file: &mut File) -> io::Result<()>;
    /// Fail unless `file`, positioned at its start, holds exactly these stored bytes.
    fn verify(&self, file: File) -> io::Result<()>;
}

impl StoredContent for [u8] {
    fn write_to(&self, file: &mut File) -> io::Result<()> {
        file.write_all(self)
    }
    fn verify(&self, file: File) -> io::Result<()> {
        verify_bytes(file, self)
    }
}

impl StoredContent for Stored<'_> {
    fn write_to(&self, file: &mut File) -> io::Result<()> {
        self.chunks(|bytes| file.write_all(bytes))
    }
    fn verify(&self, mut file: File) -> io::Result<()> {
        if !file.metadata()?.is_file() {
            return Err(differs());
        }
        let mut scratch = [0; STEP];
        self.chunks(|mut expected| {
            while !expected.is_empty() {
                let count = expected.len().min(scratch.len());
                file.read_exact(&mut scratch[..count])
                    .map_err(|_| differs())?;
                if scratch[..count] != expected[..count] {
                    return Err(differs());
                }
                expected = &expected[count..];
            }
            Ok(())
        })?;
        if file.read(&mut scratch[..1])? != 0 {
            return Err(io::Error::other("native context canonical record grew"));
        }
        Ok(())
    }
}

fn differs() -> io::Error {
    io::Error::other("native context canonical record differs")
}

/// Fail unless `file` holds `original` itself or, for a context record, a packed record that
/// rebuilds it exactly. Either form may already exist after an upgrade or an interrupted write.
pub(super) fn verify_original(
    directory: &File,
    mut file: File,
    original: &[u8],
    context: bool,
) -> io::Result<()> {
    let mut magic = [0; 4];
    let packed = context
        && file.metadata()?.len() >= HEADER_LEN as u64
        && file.read_exact(&mut magic).is_ok()
        && magic == MAGIC;
    file.rewind()?;
    if !packed {
        return verify_bytes(file, original);
    }
    let stored = file.metadata()?.len();
    let header = read_header(&mut file).map_err(|_| differs())?;
    if header.length != original.len() || header.hash != <[u8; 32]>::from(Hash::new(original)) {
        return Err(differs());
    }
    let mut offset = HEADER_LEN as u64;
    let mut position = 0;
    let mut scratch = [0; STEP];
    for _ in 0..header.pieces {
        let (piece, used) = read_piece(&mut file, offset, stored).map_err(|_| differs())?;
        offset = piece.end(offset + used);
        let end = position + piece.length;
        let expected = original.get(position..end).ok_or_else(differs)?;
        let mut source = piece.open(directory, &file)?;
        for expected in expected.chunks(STEP) {
            source
                .read_exact(&mut scratch[..expected.len()])
                .map_err(|_| differs())?;
            if scratch[..expected.len()] != *expected {
                return Err(differs());
            }
        }
        position = end;
    }
    if position != original.len() || offset != stored {
        return Err(differs());
    }
    Ok(())
}

/// Rebuild a packed record's canonical bytes for an offline rewrite, checked against its hash.
pub(super) fn rebuild(directory: &File, record: &mut File) -> io::Result<Vec<u8>> {
    let stored = record.metadata()?.len();
    let header = read_header(record)?;
    // Grow only by pieces whose stored length was checked; the header's length is unverified.
    let mut out = Vec::new();
    let mut offset = HEADER_LEN as u64;
    for _ in 0..header.pieces {
        let (piece, used) = read_piece(record, offset, stored)?;
        offset = piece.end(offset + used);
        let start = out.len();
        if start + piece.length > header.length {
            return Err(differs());
        }
        let mut source = piece.open(directory, record)?;
        out.resize(start + piece.length, 0);
        source.read_exact(&mut out[start..])?;
    }
    if offset != stored
        || out.len() != header.length
        || <[u8; 32]>::from(Hash::new(&out)) != header.hash
    {
        return Err(differs());
    }
    Ok(out)
}

struct Header {
    length: usize,
    hash: [u8; 32],
    pieces: u32,
}

fn read_header(file: &mut File) -> io::Result<Header> {
    let mut header = [0; HEADER_LEN];
    file.seek(io::SeekFrom::Start(0))?;
    file.read_exact(&mut header)?;
    if header[..4] != MAGIC {
        return Err(io::Error::other("native context record is not packed"));
    }
    let length = u64::from_le_bytes(header[4..12].try_into().expect("fixed header field"));
    Ok(Header {
        length: usize::try_from(length).map_err(io::Error::other)?,
        hash: header[12..44].try_into().expect("fixed header field"),
        pieces: u32::from_le_bytes(header[44..].try_into().expect("fixed header field")),
    })
}

/// One piece: an inline run inside the packed record, or the shared piece named by `hash`.
#[derive(Clone, Copy)]
struct PieceHeader {
    length: usize,
    /// Offset of the inline run in the packed record; `None` for a shared piece.
    inline: Option<u64>,
    hash: [u8; 32],
}

impl PieceHeader {
    /// Stored offset just past this piece, given the offset just past its header.
    fn end(&self, after_header: u64) -> u64 {
        self.inline
            .map_or(after_header, |start| start + self.length as u64)
    }

    /// Open the piece's bytes for one blocking pass: the record descriptor at its inline run,
    /// or the shared piece once its length matches.
    fn open(&self, directory: &File, record: &File) -> io::Result<File> {
        if let Some(start) = self.inline {
            let mut source = record.try_clone()?;
            source.seek(io::SeekFrom::Start(start))?;
            return Ok(source);
        }
        let shared = open_named_record(directory, &RecordName::shared(&self.hash, false))?;
        if shared.metadata()?.len() != self.length as u64 {
            return Err(differs());
        }
        Ok(shared)
    }
}

/// Read the piece header at `offset`; returns the header and its stored length.
fn read_piece(file: &mut File, offset: u64, stored: u64) -> io::Result<(PieceHeader, u64)> {
    let mut prefix = [0; SHARED_HEADER_LEN];
    file.seek(io::SeekFrom::Start(offset))?;
    file.read_exact(&mut prefix[..INLINE_HEADER_LEN])?;
    let length = u32::from_le_bytes(prefix[1..5].try_into().expect("fixed piece field"));
    let length = usize::try_from(length).map_err(io::Error::other)?;
    if length == 0 {
        return Err(io::Error::other("empty native context piece"));
    }
    let mut piece = PieceHeader {
        length,
        inline: None,
        hash: [0; 32],
    };
    match prefix[0] {
        INLINE => {
            let start = offset + INLINE_HEADER_LEN as u64;
            if start + length as u64 > stored {
                return Err(io::Error::other("native context piece exceeds its record"));
            }
            piece.inline = Some(start);
            Ok((piece, INLINE_HEADER_LEN as u64))
        }
        SHARED => {
            file.read_exact(&mut prefix[INLINE_HEADER_LEN..])?;
            piece.hash = prefix[INLINE_HEADER_LEN..]
                .try_into()
                .expect("fixed piece field");
            Ok((piece, SHARED_HEADER_LEN as u64))
        }
        _ => Err(io::Error::other("unknown native context piece")),
    }
}

/// Rebuild progress of one packed record. Retained descriptors and bytes survive every refusal.
pub(super) struct Expansion {
    length: usize,
    hash: [u8; 32],
    remaining: u32,
    offset: u64,
    piece: Option<PieceRead>,
}

struct PieceRead {
    header: PieceHeader,
    copied: usize,
    /// The shared piece descriptor, pinned once opened.
    file: Option<File>,
}

impl Expansion {
    /// Inspect a selected record. `None` means it is stored in canonical form.
    pub(super) fn start(
        record: &mut File,
        stored: usize,
        maximum: usize,
    ) -> Result<Option<Self>, NativeContextArchiveError> {
        if stored < HEADER_LEN {
            return Ok(None);
        }
        let mut magic = [0; 4];
        record.seek(io::SeekFrom::Start(0))?;
        record.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Ok(None);
        }
        let header = read_header(record)?;
        if header.length == 0 || header.length > maximum {
            return Err(NativeContextArchiveError::Limit {
                maximum,
                actual: header.length,
            });
        }
        Ok(Some(Self {
            length: header.length,
            hash: header.hash,
            remaining: header.pieces,
            offset: HEADER_LEN as u64,
            piece: None,
        }))
    }

    /// Original canonical length to allocate before rebuilding.
    pub(super) const fn length(&self) -> usize {
        self.length
    }

    /// Copy at most one 4096-byte chunk into `out`. `true` means the rebuilt record is complete
    /// and matches its original length and hash.
    pub(super) fn step(
        &mut self,
        directory: &File,
        record: &mut File,
        stored: u64,
        out: &mut ChargedBuffer<u8>,
    ) -> Result<bool, NativeContextArchiveError> {
        if self.piece.is_none() {
            if self.remaining == 0 {
                if self.offset != stored || out.as_slice().len() != self.length {
                    return Err(NativeContextArchiveError::Source(
                        "packed native context record has an inconsistent layout",
                    ));
                }
                if <[u8; 32]>::from(Hash::new(out.as_slice())) != self.hash {
                    return Err(NativeContextArchiveError::Source(
                        "packed native context record differs from its original",
                    ));
                }
                return Ok(true);
            }
            let (header, used) = read_piece(record, self.offset, stored)?;
            if out.as_slice().len() + header.length > self.length {
                return Err(NativeContextArchiveError::Source(
                    "packed native context pieces exceed their original",
                ));
            }
            self.offset = header.end(self.offset + used);
            self.remaining -= 1;
            self.piece = Some(PieceRead {
                header,
                copied: 0,
                file: None,
            });
        }
        let piece = self.piece.as_mut().expect("selected native context piece");
        let length = piece.header.length;
        let mut scratch = [0; STEP];
        let count = (length - piece.copied).min(STEP);
        if let Some(start) = piece.header.inline {
            record.seek(io::SeekFrom::Start(start + piece.copied as u64))?;
            record.read_exact(&mut scratch[..count])?;
        } else {
            if piece.file.is_none() {
                let name = RecordName::shared(&piece.header.hash, false);
                let shared = open_named_record(directory, &name)?;
                if shared.metadata()?.len() != length as u64 {
                    return Err(NativeContextArchiveError::Source(
                        "shared native context piece length differs",
                    ));
                }
                piece.file = Some(shared);
            }
            let shared = piece.file.as_mut().expect("pinned shared piece");
            shared.seek(io::SeekFrom::Start(piece.copied as u64))?;
            shared.read_exact(&mut scratch[..count])?;
        }
        out.append(&scratch[..count])?;
        piece.copied += count;
        if piece.copied == length {
            if let Some(shared) = piece.file.as_mut()
                && (shared.read(&mut scratch[..1])? != 0
                    || shared.metadata()?.len() != length as u64)
            {
                return Err(NativeContextArchiveError::Source(
                    "shared native context piece changed during exact read",
                ));
            }
            self.piece = None;
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests;
