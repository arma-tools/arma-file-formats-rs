use std::io::Cursor;

use binrw::{BinRead, BinWrite};

use super::ttile::TTILE_COMPACT_VERSION;
use crate::errors::{AffError, TerrError};

/// A `BMAT` chunk of the `TMAT` chunk: which surface material covers which part of a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileBlockMaterials {
    /// Block coordinates in the whole terrain, not the tile.
    pub x: u16,
    pub z: u16,
    /// Indices into [`Terr::materials`](crate::enfusion::terr::Terr::materials).
    pub materials: Vec<u16>,
    /// Absent if the first material covers the whole block.
    pub map: Option<MaterialMap>,
}

/// A quadtree over the texels of a block.
///
/// Texel x and z run along the terrain's x and z, starting at the block's lowest corner. Each
/// node splits its square into four quadrants, in the order `(x0, z0)`, `(x1, z0)`, `(x0, z1)`,
/// `(x1, z1)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialMap {
    /// Only stored before [`TTILE_COMPACT_VERSION`], always `0`.
    pub unknown: u32,
    /// Only stored before [`TTILE_COMPACT_VERSION`]: the texels covered by the root, `x0`, `z0`,
    /// `x1`, `z1` inclusive.
    pub rect: [u32; 4],
    pub root: MaterialNode,
    pub nodes: Vec<[MaterialNode; 4]>,
    pub bitmaps: Vec<MaterialBitmap>,
}

/// A part of a [`MaterialMap`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialNode {
    /// Split into the quadrants of [`MaterialMap::nodes`] at this index.
    Node(u32),
    /// Covered by the material at this index of [`TileBlockMaterials::materials`].
    Material(u32),
    /// Covered by the [`MaterialMap::bitmaps`] at this index.
    Bitmap(u32),
}

/// A square of texels, each an index into [`TileBlockMaterials::materials`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialBitmap {
    /// Only stored before [`TTILE_COMPACT_VERSION`]: the texels of the block covered by the
    /// bitmap, `x0`, `z0`, `x1`, `z1` inclusive.
    pub rect: [u32; 4],
    /// Number of texels, the square of the side length.
    pub texels: u32,
    /// The texels row by row along x, each [`bits_per_texel`] bits, least significant bit
    /// first. Before [`TTILE_COMPACT_VERSION`] padded to whole `u32`s.
    pub data: Vec<u8>,
}

/// Bits per texel of a [`MaterialBitmap`] in a block with `materials` materials.
#[must_use]
pub const fn bits_per_texel(version: u32, materials: usize) -> u32 {
    let max = if version >= TTILE_COMPACT_VERSION {
        materials.saturating_sub(1)
    } else {
        materials
    };
    match max.checked_ilog2() {
        Some(log) => log + 1,
        None => 1,
    }
}

impl MaterialBitmap {
    /// Texels along one side.
    #[must_use]
    pub const fn side(&self) -> u32 {
        self.texels.isqrt()
    }

    /// Value of the texel at `index`, which is `z * side + x`.
    #[must_use]
    pub fn texel(&self, index: u32, bits: u32) -> Option<u32> {
        if index >= self.texels || bits > 32 {
            return None;
        }
        let start = u64::from(index) * u64::from(bits);
        let mut value = 0;
        for i in 0..u64::from(bits) {
            let bit = start + i;
            let byte = self.data.get(usize::try_from(bit / 8).ok()?)?;
            value |= u32::from((byte >> (bit % 8)) & 1) << i;
        }
        Some(value)
    }
}

impl TileBlockMaterials {
    /// Material at texel `x`, `z` of a block that is `size` texels wide, a power of two, as an
    /// index into [`Terr::materials`](crate::enfusion::terr::Terr::materials). `version` is the
    /// [`Ttile::version`](super::Ttile::version).
    ///
    /// `size` has to be at least the resolution of the map: `64` in the game's version 9
    /// terrains, before that [`MaterialMap::rect`] tells. Larger sizes upscale the map. `None` if
    /// `x` or `z` lie outside the block, `size` is too small or the map is broken.
    #[must_use]
    pub fn material_at(&self, version: u32, x: u32, z: u32, size: u32) -> Option<u16> {
        if x >= size || z >= size {
            return None;
        }
        let Some(map) = &self.map else {
            return self.materials.first().copied();
        };

        let (mut x, mut z, mut size) = (x, z, size);
        let mut node = map.root;
        let index = loop {
            match node {
                MaterialNode::Material(index) => break index,
                MaterialNode::Bitmap(index) => {
                    let bitmap = map.bitmaps.get(index as usize)?;
                    let bitmap_side = u64::from(bitmap.side());
                    let scale = |v: u32| u64::from(v) * bitmap_side / u64::from(size);
                    let texel = u32::try_from(scale(z) * bitmap_side + scale(x)).ok()?;
                    break bitmap.texel(texel, bits_per_texel(version, self.materials.len()))?;
                }
                MaterialNode::Node(index) => {
                    let children = map.nodes.get(index as usize)?;
                    size /= 2;
                    if size == 0 {
                        return None;
                    }
                    node = children[usize::from(x >= size) + 2 * usize::from(z >= size)];
                    x %= size;
                    z %= size;
                }
            }
        };
        self.materials.get(index as usize).copied()
    }
}

