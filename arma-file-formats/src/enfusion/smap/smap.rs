use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use binrw::{BinRead, BinWrite, binrw, io::NoSeek};

use crate::{
    core::iff::FormHeader,
    errors::{AffError, SmapError},
};

const SMAP_FORM_TYPE: &[u8; 4] = b"SMAP";
/// The only known version of the `FORM` layout.
pub const SMAP_LAYERS_VERSION: u32 = 1;
/// The only known version of a layer, the `VER` chunk of its `HEAD`.
pub const SMAP_LAYER_VERSION: u16 = 1;
/// Id of [`SoundMapGrid::terrain`].
const TERRAIN_LAYER_ID: u32 = 0;
/// Id of [`SoundMapGrid::freshwater`].
const FRESHWATER_LAYER_ID: u32 = 1;
/// Sub-cells along each side of a [`SoundMapGrid`] cell in the freshwater layer.
pub const FRESHWATER_SUBCELLS: usize = 5;

/// Enfusion sound map (`.smap`), see the [module docs](super).
#[derive(Debug, Clone, PartialEq)]
pub enum Smap {
    /// The layout `MapDataExporter` writes, e.g. `Arland.smap`.
    Grid(SoundMapGrid),
    /// The `FORM` layout with extra layers, e.g. `Cain_ext.smap`.
    Layers(SoundMapLayers),
}

impl Smap {
    /// Reads either layout.
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let start = input.stream_position()?;
        let mut magic = [0; 4];
        let is_form = input.read_exact(&mut magic).is_ok() && &magic == b"FORM";
        input.seek(SeekFrom::Start(start))?;
        if is_form {
            SoundMapLayers::from(input).map(Self::Layers)
        } else {
            SoundMapGrid::from(input).map(Self::Grid)
        }
    }

    /// Writes the sound map at the current position of `out`.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        match self {
            Self::Grid(grid) => grid.write(out),
            Self::Layers(layers) => layers.write(out),
        }
    }
}

/// The sound map `MapDataExporter` (`EMapDataType.Soundmap`) writes: a grid of square cells over
/// the bounds of the world, each with a [`TerrainSound`] and, near fresh water, 5 x 5
/// [`FreshwaterSound`]s.
///
/// Both layers store each distinct record once and refer to it by a `u16` index per cell.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, PartialEq)]
pub struct SoundMapGrid {
    #[br(temp)]
    #[bw(try_calc = u32::try_from(cells.len()))]
    count: u32,
    /// Cells along `z`.
    pub cells_z: u32,
    /// Cells along `x`.
    pub cells_x: u32,
    /// Edge length of a cell in meters, `10` in the game's files.
    pub cell_size: f32,
    /// `boundMins` of the world's `GenericWorldEntity`.
    pub min: [f32; 3],
    /// [`SoundMapGrid::min`] plus the cells, at most the `boundMaxs` of the world. `y` is `0`.
    pub max: [f32; 3],
    /// Row by row along `x`, starting at the lowest `z`.
    #[br(count = count)]
    pub cells: Vec<GridCell>,
    #[br(args(TERRAIN_LAYER_ID))]
    #[bw(args(TERRAIN_LAYER_ID))]
    pub terrain: GridLayer<TerrainSound>,
    #[br(args(FRESHWATER_LAYER_ID))]
    #[bw(args(FRESHWATER_LAYER_ID))]
    pub freshwater: GridLayer<FreshwaterSound>,
}

/// A cell of a [`SoundMapGrid`].
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridCell {
    /// Index into [`GridLayer::indices`] of [`SoundMapGrid::terrain`], the index of the cell in
    /// the game's files.
    pub terrain: i32,
    /// `-1`, or the cell's number among the cells with fresh water, counted in cell order. Its
    /// [`FreshwaterSound`]s start at `freshwater * 25` in [`GridLayer::indices`] of
    /// [`SoundMapGrid::freshwater`].
    pub freshwater: i32,
}

