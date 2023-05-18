use std::mem::size_of;

use stewart::Addr;

#[test]
fn addr_option_is_same_size() {
    // This should be provided to us by the underlying Index type from thunderdome
    // But, it's good to verify just in case
    let size_plain = size_of::<Addr<()>>();
    let size_option = size_of::<Option<Addr<()>>>();
    assert_eq!(size_plain, size_option);
}
