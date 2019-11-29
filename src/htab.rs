use std::{
    collections::HashMap,
    ffi::CString,
    os::raw::{c_char, c_int, c_void},
};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct HashTable(HashMap<CString, Item>);

#[derive(Debug, Clone, PartialEq)]
struct Item {}

#[no_mangle]
pub unsafe extern "C" fn tvm_htab_create() -> *mut HashTable {
    let hashtable = Box::new(HashTable::default());
    Box::into_raw(hashtable)
}

#[no_mangle]
pub unsafe extern "C" fn tvm_htab_destroy(htab: *mut HashTable) {
    if htab.is_null() {
        // nothing to free
        return;
    }

    let hashtable = Box::from_raw(htab);
    // explicitly destroy the hashtable
    drop(hashtable);
}

#[no_mangle]
pub unsafe extern "C" fn tvm_htab_add(
    htab: *mut HashTable,
    key: *const c_char,
    value: c_int,
) -> c_int {
    unimplemented!()
}

#[no_mangle]
pub unsafe extern "C" fn tvm_htab_add_ref(
    htab: *mut HashTable,
    key: *const c_char,
    value_ptr: *mut c_void,
    length: c_int,
) -> c_int {
    unimplemented!()
}

#[no_mangle]
pub unsafe extern "C" fn tvm_htab_find(
    htab: *mut HashTable,
    key: *const c_char,
) -> c_int {
    unimplemented!()
}

#[no_mangle]
pub unsafe extern "C" fn tvm_htab_find_ref(
    htab: *mut HashTable,
    key: *const c_char,
) -> *mut char {
    unimplemented!()
}
