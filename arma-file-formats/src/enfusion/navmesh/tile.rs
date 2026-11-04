use super::io::{Bytes, PutLe, len_i32, len_u32};
use crate::errors::NavmeshError;

/// `DNAV` as a little endian `u32`, Detour's `DT_NAVMESH_MAGIC`: the first 4 bytes of a tile
/// (`VAND` in byte order).
pub const NAV_TILE_MAGIC: u32 = 0x444E_4156;
/// The only tile version the engine reads (Detour's own is 7).
pub const NAV_TILE_VERSION: i32 = 11;
/// The previous version, without [`NAV_TILE_END_MAGIC`]. Still in some of the game's
/// navmeshes, the engine refuses to load them.
pub const NAV_TILE_VERSION_10: i32 = 10;
/// The last 4 bytes of a version 11 tile, `DNAV` in byte order.
pub const NAV_TILE_END_MAGIC: u32 = 0x5641_4E44;

/// Size of a link the engine inserts after the polygons when it loads a tile.
pub const NAV_LINK_SIZE: usize = 16;

const HEADER_SIZE: usize = 92;
const EDGE_SIZE: usize = 4;
const VERTEX_SIZE: usize = 6;
const POLY_SIZE: usize = 12;
pub(super) const BV_NODE_SIZE: usize = 16;
pub(super) const OFF_MESH_CONNECTION_SIZE: usize = 36;

/// One navmesh tile: the engine's modified Detour tile (`dtMeshHeader` and its data) without
/// detail meshes, with quantized vertices and polygon edges in their own array.
///
/// All positions are world coordinates, y is up. Counts that are only the lengths of the
/// arrays here are not stored.
#[derive(Debug, Clone, PartialEq)]
pub struct NavTile {
    /// [`NAV_TILE_VERSION`] or [`NAV_TILE_VERSION_10`].
    pub version: i32,
    /// Tile column, see [`Nmn::tile`](super::Nmn::tile).
    pub x: i32,
    /// Tile row.
    pub y: i32,
    /// Layer of the tile at `x, y`, always `0`.
    pub layer: i32,
    /// Always `0`.
    pub user_id: u32,
    /// Links the engine allocates when it loads the tile.
    pub max_link_count: i32,
    /// Index of the first off-mesh connection polygon, the off-mesh connections are the last
    /// polygons.
    pub off_mesh_base: i32,
    /// The agent height the tile was built for, in meters.
    pub walkable_height: f32,
    /// The agent radius the tile was built for, in meters.
    pub walkable_radius: f32,
    /// The highest step the agent can climb, in meters.
    pub walkable_climb: f32,
    /// Bounds of the tile.
    pub bmin: [f32; 3],
    pub bmax: [f32; 3],
    /// Scale from meters above [`NavTile::bmin`] to [`NavBvNode`] units.
    pub bv_quant_factor: f32,
    /// Quantized positions, see [`NavTile::vertex_position`].
    pub vertices: Vec<[u16; 3]>,
    /// Navigation polygons, then one polygon (two vertices) per off-mesh connection.
    pub polys: Vec<NavPoly>,
    /// Edges after those of the last polygon, `0, 0` in the game's files. About 1 % of the
    /// tiles have some.
    pub unused_edges: Vec<NavPolyEdge>,
    /// Bounding volume tree over the polygons. Like in Detour it usually has room for two nodes
    /// per polygon, the unused nodes at the end are zero.
    pub bv_nodes: Vec<NavBvNode>,
    pub off_mesh_connections: Vec<NavOffMeshConnection>,
    /// Meters per quantization step of [`NavTile::vertices`]: `(bmax - bmin) / 65535`.
    pub vertex_scale: [f32; 3],
    /// Extra data that follows the tile, see [`NavTileMeta`](super::NavTileMeta).
    pub meta: Option<super::NavTileMeta>,
    /// The 4 bytes after the meta size in the `.ntile` tile table. The engine leaves them
    /// uninitialized (`0x7ff6` in most of the game's files, the top of a pointer). Kept for
    /// byte exact round trips, use `0` for new tiles.
    pub entry_padding: u32,
}

