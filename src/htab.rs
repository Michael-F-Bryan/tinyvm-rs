use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    os::raw::{c_char, c_int, c_void},
    ptr,
};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct HashTable(HashMap<CString, Item>);

#[derive(Debug, Clone, PartialEq)]
struct Item {
    value: c_int,
    value_ptr: *mut c_void,
    length: c_int,
}

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
    let hashtable = &mut *htab;
    let key = CStr::from_ptr(key).to_owned();

    let item = Item {
        value,
        value_ptr: ptr::null_mut(),
        length: 0,
    };

    hashtable.0.insert(key, item);

    // the only time insertion can fail is if allocation fails. In that case
    // we'll abort the process anyway, so if this function returns we can
    // assume it was successful (0 = success).
    0
}

#[no_mangle]
pub unsafe extern "C" fn tvm_htab_add_ref(
    htab: *mut HashTable,
    key: *const c_char,
    value_ptr: *mut c_void,
    length: c_int,
) -> c_int {
    let hashtable = &mut *htab;
    let key = CStr::from_ptr(key).to_owned();

    let item = Item {
        value_ptr,
        length,
        value: 0,
    };

    hashtable.0.insert(key, item);

    0
}

#[no_mangle]
pub unsafe extern "C" fn tvm_htab_find(
    htab: *mut HashTable,
    key: *const c_char,
) -> c_int {
    let hashtable = &mut *htab;
    let key = CStr::from_ptr(key);

    match hashtable.0.get(key) {
        Some(item) => item.value,
        None => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn tvm_htab_find_ref(
    htab: *mut HashTable,
    key: *const c_char,
) -> *mut c_char {
    let hashtable = &mut *htab;
    let key = CStr::from_ptr(key);

    match hashtable.0.get(key) {
        Some(item) => item.value_ptr as *mut c_char,
        None => ptr::null_mut(),
    }
}
