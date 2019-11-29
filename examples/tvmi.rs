use std::{env, ffi::CString};
use tinyvm::ffi;

fn main() {
    let filename = CString::new(env::args().nth(1).unwrap()).unwrap();
    // cast away the `const` because that's what libtvm expects
    let filename = filename.as_ptr() as *mut _;

    unsafe {
        let vm = ffi::tvm_vm_create();

        if !vm.is_null() && ffi::tvm_vm_interpret(vm, filename) == 0 {
            ffi::tvm_vm_run(vm);
        }

        ffi::tvm_vm_destroy(vm);
    }
}
