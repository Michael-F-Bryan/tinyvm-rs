use crate::{
    ffi::tvm_htab_ctx,
    htab::{HashTable, Item},
};
use std::{
    collections::hash_map::Entry,
    ffi::{CStr, CString},
    io::Error as IoError,
    os::raw::{c_char, c_int},
};

#[no_mangle]
pub unsafe extern "C" fn tvm_preprocess(
    src: *mut *mut c_char,
    src_len: *mut c_int,
    defines: *mut tvm_htab_ctx,
) -> c_int {
    if src.is_null() || src_len.is_null() || defines.is_null() {
        return -1;
    }

    let defines = &mut *(defines as *mut HashTable);

    let rust_src = match CStr::from_ptr(*src).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };

    match preprocess(rust_src, defines) {
        Ok(s) => {
            let preprocessed = CString::new(s).unwrap();
            // create a copy of the preprocessed string that can be free'd by C
            *src = libc::strdup(preprocessed.as_ptr());
            *src_len = libc::strlen(*src) as c_int;
            0
        },
        Err(_) => -1,
    }
}

pub fn preprocess(
    src: String,
    defines: &mut HashTable,
) -> Result<String, PreprocessingError> {
    let mut src = src;

    loop {
        let (modified, num_includes) = process_includes(src)?;
        let (modified, num_defines) = process_defines(modified, defines)?;

        if num_includes + num_defines == 0 {
            return Ok(modified);
        }

        src = modified;
    }
}

/// Scan through the input string looking for a line starting with some
/// directive, using a callback to figure out what to replace the directive line
/// with.
fn process_line_starting_with_directive<F>(
    mut src: String,
    directive: &str,
    mut replace_line: F,
) -> Result<(String, usize), PreprocessingError>
where
    F: FnMut(&str) -> Result<String, PreprocessingError>,
{
    // try to find the first instance of the directive
    let directive_delimiter = match src.find(directive) {
        Some(ix) => ix,
        None => return Ok((src, 0)),
    };

    // calculate the span from the directive to the end of the line
    let end_ix = src[directive_delimiter..]
        .find('\n')
        .map(|ix| ix + directive_delimiter)
        .unwrap_or(src.len());

    // the rest of the line after the directive
    let directive_line =
        src[directive_delimiter + directive.len()..end_ix].trim();

    // use the callback to figure out what we should replace the line with
    let replacement = replace_line(directive_line)?;

    // remove the original line
    let _ = src.drain(directive_delimiter..end_ix);
    // then insert our replacement
    src.insert_str(directive_delimiter, &replacement);

    Ok((src, 1))
}

fn process_includes(
    src: String,
) -> Result<(String, usize), PreprocessingError> {
    const TOK_INCLUDE: &str = "%include";

    process_line_starting_with_directive(src, TOK_INCLUDE, |line| {
        std::fs::read_to_string(line).map_err(|e| {
            PreprocessingError::FailedInclude {
                name: line.to_string(),
                inner: e,
            }
        })
    })
}

fn process_defines(
    src: String,
    defines: &mut HashTable,
) -> Result<(String, usize), PreprocessingError> {
    const TOK_DEFINE: &str = "%define";

    process_line_starting_with_directive(src, TOK_DEFINE, |line| {
        parse_define(line, defines)?;
        Ok(String::new())
    })
}

fn parse_define(
    line: &str,
    defines: &mut HashTable,
) -> Result<(), PreprocessingError> {
    if line.is_empty() {
        return Err(PreprocessingError::EmptyDefine);
    }

    // The syntax is "%define key value", so after removing the leading
    // "%define" everything after the next space is the value
    let first_space = line.find(' ').ok_or_else(|| {
        PreprocessingError::DefineWithoutValue(line.to_string())
    })?;

    // split the rest of the line into key and value
    let (key, value) = line.split_at(first_space);
    let value = value.trim();

    match defines.0.entry(
        CString::new(key).expect("The text shouldn't contain null bytes"),
    ) {
        // the happy case, this symbol hasn't been defined before so we can just
        // insert it.
        Entry::Vacant(vacant) => {
            vacant.insert(Item::opaque(value));
        },
        // looks like this key has already been defined, report an error
        Entry::Occupied(occupied) => {
            return Err(PreprocessingError::DuplicateDefine {
                name: key.to_string(),
                original_value: occupied
                    .get()
                    .opaque_value_str()
                    .unwrap_or("<invalid>")
                    .to_string(),
                new_value: value.to_string(),
            });
        },
    }

    Ok(())
}

