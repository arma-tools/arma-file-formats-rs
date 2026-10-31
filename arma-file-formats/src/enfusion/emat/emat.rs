use std::{collections::HashSet, io::Read};

use crate::{
    enfusion::{
        resource::ResourceRef,
        text::{Object, Value},
    },
    errors::{AffError, EmatError},
};

/// Inheritance chains deeper than this are treated as broken.
const MAX_INHERITANCE_DEPTH: usize = 32;

/// Enfusion material (`.emat`).
#[derive(Debug, Clone, PartialEq)]
pub struct Emat {
    /// Material class, which selects the shader, e.g. `MatPBRBasic` or `MatPBRMulti`.
    pub class: String,
    /// Material this one inherits from.
    pub parent: Option<ResourceRef>,
    /// All properties. Only the material's own ones until [`Emat::resolve`] merged in the
    /// inherited ones.
    pub object: Object,
}

/// A texture slot, e.g. `BCRMap "{GUID}path.edds" clampu clampv`.
#[derive(Debug, Clone, PartialEq)]
pub struct TextureRef {
    /// The texture, or the frames of an animated texture.
    pub frames: Vec<ResourceRef>,
    /// Frames per second of an animated texture (`anim( 20 )`).
    pub fps: Option<f32>,
    /// Sampler flags: `clampu`, `clampv`, `nofilter`, `noresize`.
    pub flags: Vec<String>,
}

/// Texture coordinate transform of a layer (`MatUVTransform`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvTransform {
    pub tiling: [f32; 2],
    pub offset: [f32; 2],
    /// Degrees.
    pub rotation: f32,
}

impl Default for UvTransform {
    fn default() -> Self {
        Self {
            tiling: [1.0, 1.0],
            offset: [0.0, 0.0],
            rotation: 0.0,
        }
    }
}

impl TextureRef {
    /// The texture, the first frame for animated ones.
    #[must_use]
    pub fn resource(&self) -> &ResourceRef {
        &self.frames[0]
    }

    #[must_use]
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f.eq_ignore_ascii_case(flag))
    }

    fn from_values(values: &[Value]) -> Option<Self> {
        let mut texture = Self {
            frames: Vec::new(),
            fps: None,
            flags: Vec::new(),
        };
        for value in values {
            match value {
                Value::String(text) if texture.flags.is_empty() => {
                    texture.frames.push(ResourceRef::parse(text)?);
                }
                Value::Word(flag) => texture.flags.push(flag.clone()),
                Value::Call { name, args } if name == "anim" => {
                    texture.fps = args.first().and_then(Value::as_f32);
                }
                _ => return None,
            }
        }
        let first = texture.frames.first()?;
        let is_texture = first.path.to_lowercase().ends_with(".edds")
            || (first.path.is_empty() && first.guid.is_some());
        is_texture.then_some(texture)
    }
}

impl Emat {
    pub fn parse(text: &str) -> Result<Self, AffError> {
        let mut object = Object::parse(text)?;
        let class = object.class.clone().unwrap_or_default();
        let parent = object.parent.take().as_deref().and_then(ResourceRef::parse);
        Ok(Self {
            class,
            parent,
            object,
        })
    }

    /// Reads a material, invalid UTF-8 is replaced.
    pub fn from<I: Read>(input: &mut I) -> Result<Self, AffError> {
        let mut bytes = Vec::new();
        input.read_to_end(&mut bytes)?;
        Self::parse(&String::from_utf8_lossy(&bytes))
    }

    /// The texture in slot `key`, e.g. `BCRMap`.
    #[must_use]
    pub fn texture(&self, key: &str) -> Option<TextureRef> {
        TextureRef::from_values(self.object.values(key)?)
    }

    /// Every texture slot with its key, in file order.
    pub fn textures(&self) -> impl Iterator<Item = (&str, TextureRef)> {
        self.object.entries.iter().filter_map(|entry| {
            let texture = TextureRef::from_values(&entry.values)?;
            Some((entry.key.as_str(), texture))
        })
    }

    #[must_use]
    pub fn float(&self, key: &str) -> Option<f32> {
        self.object.float(key)
    }

    /// An RGB or RGBA color, alpha defaults to `1`.
    #[must_use]
    pub fn color(&self, key: &str) -> Option<[f32; 4]> {
        let values = self
            .object
            .values(key)?
            .iter()
            .map(Value::as_f32)
            .collect::<Option<Vec<_>>>()?;
        match values[..] {
            [r, g, b] => Some([r, g, b, 1.0]),
            [r, g, b, a] => Some([r, g, b, a]),
            _ => None,
        }
    }

    /// The `MatUVTransform` in slot `key`, e.g. `BCR_1UVTransform` or `UVTransform_1`.
    /// Unset fields have their defaults.
    #[must_use]
    pub fn uv_transform(&self, key: &str) -> Option<UvTransform> {
        let object = self.object.object(key)?;
        let field = |name, default| object.float(name).unwrap_or(default);
        Some(UvTransform {
            tiling: [field("TilingU", 1.0), field("TilingV", 1.0)],
            offset: [field("OffsetU", 0.0), field("OffsetV", 0.0)],
            rotation: field("Rotation", 0.0),
        })
    }

    /// Merges the properties of every ancestor into this material, closer ancestors win.
    ///
    /// `load` returns the material for a parent reference, usually looked up by path (see
    /// [`ResourceRef::lookup_key`]). Fails if a parent can't be loaded or the chain loops.
    /// [`Emat::parent`] is kept.
    pub fn resolve<F>(self, mut load: F) -> Result<Self, AffError>
    where
        F: FnMut(&ResourceRef) -> Option<Self>,
    {
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut next = self.parent.clone();
        while let Some(parent) = next.take() {
            if !seen.insert(parent.lookup_key()) || chain.len() >= MAX_INHERITANCE_DEPTH {
                return Err(EmatError::InheritanceCycle(parent.to_string()).into());
            }
            let material =
                load(&parent).ok_or_else(|| EmatError::ParentNotFound(parent.to_string()))?;
            next.clone_from(&material.parent);
            chain.push(material);
        }

        let Some(mut object) = chain.pop().map(|root| root.object) else {
            return Ok(self);
        };
        for ancestor in chain.into_iter().rev() {
            object.merge(ancestor.object);
        }
        object.merge(self.object);
        Ok(Self {
            class: self.class,
            parent: self.parent,
            object,
        })
    }
}

impl std::str::FromStr for Emat {
    type Err = AffError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}
