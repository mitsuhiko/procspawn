/// Utility macro to spawn functions.
///
/// Since a very common pattern is to pass multiple values to a spawned
/// function by using tuples it can be cumbersome to repeat the arguments.
/// This macro makes it possible to not repeat oneself.  The first argument
/// to the macro is a tuple of the arguments to pass, the second argument
/// is the closure that should be invoked.
///
/// For each argument the following expressions are possible:
///
/// * `ident`: uses the local variable `ident` unchanged.
/// * `ident => other`: serializes `ident` and deserializes into `other`
/// * `*ident`: alias for `*ident => ident`.
/// * `mut ident`: alias for `ident => mut ident`.
///
/// Example:
///
/// ```rust,no_run
/// let a = 42u32;
/// let b = 23u32;
/// let handle = procspawn::spawn!((a, mut b) || {
///     b += 1;
///     a + b
/// });
/// ```
///
/// To spawn in a pool you need to pass the pool to it (prefixed with `in`):
///
/// ```rust,no_run
/// # let pool = procspawn::Pool::new(4).unwrap();
/// let a = 42u32;
/// let b = 23u32;
/// let handle = procspawn::spawn!(in pool, (a, mut b) || {
///     b += 1;
///     a + b
/// });
/// ```
///
#[macro_export]
macro_rules! spawn {
    (in $pool:expr, $($args:tt)*) => { $crate::_spawn_impl!(pool $pool, $($args)*) };
    ($($args:tt)*) => { $crate::_spawn_impl!(func $crate::spawn, $($args)*) }
}

/// Utility macro to spawn async functions.
///
/// This works exactly like the [`spawn!`](macro.spawn.html) macro but instead
/// will invoke [`spawn_async`](fn.spawn_async.html).
#[macro_export]
#[cfg(feature = "async")]
macro_rules! spawn_async {
    ($($args:tt)*) => { $crate::_spawn_impl!(func $crate::spawn_async, $($args)*) }
}

#[macro_export]
#[doc(hidden)]
macro_rules! _spawn_impl {
    (pool $pool:expr, () || $($body:tt)*) => {
        $pool.spawn(
            (),
            |()|
            $($body)*
        )
    };
    (pool $pool:expr, ($($param:tt)*) || $($body:tt)*) => {
        $pool.spawn(
            $crate::_spawn_call_arg!($($param)*),
            |($crate::_spawn_decl_arg!($($param)*))|
            $($body)*
        )
    };
    (func $func:path, () || $($body:tt)*) => {
        $func(
            (),
            |()|
            $($body)*
        )
    };
    (func $func:path, ($($param:tt)*) || $($body:tt)*) => {
        $func(
            $crate::_spawn_call_arg!($($param)*),
            |($crate::_spawn_decl_arg!($($param)*))|
            $($body)*
        )
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _spawn_call_arg {
    ($expr:expr => mut $x:ident, $($tt:tt)*) => {(
        $expr, $crate::_spawn_call_arg!($($tt)*)
    )};
    (*$expr:expr => $x:ident, $($tt:tt)*) => {(
        *$expr, $crate::_spawn_call_arg!($($tt)*)
    )};
    ($expr:expr => $x:ident, $($tt:tt)*) => {(
        $expr, $crate::_spawn_call_arg!($($tt)*)
    )};
    ($expr:expr => mut $x:ident) => {(
        $expr,
    )};
    ($expr:expr => $x:ident) => {(
        $expr,
    )};
    (mut $x:ident, $($tt:tt)*) => {(
        $x, $crate::_spawn_call_arg!($($tt)*)
    )};
    (mut $x:ident) => {(
        $x,
    )};
    (*$x:ident, $($tt:tt)*) => {(
        *$x, $crate::_spawn_call_arg!($($tt)*)
    )};
    ($x:ident, $($tt:tt)*) => {(
        $x, $crate::_spawn_call_arg!($($tt)*)
    )};
    (*$x:ident) => {(
        *$x,
    )};
    ($x:ident) => {(
        $x,
    )};

    ($unexpected:tt) => {
        $crate::_spawn_unexpected($unexpected);
    };
    () => (())
}

#[macro_export]
#[doc(hidden)]
macro_rules! _spawn_decl_arg {
    ($expr:expr => mut $x:ident, $($tt:tt)*) => {(
        mut $x, $crate::_spawn_decl_arg!($($tt)*)
    )};
    (*$expr:expr => $x:ident, $($tt:tt)*) => {(
        $x, $crate::_spawn_decl_arg!($($tt)*)
    )};
    ($expr:expr => $x:ident, $($tt:tt)*) => {(
        $x, $crate::_spawn_decl_arg!($($tt)*)
    )};
    ($expr:expr => mut $x:ident) => {(
        mut $x,
    )};
    (*$expr:expr => $x:ident) => {(
        $x,
    )};
    ($expr:expr => $x:ident) => {(
        $x,
    )};
    (mut $x:ident, $($tt:tt)*) => {(
        mut $x, $crate::_spawn_decl_arg!($($tt)*)
    )};
    (mut $x:ident) => {(
        mut $x,
    )};
    (*$x:ident, $($tt:tt)*) => {(
        $x, $crate::_spawn_decl_arg!($($tt)*)
    )};
    ($x:ident, $($tt:tt)*) => {(
        $x, $crate::_spawn_decl_arg!($($tt)*)
    )};
    (*$x:ident) => {(
        $x,
    )};
    ($x:ident) => {(
        $x,
    )};

    ($unexpected:tt) => {
        $crate::_spawn_unexpected($unexpected);
    };
    () => ()
}

#[macro_export]
#[doc(hidden)]
macro_rules! _spawn_unexpected {
    () => {};
}
