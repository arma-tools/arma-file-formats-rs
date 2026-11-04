use std::io::{Read, Seek, Write};

use binrw::{BinWrite, io::NoSeek};

use super::{
    NavBvNode, NavOffMeshConnection, NavTileMeta,
    io::{Bytes, PutLe, chunks, len_i32, len_u32, put_chunk, read_form},
    tile::{
        BV_NODE_SIZE, NAV_LINK_SIZE, NAV_TILE_MAGIC, OFF_MESH_CONNECTION_SIZE, read_bv_node,
        read_off_mesh_connection, write_bv_node, write_off_mesh_connection,
    },
};
use crate::{
    core::iff::FormHeader,
    errors::{AffError, NavmeshError},
};

/// Version of the stock Detour tiles in [`NtileV2`] files.
pub const DETOUR_TILE_VERSION: i32 = 7;
/// Detour's `DT_VERTS_PER_POLYGON`.
pub const DETOUR_VERTS_PER_POLYGON: usize = 6;

const NTILE_FORM_TYPE: &[u8; 4] = b"NAVD";
const TILE_ENTRY_SIZE: usize = 16;
const HEADER_SIZE: usize = 100;
const VERTEX_SIZE: usize = 12;
const POLY_SIZE: usize = 32;
const DETAIL_MESH_SIZE: usize = 12;
const DETAIL_TRI_SIZE: usize = 4;

/// Tile file (`.ntile`) of a navmesh whose [`Nmn`](super::Nmn) has IFF version 2.
///
/// A `FORM` `NAVD` like [`Ntile`](super::Ntile), but `TIHE` has 16 bytes per slot (`i32` tile
/// size, `i32` meta data size, `u64` offset) and the tiles are stock Detour tiles. The current
/// engine refuses to load them. The game still has six such navmeshes, and leftover files of
/// this kind in the `.NavData` directories of current navmeshes, past the files their `.nmn`
/// refers to. [`Ntile::from`](super::Ntile::from) fails with
/// [`NavmeshError::DetourTiles`](crate::errors::NavmeshError::DetourTiles) for them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NtileV2 {
    pub tiles: Vec<Option<DetourTile>>,
}

/// A stock Detour tile (`dtMeshHeader` and its data, version [`DETOUR_TILE_VERSION`]) built
/// with 64 bit polygon references.
#[derive(Debug, Clone, PartialEq)]
pub struct DetourTile {
    pub x: i32,
    pub y: i32,
    pub layer: i32,
    pub user_id: u32,
    /// Room for links in the file, which the engine fills in when loading (zero in files).
    pub max_link_count: i32,
    pub off_mesh_base: i32,
    pub walkable_height: f32,
    pub walkable_radius: f32,
    pub walkable_climb: f32,
    pub bmin: [f32; 3],
    pub bmax: [f32; 3],
    pub bv_quant_factor: f32,
    pub vertices: Vec<[f32; 3]>,
    pub polys: Vec<DetourPoly>,
    /// One per navigation polygon (not off-mesh connection polygon).
    pub detail_meshes: Vec<DetourPolyDetail>,
    pub detail_vertices: Vec<[f32; 3]>,
    /// Three vertex indices and edge flags.
    pub detail_triangles: Vec<[u8; 4]>,
    pub bv_nodes: Vec<NavBvNode>,
    pub off_mesh_connections: Vec<NavOffMeshConnection>,
    /// Extra data that follows the tile, see [`NavTileMeta`].
    pub meta: Option<NavTileMeta>,
}

/// Detour's `dtPoly`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetourPoly {
    pub first_link: u32,
    /// The first [`DetourPoly::vertex_count`] are used.
    pub vertices: [u16; DETOUR_VERTS_PER_POLYGON],
    /// Neighbours, encoded like [`NavPolyEdge::neighbour`](super::NavPolyEdge::neighbour).
    pub neighbours: [u16; DETOUR_VERTS_PER_POLYGON],
    pub flags: u16,
    pub vertex_count: u8,
    /// Area in the low 6 bits, type in the high 2 bits.
    pub area_and_type: u8,
}

/// Detour's `dtPolyDetail`: the detail triangles of a polygon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetourPolyDetail {
    /// First vertex in [`DetourTile::detail_vertices`] (the polygon's own vertices are not
    /// repeated there).
    pub vertex_base: u32,
    /// First triangle in [`DetourTile::detail_triangles`].
    pub triangle_base: u32,
    pub vertex_count: u8,
    pub triangle_count: u8,
}

