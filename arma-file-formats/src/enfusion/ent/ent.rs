use std::io::{Read, Write};

use super::binary::{read_ent, write_ent};
use crate::errors::AffError;

/// Binary Enfusion world (`.ent`), see the [module docs](super).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ent {
    /// Format version, `1` in all game files.
    pub version: u32,
    /// Top level entries: the entities of the world, `$grp` groups of entities and, in worlds
    /// that build on another world, a `SubScene` object.
    pub entries: Vec<Entry>,
}

/// An entry of an object body.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    /// `key value`, e.g. `coords 1 2 3`.
    Property { key: String, value: Value },
    /// A nested object or block, e.g. a component or `components { ... }`.
    Object(Object),
    /// A value without key. Arrays are stored like this, as the only entry of a block named
    /// after the array: `Filenames { [...] }`.
    Value(Value),
}

/// An object or block, `[key] [class] [name | id] [: parent] { body }`.
///
/// Like a line of the text format, see [`text::Entry`](crate::enfusion::text::Entry), the first
/// word is the key and the second the class. Which of them names the class of the object
/// depends on where it is:
///
/// | Text                                 | `key`           | `class`          |
/// | ------------------------------------ | --------------- | ---------------- |
/// | `GenericEntity GridPlane : "{..}" {` | `GenericEntity` |                  |
/// | `RigidBody "{..}" {` in `components` | `RigidBody`     |                  |
/// | `m_SpawnLogic SCR_SpawnLogic "{..}" {` | `m_SpawnLogic` | `SCR_SpawnLogic` |
/// | `$grp GenericEntity : "{..}" {`      | `$grp`          | `GenericEntity`  |
/// | `components {`                       | `components`    |                  |
/// | `GodRays {` in a `$grp`              |                 | (name `GodRays`) |
/// | `{` (child entities, group entries)  |                 |                  |
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Object {
    pub key: Option<String>,
    pub class: Option<String>,
    /// Entity name, e.g. the `GridPlane` of `GenericEntity GridPlane {`.
    pub name: Option<String>,
    /// Object id, e.g. the `"{54C4E8562DD8D20D}"` of `ShapePoint "{54C4E8562DD8D20D}" {`.
    pub id: Option<ObjectId>,
    /// GUID of the resource this object inherits from, the prefab of an entity.
    pub parent: Option<u64>,
    pub body: Body,
}

/// How an [`Object::id`] is stored. Which one is used is unknown, ids of components are
/// usually [`ObjectId::Shared`], ids of other objects [`ObjectId::Inline`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectId {
    /// Stored in the object header.
    Inline(u64),
    /// Stored in the GUID table of the file, like resource references.
    Shared(u64),
}

impl ObjectId {
    #[must_use]
    pub const fn id(self) -> u64 {
        match self {
            Self::Inline(id) | Self::Shared(id) => id,
        }
    }
}

/// The body of an [`Object`].
#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    Entries {
        /// A `u16` at the start of the body of every object instance: entities, components,
        /// entries of groups, class instances like `ShapePoint`. Plain blocks like
        /// `components { ... }` or child entity blocks have none. `0` in all game files,
        /// its meaning is unknown.
        instance: Option<u16>,
        entries: Vec<Entry>,
    },
    /// Data that is no token stream. In the game files only the value of `BSP` in
    /// `GenericWorldEntity`, which is plain text (`"428 1 41 1 256 ..."`).
    ///
    /// A body is read as raw if its first byte starts no entry.
    Raw(Vec<u8>),
}

impl Default for Body {
    fn default() -> Self {
        Self::Entries {
            instance: None,
            entries: Vec::new(),
        }
    }
}

/// A property value. Resources are referenced by GUID only, `0` for none.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i32),
    /// Bit sets like `Flags` (`EntityFlags`), `LightType` or `Rendering`.
    Flags(u64),
    Resource(u64),
    Float(f32),
    Vec2([f32; 2]),
    /// E.g. `coords` and `angles`.
    Vec3([f32; 3]),
    /// E.g. colors.
    Vec4([f32; 4]),
    Bool(bool),
    String(String),
    IntArray(Vec<i32>),
    FlagsArray(Vec<u64>),
    ResourceArray(Vec<u64>),
    FloatArray(Vec<f32>),
    Vec2Array(Vec<[f32; 2]>),
    Vec3Array(Vec<[f32; 3]>),
    Vec4Array(Vec<[f32; 4]>),
    BoolArray(Vec<bool>),
    StringArray(Vec<String>),
}

/// An entity of the world, see [`Ent::entities`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entity<'a> {
    pub class: &'a str,
    pub name: Option<&'a str>,
    /// GUID of the prefab of the entity, its own or that of its `$grp`.
    pub prefab: Option<u64>,
    /// Index of the parent entity in [`Ent::entities`], for child entities.
    pub parent: Option<usize>,
    /// The entity's object, its properties are in [`Object::entries`].
    pub object: &'a Object,
}

