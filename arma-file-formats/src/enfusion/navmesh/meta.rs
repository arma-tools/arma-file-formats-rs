use super::io::{Bytes, PutLe, len_i32, len_u32};
use crate::errors::NavmeshError;

/// The only meta data version known. The engine warns that cover points won't work for older
/// versions.
pub const NAV_TILE_META_VERSION: u32 = 2;

const LINKS_HEADER_SIZE: usize = 16;
const LINK_HEADER_SIZE: usize = 12;
const COVER_EDGE_SIZE: usize = 48;

/// Extra data of a [`NavTile`](super::NavTile) that Enfusion keeps outside the Detour tile:
/// custom links and the edges AI cover points are searched along.
#[derive(Debug, Clone, PartialEq)]
pub struct NavTileMeta {
    /// [`NAV_TILE_META_VERSION`].
    pub version: u32,
    /// Same as the tile's.
    pub x: i32,
    pub y: i32,
    pub custom_links: Vec<NavCustomLink>,
    pub cover_edges: Vec<NavCoverEdge>,
}

/// A link from a `NavmeshCustomLinkComponent` (doors, jumps, ladders, ...) that touches the
/// tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavCustomLink {
    /// Identifies the link class, the engine looks up the data size of the class with it and
    /// warns about unknown types.
    pub link_type: u64,
    /// Class specific data, 8 bytes for all classes in the game's navmeshes.
    pub data: Vec<u8>,
}

/// An edge of the navmesh along which the engine places AI cover points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavCoverEdge {
    pub start: [f32; 3],
    pub end: [f32; 3],
    /// The polygon of the tile the edge belongs to, the engine groups the edges by it.
    pub poly: u32,
    /// Unknown, `-1` in all of the game's navmeshes.
    pub unknown: [i32; 5],
}

impl NavTileMeta {
    pub(super) fn parse(data: &[u8]) -> Result<Self, NavmeshError> {
        let mut r = Bytes::new(data);
        let version = r.u32()?;
        if version != NAV_TILE_META_VERSION {
            return Err(NavmeshError::UnsupportedMetaVersion(version));
        }
        let x = r.i32()?;
        let y = r.i32()?;

        // Custom links: count, size of this section from the count on, a pointer that the
        // engine sets when loading (0 in files), then the links.
        let links_start = r.position();
        let link_count = r.count(LINK_HEADER_SIZE)?;
        let links_size = r.u32()? as usize;
        if r.u64()? != 0 {
            return Err(NavmeshError::InvalidTile("custom link pointer"));
        }
        let custom_links = (0..link_count)
            .map(|_| {
                let link_type = r.u64()?;
                let size = r.u32()? as usize;
                Ok(NavCustomLink {
                    link_type,
                    data: r.take(size)?.to_vec(),
                })
            })
            .collect::<Result<Vec<_>, NavmeshError>>()?;
        if r.position() - links_start != links_size {
            return Err(NavmeshError::InvalidTile("custom links size"));
        }

        // Cover edges: count, size of this section from the count on, the edges.
        let edges_start = r.position();
        let edge_count = r.count(COVER_EDGE_SIZE)?;
        let edges_size = r.u32()? as usize;
        let cover_edges = (0..edge_count)
            .map(|_| {
                Ok(NavCoverEdge {
                    start: r.vec3()?,
                    end: r.vec3()?,
                    poly: r.u32()?,
                    unknown: [r.i32()?, r.i32()?, r.i32()?, r.i32()?, r.i32()?],
                })
            })
            .collect::<Result<Vec<_>, NavmeshError>>()?;
        if r.position() - edges_start != edges_size {
            return Err(NavmeshError::InvalidTile("cover edges size"));
        }
        if r.remaining() != 0 {
            return Err(NavmeshError::InvalidTile("data after the meta data"));
        }

        Ok(Self {
            version,
            x,
            y,
            custom_links,
            cover_edges,
        })
    }

    pub(super) fn write(&self, out: &mut Vec<u8>) -> Result<(), NavmeshError> {
        out.put_u32(self.version);
        out.put_i32(self.x);
        out.put_i32(self.y);

        let links_size: usize = LINKS_HEADER_SIZE
            + self
                .custom_links
                .iter()
                .map(|l| LINK_HEADER_SIZE + l.data.len())
                .sum::<usize>();
        out.put_i32(len_i32(self.custom_links.len())?);
        out.put_u32(len_u32(links_size)?);
        out.put_u64(0);
        for link in &self.custom_links {
            out.put_u64(link.link_type);
            out.put_u32(len_u32(link.data.len())?);
            out.extend_from_slice(&link.data);
        }

        out.put_i32(len_i32(self.cover_edges.len())?);
        out.put_u32(len_u32(8 + self.cover_edges.len() * COVER_EDGE_SIZE)?);
        for edge in &self.cover_edges {
            out.put_vec3(edge.start);
            out.put_vec3(edge.end);
            out.put_u32(edge.poly);
            edge.unknown.iter().for_each(|&v| out.put_i32(v));
        }
        Ok(())
    }
}
