//! Tests for the navmesh (`.nmn`, `.ntile`) readers and writers.
//!
//! The hand built files follow the layout of the engine's loader. `navmesh_game_*` read the
//! game's files in `test-data/navmesh_in/`: the complete `SF-Tutorial` navmesh, the IFF version
//! 2 `CTI_Campaign_Montignac_LowRes.nmn` and single `.ntile` files of other navmeshes (small ones
//! with off-mesh connections, custom links, cover edges, unused edges, empty slots, version 10
//! and stock Detour tiles).

use std::io::Cursor;

use arma_file_formats::{
    enfusion::navmesh::{
        DETOUR_TILE_VERSION, DetourPolyDetail, NAV_TILE_VERSION, NAV_TILE_VERSION_10,
        NMN_IFF_VERSION, NMN_IFF_VERSION_2, NavBvNode, NavCoverPoint, NavCustomLink,
        NavCustomLinkKind, NavMeshParams, NavOffMeshConnection, NavPoly, NavPolyEdge, NavTile,
        NavTileMeta, Nmn, NmnTile, Ntile, NtileV2,
    },
    errors::{AffError, NavmeshError},
};

fn navmesh_error(result: Result<impl std::fmt::Debug, AffError>) -> NavmeshError {
    match result {
        Err(AffError::NavmeshError(e)) => e,
        other => panic!("expected a NavmeshError, got {other:?}"),
    }
}

fn form(form_type: &[u8; 4], chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut body = form_type.to_vec();
    for (tag, data) in chunks {
        body.extend(*tag);
        body.extend((data.len() as u32).to_be_bytes());
        body.extend(data);
    }
    let mut out = b"FORM".to_vec();
    out.extend((body.len() as u32).to_be_bytes());
    out.extend(body);
    out
}

/// Little endian byte builder.
#[derive(Default)]
struct Le(Vec<u8>);

impl Le {
    fn u8(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }
    fn u16(mut self, v: &[u16]) -> Self {
        v.iter().for_each(|v| self.0.extend(v.to_le_bytes()));
        self
    }
    fn i32(mut self, v: &[i32]) -> Self {
        v.iter().for_each(|v| self.0.extend(v.to_le_bytes()));
        self
    }
    fn u64(mut self, v: u64) -> Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn f32(mut self, v: &[f32]) -> Self {
        v.iter().for_each(|v| self.0.extend(v.to_le_bytes()));
        self
    }
    fn bytes(mut self, v: &[u8]) -> Self {
        self.0.extend(v);
        self
    }
}

// ---------------------------------------------------------------------------------------------
// .nmn

fn nmn_bytes(iff_version: i32, path: &[u8], entries: &[[i32; 4]]) -> Vec<u8> {
    let mut table = Le::default();
    for e in entries {
        table = table.i32(e);
    }
    Le::default()
        .bytes(b"EV\x02\x00")
        .i32(&[iff_version])
        .f32(&[10.0, 0.0, -24.0, 24.0, 24.0])
        .i32(&[4, 0x10_0000])
        .u16(&[path.len() as u16, 0x900D])
        .bytes(path)
        .bytes(&form(b"NAVM", &[(b"TFHE", table.0)]))
        .0
}

#[test]
fn nmn_layout_test() {
    let bytes = nmn_bytes(
        3,
        b"",
        &[[-1, 0, 0, 0], [0, 0, 0, 1], [0, 0, 0, 2], [0, 0, 0, 3]],
    );
    let nmn = Nmn::from(&mut Cursor::new(&bytes)).unwrap();
    assert_eq!(nmn.iff_version, NMN_IFF_VERSION);
    assert_eq!(
        nmn.params,
        NavMeshParams {
            origin: [10.0, 0.0, -24.0],
            tile_width: 24.0,
            tile_height: 24.0,
            max_tiles: 4,
            max_polys: 0x10_0000,
        }
    );
    assert_eq!(nmn.path, "");
    assert_eq!(nmn.path_padding, 0x900D);
    assert_eq!(
        nmn.tiles[0],
        NmnTile {
            x: -1,
            y: 0,
            file: 0,
            index: 0
        }
    );
    assert_eq!(nmn.file_count(), 1);
    // The entries after the last tile are 0, 0 too, the first match is the real tile.
    assert_eq!(nmn.tile(0, 0).unwrap().index, 1);
    assert_eq!(nmn.tile(1, 0), None);
    // Tile -1 covers x from -14 to 10.
    assert_eq!(nmn.tile_at(-13.9, -24.0), (-1, 0));
    assert_eq!(nmn.tile_at(10.0, -0.1), (0, 0));

    let mut out = Vec::new();
    nmn.write(&mut out).unwrap();
    assert_eq!(out, bytes);
}