#[derive(Debug)]
pub enum PreprocessingError {
    FailedInclude {
        name: String,
        inner: IoError,
    },
    DuplicateDefine {
        name: String,
        original_value: String,
        new_value: String,
    },
    EmptyDefine,
    DefineWithoutValue(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi;
    use std::{
        ffi::{CStr, CString},
        io::Write,
        os::raw::c_int,
    };
    use tempfile::NamedTempFile;

    #[test]
    fn empty_string() {
        let src = String::from("");
        let mut hashtable = HashTable::default();

        let (got, replacements) = process_defines(src, &mut hashtable).unwrap();

        assert!(got.is_empty());
        assert_eq!(replacements, 0);
        assert!(hashtable.0.is_empty());
    }

    #[test]
    fn false_percent() {
        let src = String::from("this string contains a % symbol");
        let mut hashtable = HashTable::default();

        let (got, replacements) =
            process_defines(src.clone(), &mut hashtable).unwrap();

        assert_eq!(got, src);
        assert_eq!(replacements, 0);
        assert!(hashtable.0.is_empty());
    }

    #[test]
    fn define_without_key_and_value() {
        let src = String::from("%define\n");
        let mut hashtable = HashTable::default();

        let err = process_defines(src.clone(), &mut hashtable).unwrap_err();

        match err {
            PreprocessingError::EmptyDefine => {},
            other => panic!("Expected EmptyDefine, found {:?}", other),
        }
    }

    #[test]
    fn define_without_value() {
        let src = String::from("%define key\n");
        let mut hashtable = HashTable::default();

        let err = process_defines(src.clone(), &mut hashtable).unwrap_err();

        match err {
            PreprocessingError::DefineWithoutValue(key) => {
                assert_eq!(key, "key")
            },
            other => panic!("Expected DefineWithoutValue, found {:?}", other),
        }
    }

    #[test]
    fn valid_define() {
        let src = String::from("%define key value\n");
        let mut hashtable = HashTable::default();

        let (got, num_defines) =
            process_defines(src.clone(), &mut hashtable).unwrap();

        assert_eq!(got, "\n");
        assert_eq!(num_defines, 1);
        assert_eq!(hashtable.0.len(), 1);
        let key = CString::new("key").unwrap();
        let item = hashtable.0.get(&key).unwrap();
        assert_eq!(item.opaque_value_str().unwrap(), "value");
    }

    #[test]
    fn find_all_defines() {
        let src = String::from(
            "%define true 1\nsome random text\n%define FOO_BAR -42\n",
        );
        let original_length = src.len();
        let src = CString::new(src).unwrap();

        unsafe {
            // get a copy of `src` that was allocated using C's malloc
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

            // make sure the "true" and "FOO_BAR" defines were set
            let true_define =
                ffi::tvm_htab_find_ref(defines, b"true\0".as_ptr().cast());
            let got = CStr::from_ptr(true_define).to_str().unwrap();
            assert_eq!(got, "1");
            let foo_bar =
                ffi::tvm_htab_find_ref(defines, b"FOO_BAR\0".as_ptr().cast());
            let got = CStr::from_ptr(foo_bar).to_str().unwrap();
            assert_eq!(got, "-42");

            // clean up our hashtable and copied source text
            ffi::tvm_htab_destroy(defines);
            libc::free(src.cast());
        }
    }

    #[test]
    fn include_another_file() {
        const TOP_LEVEL: &str = "first line\n%include nested\nlast line\n";
        const NESTED: &str = "nested";

        // the preprocessor imports files from the filesystem, so we need to
        // copy NESTED to a temporary location
        let mut nested = NamedTempFile::new().unwrap();
        nested.write_all(NESTED.as_bytes()).unwrap();
        let nested_filename = nested.path().display().to_string();

        // substitute the full path to the "nested" file
        let top_level_src = TOP_LEVEL.replace("nested", &nested_filename);
        std::fs::write(&nested, NESTED).unwrap();

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

            // after preprocessing, all include and define lines should have
            // been removed
            assert_eq!(got, "first line\nnested\nlast line\n");

            ffi::tvm_htab_destroy(defines);
            libc::free(src as *mut _);
        }
    }
}
