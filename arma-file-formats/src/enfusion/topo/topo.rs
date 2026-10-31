use std::io::{Read, Write};

use crate::errors::{AffError, TopoError};

/// A point, `x` and `z` in meters.
///
/// Most points are map coordinates, with `z` mirrored: the depth of the terrain minus the world
/// `z`, see [`flip_z`]. [`BuildingPoint`]s and some [`Hill`]s are world coordinates, the other
/// hills are scaled map coordinates.
pub type Point = [f32; 2];

/// Converts between map and world coordinates: `z` is mirrored at the middle of the terrain,
/// `depth` is its extent along `z`, see
/// [`TerrHead::size_meters`](crate::enfusion::terr::TerrHead::size_meters).
#[must_use]
pub fn flip_z([x, z]: Point, depth: f32) -> Point {
    [x, depth - z]
}

/// Enfusion 2D map geometry (`.topo`), see the [module docs](super).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Topo {
    /// `ROAD` chunk.
    pub roads: Roads,
    /// `PWLN` chunk, the power lines of each level of detail. All levels are the same in the
    /// game's files.
    pub power_lines: Vec<Vec<PowerLine>>,
    /// `BULD` chunk, one category per [`BuildingType`] and a fifth one, empty in the game's
    /// files.
    pub buildings: Vec<Buildings>,
    /// `AREA` chunk.
    pub areas: Areas,
    /// `WATR` chunk, the water bodies of each level of detail.
    pub water: Vec<Vec<WaterBody>>,
    /// `HILL` chunk. `None` if the chunk is empty, as in Cain, `Some` with no hills is a count
    /// of `0`.
    pub hills: Option<Vec<Hill>>,
    /// Chunks this reader does not know, in file order. They are written after the known chunks.
    pub unknown_chunks: Vec<TopoChunk>,
}

/// A chunk of the file: tag, version and contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopoChunk {
    pub tag: [u8; 4],
    pub version: u32,
    pub data: Vec<u8>,
}

/// Contents of the `ROAD` chunk.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Roads {
    /// Roads per [`RoadType`]. Usually the count of each type in the first level of detail,
    /// Eden has one road more of type 2 and one less of type 3.
    pub type_counts: [u32; 8],
    /// The roads of each level of detail, from the most detailed. Every level has the same
    /// roads in the same order, sorted by type from the highest. The other levels have fewer
    /// vertices on curves, but a straight road of one segment has three in them.
    pub lods: Vec<Vec<Road>>,
}

/// A road as a strip of quads, one per `RoadEntity` whose `Type` is a [`RoadType`]. Entities
/// of type 255 are left out, connected entities are not merged.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Road {
    /// [`RoadType`], the `Type` of the entity.
    pub road_type: u8,
    /// Four corners per segment: left and right at the start, right and left at the end, from
    /// the first spline point to the last. Left and right as seen in the world, looking along
    /// the road, the mirrored map coordinates swap them. The end of a segment is the start of
    /// the next one and the distance between left and right is the `Width` of the entity.
    pub vertices: Vec<Point>,
    /// Vertex offsets into [`Road::vertices`], multiples of 4 that start a segment. A few roads
    /// have one or two, their meaning is unknown: the strip is continuous there.
    pub unknown: Vec<u32>,
}

/// `RoadType` of the game scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RoadType {
    Runway,
    RoadMain,
    RoadMinor,
    RoadTrail,
    PathTourist,
    PathGeneral,
    Bridge,
    NotARoad,
}

impl RoadType {
    pub const ALL: [Self; 8] = [
        Self::Runway,
        Self::RoadMain,
        Self::RoadMinor,
        Self::RoadTrail,
        Self::PathTourist,
        Self::PathGeneral,
        Self::Bridge,
        Self::NotARoad,
    ];
}

impl Road {
    /// [`Road::road_type`] if it is a known type.
    #[must_use]
    pub fn kind(&self) -> Option<RoadType> {
        RoadType::ALL.get(usize::from(self.road_type)).copied()
    }
}

/// A power line.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PowerLine {
    /// `EMapPowerlineType` of the game scripts: very high, high or low voltage.
    pub power_line_type: u32,
    pub points: Vec<Point>,
}