#[test]
fn nmn_path_test() {
    let bytes = nmn_bytes(2, b"worlds/Test", &[[0, 0, 0, 0]]);
    let nmn = Nmn::from(&mut Cursor::new(&bytes)).unwrap();
    assert_eq!(nmn.iff_version, NMN_IFF_VERSION_2);
    assert_eq!(nmn.path, "worlds/Test");
    let mut out = Vec::new();
    nmn.write(&mut out).unwrap();
    assert_eq!(out, bytes);

    let bytes = nmn_bytes(3, &[b'a'; 0x119], &[]);
    assert_eq!(
        navmesh_error(Nmn::from(&mut Cursor::new(&bytes))),
        NavmeshError::PathTooLong(0x119)
    );
}

#[test]
fn nmn_errors_test() {
    let mut bytes = nmn_bytes(3, b"", &[]);
    bytes[0] = b'X';
    assert!(matches!(
        navmesh_error(Nmn::from(&mut Cursor::new(&bytes))),
        NavmeshError::UnknownMagic(_)
    ));
    let bytes = nmn_bytes(4, b"", &[]);
    assert_eq!(
        navmesh_error(Nmn::from(&mut Cursor::new(&bytes))),
        NavmeshError::UnsupportedVersion(4)
    );
    let mut bytes = nmn_bytes(3, b"", &[[0, 0, 0, 0]]);
    let len = bytes.len();
    bytes[len - 24..len - 20].copy_from_slice(b"TIHE");
    assert!(matches!(
        navmesh_error(Nmn::from(&mut Cursor::new(&bytes))),
        NavmeshError::UnexpectedChunk {
            expected: "TFHE",
            ..
        }
    ));
}

#[test]
fn nmn_ntile_path_test() {
    assert_eq!(
        Nmn::ntile_path("worlds/SP/Tutorial/Tutorial_Navmesh_Soldier.nmn", 12),
        "worlds/SP/Tutorial/.NavData/Tutorial_Navmesh_Soldier/navData_12.ntile"
    );
    assert_eq!(
        Nmn::ntile_path("Mesh.nmn", 0),
        ".NavData/Mesh/navData_0.ntile"
    );
}

// ---------------------------------------------------------------------------------------------
// .ntile

/// A version 11 (or 10) tile at 2, 3 with two triangles, an off-mesh connection, one unused
/// edge and 5 vertices (2 bytes of padding), as the engine stores it.
fn tile_bytes(version: i32) -> Vec<u8> {
    let mut t = Le::default()
        .bytes(b"VAND")
        .i32(&[version, 2, 3, 0, 0])
        // polygons, vertices, edges, links, BV nodes, off-mesh connections, off-mesh base
        .i32(&[3, 5, 9, 20, 6, 1, 2])
        .f32(&[1.8, 0.45, 0.2])
        .f32(&[48.0, 10.0, 72.0, 72.0, 30.0, 96.0, 5.0])
        // Edges: triangle 0 1 2 (edge 0-1 is a portal to +x, 2-0 borders polygon 1), triangle
        // 0 2 3, the off-mesh connection 3 4, an unused edge.
        .u16(&[0, 0x8000, 1, 0, 2, 2])
        .u16(&[0, 1, 2, 0, 3, 0])
        .u16(&[3, 0, 4, 0])
        .u16(&[0, 0])
        // Vertices and padding.
        .u16(&[
            0, 0, 0, 65535, 0, 0, 65535, 65535, 65535, 0, 0, 65535, 0, 32768, 0,
        ])
        .u16(&[0])
        // Polygons: first link, first edge, flags, edge count, area and type.
        .i32(&[0, 0])
        .u16(&[1])
        .u8(3)
        .u8(4)
        .i32(&[0, 3])
        .u16(&[1])
        .u8(3)
        .u8(5)
        .i32(&[0, 6])
        .u16(&[4])
        .u8(2)
        .u8(0x41);
    // BV nodes: root, two leaves, three unused.
    t = t
        .u16(&[0, 0, 0, 120, 100, 120])
        .i32(&[-3])
        .u16(&[0, 0, 0, 120, 100, 120])
        .i32(&[0])
        .u16(&[0, 0, 0, 120, 50, 120])
        .i32(&[1])
        .bytes(&[0; 48]);
    // Off-mesh connection.
    t = t
        .f32(&[48.0, 10.0, 96.0, 49.0, 12.0, 97.0, 1.0])
        .u16(&[2])
        .u8(1)
        .u8(0xFF)
        .i32(&[7])
        .f32(&[24.0 / 65535.0, 20.0 / 65535.0, 24.0 / 65535.0]);
    if version == NAV_TILE_VERSION {
        t = t.bytes(b"DNAV");
    }
    t.0
}

