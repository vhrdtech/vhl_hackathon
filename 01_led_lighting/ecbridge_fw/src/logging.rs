#[macro_export]
macro_rules! _level_to_color {
    (trace) => { crate::vt100::CYAN };
    (debug) => { crate::vt100::DEFAULT };
    (info) => { crate::vt100::GREEN };
    (warn) => { crate::vt100::YELLOW };
    (error) => { crate::vt100::RED };
}

#[macro_export]
macro_rules! _log_internal {
    ($level: ident, => $terminal:expr) => {
        #[cfg(feature = "log-text-rtt")]
        rtt_target::rprintln!(=> $terminal);
    };
    ($level: ident, => $terminal:expr, $fmt:expr) => {
        #[cfg(feature = "log-text-rtt")] {
            rtt_target::rprint!(=> $terminal, "{}", crate::_level_to_color!($level));
            rtt_target::rprintln!(=> $terminal, $fmt);
            rtt_target::rprint!(=> $terminal, crate::vt100::DEFAULT);
        }
    };
    ($level: ident, => $terminal:expr, $fmt:expr, $($arg:tt)*) => {
        #[cfg(feature = "log-text-rtt")] {
            rtt_target::rprint!(=> $terminal, "{}", crate::_level_to_color!($level));
            rtt_target::rprintln!(=> $terminal, $fmt, $($arg)*);
            rtt_target::rprint!(=> $terminal, crate::vt100::DEFAULT);
        }
    };
    ($level: ident) => {
        #[cfg(feature = "log-text-rtt")]
        rtt_target::rprintln!(=>T);
    };
    ($level: ident, $fmt:expr) => {
        #[cfg(feature = "log-text-rtt")] {
            rtt_target::rprint!(=>T, "{}", crate::_level_to_color!($level));
            rtt_target::rprintln!(=>T, $fmt);
            rtt_target::rprint!(=>T, crate::vt100::DEFAULT);
        }
    };
    ($level: ident, $fmt:expr, $($arg:tt)*) => {
        #[cfg(feature = "log-text-rtt")] {
            rtt_target::rprint!(=>T, "{}", crate::_level_to_color!($level));
            rtt_target::rprintln!(=>T, $fmt, $($arg)*);
            rtt_target::rprint!(=>T, crate::vt100::DEFAULT);
        }
    };
}

#[cfg(feature = "log-level-trace")]
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        crate::_log_internal!(trace, $($arg)*);
    };
}
#[cfg(not(feature = "log-level-trace"))]
#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {}
}
pub use trace;

#[cfg(feature = "log-level-debug")]
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        crate::_log_internal!(debug, $($arg)*);
    };
}
#[cfg(not(feature = "log-level-debug"))]
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {}
}
pub use debug;

#[cfg(feature = "log-level-info")]
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        crate::_log_internal!(info, $($arg)*);
    };
}
#[cfg(not(feature = "log-level-info"))]
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {}
}
pub use info;

#[cfg(feature = "log-level-warn")]
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        crate::_log_internal!(warn, $($arg)*);
    };
}
#[cfg(not(feature = "log-level-warn"))]
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {}
}
pub use log_warn;

#[cfg(feature = "log-level-error")]
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        crate::_log_internal!(error, $($arg)*);
    };
}
#[cfg(not(feature = "log-level-error"))]
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {}
}
pub use error;