/// `EMapBuildingType` of the game scripts, the index into [`Topo::buildings`]. In the game's
/// maps the walls are thousands of walls and fences around fields, only Eden has castle
/// footprints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildingType {
    Building,
    CastleBuilding,
    CastleWall,
    Wall,
}

/// A building category.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Buildings {
    /// Footprint polygons, usually 4 points.
    pub footprints: Vec<Vec<Point>>,
    /// The buildings themselves, only in the first category: one per `SCR_DestructibleBuildingEntity`,
    /// about one per two footprints.
    pub points: Vec<BuildingPoint>,
}

/// A building of a [`Buildings`] category.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BuildingPoint {
    /// `0` in the game's files.
    pub unknown: u32,
    /// World coordinates, not mirrored: about the origin of the entity, within a meter.
    pub position: Point,
    /// The forward vector of the entity, `x` and `z`: the sine and cosine of its yaw. Nearly all
    /// have length 1, a few 0.9 to 1.1, likely scaled with their entity.
    pub direction: [f32; 2],
}

/// Contents of the `AREA` chunk.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Areas {
    /// Polygons of each level of detail for `EMapAreaType.AT_TREE`. Empty in the game's files,
    /// the exporter leaves out generator areas by default.
    pub trees: Vec<Vec<Vec<Point>>>,
    /// The same for `EMapAreaType.AT_OTHER`.
    pub other: Vec<Vec<Vec<Point>>>,
    /// Positions of trees, the origins of `Tree` entities. Whether a tree is included depends on
    /// its prefab: Arland has 65909 of its 118060 trees, of 46 of its 86 tree prefabs.
    pub points: Vec<Point>,
}

/// A water body as a triangle list.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WaterBody {
    /// Kind of water body, `0` to `3`, not named in the game scripts. `2` is a `RiverEntity`, `3`
    /// a `LakeGeneratorEntity`. In Eden `1` are large lakes and `0` narrow streams.
    pub water_type: u32,
    /// `0` in the game's files.
    pub unknown: u8,
    pub vertices: Vec<Point>,
    /// Three indices into [`WaterBody::vertices`] per triangle. In the game's files every
    /// vertex is used once, in order.
    pub indices: Vec<u32>,
}

/// A hill top. There are two kinds, told apart by their height:
///
/// - A whole number: what Workbench's exporter writes. The height is the highest 5 m contour
///   around the peak plus 3 m (`58`, `63`, ...). For an isolated round or flat top that is
///   `5 * floor((top - 0.5) / 5) + 3`, a sharp peak or one next to a higher one can get a level
///   more or less. The position is the peak in map coordinates, in units of half a terrain grid
///   cell (meters on the game's 2 m grids), see [`Hill::map_position`]. Hills whose contour is
///   not above the minimum hill height of the export (50 by default) are left out.
/// - A fraction: the exact terrain height, at a position in world coordinates. Only in the
///   game's files, Workbench 1.8 did not write any for its probes.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Hill {
    /// `0` in the game's files and Workbench's exports.
    pub unknown_0: f32,
    /// Height in meters, see [`Hill`].
    pub height: f32,
    /// Map or world coordinates, see [`Hill`].
    pub position: Point,
    /// `0` in the game's files and Workbench's exports.
    pub unknown_16: [f32; 2],
}

impl Hill {
    /// Whether the height is a contour level, see [`Hill`].
    #[must_use]
    pub fn is_contour(&self) -> bool {
        self.height.fract() == 0.0
    }

    /// The position in map coordinates (meters, `z` mirrored, see [`flip_z`]): a contour hill's
    /// position scaled by half the terrain's `grid_cell_size`
    /// ([`TerrHead`](crate::enfusion::terr::TerrHead)), or a fractional hill's world position
    /// mirrored at the terrain's `depth`.
    #[must_use]
    pub fn map_position(&self, grid_cell_size: f32, depth: f32) -> Point {
        if self.is_contour() {
            self.position.map(|v| v * grid_cell_size / 2.0)
        } else {
            flip_z(self.position, depth)
        }
    }
}