/// Meta data with one custom link and one cover edge.
fn meta_bytes() -> Vec<u8> {
    Le::default()
        .i32(&[2, 2, 3])
        .i32(&[1, 36])
        .u64(0)
        .u64(0xF27D_BC48)
        .i32(&[8])
        .bytes(&[1, 2, 3, 4, 5, 6, 7, 8])
        .i32(&[1, 56])
        .f32(&[50.0, 11.0, 80.0, 51.0, 11.0, 80.0])
        .i32(&[1, -1, -1, -1, -1, -1])
        .0
}

fn ntile_bytes(tile: &[u8], meta: &[u8], padding: i32) -> Vec<u8> {
    let table = Le::default()
        .i32(&[
            (tile.len() + 20 * 16) as i32,
            20,
            meta.len() as i32,
            padding,
        ])
        .u64(0)
        .bytes(&[0; 24]);
    let data = [tile, meta].concat();
    form(b"NAVD", &[(b"TIHE", table.0), (b"DATA", data)])
}

#[test]
fn ntile_layout_test() {
    let tile = tile_bytes(NAV_TILE_VERSION);
    assert_eq!(tile.len(), 92 + 9 * 4 + 32 + 3 * 12 + 6 * 16 + 36 + 16);
    let bytes = ntile_bytes(&tile, &meta_bytes(), 0x7FF6);
    let ntile = Ntile::from(&mut Cursor::new(&bytes)).unwrap();
    assert_eq!(ntile.tiles.len(), 2);
    assert!(ntile.tiles[1].is_none());
    let t = ntile.tiles[0].as_ref().unwrap();
    assert_eq!(t.data_size(), tile.len());
    assert_eq!((t.version, t.x, t.y, t.layer, t.user_id), (11, 2, 3, 0, 0));
    assert_eq!((t.max_link_count, t.off_mesh_base), (20, 2));
    assert_eq!(t.walkable_height, 1.8);
    assert_eq!((t.bmin, t.bmax), ([48.0, 10.0, 72.0], [72.0, 30.0, 96.0]));
    assert_eq!(t.bv_quant_factor, 5.0);
    assert_eq!(t.entry_padding, 0x7FF6);

    assert_eq!(t.vertices.len(), 5);
    assert_eq!(t.vertex_position(2), Some([72.0, 30.0, 96.0]));
    let mid = t.vertex_position(4).unwrap();
    assert!((mid[1] - 20.0).abs() < 1e-3);
    assert_eq!(t.vertex_position(5), None);

    assert_eq!(t.polys.len(), 3);
    let p = &t.polys[0];
    assert_eq!(
        (p.flags, p.area(), p.poly_type()),
        (1, 4, NavPoly::TYPE_GROUND)
    );
    assert_eq!(p.edges.len(), 3);
    assert_eq!(p.edges[0].portal_side(), Some(0));
    assert_eq!(p.edges[0].neighbour_poly(), None);
    assert_eq!(p.edges[1].neighbour_poly(), None);
    assert_eq!(p.edges[2].neighbour_poly(), Some(1));
    assert_eq!(t.polys[1].edges[0].neighbour_poly(), Some(0));
    let om = &t.polys[2];
    assert_eq!(om.poly_type(), NavPoly::TYPE_OFF_MESH_CONNECTION);
    assert_eq!(
        om.edges.iter().map(|e| e.vertex).collect::<Vec<_>>(),
        [3, 4]
    );
    assert_eq!(t.unused_edges, [NavPolyEdge::default()]);

    assert_eq!(t.bv_nodes.len(), 6);
    assert_eq!(t.bv_nodes[0].index, -3);
    assert_eq!(t.bv_nodes[5], NavBvNode::default());
    assert_eq!(
        t.off_mesh_connections,
        [NavOffMeshConnection {
            start: [48.0, 10.0, 96.0],
            end: [49.0, 12.0, 97.0],
            radius: 1.0,
            poly: 2,
            flags: 1,
            side: 0xFF,
            user_id: 7,
        }]
    );

    let meta = t.meta.as_ref().unwrap();
    assert_eq!((meta.version, meta.x, meta.y), (2, 2, 3));
    assert_eq!(
        meta.custom_links,
        [NavCustomLink {
            link_type: 0xF27D_BC48,
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        }]
    );
    assert_eq!(meta.custom_links[0].kind(), Some(NavCustomLinkKind::Door));
    assert_eq!(meta.custom_links[0].id(), Some(0x0807_0605_0403_0201));
    assert_eq!(
        meta.cover_points,
        [NavCoverPoint {
            position: [50.0, 11.0, 80.0],
            top: [51.0, 11.0, 80.0],
            poly: 1,
            reserved: [-1; 5],
        }]
    );

    let mut out = Vec::new();
    ntile.write(&mut out).unwrap();
    assert_eq!(out, bytes);
}