type Reader<'a> = Cursor<&'a [u8]>;

fn le<T>(r: &mut Reader) -> Option<T>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    T::read_le(r).ok()
}

const fn remaining(r: &Reader) -> u64 {
    r.get_ref().len() as u64 - r.position()
}

fn bytes(r: &mut Reader, len: u64) -> Option<Vec<u8>> {
    if len > remaining(r) {
        return None;
    }
    let start = r.position() as usize;
    r.set_position(r.position() + len);
    Some(r.get_ref()[start..start + len as usize].to_vec())
}

/// Reads the contents of a `BMAT` chunk, which have to be used up exactly.
pub(super) fn read_block_materials(data: &[u8], version: u32) -> Option<TileBlockMaterials> {
    let r = &mut Cursor::new(data);
    let x = le(r)?;
    let z = le(r)?;
    let count: u16 = le(r)?;
    let materials = (0..count).map(|_| le(r)).collect::<Option<Vec<u16>>>()?;

    let map = if remaining(r) == 0 {
        None
    } else if version >= TTILE_COMPACT_VERSION {
        Some(read_compact_map(r, materials.len())?)
    } else {
        let tag: [u8; 4] = le(r)?;
        let size = u32::read_be(r).ok()?;
        if &tag != b"QTRE" || u64::from(size) != remaining(r) {
            return None;
        }
        Some(read_legacy_map(r, materials.len())?)
    };

    (remaining(r) == 0).then_some(TileBlockMaterials {
        x,
        z,
        materials,
        map,
    })
}

const COMPACT_MATERIAL: u32 = 0x8000;
const LEGACY_MATERIAL: u32 = 0x8000_0000;

/// Decodes a node, `material` is the flag of materials, the next lower bit that of bitmaps.
const fn decode_node(value: u32, material: u32) -> MaterialNode {
    let bitmap = material >> 1;
    if value & material != 0 {
        MaterialNode::Material(value & !material)
    } else if value & bitmap != 0 {
        MaterialNode::Bitmap(value & !bitmap)
    } else {
        MaterialNode::Node(value)
    }
}

const fn encode_node(node: MaterialNode, material: u32) -> Result<u32, TerrError> {
    let bitmap = material >> 1;
    let (index, flag, limit) = match node {
        MaterialNode::Node(index) => (index, 0, bitmap),
        MaterialNode::Bitmap(index) => (index, bitmap, bitmap),
        MaterialNode::Material(index) => (index, material, material),
    };
    if index < limit {
        Ok(index | flag)
    } else {
        Err(TerrError::Unwritable("TMAT"))
    }
}

fn read_compact_map(r: &mut Reader, materials: usize) -> Option<MaterialMap> {
    let node = |value: u16| decode_node(u32::from(value), COMPACT_MATERIAL);
    let count: u16 = le(r)?;
    let root = node(le(r)?);
    let nodes = (0..count)
        .map(|_| le::<[u16; 4]>(r).map(|children| children.map(node)))
        .collect::<Option<_>>()?;

    let mut bitmaps = Vec::new();
    if remaining(r) > 0 {
        let count: u16 = le(r)?;
        let bits = bits_per_texel(TTILE_COMPACT_VERSION, materials);
        for _ in 0..count {
            let texels: u32 = le(r)?;
            let data = bytes(r, (u64::from(texels) * u64::from(bits)).div_ceil(8))?;
            bitmaps.push(MaterialBitmap {
                rect: [0; 4],
                texels,
                data,
            });
        }
    }

    Some(MaterialMap {
        unknown: 0,
        rect: [0; 4],
        root,
        nodes,
        bitmaps,
    })
}