pub const TOPO_MAGIC: &[u8; 4] = b"TOPO";
const END_MAGIC: &[u8; 4] = b"_EOF";
const HEADER_SIZE: usize = 20;

/// Known chunks with the only version this reader supports, in file order.
const ROAD: (&str, u32) = ("ROAD", 2);
const PWLN: (&str, u32) = ("PWLN", 1);
const BULD: (&str, u32) = ("BULD", 1);
const AREA: (&str, u32) = ("AREA", 1);
const WATR: (&str, u32) = ("WATR", 1);
const HILL: (&str, u32) = ("HILL", 0);
const TOPO_VERSION: u32 = 1;

fn tag(name: &str) -> [u8; 4] {
    name.as_bytes().try_into().unwrap_or_default()
}

impl Topo {
    /// Reads a topo, the rest of `input` has to be the file.
    pub fn from<I: Read>(input: &mut I) -> Result<Self, AffError> {
        let mut data = Vec::new();
        input.read_to_end(&mut data)?;
        Ok(read_topo(&data)?)
    }

    /// Writes the topo at the current position of `out`.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        out.write_all(&write_topo(self)?)?;
        Ok(())
    }
}

/// Header: tag, `u32` version, `u32` size, `u32` end offset in the file (start of the contents
/// plus size) and `u32` 0. The chunk has to end before `limit`.
fn read_header(data: &[u8], start: usize, limit: usize) -> Result<(TopoChunk, usize), TopoError> {
    let header = data
        .get(start..start + HEADER_SIZE)
        .ok_or(TopoError::Truncated(start))?;
    let u32_at =
        |i: usize| u32::from_le_bytes([header[i], header[i + 1], header[i + 2], header[i + 3]]);
    let tag = [header[0], header[1], header[2], header[3]];
    let (version, size, end) = (u32_at(4), u32_at(8) as usize, u32_at(12) as usize);
    let contents = start + HEADER_SIZE;
    if contents.checked_add(size) != Some(end) || end > limit {
        return Err(TopoError::InvalidHeader(start));
    }
    let chunk = TopoChunk {
        tag,
        version,
        data: data[contents..end].to_vec(),
    };
    Ok((chunk, end))
}

// The closures can't be method paths, `Reader::roads` has a fixed lifetime.
#[allow(clippy::redundant_closure_for_method_calls)]
fn read_topo(data: &[u8]) -> Result<Topo, TopoError> {
    if data.get(..4) != Some(TOPO_MAGIC) {
        return Err(TopoError::UnknownMagic(
            data.get(..4).unwrap_or(data).to_vec(),
        ));
    }
    let (topo, end) = read_header(data, 0, data.len())?;
    if topo.version != TOPO_VERSION {
        return Err(TopoError::UnsupportedVersion {
            chunk: "TOPO",
            version: topo.version,
        });
    }
    if data.get(end..) != Some(END_MAGIC) {
        return Err(TopoError::Truncated(end));
    }

    let mut chunks = Vec::new();
    let mut pos = HEADER_SIZE;
    while pos < end {
        let (chunk, next) = read_header(data, pos, end)?;
        chunks.push(chunk);
        pos = next;
    }

    let mut take = |(name, version): (&'static str, u32)| {
        let index = chunks
            .iter()
            .position(|c| c.tag == tag(name))
            .ok_or(TopoError::ChunkMissing(name))?;
        let chunk = chunks.remove(index);
        if chunk.version != version {
            return Err(TopoError::UnsupportedVersion {
                chunk: name,
                version: chunk.version,
            });
        }
        Ok((chunk.data, name))
    };

    Ok(Topo {
        roads: parse(take(ROAD)?, |r| r.roads())?,
        power_lines: parse(take(PWLN)?, |r| r.power_lines())?,
        buildings: parse(take(BULD)?, |r| r.buildings())?,
        areas: parse(take(AREA)?, |r| r.areas())?,
        water: parse(take(WATR)?, |r| r.water())?,
        hills: match take(HILL)? {
            (data, _) if data.is_empty() => None,
            chunk => Some(parse(chunk, |r| r.hills())?),
        },
        unknown_chunks: chunks,
    })
}