/// A layer of a [`SoundMapGrid`]: `u32` record count, `u32` index count, `u32` layer id (`0`
/// terrain, `1` freshwater), the records and a `u16` index per (sub-)cell.
#[binrw]
#[brw(little, import(id: u32))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GridLayer<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()> + 'static,
{
    #[br(temp)]
    #[bw(try_calc = u32::try_from(records.len()))]
    record_count: u32,
    #[br(temp)]
    #[bw(try_calc = u32::try_from(indices.len()))]
    index_count: u32,
    #[br(temp, assert(layer_id == id, "sound map layer {layer_id} instead of {id}"))]
    #[bw(calc = id)]
    layer_id: u32,
    /// The distinct records.
    #[br(count = record_count)]
    pub records: Vec<T>,
    /// Index into [`GridLayer::records`] per (sub-)cell.
    #[br(count = index_count)]
    pub indices: Vec<u16>,
}

impl<T> GridLayer<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()> + Copy + 'static,
{
    /// The record of entry `index` of [`GridLayer::indices`].
    #[must_use]
    pub fn get(&self, index: usize) -> Option<T> {
        let record = *self.indices.get(index)?;
        self.records.get(usize::from(record)).copied()
    }
}

/// A record of the terrain layer, values from `0` to `255`.
///
/// The game reads sea, forest and city with `SoundWorld.GetMapValuesAtPos`, which also returns a
/// meadow value that is not stored, and coast with `GetMapValuesAdvAtPos`.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerrainSound {
    /// `255` over the ocean, `0` on land, blurred with the `SoundWorld`'s `FilterSea`.
    pub sea: u8,
    /// Density of trees, rising with the trees of the cell, blurred with `FilterForest`.
    pub forest: u8,
    /// Density of buildings, blurred with `FilterCity`.
    pub city: u8,
    /// A band along the coastline on both sides, blurred with `FilterCoast`.
    pub coast: u8,
    /// Height of the terrain at the cell center in meters, rounded, `0` below sea level and `255`
    /// from 255 m.
    pub height: u8,
    /// `0` to `2` in the game's files, mostly `0`.
    pub unknown_5: u8,
}

/// A record of the freshwater layer, values from `0` to `255`. The game reads them with
/// `SoundWorld.GetMapValuesAdvAtPos`.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FreshwaterSound {
    /// High in rivers, `0` in lakes.
    pub river: u8,
    /// In rivers only, rising with the slope of the terrain.
    pub river_slope: u8,
    /// In lakes only.
    pub lake: u8,
    /// `0` in the game's files.
    pub unknown_3: u8,
}

