#[cfg(feature = "std")]
macro_rules! v_debug {
    ($($arg:tt)*) => { tracing::debug!($($arg)*) };
}

// Without std a message is dropped, but `format_args!` still type-checks it and keeps the
// variables it names used.
#[cfg(not(feature = "std"))]
macro_rules! v_debug {
    ($($arg:tt)*) => {{
        let _ = format_args!($($arg)*);
    }};
}

#[cfg(feature = "std")]
macro_rules! v_error {
    ($($arg:tt)*) => { tracing::error!($($arg)*) };
}

#[cfg(not(feature = "std"))]
macro_rules! v_error {
    ($($arg:tt)*) => {{
        let _ = format_args!($($arg)*);
    }};
}