/// Parses the contents of a chunk, which have to be used up exactly.
fn parse<T>(
    (data, name): (Vec<u8>, &'static str),
    read: impl FnOnce(&mut Reader) -> Option<T>,
) -> Result<T, TopoError> {
    let mut reader = Reader {
        data: &data,
        pos: 0,
    };
    read(&mut reader)
        .filter(|_| reader.pos == data.len())
        .ok_or(TopoError::InvalidChunk(name))
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        let bytes = self.data.get(self.pos..self.pos + N)?;
        self.pos += N;
        bytes.try_into().ok()
    }

    fn u8(&mut self) -> Option<u8> {
        self.array::<1>().map(|[b]| b)
    }

    fn u32(&mut self) -> Option<u32> {
        self.array().map(u32::from_le_bytes)
    }

    fn f32(&mut self) -> Option<f32> {
        self.array().map(f32::from_le_bytes)
    }

    fn point(&mut self) -> Option<Point> {
        Some([self.f32()?, self.f32()?])
    }

    fn points(&mut self) -> Option<Vec<Point>> {
        self.list(8, Self::point)
    }

    /// A `u32` count and the items, each at least `min_size` bytes, so a bogus count fails
    /// before allocating.
    fn list<T>(
        &mut self,
        min_size: usize,
        mut read: impl FnMut(&mut Self) -> Option<T>,
    ) -> Option<Vec<T>> {
        let count = self.u32()? as usize;
        if count.checked_mul(min_size)? > self.data.len() - self.pos {
            return None;
        }
        (0..count).map(|_| read(self)).collect()
    }
}

fn write_topo(topo: &Topo) -> Result<Vec<u8>, TopoError> {
    let mut chunks = Vec::new();
    let mut chunk = |(name, version): (&'static str, u32), write: &dyn Fn(&mut Writer)| {
        let mut writer = Writer::default();
        write(&mut writer);
        chunks.push((tag(name), version, writer.finish(name)?));
        Ok::<_, TopoError>(())
    };

    chunk(ROAD, &|w| {
        topo.roads.type_counts.iter().for_each(|&c| w.u32(c));
        w.list(&topo.roads.lods, |w, lod| {
            w.list(lod, |w, road| {
                w.out.push(road.road_type);
                w.points(&road.vertices);
                w.list(&road.unknown, |w, &v| w.u32(v));
            });
        });
    })?;
    chunk(PWLN, &|w| {
        w.list(&topo.power_lines, |w, lod| {
            w.list(lod, |w, line| {
                w.u32(line.power_line_type);
                w.points(&line.points);
            });
        });
    })?;
    chunk(BULD, &|w| {
        w.list(&topo.buildings, |w, category| {
            w.list(&category.footprints, |w, footprint| w.points(footprint));
            w.list(&category.points, |w, point| {
                w.u32(point.unknown);
                w.point(point.position);
                w.point(point.direction);
            });
        });
    })?;
    chunk(AREA, &|w| {
        for lods in [&topo.areas.trees, &topo.areas.other] {
            w.list(lods, |w, lod| w.list(lod, |w, polygon| w.points(polygon)));
        }
        w.points(&topo.areas.points);
    })?;
    chunk(WATR, &|w| {
        w.list(&topo.water, |w, lod| {
            w.list(lod, |w, body| {
                w.u32(body.water_type);
                w.out.push(body.unknown);
                w.points(&body.vertices);
                w.list(&body.indices, |w, &i| w.u32(i));
            });
        });
    })?;
    chunk(HILL, &|w| {
        if let Some(hills) = &topo.hills {
            w.list(hills, |w, hill| {
                w.f32(hill.unknown_0);
                w.f32(hill.height);
                w.point(hill.position);
                w.point(hill.unknown_16);
            });
        }
    })?;

    let known = chunks.iter().map(|(t, v, d)| (*t, *v, d.as_slice()));
    let unknown = topo
        .unknown_chunks
        .iter()
        .map(|c| (c.tag, c.version, c.data.as_slice()));
    let mut out = vec![0; HEADER_SIZE];
    for (tag, version, data) in known.chain(unknown) {
        put_header(&mut out, tag, version, data.len())?;
        out.extend(data);
    }
    let mut header = Vec::with_capacity(HEADER_SIZE);
    put_header(
        &mut header,
        *TOPO_MAGIC,
        TOPO_VERSION,
        out.len() - HEADER_SIZE,
    )?;
    out[..HEADER_SIZE].copy_from_slice(&header);
    out.extend(END_MAGIC);
    Ok(out)
}

