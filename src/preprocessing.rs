#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi;
    use std::{
        ffi::{CStr, CString},
        os::raw::c_int,
    };

    const TOP_LEVEL: &str =
        "first line\n%include nested\n%define top_level 1\nlast line\n";
    const NESTED: &str = "nested\n";

    #[test]
    fn find_all_defines() {
        let src = "%define true 1\nsome random text\n%define FOO_BAR -42\n";
        let original_length = src.len();
        let src = CString::new(src).unwrap();

        unsafe {
            let mut src = libc::strdup(src.as_ptr());
            let mut len = original_length as c_int;
            let defines = ffi::tvm_htab_create();

            let ret = ffi::tvm_preprocess(&mut src, &mut len, defines);

            // preprocessing should have been successful
            assert_eq!(ret, 0);

            // make sure the define lines were removed
            let preprocessed = CStr::from_ptr(src).to_bytes();
            let preprocessed =
                std::str::from_utf8(&preprocessed[..len as usize]).unwrap();
            assert_eq!(preprocessed, "\nsome random text\n\n");

            // make sure the defines were set
            let true_define =
                ffi::tvm_htab_find_ref(defines, b"true\0".as_ptr().cast());
            let got = CStr::from_ptr(true_define).to_str().unwrap();
            assert_eq!(got, "1");
            let foo_bar =
                ffi::tvm_htab_find_ref(defines, b"FOO_BAR\0".as_ptr().cast());
            let got = CStr::from_ptr(foo_bar).to_str().unwrap();
            assert_eq!(got, "-42");

            ffi::tvm_htab_destroy(defines);
            libc::free(src.cast());
        }
    }

    #[test]
    fn sanity_check() {
        // set up a directory structure something like
        // - temp-dir-1234/
        //   - nested.vm
        let temp = tempfile::tempdir().unwrap();
        let nested = temp.path().join("nested.vm");

        let top_level_src =
            TOP_LEVEL.replace("nested", nested.display().to_string().as_str());
        std::fs::write(&nested, NESTED).unwrap();

        // after preprocessing, all include and define lines should have been
        // removed
        let expected = "first line\nnested\n\nlast line\n";

        unsafe {
            let top_level_src = CString::new(top_level_src).unwrap();
            // create a copy of the top_level_src which can be freed by C
            let mut src = libc::strdup(top_level_src.as_ptr());
            let mut len = libc::strlen(src) as c_int;
            let defines = ffi::tvm_htab_create();

            // after all that setup code we can *finally* call the preprocessor
            let ret = ffi::tvm_preprocess(&mut src, &mut len, defines);

            assert_eq!(ret, 0);

            // make sure the define and import lines were removed
            let preprocessed = CStr::from_ptr(src).to_bytes();
            let got =
                std::str::from_utf8(&preprocessed[..len as usize]).unwrap();

            assert_eq!(got, expected);

            ffi::tvm_htab_destroy(defines);
            libc::free(src as *mut _);
        }
    }
}