impl SoundMapGrid {
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        // Parsing from memory is much faster than from a reader for the millions of cells.
        let start = input.stream_position()?;
        let mut data = Vec::new();
        input.read_to_end(&mut data)?;
        let mut cursor = Cursor::new(&data);
        let grid = Self::read(&mut cursor).map_err(|_| SmapError::InvalidGrid)?;
        input.seek(SeekFrom::Start(start + cursor.position()))?;
        let cells = u64::from(grid.cells_x) * u64::from(grid.cells_z);
        if grid.cells.len() as u64 != cells {
            return Err(SmapError::InvalidGrid.into());
        }
        Ok(grid)
    }

    /// Writes the sound map at the current position of `out`.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let cells = u64::from(self.cells_x) * u64::from(self.cells_z);
        if self.cells.len() as u64 != cells {
            return Err(SmapError::Unwritable("cells").into());
        }
        let mut data = Cursor::new(Vec::new());
        BinWrite::write(self, &mut data).map_err(|_| SmapError::Unwritable("grid"))?;
        out.write_all(&data.into_inner())?;
        Ok(())
    }

    /// Index into [`SoundMapGrid::cells`] of the cell at a world position (`x`, `z`).
    #[must_use]
    pub fn cell_index(&self, [x, z]: [f32; 2]) -> Option<usize> {
        let cell = |v: f32, min: f32, count: u32| {
            let cell = ((v - min) / self.cell_size).floor();
            // Also rejects NaN.
            (cell >= 0.0 && cell < count as f32).then_some(cell as usize)
        };
        let cx = cell(x, self.min[0], self.cells_x)?;
        let cz = cell(z, self.min[2], self.cells_z)?;
        Some(cz * self.cells_x as usize + cx)
    }

    /// The terrain record at a world position (`x`, `z`).
    #[must_use]
    pub fn terrain_at(&self, position: [f32; 2]) -> Option<TerrainSound> {
        let cell = self.cells.get(self.cell_index(position)?)?;
        self.terrain.get(usize::try_from(cell.terrain).ok()?)
    }

    /// The freshwater record of the 2 m sub-cell at a world position (`x`, `z`), `None` away from
    /// fresh water. Sub-cells are stored like the cells, row by row along `x` from the lowest `z`:
    /// of the orders tried, this matches the rivers and lakes of the game's maps best.
    #[must_use]
    pub fn freshwater_at(&self, [x, z]: [f32; 2]) -> Option<FreshwaterSound> {
        let index = self.cell_index([x, z])?;
        let first = usize::try_from(self.cells.get(index)?.freshwater).ok()?;
        let sub_size = self.cell_size / FRESHWATER_SUBCELLS as f32;
        let (cx, cz) = (index % self.cells_x as usize, index / self.cells_x as usize);
        let sub = |v: f32, min: f32, cell: usize| {
            let offset = v - (cell as f32).mul_add(self.cell_size, min);
            ((offset / sub_size) as usize).min(FRESHWATER_SUBCELLS - 1)
        };
        let (sx, sz) = (sub(x, self.min[0], cx), sub(z, self.min[2], cz));
        self.freshwater
            .get(first * FRESHWATER_SUBCELLS * FRESHWATER_SUBCELLS + sz * FRESHWATER_SUBCELLS + sx)
    }
}

/// The `FORM` layout: extra layers of square blocks, each a grid of tiles with a `u8` value.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundMapLayers {
    /// Contents of the `VERS` chunk, [`SMAP_LAYERS_VERSION`].
    pub version: u32,
    /// Lowest `x`, `y` and `z` of the `HEAD` chunk.
    pub min: [f32; 3],
    /// Highest `x`, `y` and `z` of the `HEAD` chunk.
    pub max: [f32; 3],
    /// The last `u32` of the `HEAD` chunk, `1` in `Cain_ext.smap`, which has one layer.
    pub unknown_24: u32,
    /// One per chunk whose tag starts with `L$`, in file order.
    pub layers: Vec<SoundLayer>,
    /// Chunks this reader does not know, in file order. They are written after the known chunks.
    pub unknown_chunks: Vec<SmapChunk>,
}

/// A layer of [`SoundMapLayers`], e.g. the `SoundMapLayerGrass` of a `SoundWorld` (chunk `L$10`)
/// with its `Block` and `Tile` sizes.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundLayer {
    /// The chunk tag, `L$10`.
    pub tag: [u8; 4],
    /// The `VER` chunk at the start of the layer's `HEAD`, [`SMAP_LAYER_VERSION`].
    pub version: u16,
    pub min: [f32; 3],
    pub max: [f32; 3],
    /// Edge length of a block in meters, `256`.
    pub block_size: u32,
    /// Edge length of a tile in meters, `2`.
    pub tile_size: u32,
    /// Blocks along `x`.
    pub blocks_x: u16,
    /// Blocks along `z`.
    pub blocks_z: u16,
    /// The `BLCK` chunks of `BLKS`, in file order, which is not the order of their position.
    pub blocks: Vec<SoundBlock>,
}

