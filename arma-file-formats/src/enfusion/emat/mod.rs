//! Enfusion material (`.emat`).
//!
//! Materials are [text resources](crate::enfusion::text). The class selects the shader, the
//! properties are its parameters. Materials usually inherit from a base material and only
//! override some properties, [`Emat::resolve`] merges the chain.
//!
//! Texture slots hold a [`ResourceRef`](crate::enfusion::resource::ResourceRef) to an `.edds`.
//! The most common classes in Arma Reforger:
//!
//! - `MatPBRBasic`: `BCRMap` (color, roughness), `NMOMap` (normal, metalness, occlusion),
//!   `Color` tint, optionally `OpacityMap` and `EmissiveMap`.
//! - `MatPBRMulti`: up to four layers `BCR_n` / `NMO_n` tinted with `Color_n` and tiled with
//!   `UVTransform_n` / `BCR_nUVTransform`, blended by the channels of `MaskMap` (sampled with
//!   the UV set named in `UVSrcGlobalMaps`), plus `GlobalNMOMap`.
//!
//! ```
//! use arma_file_formats::enfusion::emat::Emat;
//!
//! let text = r#"MatPBRBasic {
//!  Color 0.38 0.4 0.36 1
//!  BCRMap "{F8729AC879E5D6C1}Assets/Pouch_BCR.edds" clampu
//! }"#;
//! let material = Emat::parse(text)?;
//! let texture = material.texture("BCRMap").unwrap();
//! assert_eq!(texture.resource().path, "Assets/Pouch_BCR.edds");
//! assert!(texture.has_flag("clampu"));
//! assert_eq!(material.color("Color"), Some([0.38, 0.4, 0.36, 1.0]));
//! # Ok::<(), arma_file_formats::errors::AffError>(())
//! ```

#[allow(clippy::module_inception)]
mod emat;

pub use self::emat::*;
