#![cfg(feature = "test-support")]
use std::process::Command;
use std::sync::Mutex;

use lazy_static::lazy_static;

lazy_static! {
    static ref TEST_NAME: Mutex<Option<String>> = Mutex::new(None);
}

fn get_test_function() -> Option<String> {
    let backtrace = backtrace::Backtrace::new();
    let frames = backtrace.frames();
    let mut state = 0;

    for symbol in frames
        .iter()
        .rev()
        .flat_map(|x| x.symbols())
        .filter_map(|x| x.name())
        .map(|x| format!("{}", x))
    {
        if state == 0 {
            if symbol.starts_with("test::run_test::") {
                state = 1;
            }
        } else if state == 1 && symbol.starts_with("core::ops::function::FnOnce::call_once") {
            state = 2;
        } else if state == 2 {
            let mut rv = &symbol[..symbol.len() - 19];
            if rv.ends_with("::{{closure}}") {
                rv = &rv[..rv.len() - 13];
            }
            return Some(rv.to_string());
        }
    }

    None
}

pub fn detect() {
    if let Some(test_func) = get_test_function() {
        *TEST_NAME.lock().unwrap() = test_func.rsplitn(2, "::").next().map(|x| x.to_string());
    }
}

pub fn update_command_for_tests(cmd: &mut Command) {
    if let Some(ref test_name) = *TEST_NAME.lock().unwrap() {
        cmd.arg(test_name);
        cmd.arg("--exact");
        cmd.arg("--test-threads=1");
        cmd.arg("-q");
    }
}