/// A `BLCK` chunk: a block of [`SoundBlock::resolution`] x [`SoundBlock::resolution`] tiles.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, PartialEq)]
pub struct SoundBlock {
    /// Column of the block, its `min` is the layer's `min` plus `x * block_size`.
    pub x: u16,
    /// Row of the block.
    pub z: u16,
    pub min: [f32; 3],
    pub max: [f32; 3],
    /// Tiles along each side, `128`.
    pub resolution: u16,
    /// Edge length of a tile in meters, `2`.
    pub tile_size: u16,
    /// Size of the decompressed data: `resolution * resolution`, or `1` if all tiles have the
    /// same value.
    pub size: u32,
    /// Has always been equal to [`SoundBlock::size`].
    pub unknown_36: u32,
    #[br(temp)]
    #[bw(try_calc = u32::try_from(data.len()))]
    compressed_size: u32,
    /// An LZ4 block (no frame) of the values, see [`SoundBlock::values`].
    #[br(count = compressed_size)]
    pub data: Vec<u8>,
}

impl SoundBlock {
    /// The decompressed values, row by row along `x`, starting at the lowest `z`. A uniform block
    /// is expanded to all its tiles.
    pub fn values(&self) -> Result<Vec<u8>, AffError> {
        let size = self.size as usize;
        let values = lz4_flex::block::decompress(&self.data, size)
            .ok()
            .filter(|v| v.len() == size)
            .ok_or(SmapError::Decompression(self.x, self.z))?;
        let tiles = usize::from(self.resolution) * usize::from(self.resolution);
        match values[..] {
            [value] => Ok(vec![value; tiles]),
            _ if values.len() == tiles => Ok(values),
            _ => Err(SmapError::Decompression(self.x, self.z).into()),
        }
    }

    /// Replaces the values, `resolution * resolution` of them, row by row along `x`. A uniform
    /// block is stored as a single value, as the game does.
    pub fn set_values(&mut self, values: &[u8]) -> Result<(), AffError> {
        let tiles = usize::from(self.resolution) * usize::from(self.resolution);
        if values.len() != tiles {
            return Err(SmapError::Unwritable("block values").into());
        }
        let stored = match values {
            [first, rest @ ..] if rest.iter().all(|v| v == first) => &values[..1],
            _ => values,
        };
        self.size = u32::try_from(stored.len()).map_err(|_| SmapError::TooLarge)?;
        self.unknown_36 = self.size;
        self.data = lz4_flex::block::compress(stored);
        Ok(())
    }
}

impl SoundLayer {
    /// The block whose area holds a world position (`x`, `z`).
    #[must_use]
    pub fn block_at(&self, [x, z]: [f32; 2]) -> Option<&SoundBlock> {
        let cell = |v: f32, min: f32| {
            let cell = ((v - min) / self.block_size as f32).floor();
            (cell >= 0.0 && cell < f32::from(u16::MAX)).then_some(cell as u16)
        };
        let (bx, bz) = (cell(x, self.min[0])?, cell(z, self.min[2])?);
        self.blocks.iter().find(|b| b.x == bx && b.z == bz)
    }

    /// The value of the tile at a world position (`x`, `z`), `None` outside the blocks.
    /// Decompresses the block.
    pub fn value_at(&self, [x, z]: [f32; 2]) -> Result<Option<u8>, AffError> {
        let Some(block) = self.block_at([x, z]) else {
            return Ok(None);
        };
        let tile = |v: f32, min: f32| {
            let tile = ((v - min) / f32::from(block.tile_size)).floor();
            (tile >= 0.0 && tile < f32::from(block.resolution)).then_some(tile as usize)
        };
        let origin = |axis: usize, index: u16| {
            f32::from(index).mul_add(self.block_size as f32, self.min[axis])
        };
        let (Some(tx), Some(tz)) = (tile(x, origin(0, block.x)), tile(z, origin(2, block.z)))
        else {
            return Ok(None);
        };
        let values = block.values()?;
        Ok(values.get(tz * usize::from(block.resolution) + tx).copied())
    }
}