#[test]
fn ntile_version_10_test() {
    let tile = tile_bytes(NAV_TILE_VERSION_10);
    let bytes = ntile_bytes(&tile, &[], 0);
    let ntile = Ntile::from(&mut Cursor::new(&bytes)).unwrap();
    let t = ntile.tiles[0].as_ref().unwrap();
    assert_eq!(t.version, NAV_TILE_VERSION_10);
    assert_eq!(t.data_size(), tile.len());
    assert!(t.meta.is_none());
    let mut out = Vec::new();
    ntile.write(&mut out).unwrap();
    assert_eq!(out, bytes);
}

#[test]
fn ntile_build_test() {
    // A tile built in code, read back the same.
    let tile = NavTile {
        version: NAV_TILE_VERSION,
        x: 0,
        y: 0,
        layer: 0,
        user_id: 0,
        max_link_count: 4,
        off_mesh_base: 1,
        walkable_height: 2.0,
        walkable_radius: 0.5,
        walkable_climb: 0.3,
        bmin: [0.0; 3],
        bmax: [24.0, 1.0, 24.0],
        bv_quant_factor: 5.0,
        vertices: vec![[0, 0, 0], [65535, 0, 0], [0, 0, 65535]],
        polys: vec![NavPoly {
            first_link: 0,
            edges: (0..3)
                .map(|vertex| NavPolyEdge {
                    vertex,
                    neighbour: 0,
                })
                .collect(),
            flags: 1,
            area_and_type: 1,
        }],
        unused_edges: vec![],
        bv_nodes: vec![NavBvNode::default(); 2],
        off_mesh_connections: vec![],
        vertex_scale: [24.0 / 65535.0, 1.0 / 65535.0, 24.0 / 65535.0],
        meta: Some(NavTileMeta {
            version: 2,
            x: 0,
            y: 0,
            custom_links: vec![],
            cover_points: vec![],
        }),
        entry_padding: 0,
    };
    let ntile = Ntile {
        tiles: vec![None, Some(tile)],
    };
    let mut out = Vec::new();
    ntile.write(&mut out).unwrap();
    assert_eq!(Ntile::from(&mut Cursor::new(&out)).unwrap(), ntile);
}

