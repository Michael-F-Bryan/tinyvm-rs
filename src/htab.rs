use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    os::raw::{c_char, c_int, c_void},
    ptr,
};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct HashTable(pub(crate) HashMap<CString, Item>);

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Item {
    /// An integer value.
    value: c_int,
    /// An opaque value used with [`tvm_htab_add_ref()`].
    ///
    /// # Safety
    ///
    /// Storing the contents of a `void *` in a `Vec<u8>` *would* normally
    /// result in alignment issues, but we've got access to the `libtvm` source
    /// code and know it will only ever store `char *` strings.
    opaque_value: Vec<u8>,
}

impl Item {
    pub(crate) fn integer(value: c_int) -> Item {
        Item {
            value,
            opaque_value: Vec::new(),
        }
    }

    pub(crate) fn opaque<V>(opaque_value: V) -> Item
    where
        V: Into<Vec<u8>>,
    {
        Item {
            value: 0,
            opaque_value: opaque_value.into(),
        }
    }

    pub(crate) fn from_void(pointer: *mut c_void, length: c_int) -> Item {
        // we need to create an owned copy of the value
        let opaque_value = if pointer.is_null() {
            Vec::new()
        } else {
            unsafe {
                std::slice::from_raw_parts(pointer as *mut u8, length as usize)
                    .to_owned()
            }
        };

        Item::opaque(opaque_value)
    }

    pub(crate) fn opaque_value(&self) -> &[u8] { &self.opaque_value }

    pub(crate) fn opaque_value_str(&self) -> Option<&str> {
        std::str::from_utf8(self.opaque_value()).ok()
    }
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

    hashtable.0.insert(key, Item::integer(value));

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

    hashtable.0.insert(key, Item::from_void(value_ptr, length));

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
        Some(item) => item.opaque_value.as_ptr() as *mut c_char,
        None => ptr::null_mut(),
    }
}