/// A polygon of a [`NavTile`], Detour's `dtPoly`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavPoly {
    /// Index of the first link, filled in by the engine when it loads the tile, `0` in files.
    pub first_link: u32,
    /// The corners of the polygon, each with the neighbour across the edge to the next corner.
    pub edges: Vec<NavPolyEdge>,
    /// Polygon flags, defined by the navmesh project (`NavmeshAreaFlagConfig`).
    pub flags: u16,
    /// Area type in the low 6 bits (`NavmeshAreaTypeConfig`), [`NavPoly::poly_type`] in the
    /// high 2 bits.
    pub area_and_type: u8,
}

/// A corner of a [`NavPoly`] and its edge to the next corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NavPolyEdge {
    /// Index into [`NavTile::vertices`].
    pub vertex: u16,
    /// Detour's neighbour encoding: `0` for no neighbour, the polygon index + 1 for a polygon
    /// of the same tile, or [`NavPolyEdge::PORTAL`] with the side of the tile (`0` +x, `2` +z,
    /// `4` -x, `6` -z in Detour) for an edge that continues in the neighbouring tile.
    pub neighbour: u16,
}

/// A node of a tile's bounding volume tree, Detour's `dtBVNode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NavBvNode {
    /// Bounds above the tile's `bmin`, times its `bv_quant_factor`.
    pub bmin: [u16; 3],
    pub bmax: [u16; 3],
    /// Polygon index for a leaf, the negated number of nodes to skip otherwise.
    pub index: i32,
}

/// A link between two points that are not connected by the mesh, Detour's
/// `dtOffMeshConnection`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavOffMeshConnection {
    pub start: [f32; 3],
    pub end: [f32; 3],
    /// Radius around the end points to search for polygons.
    pub radius: f32,
    /// Polygon of this connection in the tile's polygons.
    pub poly: u16,
    /// Detour: bit 0 means bidirectional.
    pub flags: u8,
    /// Detour: side of the tile the end point is in, `0xFF` for this tile.
    pub side: u8,
    pub user_id: u32,
}

impl NavPoly {
    /// Detour's `DT_POLYTYPE_GROUND`.
    pub const TYPE_GROUND: u8 = 0;
    /// Detour's `DT_POLYTYPE_OFFMESH_CONNECTION`.
    pub const TYPE_OFF_MESH_CONNECTION: u8 = 1;

    #[must_use]
    pub const fn area(&self) -> u8 {
        self.area_and_type & 0x3F
    }

    #[must_use]
    pub const fn poly_type(&self) -> u8 {
        self.area_and_type >> 6
    }
}

impl NavPolyEdge {
    /// Detour's `DT_EXT_LINK`.
    pub const PORTAL: u16 = 0x8000;

    /// The polygon of the same tile across the edge.
    #[must_use]
    pub const fn neighbour_poly(&self) -> Option<u16> {
        if self.neighbour == 0 || self.neighbour & Self::PORTAL != 0 {
            None
        } else {
            Some(self.neighbour - 1)
        }
    }

    /// The side of the tile the edge leads out of, if it continues in the neighbouring tile.
    #[must_use]
    pub const fn portal_side(&self) -> Option<u8> {
        if self.neighbour & Self::PORTAL == 0 {
            None
        } else {
            Some((self.neighbour & 0xFF) as u8)
        }
    }
}

/// The counts of a tile header.
struct Counts {
    polys: usize,
    vertices: usize,
    edges: usize,
    bv_nodes: usize,
    off_mesh_connections: usize,
}

impl NavTile {
    /// World position of [`NavTile::vertices`] entry `index`.
    #[must_use]
    pub fn vertex_position(&self, index: usize) -> Option<[f32; 3]> {
        let v = self.vertices.get(index)?;
        Some(std::array::from_fn(|i| {
            f32::from(v[i]).mul_add(self.vertex_scale[i], self.bmin[i])
        }))
    }

    /// Size of the tile in the `.ntile` `DATA` chunk, without its meta data.
    #[must_use]
    pub fn data_size(&self) -> usize {
        HEADER_SIZE
            + self.edge_count() * EDGE_SIZE
            + (self.vertices.len() * VERTEX_SIZE).next_multiple_of(4)
            + self.polys.len() * POLY_SIZE
            + self.bv_nodes.len() * BV_NODE_SIZE
            + self.off_mesh_connections.len() * OFF_MESH_CONNECTION_SIZE
            + self.trailer_size()
    }