#[test]
fn ntile_errors_test() {
    let tile = tile_bytes(NAV_TILE_VERSION);
    let read = |bytes: &[u8]| navmesh_error(Ntile::from(&mut Cursor::new(bytes)));

    let mut bad = tile.clone();
    let len = bad.len();
    bad[len - 4..].copy_from_slice(b"XNAV");
    assert_eq!(
        read(&ntile_bytes(&bad, &[], 0)),
        NavmeshError::InvalidTile("tile end magic")
    );

    let mut bad = tile.clone();
    bad[4] = 9;
    assert_eq!(
        read(&ntile_bytes(&bad, &[], 0)),
        NavmeshError::UnsupportedTileVersion(9)
    );

    // The first edge of polygon 1 (at 92 + 36 + 32 + 12 + 4) must follow polygon 0's edges.
    let mut bad = tile.clone();
    bad[176] = 4;
    assert_eq!(
        read(&ntile_bytes(&bad, &[], 0)),
        NavmeshError::InvalidTile("polygon edges out of order")
    );

    let mut meta = meta_bytes();
    meta[0] = 1;
    assert_eq!(
        read(&ntile_bytes(&tile, &meta, 0)),
        NavmeshError::UnsupportedMetaVersion(1)
    );

    // Data after the last tile.
    let mut bytes = ntile_bytes(&tile, &[], 0);
    let data_size_at = 8 + 4 + 8 + 48 + 4;
    let size = u32::from_be_bytes(bytes[data_size_at..data_size_at + 4].try_into().unwrap());
    bytes[data_size_at..data_size_at + 4].copy_from_slice(&(size + 4).to_be_bytes());
    bytes.extend([0; 4]);
    let form_size = u32::from_be_bytes(bytes[4..8].try_into().unwrap());
    bytes[4..8].copy_from_slice(&(form_size + 4).to_be_bytes());
    assert_eq!(read(&bytes), NavmeshError::TrailingData);

    // An empty slot with something in it.
    let table = Le::default().i32(&[0, 0, 0, 0]).u64(8).0;
    let bytes = form(b"NAVD", &[(b"TIHE", table), (b"DATA", vec![])]);
    assert_eq!(read(&bytes), NavmeshError::InvalidChunk("TIHE"));
}

// ---------------------------------------------------------------------------------------------
// Stock Detour tiles (IFF version 2)

/// A Detour version 7 tile: one quad, two detail triangles, 4 links.
fn detour_tile_bytes() -> Vec<u8> {
    Le::default()
        .bytes(b"VAND")
        .i32(&[DETOUR_TILE_VERSION, 5, 6, 0, 0])
        // polygons, vertices, links, detail meshes, detail vertices, detail triangles, BV
        // nodes, off-mesh connections, off-mesh base
        .i32(&[1, 4, 4, 1, 1, 2, 2, 0, 1])
        .f32(&[3.0, 2.0, 0.2])
        .f32(&[0.0, 0.0, 0.0, 24.0, 5.0, 24.0, 5.0])
        .f32(&[
            0.0, 1.0, 0.0, 24.0, 1.0, 0.0, 24.0, 1.0, 24.0, 0.0, 1.0, 24.0,
        ])
        .i32(&[0])
        .u16(&[0, 1, 2, 3, 0, 0, 0x8000, 0x8002, 0x8004, 0x8006, 0, 0, 2])
        .u8(4)
        .u8(2)
        .bytes(&[0; 4 * 16])
        .i32(&[0, 0])
        .u8(1)
        .u8(2)
        .u16(&[0])
        .f32(&[12.0, 1.5, 12.0])
        .bytes(&[0, 1, 4, 0, 1, 2, 4, 0])
        .u16(&[0, 0, 0, 120, 10, 120])
        .i32(&[0])
        .bytes(&[0; 16])
        .0
}

