#![warn(missing_docs)]
#![warn(clippy::cargo)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![allow(missing_docs)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::no_effect_underscore_binding)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::float_cmp)]
#![allow(clippy::multiple_crate_versions)]

pub mod errors;

#[cfg(feature = "real_virtuality")]
pub mod real_virtuality;

#[cfg(feature = "enfusion")]
pub mod enfusion;
