//! This test checks how the JSON-serialized schema looks.
#![allow(unexpected_cfgs)]
#![allow(dead_code)]
mod common;
use iroha_schema::IntoSchema;
macro_rules! check_schemas {
    ($($case:literal => $ty:ident / $name:ident: $item:item)*) => {{
        $(
            #[derive(IntoSchema)]
            $item
            $crate::common::assert_root_schema::<$ty>(
                $case,
                stringify!($ty),
                stringify!($name),
            );
        )*
    }};
}
#[test]
fn test_struct() {
    check_schemas! {
        "schema_json.test_struct.empty_named" => EmptyNamedStruct / EmptyNamedStruct:
        struct EmptyNamedStruct {}
        "schema_json.test_struct.empty_tuple" => EmptyTupleStruct / EmptyTupleStruct:
        struct EmptyTupleStruct();
        "schema_json.test_struct.unit" => UnitStruct / UnitStruct:
        struct UnitStruct;
        "schema_json.test_struct.normal" => NormalStruct / NormalStruct:
        struct NormalStruct {
            normal_field_1: u32,
            normal_field_2: u32,
        }
        "schema_json.test_struct.newtype" => NewtypeStruct / NewtypeStruct:
        struct NewtypeStruct(u32);
        "schema_json.test_struct.tuple" => TupleStruct / TupleStruct:
        struct TupleStruct(u32, u32);
    }
}
#[test]
fn test_struct_codec_attr() {
    check_schemas! {
        "schema_json.test_struct_codec_attr.skip" => SkipField / SkipField:
        struct SkipField {
            #[codec(skip)]
            skipped_field: u32,
            normal_field: u32,
        }
        "schema_json.test_struct_codec_attr.compact" => CompactField / CompactField:
        struct CompactField {
            #[codec(compact)]
            compact_field: u32,
        }
    }
}
#[test]
fn test_transparent() {
    check_schemas! {
        "schema_json.test_transparent.inferred" => TransparentStruct / u32:
        #[schema(transparent)]
        struct TransparentStruct(u32);
        "schema_json.test_transparent.explicit_int" => TransparentStructExplicitInt / u32:
        #[schema(transparent = "u32")]
        struct TransparentStructExplicitInt { a: u32, b: i32 }
        "schema_json.test_transparent.explicit_string" => TransparentStructExplicitString / String:
        #[schema(transparent = "String")]
        struct TransparentStructExplicitString { a: u32, b: i32 }
        "schema_json.test_transparent.enum" => TransparentEnum / String:
        #[schema(transparent = "String")]
        enum TransparentEnum { Variant1, Variant2 }
    }
}
#[test]
fn test_enum() {
    check_schemas! {
        "schema_json.test_enum.empty" => EmptyEnum / EmptyEnum:
        enum EmptyEnum {}
        "schema_json.test_enum.dataless" => DatalessEnum / DatalessEnum:
        enum DatalessEnum { Variant1, Variant2 }
        "schema_json.test_enum.data" => DataEnum / DataEnum:
        enum DataEnum { Variant1(u32), Variant3(String) }
    }
}
#[test]
fn test_enum_with_norito_rename_all() {
    check_schemas! {
        "schema_json.test_enum_with_norito_rename_all" => BackendTag / BackendTag:
        #[norito(rename_all = "kebab-case")]
        enum BackendTag {
            Halo2IpaPasta,
            #[norito(rename = "halo2-bn254")]
            Halo2Bn254,
            Unsupported,
        }
    }
}
#[test]
fn test_enum_codec_attr() {
    check_schemas! {
        "schema_json.test_enum_codec_attr.skip" => SkipEnum / SkipEnum:
        enum SkipEnum { #[codec(skip)] Variant1, Variant2 }
        "schema_json.test_enum_codec_attr.index" => IndexEnum / IndexEnum:
        enum IndexEnum { Variant1 = 12, #[codec(index = 42)] Variant2 }
        "schema_json.test_enum_codec_attr.index_data" => IndexDataEnum / IndexDataEnum:
        enum IndexDataEnum { #[codec(index = 42)] Variant2(u32) }
    }
}