#[test]
fn ntile_v2_layout_test() {
    let tile = detour_tile_bytes();
    let table = Le::default()
        .i32(&[tile.len() as i32, 0])
        .u64(0)
        .bytes(&[0; 16])
        .0;
    let bytes = form(b"NAVD", &[(b"TIHE", table.clone()), (b"DATA", tile)]);
    assert_eq!(
        navmesh_error(Ntile::from(&mut Cursor::new(&bytes))),
        NavmeshError::DetourTiles
    );
    let ntile = NtileV2::from(&mut Cursor::new(&bytes)).unwrap();
    assert_eq!(ntile.tiles.len(), 2);
    let t = ntile.tiles[0].as_ref().unwrap();
    assert_eq!((t.x, t.y, t.max_link_count), (5, 6, 4));
    assert_eq!(t.vertices[2], [24.0, 1.0, 24.0]);
    assert_eq!(t.polys[0].vertices, [0, 1, 2, 3, 0, 0]);
    assert_eq!(t.polys[0].neighbours[1], 0x8002);
    assert_eq!((t.polys[0].vertex_count, t.polys[0].area_and_type), (4, 2));
    assert_eq!(
        t.detail_meshes,
        [DetourPolyDetail {
            vertex_base: 0,
            triangle_base: 0,
            vertex_count: 1,
            triangle_count: 2,
        }]
    );
    assert_eq!(t.detail_vertices, [[12.0, 1.5, 12.0]]);
    assert_eq!(t.detail_triangles, [[0, 1, 4, 0], [1, 2, 4, 0]]);
    assert_eq!(t.bv_nodes.len(), 2);
    let mut out = Vec::new();
    ntile.write(&mut out).unwrap();
    assert_eq!(out, bytes);

    // Without tiles only the table entry size tells the versions apart.
    let empty = form(b"NAVD", &[(b"TIHE", vec![0; 16 * 100]), (b"DATA", vec![])]);
    assert_eq!(
        navmesh_error(Ntile::from(&mut Cursor::new(&empty))),
        NavmeshError::DetourTiles
    );
    assert_eq!(
        NtileV2::from(&mut Cursor::new(&empty)).unwrap().tiles,
        vec![None; 100]
    );
}

// ---------------------------------------------------------------------------------------------
// The game's files

const GAME_DIR: &str = "./tests/enfusion/test-data/navmesh_in/";

