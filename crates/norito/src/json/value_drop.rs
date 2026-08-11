//! Fixed-stack cleanup state for recursively owned JSON values.

use super::{
    Error, MAX_JSON_VALUE_NESTING_DEPTH, Value, native::Map, try_decode_vec_with_capacity,
};

enum ValueDropFrame {
    Array(std::vec::IntoIter<Value>),
    Object(std::collections::btree_map::IntoValues<String, Value>),
}

impl ValueDropFrame {
    fn next(&mut self) -> Option<Value> {
        match self {
            Self::Array(values) => values.next(),
            Self::Object(values) => values.next(),
        }
    }
}

struct ValueDropStack {
    frames: [Option<ValueDropFrame>; MAX_JSON_VALUE_NESTING_DEPTH],
    len: usize,
}

impl ValueDropStack {
    fn new() -> Self {
        Self {
            frames: std::array::from_fn(|_| None),
            len: 0,
        }
    }

    fn push(&mut self, frame: ValueDropFrame) -> Result<(), ValueDropFrame> {
        let Some(slot) = self.frames.get_mut(self.len) else {
            return Err(frame);
        };
        *slot = Some(frame);
        self.len += 1;
        Ok(())
    }

    fn next(&mut self) -> Option<Value> {
        loop {
            let frame = self.frames.get_mut(self.len.checked_sub(1)?)?;
            if let Some(value) = frame
                .as_mut()
                .expect("bounded JSON drop stack contains every live frame")
                .next()
            {
                return Some(value);
            }
            *frame = None;
            self.len -= 1;
        }
    }
}

fn drop_overdeep_json_frame(frame: ValueDropFrame) {
    match frame {
        ValueDropFrame::Array(values) => {
            for value in values {
                drop_json_value_iteratively(value);
            }
        }
        ValueDropFrame::Object(values) => {
            for value in values {
                drop_json_value_iteratively(value);
            }
        }
    }
}

/// Drop a recursive JSON value without an attacker-sized cleanup `Vec`.
///
/// Parsed values are limited to [`MAX_JSON_VALUE_NESTING_DEPTH`], so the fixed
/// stack covers every parser-produced value without heap allocation. The
/// over-depth branch only supports manually constructed public [`Value`] trees
/// and recursively partitions their remaining siblings.
#[doc(hidden)]
pub fn drop_json_value_iteratively(value: Value) {
    let mut stack = ValueDropStack::new();
    let mut next = Some(value);
    loop {
        if let Some(value) = next.take() {
            let (first, remaining) = match value {
                Value::Array(values) => {
                    let mut values = values.into_iter();
                    let first = values.next();
                    (
                        first,
                        (values.len() != 0).then_some(ValueDropFrame::Array(values)),
                    )
                }
                Value::Object(values) => {
                    let mut values = values.into_values();
                    let first = values.next();
                    (
                        first,
                        (values.len() != 0).then_some(ValueDropFrame::Object(values)),
                    )
                }
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => (None, None),
            };
            if let Some(frame) = remaining
                && let Err(overdeep) = stack.push(frame)
            {
                drop_overdeep_json_frame(overdeep);
            }
            next = first;
            continue;
        }
        let Some(value) = stack.next() else {
            break;
        };
        next = Some(value);
    }
}

pub(super) struct IterativeValueDropGuard(Option<Value>);

impl IterativeValueDropGuard {
    pub(super) fn new(value: Value) -> Self {
        Self(Some(value))
    }

    pub(super) fn take(&mut self) -> Value {
        self.0.take().expect("iterative JSON value guard is empty")
    }
}

impl Drop for IterativeValueDropGuard {
    fn drop(&mut self) {
        if let Some(value) = self.0.take() {
            drop_json_value_iteratively(value);
        }
    }
}

pub(super) enum ValueParseFrame {
    Array {
        values: Vec<Value>,
        child_depth: usize,
    },
    Object {
        values: Map,
        key: Option<String>,
        child_depth: usize,
    },
}

impl ValueParseFrame {
    fn drop_values(self) {
        match self {
            Self::Array { values, .. } => {
                drop_json_value_iteratively(Value::Array(values));
            }
            Self::Object { values, .. } => {
                drop_json_value_iteratively(Value::Object(values));
            }
        }
    }

    pub(super) fn finish(self) -> Value {
        match self {
            Self::Array { values, .. } => Value::Array(values),
            Self::Object { values, .. } => Value::Object(values),
        }
    }
}

pub(super) struct ValueParseState {
    pub(super) frames: Vec<ValueParseFrame>,
    pub(super) completed: Option<Value>,
}

impl ValueParseState {
    pub(super) fn with_frame_capacity(frames: usize) -> Result<Self, Error> {
        Ok(Self {
            frames: try_decode_vec_with_capacity(frames)?,
            completed: None,
        })
    }

    pub(super) fn take_completed(&mut self) -> Value {
        self.completed
            .take()
            .expect("iterative JSON parser has no completed value")
    }
}

impl Drop for ValueParseState {
    fn drop(&mut self) {
        if let Some(value) = self.completed.take() {
            drop_json_value_iteratively(value);
        }
        for frame in self.frames.drain(..) {
            frame.drop_values();
        }
    }
}
