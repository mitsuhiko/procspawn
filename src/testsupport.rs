#![cfg(feature = "test-support")]
use std::env;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use crate::core::mark_initialized;

static TEST_MODE: AtomicBool = AtomicBool::new(false);
static TEST_MODULE: AtomicPtr<String> = AtomicPtr::new(std::ptr::null_mut());

// we need this.
pub use small_ctor::ctor;

/// Supports the use of procspawn in tests.
///
/// Due to limitations in rusttest it's currently not easily possible to use
/// procspawn with rusttest.  The workaround is to call use this macro toplevel
/// which will define a dummy test which is used to invoke all subprocesses.
///
/// ```rust,no_run
/// procspawn::enable_test_support!();
/// ```
///
/// Requires the `test-support` feature.
#[macro_export]
macro_rules! enable_test_support {
    () => {
        #[$crate::testsupport::ctor]
        unsafe fn __procspawn_test_support_init() {
            // strip the crate name from the module path
            let module_path = std::module_path!().splitn(2, "::").nth(1);
            $crate::testsupport::enable(module_path);
        }

        #[test]
        fn procspawn_test_helper() {
            $crate::init();
        }
    };
}

pub fn enable(module: Option<&str>) {
    if TEST_MODE.swap(true, Ordering::SeqCst) {
        panic!("procspawn testmode can only be enabled once");
    }

    if let Some(module) = module {
        let ptr = Box::into_raw(Box::new(module.to_string()));
        TEST_MODULE.store(ptr, Ordering::SeqCst);
    }

    mark_initialized();
}

pub struct TestMode {
    pub can_pass_args: bool,
    pub should_silence_stdout: bool,
}

fn test_helper_path() -> String {
    match unsafe { TEST_MODULE.load(Ordering::SeqCst).as_ref() } {
        Some(module) => format!("{}::procspawn_test_helper", module),
        None => "procspawn_test_helper".to_string(),
    }
}

pub fn update_command_for_tests(cmd: &mut Command) -> Option<TestMode> {
    if TEST_MODE.load(Ordering::SeqCst) {
        cmd.arg(test_helper_path());
        cmd.arg("--exact");
        cmd.arg("--test-threads=1");
        cmd.arg("-q");
        Some(TestMode {
            can_pass_args: false,
            should_silence_stdout: !env::args().any(|x| x == "--show-output" || x == "--nocapture"),
        })
    } else {
        None
    }
}