fn game_file(path: &str) -> Vec<u8> {
    std::fs::read(format!("{GAME_DIR}{path}")).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn game_nmn(name: &str) -> Nmn {
    Nmn::from(&mut Cursor::new(game_file(&format!("{name}.nmn")))).unwrap()
}

fn game_ntile(path: &str) -> Ntile {
    Ntile::from(&mut Cursor::new(game_file(&format!(
        ".NavData/{path}.ntile"
    ))))
    .unwrap()
}

fn game_ntile_v2(path: &str) -> NtileV2 {
    NtileV2::from(&mut Cursor::new(game_file(&format!(
        ".NavData/{path}.ntile"
    ))))
    .unwrap()
}

fn files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn navmesh_game_round_trip_test() {
    let mut paths = Vec::new();
    files(GAME_DIR.as_ref(), &mut paths);
    let mut counts = [0; 3];
    for path in paths {
        let bytes = std::fs::read(&path).unwrap();
        let mut out = Vec::new();
        let name = path.display();
        if path.extension().is_some_and(|e| e == "nmn") {
            let nmn = Nmn::from(&mut Cursor::new(&bytes)).unwrap_or_else(|e| panic!("{name}: {e}"));
            nmn.write(&mut out).unwrap();
            counts[0] += 1;
        } else {
            match Ntile::from(&mut Cursor::new(&bytes)) {
                Ok(ntile) => {
                    ntile.write(&mut out).unwrap();
                    counts[1] += 1;
                }
                Err(AffError::NavmeshError(NavmeshError::DetourTiles)) => {
                    let ntile = NtileV2::from(&mut Cursor::new(&bytes))
                        .unwrap_or_else(|e| panic!("{name}: {e}"));
                    ntile.write(&mut out).unwrap();
                    counts[2] += 1;
                }
                Err(e) => panic!("{name}: {e}"),
            }
        }
        assert!(out == bytes, "{name} does not round trip");
    }
    assert_eq!(counts, [2, 5, 3]);
}

#[test]
fn navmesh_game_sf_tutorial_test() {
    let nmn = game_nmn("SF-Tutorial");
    assert_eq!(nmn.iff_version, NMN_IFF_VERSION);
    assert_eq!(nmn.params.origin, [23.98, 0.0, 0.0]);
    assert_eq!((nmn.params.tile_width, nmn.params.max_tiles), (24.0, 100));
    assert_eq!(nmn.path_padding, 0x900D);
    assert_eq!(nmn.file_count(), 1);
    let ntile = game_ntile("SF-Tutorial/navData_0");
    assert_eq!(ntile.tiles.len(), 100);

    // Every entry points at its tile, which lies where the grid puts it.
    for (i, entry) in nmn.tiles.iter().enumerate() {
        assert_eq!((entry.file, entry.index), (0, i as u32));
        let tile = ntile.tiles[i].as_ref().unwrap();
        assert_eq!((tile.x, tile.y), (entry.x, entry.y));
        let x = nmn.params.origin[0] + entry.x as f32 * nmn.params.tile_width;
        let z = nmn.params.origin[2] + entry.y as f32 * nmn.params.tile_height;
        assert!((tile.bmin[0] - x).abs() < 1e-3 && (tile.bmin[2] - z).abs() < 1e-3);
        assert_eq!(nmn.tile_at(x + 1.0, z + 1.0), (entry.x, entry.y));
        assert_eq!(tile.version, NAV_TILE_VERSION);
        assert_eq!(tile.entry_padding, 0x7FF6);
        let polys = tile.polys.len();
        for (v, _) in tile.vertices.iter().enumerate() {
            let p = tile.vertex_position(v).unwrap();
            assert!((0..3).all(|a| p[a] >= tile.bmin[a] - 1e-3 && p[a] <= tile.bmax[a] + 1e-3));
        }
        for edge in tile.polys.iter().flat_map(|p| &p.edges) {
            assert!(usize::from(edge.vertex) < tile.vertices.len());
            assert!(edge.neighbour_poly().is_none_or(|n| usize::from(n) < polys));
        }
        if let Some(meta) = &tile.meta {
            assert_eq!((meta.version, meta.x, meta.y), (2, tile.x, tile.y));
            for point in &meta.cover_points {
                assert!((point.poly as usize) < polys);
                // The obstacle top is above the point the character stands on.
                assert!(point.top[1] > point.position[1]);
                assert_eq!(point.reserved, [-1; 5]);
            }
        }
    }
    assert_eq!(nmn.tile(-1, 0).map(|t| t.index), Some(0));

    let tile = ntile.tiles[0].as_ref().unwrap();
    assert_eq!((tile.x, tile.y), (-1, 0));
    assert_eq!(
        (tile.polys.len(), tile.vertices.len(), tile.bv_nodes.len()),
        (21, 28, 42)
    );
    assert_eq!((tile.max_link_count, tile.off_mesh_base), (96, 21));
    assert!((tile.walkable_height - 1.8).abs() < 1e-6);
    assert!((tile.walkable_radius - 0.45).abs() < 1e-6);
    assert_eq!(tile.bmax, [23.98, 1001.8, 24.0]);
    let meta = tile.meta.as_ref().unwrap();
    assert!(meta.custom_links.is_empty());
    assert_eq!(meta.cover_points.len(), 3);
    let point = meta.cover_points[0];
    assert_eq!(point.poly, 10);
    assert_eq!(point.position, [16.01493, 2.5, 22.575]);
    assert_eq!(point.top, [15.955001, 3.4, 24.075]);
}

#[test]
fn navmesh_game_off_mesh_test() {
    let ntile = game_ntile("GM_Cain/navData_824");
    let tile = ntile
        .iter()
        .find(|t| !t.off_mesh_connections.is_empty())
        .unwrap();
    assert_eq!((tile.x, tile.y), (326, 154));
    let con = tile.off_mesh_connections[0];
    assert_eq!(con.poly as i32, tile.off_mesh_base);
    assert_eq!((con.radius, con.flags, con.side), (1.0, 1, 0xFF));
    let poly = &tile.polys[usize::from(con.poly)];
    assert_eq!(poly.poly_type(), NavPoly::TYPE_OFF_MESH_CONNECTION);
    assert_eq!(poly.area(), 8);
    assert_eq!(poly.edges.len(), 2);
    let start = tile.vertex_position(poly.edges[0].vertex.into()).unwrap();
    assert!((0..3).all(|i| (start[i] - con.start[i]).abs() < 0.02));
    let meta = tile.meta.as_ref().unwrap();
    assert_eq!(meta.custom_links.len(), 1);
    assert_eq!(meta.custom_links[0].link_type, NavCustomLink::LADDER);
    assert_eq!(meta.custom_links[0].kind(), Some(NavCustomLinkKind::Ladder));
    assert_eq!(meta.custom_links[0].id(), Some(0xAB22_61C0_D7AD_9ECB));
    let ntile = game_ntile("GM_Cain_vehicles/navData_988");
    let kinds: Vec<_> = ntile
        .iter()
        .filter_map(|t| t.meta.as_ref())
        .flat_map(|m| m.custom_links.iter().map(NavCustomLink::kind))
        .collect();
    assert_eq!(kinds, [Some(NavCustomLinkKind::Door); 4]);

    let ntile = game_ntile("GM_Cain_vehicles/navData_988");
    let unused: Vec<_> = ntile.iter().flat_map(|t| &t.unused_edges).collect();
    assert_eq!(unused.len(), 2);
    assert!(unused.iter().all(|e| **e == NavPolyEdge::default()));
}

#[test]
fn navmesh_game_partial_and_old_test() {
    // The last file of a navmesh, only 8 of its slots have tiles.
    let ntile = game_ntile("CTI_Campaign_Eden_vehicles/navData_2804");
    assert_eq!((ntile.tiles.len(), ntile.iter().count()), (100, 8));
    assert!(ntile.tiles[8..].iter().all(Option::is_none));

    let ntile = game_ntile("LowResCain/navData_630");
    assert_eq!(ntile.tiles.len(), 1);
    assert_eq!(ntile.iter().next().unwrap().version, NAV_TILE_VERSION_10);
}

#[test]
fn navmesh_game_iff_2_test() {
    let nmn = game_nmn("CTI_Campaign_Montignac_LowRes");
    assert_eq!(nmn.iff_version, NMN_IFF_VERSION_2);
    assert_eq!((nmn.params.tile_width, nmn.params.max_tiles), (51.0, 63001));
    assert_eq!(nmn.tiles.len(), 63001);
    assert_eq!((nmn.tiles[0].x, nmn.tiles[0].y), (29, 78));
    let used = nmn
        .tiles
        .iter()
        .rposition(|t| (t.x, t.y) != (0, 0))
        .unwrap()
        + 1;
    assert_eq!(used, 14652);
    assert!(
        nmn.tiles
            .iter()
            .enumerate()
            .all(|(i, t)| (t.file, t.index) == ((i / 100) as u32, (i % 100) as u32))
    );

    let ntile = game_ntile_v2("CTI_Campaign_Arland_vehicles/navData_1159");
    assert_eq!((ntile.tiles.len(), ntile.iter().count()), (100, 17));
    let tile = ntile.iter().next().unwrap();
    assert_eq!((tile.x, tile.y), (324, 340));
    assert_eq!(tile.bmin, [3888.0, 0.0, 4080.0]);
    assert_eq!(
        tile.polys[0].neighbours[..4],
        [0x8002, 0x8000, 0x8006, 0x8004]
    );

    for path in [
        "CTI_Campaign_Arland_vehicles/navData_1159",
        "Coop_CombatOps_Eden_vehicles/navData_8384",
        "GM_Eden_vehicles/navData_5865",
    ] {
        for tile in game_ntile_v2(path).iter() {
            let ground = tile.polys.len() - tile.off_mesh_connections.len();
            assert_eq!(tile.detail_meshes.len(), ground, "{path}");
            assert_eq!(tile.off_mesh_base as usize, ground, "{path}");
            for d in &tile.detail_meshes {
                assert!(
                    (d.triangle_base + u32::from(d.triangle_count)) as usize
                        <= tile.detail_triangles.len()
                );
            }
        }
    }
    let links: usize = game_ntile_v2("Coop_CombatOps_Eden_vehicles/navData_8384")
        .iter()
        .filter_map(|t| t.meta.as_ref())
        .map(|m| m.custom_links.len())
        .sum();
    assert_eq!(links, 4);
}