    fn edge_count(&self) -> usize {
        self.polys.iter().map(|p| p.edges.len()).sum::<usize>() + self.unused_edges.len()
    }

    const fn trailer_size(&self) -> usize {
        if self.version == NAV_TILE_VERSION_10 {
            12
        } else {
            16
        }
    }

    pub(super) fn parse(data: &[u8]) -> Result<Self, NavmeshError> {
        let mut r = Bytes::new(data);
        if r.u32()? != NAV_TILE_MAGIC {
            return Err(NavmeshError::InvalidTile("tile magic"));
        }
        let version = r.i32()?;
        if version != NAV_TILE_VERSION && version != NAV_TILE_VERSION_10 {
            return Err(NavmeshError::UnsupportedTileVersion(version));
        }
        let x = r.i32()?;
        let y = r.i32()?;
        let layer = r.i32()?;
        let user_id = r.u32()?;
        let polys = r.count(POLY_SIZE)?;
        let vertices = r.count(VERTEX_SIZE)?;
        let edges = r.count(EDGE_SIZE)?;
        let max_link_count = r.i32()?;
        let counts = Counts {
            polys,
            vertices,
            edges,
            bv_nodes: r.count(BV_NODE_SIZE)?,
            off_mesh_connections: r.count(OFF_MESH_CONNECTION_SIZE)?,
        };
        let mut tile = Self {
            version,
            x,
            y,
            layer,
            user_id,
            max_link_count,
            off_mesh_base: r.i32()?,
            walkable_height: r.f32()?,
            walkable_radius: r.f32()?,
            walkable_climb: r.f32()?,
            bmin: r.vec3()?,
            bmax: r.vec3()?,
            bv_quant_factor: r.f32()?,
            vertices: Vec::new(),
            polys: Vec::new(),
            unused_edges: Vec::new(),
            bv_nodes: Vec::new(),
            off_mesh_connections: Vec::new(),
            vertex_scale: [0.0; 3],
            meta: None,
            entry_padding: 0,
        };
        tile.parse_data(&mut r, &counts)?;
        tile.vertex_scale = r.vec3()?;
        if version == NAV_TILE_VERSION && r.u32()? != NAV_TILE_END_MAGIC {
            return Err(NavmeshError::InvalidTile("tile end magic"));
        }
        if r.remaining() != 0 {
            return Err(NavmeshError::InvalidTile("data after the tile"));
        }
        Ok(tile)
    }

    fn parse_data(&mut self, r: &mut Bytes, counts: &Counts) -> Result<(), NavmeshError> {
        let edges = (0..counts.edges)
            .map(|_| {
                Ok(NavPolyEdge {
                    vertex: r.u16()?,
                    neighbour: r.u16()?,
                })
            })
            .collect::<Result<Vec<_>, NavmeshError>>()?;
        self.vertices = (0..counts.vertices)
            .map(|_| Ok([r.u16()?, r.u16()?, r.u16()?]))
            .collect::<Result<_, NavmeshError>>()?;
        if r.take((counts.vertices * VERTEX_SIZE) % 4)?
            .iter()
            .any(|&b| b != 0)
        {
            return Err(NavmeshError::InvalidTile("vertex padding"));
        }

        let mut next_edge = 0;
        self.polys = (0..counts.polys)
            .map(|_| {
                let first_link = r.u32()?;
                let first_edge = r.u32()? as usize;
                let flags = r.u16()?;
                let edge_count = usize::from(r.u8()?);
                let area_and_type = r.u8()?;
                // The engine stores the edges of every polygon after those of the previous one,
                // so they can be kept per polygon.
                if first_edge != next_edge {
                    return Err(NavmeshError::InvalidTile("polygon edges out of order"));
                }
                next_edge += edge_count;
                Ok(NavPoly {
                    first_link,
                    edges: edges
                        .get(first_edge..next_edge)
                        .ok_or(NavmeshError::InvalidTile("polygon edges"))?
                        .to_vec(),
                    flags,
                    area_and_type,
                })
            })
            .collect::<Result<_, NavmeshError>>()?;
        self.unused_edges = edges[next_edge..].to_vec();

        self.bv_nodes = (0..counts.bv_nodes)
            .map(|_| read_bv_node(r))
            .collect::<Result<_, _>>()?;
        self.off_mesh_connections = (0..counts.off_mesh_connections)
            .map(|_| read_off_mesh_connection(r))
            .collect::<Result<_, _>>()?;
        Ok(())
    }

