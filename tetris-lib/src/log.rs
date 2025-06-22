// Logging abstraction for tetris-lib
// This module provides logging macros that can work with different backends:
// - defmt for embedded targets
// - standard log crate for console/std targets

#[cfg(feature = "defmt-log")]
pub use defmt::{debug, error, info, trace, warn};

#[cfg(feature = "std-log")]
pub use log::{debug, error, info, trace, warn};

// If no logging feature is enabled, provide no-op macros
#[cfg(not(any(feature = "defmt-log", feature = "std-log")))]
#[macro_export]
macro_rules! trace {
    ($($args:tt)*) => {};
}

#[cfg(not(any(feature = "defmt-log", feature = "std-log")))]
#[macro_export]
macro_rules! debug {
    ($($args:tt)*) => {};
}

#[cfg(not(any(feature = "defmt-log", feature = "std-log")))]
#[macro_export]
macro_rules! info {
    ($($args:tt)*) => {};
}

#[cfg(not(any(feature = "defmt-log", feature = "std-log")))]
#[macro_export]
macro_rules! warn {
    ($($args:tt)*) => {};
}

#[cfg(not(any(feature = "defmt-log", feature = "std-log")))]
#[macro_export]
macro_rules! error {
    ($($args:tt)*) => {};
}

// Re-export the macros for convenience when no logging is enabled
#[cfg(not(any(feature = "defmt-log", feature = "std-log")))]
pub use {debug, error, info, trace};

// Note: warn is not re-exported due to name conflict with built-in attribute
