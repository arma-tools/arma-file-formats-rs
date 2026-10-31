//! Enfusion text resources, the format of `.emat` materials and of `.et` / `.conf` files.
//!
//! A document is one root object: a class, optionally the resource it inherits from, and a
//! body in braces. The body is line based, every line is a key followed by values:
//!
//! ```text
//! MatPBRBasic : "{5CDDE2709BA9A5EE}Assets/_BaseMaterials/Characters_MatPBRBasic.emat" {
//!  Color 0.384 0.396 0.362 1
//!  BCRMap "{F8729AC879E5D6C1}Assets/Characters/Vests/Data/Pouch_BCR.edds" clampu clampv
//!  "Wind power" 2
//!  BCR_1UVTransform MatUVTransform "{51823C7CB6F81D33}" {
//!   TilingU 4
//!  }
//!  CausticMap "{69750C9CCD3FE588}Caustics_00.edds" "{9D7D4B04C0E28DFC}Caustics_01.edds" anim( 20 )
//! }
//! ```
//!
//! - Values are quoted strings, unquoted words (numbers, identifiers) or calls like `anim( 20 )`.
//!   Keys may be quoted.
//! - A line ending in `{` opens a nested object: `Key [Class] ["id"] [: "parent"] {`.
//! - Lines inside lists or matrices have no key (`"Enabled"`, `1 0 0 0`), see [`Entry`].
//!
//! ```
//! use arma_file_formats::enfusion::text::Object;
//!
//! let material = Object::parse("MatPBRBasic {\n Color 1 0.5 0 1\n}\n")?;
//! assert_eq!(material.class.as_deref(), Some("MatPBRBasic"));
//! assert_eq!(material.float("Color"), Some(1.0));
//! # Ok::<(), arma_file_formats::errors::AffError>(())
//! ```

#[allow(clippy::module_inception)]
mod text;

pub use self::text::*;
