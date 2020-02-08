#![cfg(feature = "test-support")]
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

static TEST_MODE: AtomicBool = AtomicBool::new(false);

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
        #[ctor::ctor]
        #[used]
        fn __procspawn_test_support_init() {
            $crate::testsupport::enable();
            panic!("hmm");
        }

        #[test]
        fn __procspawn_test_helper() {
            $crate::init();
        }
    };
}

pub fn enable() {
    TEST_MODE.store(true, Ordering::SeqCst);
}

pub fn update_command_for_tests(cmd: &mut Command) {
    if TEST_MODE.load(Ordering::SeqCst) {
        cmd.arg("__procspawn_test_helper");
        cmd.arg("--exact");
        cmd.arg("--test-threads=1");
        cmd.arg("-q");
    }
}