impl Ent {
    /// Reads a binary world, the rest of `input` has to be the file.
    pub fn from<I: Read>(input: &mut I) -> Result<Self, AffError> {
        let mut data = Vec::new();
        input.read_to_end(&mut data)?;
        Ok(read_ent(&data)?)
    }

    /// Writes the world at the current position of `out`. The string and GUID tables are built
    /// in the order of first use, like the game files.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        out.write_all(&write_ent(self)?)?;
        Ok(())
    }

    /// GUID of the world this one builds on, from its `SubScene` object.
    #[must_use]
    pub fn sub_scene_parent(&self) -> Option<u64> {
        let sub_scene = self
            .entries
            .iter()
            .filter_map(Entry::as_object)
            .find(|o| o.key.as_deref() == Some(SUB_SCENE) && o.class.is_none())?;
        match sub_scene.get("Parent") {
            Some(Value::Resource(guid)) => Some(*guid),
            _ => None,
        }
    }

    /// All entities of the world in file order, with groups resolved and each parent before its
    /// children. The `SubScene` object is no entity.
    ///
    /// Entities are the objects of the top level and of the child blocks, the keyless blocks in
    /// entity bodies. A `$grp` object groups entities of its class and prefab, its entries are
    /// the entities, optionally named.
    #[must_use]
    pub fn entities(&self) -> Vec<Entity<'_>> {
        let mut entities = Vec::new();
        collect_entities(&self.entries, None, true, &mut entities);
        entities
    }
}

const GROUP: &str = "$grp";
const SUB_SCENE: &str = "SubScene";

fn collect_entities<'a>(
    entries: &'a [Entry],
    parent: Option<usize>,
    top_level: bool,
    entities: &mut Vec<Entity<'a>>,
) {
    for object in entries.iter().filter_map(Entry::as_object) {
        match object.key.as_deref() {
            Some(GROUP) => {
                let Some(class) = object.class.as_deref() else {
                    continue;
                };
                for entry in object.entries().iter().filter_map(Entry::as_object) {
                    push_entity(class, object.parent, entry, parent, entities);
                }
            }
            Some(SUB_SCENE) if top_level => {}
            Some(class) => push_entity(class, None, object, parent, entities),
            None => {}
        }
    }
}

fn push_entity<'a>(
    class: &'a str,
    group_prefab: Option<u64>,
    object: &'a Object,
    parent: Option<usize>,
    entities: &mut Vec<Entity<'a>>,
) {
    let index = entities.len();
    entities.push(Entity {
        class,
        name: object.name.as_deref(),
        prefab: object.parent.or(group_prefab),
        parent,
        object,
    });
    for children in object.entries().iter().filter_map(Entry::as_object) {
        if children.is_plain_block() && children.key.is_none() {
            collect_entities(children.entries(), Some(index), false, entities);
        }
    }
}

impl Entry {
    #[must_use]
    pub const fn as_object(&self) -> Option<&Object> {
        match self {
            Self::Object(object) => Some(object),
            _ => None,
        }
    }
}

impl Object {
    /// The entries of the body, none for a [`Body::Raw`].
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        match &self.body {
            Body::Entries { entries, .. } => entries,
            Body::Raw(_) => &[],
        }
    }

    /// The entries of the body, `None` for a [`Body::Raw`].
    pub const fn entries_mut(&mut self) -> Option<&mut Vec<Entry>> {
        match &mut self.body {
            Body::Entries { entries, .. } => Some(entries),
            Body::Raw(_) => None,
        }
    }

    /// The value of the first property with this key. For an array this is the value inside
    /// the block named `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries().iter().find_map(|entry| match entry {
            Entry::Property { key: k, value } if k == key => Some(value),
            Entry::Object(object) if object.key.as_deref() == Some(key) => {
                match (object.is_plain_block(), object.entries()) {
                    (true, [Entry::Value(value)]) => Some(value),
                    _ => None,
                }
            }
            _ => None,
        })
    }

    /// The first nested object with this key.
    #[must_use]
    pub fn object(&self, key: &str) -> Option<&Self> {
        self.entries()
            .iter()
            .filter_map(Entry::as_object)
            .find(|object| object.key.as_deref() == Some(key))
    }

    /// `coords`, the position of an entity relative to its parent.
    #[must_use]
    pub fn coords(&self) -> Option<[f32; 3]> {
        match self.get("coords") {
            Some(Value::Vec3(coords)) => Some(*coords),
            _ => None,
        }
    }

    /// Whether this is a block without class, name, id, parent and instance header.
    const fn is_plain_block(&self) -> bool {
        self.class.is_none()
            && self.name.is_none()
            && self.id.is_none()
            && self.parent.is_none()
            && matches!(self.body, Body::Entries { instance: None, .. })
    }
}
