use crate::htab::{HashTable, Item};
use std::{collections::hash_map::Entry, ffi::CString, io::Error as IoError};

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

fn process_includes(
    src: String,
) -> Result<(String, usize), PreprocessingError> {
    if let Some(ix) = src.find("%import") {
        let (before, after) = src.split_at(ix);
        let end_of_line = after.find("\n").unwrap_or(after.len());
        let (line, rest) = after.split_at(end_of_line);

        let included_file = parse_include_line(line)?;

        let loaded = std::fs::read_to_string(included_file).map_err(|e| {
            PreprocessingError::FailedInclude {
                name: included_file.to_string(),
                inner: e,
            }
        })?;

        let mut buffer = String::new();
        buffer.push_str(before);
        buffer.push_str(&loaded);
        buffer.push_str(rest);

        Ok((buffer, 1))
    } else {
        Ok((src.to_string(), 0))
    }
}

fn parse_include_line(_line: &str) -> Result<&str, PreprocessingError> {
    unimplemented!()
}

fn process_defines(
    mut src: String,
    defines: &mut HashTable,
) -> Result<(String, usize), PreprocessingError> {
    const TOK_DEFINE: &str = "%define";

    // try to find the first "%define"
    let directive_delimiter = match src.find(TOK_DEFINE) {
        Some(ix) => ix,
        None => return Ok((src, 0)),
    };

    // find the span from %define to the end of the line
    let begin = &src[directive_delimiter..];
    let end = begin.find('\n').unwrap_or(begin.len());

    let define_line = &begin[..end];
    let rest_of_define_line = define_line[TOK_DEFINE.len()..].trim();

    if rest_of_define_line.is_empty() {
        return Err(PreprocessingError::EmptyDefine);
    }

    // The syntax is "%define key value", so after removing the leading
    // "%define" everything after the next space is the value
    let first_space = rest_of_define_line.find(' ').ok_or_else(|| {
        PreprocessingError::DefineWithoutValue(rest_of_define_line.to_string())
    })?;

    // split the rest of the line into key and value
    let (key, value) = rest_of_define_line.split_at(first_space);
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

    // we've processed the %define line, so remove it from the original source
    // text
    let _ = src.drain(directive_delimiter..directive_delimiter + end);

    Ok((src, 1))
}

fn _parse_define(
    _line: &str,
    _defines: &mut HashTable,
) -> Result<(), PreprocessingError> {
    unimplemented!()
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

        let _ = process_defines(src.clone(), &mut hashtable).unwrap();

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
        const NESTED: &str = "nested\n";

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