/// A chunk of the `FORM`, with a big endian tag and size.
#[binrw]
#[brw(big)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmapChunk {
    pub tag: [u8; 4],
    #[br(temp)]
    #[bw(try_calc = u32::try_from(data.len()))]
    size: u32,
    #[br(count = size)]
    pub data: Vec<u8>,
}

/// Contents of the top `HEAD` chunk.
#[binrw]
#[brw(little)]
struct LayersHead {
    min: [f32; 3],
    max: [f32; 3],
    unknown_24: u32,
}

/// Contents of a layer's `HEAD` chunk after its `VER` chunk.
#[binrw]
#[brw(little)]
struct LayerHead {
    min: [f32; 3],
    max: [f32; 3],
    block_size: u32,
    tile_size: u32,
    blocks_x: u16,
    blocks_z: u16,
}

impl SoundMapLayers {
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let start = input.stream_position()?;
        let form = FormHeader::read_form(input)?;
        if &form.form_type != SMAP_FORM_TYPE {
            return Err(SmapError::UnknownFormType(form.form_type_lossy()).into());
        }
        let form_end = form.end(start);
        let mut chunks = Vec::new();
        while input.stream_position()? < form_end {
            chunks.push(SmapChunk::read(input)?);
        }
        input.seek(SeekFrom::Start(form_end))?;

        let mut required = |tag: &'static str| {
            chunks
                .iter()
                .position(|c| c.tag == tag.as_bytes())
                .map(|i| chunks.remove(i))
                .ok_or(SmapError::ChunkMissing(tag))
        };
        let version: u32 = parse(&required("VERS")?.data, "VERS")?;
        if version != SMAP_LAYERS_VERSION {
            return Err(SmapError::UnsupportedVersion(version).into());
        }
        let head: LayersHead = parse(&required("HEAD")?.data, "HEAD")?;

        let (layers, unknown_chunks): (Vec<_>, Vec<_>) =
            chunks.into_iter().partition(|c| c.tag.starts_with(b"L$"));
        let layers = layers
            .into_iter()
            .map(|chunk| read_layer(&chunk))
            .collect::<Result<_, _>>()?;

        Ok(Self {
            version,
            min: head.min,
            max: head.max,
            unknown_24: head.unknown_24,
            layers,
            unknown_chunks,
        })
    }

    /// Writes the sound map at the current position of `out`.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let head = LayersHead {
            min: self.min,
            max: self.max,
            unknown_24: self.unknown_24,
        };
        let mut chunks = vec![
            SmapChunk {
                tag: *b"VERS",
                data: serialize(&self.version)?,
            },
            SmapChunk {
                tag: *b"HEAD",
                data: serialize(&head)?,
            },
        ];
        for layer in &self.layers {
            chunks.push(write_layer(layer)?);
        }

        let body = serialize_chunks(chunks.iter().chain(&self.unknown_chunks))?;
        FormHeader {
            size: u32::try_from(body.len() + 4).map_err(|_| SmapError::TooLarge)?,
            form_type: *SMAP_FORM_TYPE,
        }
        .write(&mut NoSeek::new(&mut *out))?;
        out.write_all(&body)?;
        Ok(())
    }
}

