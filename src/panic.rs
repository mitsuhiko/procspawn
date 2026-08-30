use std::any::Any;
use std::cell::RefCell;
use std::panic;

use crate::error::{Location, PanicInfo};

thread_local! {
    static PANIC_INFO: RefCell<Option<PanicInfo>> = const { RefCell::new(None) };
}

#[derive(Copy, Clone)]
pub enum BacktraceCapture {
    No,
    #[cfg(feature = "backtrace")]
    Resolved,
    #[cfg(feature = "backtrace")]
    Unresolved,
}

pub fn reset_panic_info() {
    PANIC_INFO.with(|pi| {
        *pi.borrow_mut() = None;
    });
}

pub fn take_panic(panic: &dyn Any) -> PanicInfo {
    PANIC_INFO
        .with(|pi| pi.borrow_mut().take())
        .unwrap_or_else(move || serialize_panic(panic))
}

pub fn panic_handler(info: &panic::PanicHookInfo<'_>, capture_backtraces: BacktraceCapture) {
    PANIC_INFO.with(|pi| {
        #[allow(unused_mut)]
        let mut panic = serialize_panic(info.payload());
        match capture_backtraces {
            BacktraceCapture::No => {}
            #[cfg(feature = "backtrace")]
            BacktraceCapture::Resolved => {
                panic.backtrace = Some(backtrace::Backtrace::new());
            }
            #[cfg(feature = "backtrace")]
            BacktraceCapture::Unresolved => {
                panic.backtrace = Some(backtrace::Backtrace::new_unresolved());
            }
        }
        panic.location = info.location().map(Location::from_std);
        *pi.borrow_mut() = Some(panic);
    });
}

pub fn init_panic_hook(capture_backtraces: BacktraceCapture) {
    let next = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        panic_handler(info, capture_backtraces);
        next(info);
    }));
}

fn serialize_panic(panic: &dyn Any) -> PanicInfo {
    PanicInfo::new(match panic.downcast_ref::<&'static str>() {
        Some(s) => s,
        None => match panic.downcast_ref::<String>() {
            Some(s) => &s[..],
            None => "Box<Any>",
        },
    })
}
