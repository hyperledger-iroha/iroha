use norito::core::Archived;

#[test]
fn print_archived_box_ty() {
    println!("{}", core::any::type_name::<Archived<Box<u32>>>());
}