/// A layer chunk: `HEAD` (a `VER` chunk with a `u16` version, then [`LayerHead`]) and `BLKS`
/// (`BLCK` chunks).
fn read_layer(chunk: &SmapChunk) -> Result<SoundLayer, AffError> {
    let sub_chunks = read_chunks(&chunk.data, "layer")?;
    let [head, blocks] = &sub_chunks[..] else {
        return Err(SmapError::InvalidChunk("layer").into());
    };
    if &head.tag != b"HEAD" || &blocks.tag != b"BLKS" {
        return Err(SmapError::InvalidChunk("layer").into());
    }
    let mut cursor = Cursor::new(&head.data);
    let ver = SmapChunk::read(&mut cursor).map_err(|_| SmapError::InvalidChunk("HEAD"))?;
    let version = match (&ver.tag, &ver.data[..]) {
        (b"VER\0", &[a, b]) => u16::from_le_bytes([a, b]),
        _ => return Err(SmapError::InvalidChunk("HEAD").into()),
    };
    if version != SMAP_LAYER_VERSION {
        return Err(SmapError::UnsupportedVersion(u32::from(version)).into());
    }
    let rest = &head.data[cursor.position() as usize..];
    let layer_head: LayerHead = parse(rest, "HEAD")?;

    let blocks = read_chunks(&blocks.data, "BLKS")?
        .iter()
        .map(|c| {
            if &c.tag != b"BLCK" {
                return Err(SmapError::InvalidChunk("BLKS"));
            }
            parse(&c.data, "BLCK")
        })
        .collect::<Result<_, _>>()?;

    Ok(SoundLayer {
        tag: chunk.tag,
        version,
        min: layer_head.min,
        max: layer_head.max,
        block_size: layer_head.block_size,
        tile_size: layer_head.tile_size,
        blocks_x: layer_head.blocks_x,
        blocks_z: layer_head.blocks_z,
        blocks,
    })
}

fn write_layer(layer: &SoundLayer) -> Result<SmapChunk, AffError> {
    let ver = SmapChunk {
        tag: *b"VER\0",
        data: layer.version.to_le_bytes().to_vec(),
    };
    let layer_head = LayerHead {
        min: layer.min,
        max: layer.max,
        block_size: layer.block_size,
        tile_size: layer.tile_size,
        blocks_x: layer.blocks_x,
        blocks_z: layer.blocks_z,
    };
    let head = [serialize_chunks([&ver])?, serialize(&layer_head)?].concat();
    let blocks = layer
        .blocks
        .iter()
        .map(|b| {
            Ok(SmapChunk {
                tag: *b"BLCK",
                data: serialize(b)?,
            })
        })
        .collect::<Result<Vec<_>, AffError>>()?;
    let sub_chunks = [
        SmapChunk {
            tag: *b"HEAD",
            data: head,
        },
        SmapChunk {
            tag: *b"BLKS",
            data: serialize_chunks(&blocks)?,
        },
    ];
    Ok(SmapChunk {
        tag: layer.tag,
        data: serialize_chunks(&sub_chunks)?,
    })
}

/// Splits `data` into chunks, which have to use it up exactly.
fn read_chunks(data: &[u8], name: &'static str) -> Result<Vec<SmapChunk>, SmapError> {
    let mut cursor = Cursor::new(data);
    let mut chunks = Vec::new();
    while (cursor.position() as usize) < data.len() {
        chunks.push(SmapChunk::read(&mut cursor).map_err(|_| SmapError::InvalidChunk(name))?);
    }
    Ok(chunks)
}

fn serialize_chunks<'a>(
    chunks: impl IntoIterator<Item = &'a SmapChunk>,
) -> Result<Vec<u8>, AffError> {
    let mut out = Cursor::new(Vec::new());
    for chunk in chunks {
        chunk.write(&mut out)?;
    }
    Ok(out.into_inner())
}

/// Parses contents, which have to be used up exactly.
fn parse<T>(data: &[u8], name: &'static str) -> Result<T, SmapError>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    let mut cursor = Cursor::new(data);
    T::read_le(&mut cursor)
        .ok()
        .filter(|_| cursor.position() == data.len() as u64)
        .ok_or(SmapError::InvalidChunk(name))
}

fn serialize<T>(value: &T) -> Result<Vec<u8>, AffError>
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    let mut data = Cursor::new(Vec::new());
    value.write_le(&mut data)?;
    Ok(data.into_inner())
}