impl NtileV2 {
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let body = read_form(input, *NTILE_FORM_TYPE)?;
        let [table, data] = chunks(&body, ["TIHE", "DATA"])?;
        Ok(Self::parse(table, data)?)
    }

    fn parse(table: &[u8], data: &[u8]) -> Result<Self, NavmeshError> {
        if !table.len().is_multiple_of(TILE_ENTRY_SIZE) {
            return Err(NavmeshError::InvalidChunk("TIHE"));
        }
        let mut t = Bytes::new(table);
        let mut next_offset = 0;
        let mut tiles = Vec::with_capacity(table.len() / TILE_ENTRY_SIZE);
        for _ in 0..table.len() / TILE_ENTRY_SIZE {
            let size = t.i32()?;
            let meta_size = t.i32()?;
            let offset = t.u64()?;
            if size == 0 {
                if (meta_size, offset) != (0, 0) {
                    return Err(NavmeshError::InvalidChunk("TIHE"));
                }
                tiles.push(None);
                continue;
            }
            let (Ok(size), Ok(meta_size), Ok(offset)) = (
                usize::try_from(size),
                usize::try_from(meta_size),
                usize::try_from(offset),
            ) else {
                return Err(NavmeshError::InvalidChunk("TIHE"));
            };
            if offset != next_offset {
                return Err(NavmeshError::InvalidChunk("TIHE"));
            }
            let mut d = Bytes::new(data.get(offset..).ok_or(NavmeshError::Truncated)?);
            let mut tile = DetourTile::parse(d.take(size)?)?;
            if meta_size != 0 {
                tile.meta = Some(NavTileMeta::parse(d.take(meta_size)?)?);
            }
            tiles.push(Some(tile));
            next_offset = offset + size + meta_size;
        }
        if next_offset != data.len() {
            return Err(NavmeshError::TrailingData);
        }
        Ok(Self { tiles })
    }

    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let mut table = Vec::with_capacity(self.tiles.len() * TILE_ENTRY_SIZE);
        let mut data = Vec::new();
        for tile in &self.tiles {
            let Some(tile) = tile else {
                table.resize(table.len() + TILE_ENTRY_SIZE, 0);
                continue;
            };
            let offset = data.len();
            tile.write_data(&mut data)?;
            let size = data.len() - offset;
            if let Some(meta) = &tile.meta {
                meta.write(&mut data)?;
            }
            table.put_i32(len_i32(size)?);
            table.put_i32(len_i32(data.len() - offset - size)?);
            table.put_u64(offset as u64);
        }

        let mut body = Vec::with_capacity(table.len() + data.len() + 16);
        put_chunk(&mut body, *b"TIHE", &table)?;
        put_chunk(&mut body, *b"DATA", &data)?;
        FormHeader {
            size: len_u32(body.len() + 4)?,
            form_type: *NTILE_FORM_TYPE,
        }
        .write(&mut NoSeek::new(&mut *out))?;
        out.write_all(&body)?;
        Ok(())
    }

    /// The tiles of the file that exist.
    pub fn iter(&self) -> impl Iterator<Item = &DetourTile> {
        self.tiles.iter().flatten()
    }
}

impl DetourTile {
    pub(super) fn parse(data: &[u8]) -> Result<Self, NavmeshError> {
        let mut r = Bytes::new(data);
        if r.u32()? != NAV_TILE_MAGIC {
            return Err(NavmeshError::InvalidTile("tile magic"));
        }
        let version = r.i32()?;
        if version != DETOUR_TILE_VERSION {
            return Err(NavmeshError::UnsupportedTileVersion(version));
        }
        let x = r.i32()?;
        let y = r.i32()?;
        let layer = r.i32()?;
        let user_id = r.u32()?;
        let poly_count = r.count(POLY_SIZE)?;
        let vert_count = r.count(VERTEX_SIZE)?;
        let max_link_count = r.i32()?;
        let links =
            usize::try_from(max_link_count).map_err(|_| NavmeshError::InvalidTile("link count"))?;
        let detail_mesh_count = r.count(DETAIL_MESH_SIZE)?;
        let detail_vert_count = r.count(VERTEX_SIZE)?;
        let detail_tri_count = r.count(DETAIL_TRI_SIZE)?;
        let bv_node_count = r.count(BV_NODE_SIZE)?;
        let off_mesh_count = r.count(OFF_MESH_CONNECTION_SIZE)?;
        let off_mesh_base = r.i32()?;
        let walkable_height = r.f32()?;
        let walkable_radius = r.f32()?;
        let walkable_climb = r.f32()?;
        let bmin = r.vec3()?;
        let bmax = r.vec3()?;
        let bv_quant_factor = r.f32()?;

        let vertices = (0..vert_count)
            .map(|_| r.vec3())
            .collect::<Result<_, _>>()?;
        let polys = (0..poly_count)
            .map(|_| read_poly(&mut r))
            .collect::<Result<_, _>>()?;
        let link_bytes = links
            .checked_mul(NAV_LINK_SIZE)
            .ok_or(NavmeshError::Truncated)?;
        if r.take(link_bytes)?.iter().any(|&b| b != 0) {
            return Err(NavmeshError::InvalidTile("links are not zero"));
        }
        let detail_meshes = (0..detail_mesh_count)
            .map(|_| read_detail(&mut r))
            .collect::<Result<_, _>>()?;
        let detail_vertices = (0..detail_vert_count)
            .map(|_| r.vec3())
            .collect::<Result<_, _>>()?;
        let detail_triangles = (0..detail_tri_count)
            .map(|_| r.array())
            .collect::<Result<_, _>>()?;
        let bv_nodes = (0..bv_node_count)
            .map(|_| read_bv_node(&mut r))
            .collect::<Result<_, _>>()?;
        let off_mesh_connections = (0..off_mesh_count)
            .map(|_| read_off_mesh_connection(&mut r))
            .collect::<Result<_, _>>()?;
        if r.remaining() != 0 {
            return Err(NavmeshError::InvalidTile("data after the tile"));
        }

        Ok(Self {
            x,
            y,
            layer,
            user_id,
            max_link_count,
            off_mesh_base,
            walkable_height,
            walkable_radius,
            walkable_climb,
            bmin,
            bmax,
            bv_quant_factor,
            vertices,
            polys,
            detail_meshes,
            detail_vertices,
            detail_triangles,
            bv_nodes,
            off_mesh_connections,
            meta: None,
        })
    }

