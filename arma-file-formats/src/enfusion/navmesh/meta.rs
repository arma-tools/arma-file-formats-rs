use super::io::{Bytes, PutLe, len_i32, len_u32};
use crate::errors::NavmeshError;

/// The only meta data version known. The engine warns that cover points won't work for older
/// versions.
pub const NAV_TILE_META_VERSION: u32 = 2;

const LINKS_HEADER_SIZE: usize = 16;
const LINK_HEADER_SIZE: usize = 12;
const COVER_POINT_SIZE: usize = 48;

/// Extra data of a [`NavTile`](super::NavTile) that Enfusion keeps outside the Detour tile:
/// custom links and AI cover points.
#[derive(Debug, Clone, PartialEq)]
pub struct NavTileMeta {
    /// [`NAV_TILE_META_VERSION`].
    pub version: u32,
    /// Same as the tile's.
    pub x: i32,
    pub y: i32,
    pub custom_links: Vec<NavCustomLink>,
    /// Cover points Workbench generated for the tile ("Generate Cover Points" of the navmesh
    /// project), in generation order.
    pub cover_points: Vec<NavCoverPoint>,
}

/// A link from a `NavmeshCustomLinkComponent` (door, jump or ladder) that touches the tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavCustomLink {
    /// The link class, see [`NavCustomLink::kind`]. The engine looks up the data size of the
    /// class with it and warns about unknown types.
    pub link_type: u64,
    /// Class specific data. For the three link classes 8 bytes, the link's ID, see
    /// [`NavCustomLink::id`].
    pub data: Vec<u8>,
}

/// The link classes of [`NavCustomLink::link_type`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavCustomLinkKind {
    /// `NavmeshDoorLink`.
    Door,
    /// `NavmeshJumpLink`.
    Jump,
    /// `NavmeshLadderLink`.
    Ladder,
}

/// A spot next to an obstacle where AI can take cover.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavCoverPoint {
    /// Where the character stands: a point on a border edge of [`NavCoverPoint::poly`],
    /// Workbench probes the edges in steps of `CoverLineProbeStep` meters.
    pub position: [f32; 3],
    /// Top of the obstacle that gives the cover, the top of the blocking heightfield cell next
    /// to the position. Its height above the position is the height of the cover.
    pub top: [f32; 3],
    /// The polygon of the tile the point is on, the engine looks the points up by it.
    pub poly: u32,
    /// An `i32` and two 64 bit values Workbench always writes as `-1`, presumably state the
    /// engine fills in at runtime.
    pub reserved: [i32; 5],
}

impl NavCustomLink {
    /// [`NavCustomLink::link_type`] of `NavmeshDoorLink`.
    pub const DOOR: u64 = 0xF27D_BC48;
    /// [`NavCustomLink::link_type`] of `NavmeshJumpLink`.
    pub const JUMP: u64 = 0xE19D_98D0;
    /// [`NavCustomLink::link_type`] of `NavmeshLadderLink`.
    pub const LADDER: u64 = 0x1223_EC00;

    /// The link class, `None` for an unknown type.
    #[must_use]
    pub const fn kind(&self) -> Option<NavCustomLinkKind> {
        match self.link_type {
            Self::DOOR => Some(NavCustomLinkKind::Door),
            Self::JUMP => Some(NavCustomLinkKind::Jump),
            Self::LADDER => Some(NavCustomLinkKind::Ladder),
            _ => None,
        }
    }

    /// The ID of the link, if [`NavCustomLink::data`] has 8 bytes: an FNV-1a 64 hash Workbench
    /// computes from the link class and the link's end points.
    #[must_use]
    pub fn id(&self) -> Option<u64> {
        Some(u64::from_le_bytes(self.data.as_slice().try_into().ok()?))
    }
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

        // Cover points: count, size of this section from the count on, the points.
        let points_start = r.position();
        let point_count = r.count(COVER_POINT_SIZE)?;
        let points_size = r.u32()? as usize;
        let cover_points = (0..point_count)
            .map(|_| {
                Ok(NavCoverPoint {
                    position: r.vec3()?,
                    top: r.vec3()?,
                    poly: r.u32()?,
                    reserved: [r.i32()?, r.i32()?, r.i32()?, r.i32()?, r.i32()?],
                })
            })
            .collect::<Result<Vec<_>, NavmeshError>>()?;
        if r.position() - points_start != points_size {
            return Err(NavmeshError::InvalidTile("cover points size"));
        }
        if r.remaining() != 0 {
            return Err(NavmeshError::InvalidTile("data after the meta data"));
        }

        Ok(Self {
            version,
            x,
            y,
            custom_links,
            cover_points,
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

        out.put_i32(len_i32(self.cover_points.len())?);
        out.put_u32(len_u32(8 + self.cover_points.len() * COVER_POINT_SIZE)?);
        for point in &self.cover_points {
            out.put_vec3(point.position);
            out.put_vec3(point.top);
            out.put_u32(point.poly);
            for &v in &point.reserved {
                out.put_i32(v);
            }
        }
        Ok(())
    }
}