    pub(super) fn write_data(&self, out: &mut Vec<u8>) -> Result<(), NavmeshError> {
        let start = out.len();
        if self.version != NAV_TILE_VERSION && self.version != NAV_TILE_VERSION_10 {
            return Err(NavmeshError::UnsupportedTileVersion(self.version));
        }
        out.put_u32(NAV_TILE_MAGIC);
        out.put_i32(self.version);
        out.put_i32(self.x);
        out.put_i32(self.y);
        out.put_i32(self.layer);
        out.put_u32(self.user_id);
        out.put_i32(len_i32(self.polys.len())?);
        out.put_i32(len_i32(self.vertices.len())?);
        out.put_i32(len_i32(self.edge_count())?);
        out.put_i32(self.max_link_count);
        out.put_i32(len_i32(self.bv_nodes.len())?);
        out.put_i32(len_i32(self.off_mesh_connections.len())?);
        out.put_i32(self.off_mesh_base);
        out.put_f32(self.walkable_height);
        out.put_f32(self.walkable_radius);
        out.put_f32(self.walkable_climb);
        out.put_vec3(self.bmin);
        out.put_vec3(self.bmax);
        out.put_f32(self.bv_quant_factor);

        for edge in self
            .polys
            .iter()
            .flat_map(|p| &p.edges)
            .chain(&self.unused_edges)
        {
            out.put_u16(edge.vertex);
            out.put_u16(edge.neighbour);
        }
        for &c in self.vertices.iter().flatten() {
            out.put_u16(c);
        }
        out.resize(out.len() + (self.vertices.len() * VERTEX_SIZE) % 4, 0);
        let mut first_edge = 0;
        for poly in &self.polys {
            out.put_u32(poly.first_link);
            out.put_u32(len_u32(first_edge)?);
            out.put_u16(poly.flags);
            out.put_u8(
                u8::try_from(poly.edges.len())
                    .map_err(|_| NavmeshError::Unwritable("polygon edges"))?,
            );
            out.put_u8(poly.area_and_type);
            first_edge += poly.edges.len();
        }
        for node in &self.bv_nodes {
            write_bv_node(out, node);
        }
        for con in &self.off_mesh_connections {
            write_off_mesh_connection(out, con);
        }
        out.put_vec3(self.vertex_scale);
        if self.version == NAV_TILE_VERSION {
            out.put_u32(NAV_TILE_END_MAGIC);
        }
        debug_assert_eq!(out.len() - start, self.data_size());
        Ok(())
    }
}

pub(super) fn read_bv_node(r: &mut Bytes) -> Result<NavBvNode, NavmeshError> {
    Ok(NavBvNode {
        bmin: [r.u16()?, r.u16()?, r.u16()?],
        bmax: [r.u16()?, r.u16()?, r.u16()?],
        index: r.i32()?,
    })
}

pub(super) fn write_bv_node(out: &mut Vec<u8>, node: &NavBvNode) {
    for &c in node.bmin.iter().chain(&node.bmax) {
        out.put_u16(c);
    }
    out.put_i32(node.index);
}

pub(super) fn read_off_mesh_connection(
    r: &mut Bytes,
) -> Result<NavOffMeshConnection, NavmeshError> {
    Ok(NavOffMeshConnection {
        start: r.vec3()?,
        end: r.vec3()?,
        radius: r.f32()?,
        poly: r.u16()?,
        flags: r.u8()?,
        side: r.u8()?,
        user_id: r.u32()?,
    })
}

pub(super) fn write_off_mesh_connection(out: &mut Vec<u8>, con: &NavOffMeshConnection) {
    out.put_vec3(con.start);
    out.put_vec3(con.end);
    out.put_f32(con.radius);
    out.put_u16(con.poly);
    out.put_u8(con.flags);
    out.put_u8(con.side);
    out.put_u32(con.user_id);
}