/// Appends the header of a chunk of `size` bytes that starts at the end of `out`, the file so
/// far.
fn put_header(out: &mut Vec<u8>, tag: [u8; 4], version: u32, size: usize) -> Result<(), TopoError> {
    let end = u32::try_from(out.len() + HEADER_SIZE + size).map_err(|_| TopoError::TooLarge)?;
    out.extend(tag);
    for value in [version, size as u32, end, 0] {
        out.extend(value.to_le_bytes());
    }
    Ok(())
}

#[derive(Default)]
struct Writer {
    out: Vec<u8>,
    /// Set when a count does not fit a `u32`.
    too_long: bool,
}

impl Writer {
    fn finish(self, name: &'static str) -> Result<Vec<u8>, TopoError> {
        if self.too_long {
            return Err(TopoError::Unwritable(name));
        }
        Ok(self.out)
    }

    fn u32(&mut self, value: u32) {
        self.out.extend(value.to_le_bytes());
    }

    fn f32(&mut self, value: f32) {
        self.out.extend(value.to_le_bytes());
    }

    fn point(&mut self, [x, z]: Point) {
        self.f32(x);
        self.f32(z);
    }

    fn points(&mut self, points: &[Point]) {
        self.list(points, |w, &point| w.point(point));
    }

    fn list<T>(&mut self, items: &[T], mut write: impl FnMut(&mut Self, &T)) {
        let count = u32::try_from(items.len()).unwrap_or_else(|_| {
            self.too_long = true;
            0
        });
        self.u32(count);
        for item in items {
            write(self, item);
        }
    }
}

/// Contents of the known chunks.
impl Reader<'_> {
    fn roads(&mut self) -> Option<Roads> {
        let mut type_counts = [0; 8];
        for count in &mut type_counts {
            *count = self.u32()?;
        }
        let lods = self.list(4, |r| {
            r.list(9, |r| {
                Some(Road {
                    road_type: r.u8()?,
                    vertices: r.points()?,
                    unknown: r.list(4, Self::u32)?,
                })
            })
        })?;
        Some(Roads { type_counts, lods })
    }

    fn power_lines(&mut self) -> Option<Vec<Vec<PowerLine>>> {
        self.list(4, |r| {
            r.list(8, |r| {
                Some(PowerLine {
                    power_line_type: r.u32()?,
                    points: r.points()?,
                })
            })
        })
    }

    fn buildings(&mut self) -> Option<Vec<Buildings>> {
        self.list(8, |r| {
            Some(Buildings {
                footprints: r.list(4, Self::points)?,
                points: r.list(20, |r| {
                    Some(BuildingPoint {
                        unknown: r.u32()?,
                        position: r.point()?,
                        direction: r.point()?,
                    })
                })?,
            })
        })
    }

    fn areas(&mut self) -> Option<Areas> {
        let trees = self.list(4, |r| r.list(4, Self::points))?;
        let other = self.list(4, |r| r.list(4, Self::points))?;
        Some(Areas {
            trees,
            other,
            points: self.points()?,
        })
    }

    fn water(&mut self) -> Option<Vec<Vec<WaterBody>>> {
        self.list(4, |r| {
            r.list(13, |r| {
                Some(WaterBody {
                    water_type: r.u32()?,
                    unknown: r.u8()?,
                    vertices: r.points()?,
                    indices: r.list(4, Self::u32)?,
                })
            })
        })
    }

    fn hills(&mut self) -> Option<Vec<Hill>> {
        self.list(24, |r| {
            Some(Hill {
                unknown_0: r.f32()?,
                height: r.f32()?,
                position: r.point()?,
                unknown_16: r.point()?,
            })
        })
    }
}
