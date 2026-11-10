//! Static index for the compiled State registry. Only per-cut row frames consume the State pool.
use super::*;
use std::sync::OnceLock;

const fn field_count(fields: &[Field]) -> usize {
    let mut count = fields.len();
    let mut index = 0;
    while index < fields.len() {
        if let Role::Canonical(Canonical::Owner(children)) = fields[index].role {
            count += field_count(children);
        }
        index += 1;
    }
    count
}

pub(super) const FIELD_COUNT: usize = field_count(STATE_FIELDS);
const _: () = assert!(FIELD_COUNT > 0 && FIELD_COUNT < u16::MAX as usize);

pub(super) fn is_canonical(field: &Field) -> bool {
    matches!(
        field.role,
        Role::Canonical(Canonical::Table { .. } | Canonical::Cell(_))
    )
}

pub(super) struct Registry {
    pub(super) fields: [&'static Field; FIELD_COUNT],
    count: usize,
    duplicate: Option<&'static str>,
    invalid: Option<&'static str>,
}

impl Registry {
    pub(super) fn canonical_count(&self) -> Result<usize, CaptureError> {
        if let Some(id) = self.duplicate {
            return Err(CaptureError::Duplicate(id));
        }
        if let Some(id) = self.invalid {
            return Err(CaptureError::Schema(id));
        }
        Ok(self.count)
    }

    pub(super) fn index(&self, id: &'static str) -> Result<usize, CaptureError> {
        self.fields
            .binary_search_by_key(&id, |field| field.id)
            .map_err(|_| CaptureError::Unexpected(id))
    }
}

pub(super) fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut fields = [&STATE_FIELDS[0]; FIELD_COUNT];
        let mut next = 0;
        let mut count = 0;
        let mut invalid = None;
        visit(STATE_FIELDS, &mut |field| {
            fields[next] = field;
            next += 1;
            let mut check = |schema| {
                let accepted = match schema {
                    Schema::Norito { layout, .. } => layout == V1_LAYOUT,
                    Schema::Semantic {
                        identity,
                        encoder,
                        layout,
                    } => layout == V1_LAYOUT && !identity.is_empty() && !encoder.is_empty(),
                    Schema::Required { .. } => false,
                };
                if !accepted {
                    invalid.get_or_insert(field.id);
                }
            };
            match field.role {
                Role::Canonical(Canonical::Table { key, value }) => {
                    count += 1;
                    check(key);
                    check(value);
                }
                Role::Canonical(Canonical::Cell(schema)) => {
                    count += 1;
                    check(schema);
                }
                _ => {}
            }
        });
        fields.sort_unstable_by_key(|field| field.id);
        let duplicate = fields
            .windows(2)
            .find(|pair| pair[0].id == pair[1].id)
            .map(|pair| pair[0].id);
        Registry {
            fields,
            count,
            invalid,
            duplicate,
        }
    })
}