/// Reads the contents of a `QTRE` chunk.
fn read_legacy_map(r: &mut Reader, materials: usize) -> Option<MaterialMap> {
    let node = |value: u32| decode_node(value, LEGACY_MATERIAL);
    let count: u32 = le(r)?;
    let root = node(le(r)?);
    let unknown = le(r)?;
    let rect = le(r)?;
    let nodes = (0..count)
        .map(|_| le::<[u32; 4]>(r).map(|children| children.map(node)))
        .collect::<Option<_>>()?;

    let count: u32 = le(r)?;
    let mut bitmaps = Vec::new();
    for _ in 0..count {
        let rect = le(r)?;
        // The palette size, always the number of materials.
        let palette: u32 = le(r)?;
        if palette as usize != materials {
            return None;
        }
        let texels = le(r)?;
        let words: u32 = le(r)?;
        let data = bytes(r, u64::from(words) * 4)?;
        bitmaps.push(MaterialBitmap { rect, texels, data });
    }

    Some(MaterialMap {
        unknown,
        rect,
        root,
        nodes,
        bitmaps,
    })
}

/// Writes the contents of a `BMAT` chunk.
pub(super) fn write_block_materials(
    block: &TileBlockMaterials,
    version: u32,
) -> Result<Vec<u8>, AffError> {
    let out = &mut Cursor::new(Vec::new());
    let count = u16::try_from(block.materials.len()).map_err(|_| TerrError::Unwritable("TMAT"))?;
    (block.x, block.z, count).write_le(out)?;
    block.materials.write_le(out)?;

    if let Some(map) = &block.map {
        if version >= TTILE_COMPACT_VERSION {
            write_compact_map(map, block.materials.len(), out)?;
        } else {
            let qtre = &mut Cursor::new(Vec::new());
            write_legacy_map(map, block.materials.len(), qtre)?;
            let qtre = qtre.get_ref();
            b"QTRE".write_le(out)?;
            u32::try_from(qtre.len())
                .map_err(|_| TerrError::TooLarge)?
                .write_be(out)?;
            qtre.write_le(out)?;
        }
    }
    Ok(out.get_ref().clone())
}

fn write_compact_map(
    map: &MaterialMap,
    materials: usize,
    out: &mut Cursor<Vec<u8>>,
) -> Result<(), AffError> {
    let unwritable = || TerrError::Unwritable("TMAT");
    let node = |node| {
        encode_node(node, COMPACT_MATERIAL).map(|v| v as u16) // below 0x10000
    };
    let count = u16::try_from(map.nodes.len()).map_err(|_| unwritable())?;
    (count, node(map.root)?).write_le(out)?;
    for children in &map.nodes {
        for child in children {
            node(*child)?.write_le(out)?;
        }
    }

    if map.bitmaps.is_empty() {
        return Ok(());
    }
    let bits = bits_per_texel(TTILE_COMPACT_VERSION, materials);
    u16::try_from(map.bitmaps.len())
        .map_err(|_| unwritable())?
        .write_le(out)?;
    for bitmap in &map.bitmaps {
        let len = (u64::from(bitmap.texels) * u64::from(bits)).div_ceil(8);
        if bitmap.data.len() as u64 != len {
            return Err(unwritable().into());
        }
        bitmap.texels.write_le(out)?;
        bitmap.data.write_le(out)?;
    }
    Ok(())
}

fn write_legacy_map(
    map: &MaterialMap,
    materials: usize,
    out: &mut Cursor<Vec<u8>>,
) -> Result<(), AffError> {
    let unwritable = || TerrError::Unwritable("TMAT");
    let node = |node| encode_node(node, LEGACY_MATERIAL);
    let count = u32::try_from(map.nodes.len()).map_err(|_| unwritable())?;
    (count, node(map.root)?, map.unknown, map.rect).write_le(out)?;
    for children in &map.nodes {
        for child in children {
            node(*child)?.write_le(out)?;
        }
    }

    let palette = u32::try_from(materials).map_err(|_| unwritable())?;
    u32::try_from(map.bitmaps.len())
        .map_err(|_| unwritable())?
        .write_le(out)?;
    for bitmap in &map.bitmaps {
        if bitmap.data.len() % 4 != 0 {
            return Err(unwritable().into());
        }
        let words = u32::try_from(bitmap.data.len() / 4).map_err(|_| unwritable())?;
        (bitmap.rect, palette, bitmap.texels, words).write_le(out)?;
        bitmap.data.write_le(out)?;
    }
    Ok(())
}