    pub(super) fn write_data(&self, out: &mut Vec<u8>) -> Result<(), NavmeshError> {
        let links = usize::try_from(self.max_link_count)
            .map_err(|_| NavmeshError::Unwritable("max_link_count"))?;
        let start = out.len();
        out.put_u32(NAV_TILE_MAGIC);
        out.put_i32(DETOUR_TILE_VERSION);
        out.put_i32(self.x);
        out.put_i32(self.y);
        out.put_i32(self.layer);
        out.put_u32(self.user_id);
        out.put_i32(len_i32(self.polys.len())?);
        out.put_i32(len_i32(self.vertices.len())?);
        out.put_i32(self.max_link_count);
        out.put_i32(len_i32(self.detail_meshes.len())?);
        out.put_i32(len_i32(self.detail_vertices.len())?);
        out.put_i32(len_i32(self.detail_triangles.len())?);
        out.put_i32(len_i32(self.bv_nodes.len())?);
        out.put_i32(len_i32(self.off_mesh_connections.len())?);
        out.put_i32(self.off_mesh_base);
        out.put_f32(self.walkable_height);
        out.put_f32(self.walkable_radius);
        out.put_f32(self.walkable_climb);
        out.put_vec3(self.bmin);
        out.put_vec3(self.bmax);
        out.put_f32(self.bv_quant_factor);
        debug_assert_eq!(out.len() - start, HEADER_SIZE);

        for &v in &self.vertices {
            out.put_vec3(v);
        }
        for poly in &self.polys {
            out.put_u32(poly.first_link);
            for &v in poly.vertices.iter().chain(&poly.neighbours) {
                out.put_u16(v);
            }
            out.put_u16(poly.flags);
            out.put_u8(poly.vertex_count);
            out.put_u8(poly.area_and_type);
        }
        out.resize(out.len() + links * NAV_LINK_SIZE, 0);
        for detail in &self.detail_meshes {
            out.put_u32(detail.vertex_base);
            out.put_u32(detail.triangle_base);
            out.put_u8(detail.vertex_count);
            out.put_u8(detail.triangle_count);
            out.put_u16(0);
        }
        for &v in &self.detail_vertices {
            out.put_vec3(v);
        }
        for tri in &self.detail_triangles {
            out.extend_from_slice(tri);
        }
        for node in &self.bv_nodes {
            write_bv_node(out, node);
        }
        for con in &self.off_mesh_connections {
            write_off_mesh_connection(out, con);
        }
        Ok(())
    }
}

fn read_poly(r: &mut Bytes) -> Result<DetourPoly, NavmeshError> {
    let first_link = r.u32()?;
    let mut vertices = [0; DETOUR_VERTS_PER_POLYGON];
    for v in &mut vertices {
        *v = r.u16()?;
    }
    let mut neighbours = [0; DETOUR_VERTS_PER_POLYGON];
    for n in &mut neighbours {
        *n = r.u16()?;
    }
    Ok(DetourPoly {
        first_link,
        vertices,
        neighbours,
        flags: r.u16()?,
        vertex_count: r.u8()?,
        area_and_type: r.u8()?,
    })
}

fn read_detail(r: &mut Bytes) -> Result<DetourPolyDetail, NavmeshError> {
    let detail = DetourPolyDetail {
        vertex_base: r.u32()?,
        triangle_base: r.u32()?,
        vertex_count: r.u8()?,
        triangle_count: r.u8()?,
    };
    if r.u16()? != 0 {
        return Err(NavmeshError::InvalidTile("detail mesh padding"));
    }
    Ok(detail)
}
