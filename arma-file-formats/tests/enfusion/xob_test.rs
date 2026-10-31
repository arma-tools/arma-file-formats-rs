//! Black box tests for the XOB reader.
//!
//! The probes in `xob_probes/` were authored in Blender (`make_probes.py`), exported to FBX and
//! converted to XOB by Arma Reforger Workbench. Workbench also writes the imported mesh as a
//! text `.txo` file, which serves as the expected result for the binary `.xob`.
//! Workbench maps Blender coordinates `(x, y, z)` to `(x, z, y)` and flips `v`.

use std::{fs::File, io::BufReader, path::Path};

use arma_file_formats::enfusion::xob::{
    Xob, XobCollider, XobColliderShape, XobMesh, unpack_vector,
};

const PROBE_PATH_PREFIX: &str = "./tests/enfusion/xob_probes/";

const POSITION_EPS: f32 = 1e-4;
const NORMAL_EPS: f32 = 0.01;
const UV_EPS: f32 = 2e-4;

fn read_probe(name: &str) -> Xob {
    let file = File::open(format!("{PROBE_PATH_PREFIX}{name}.xob")).unwrap();
    Xob::from(&mut BufReader::new(file)).unwrap()
}

fn close(a: &[f32], b: &[f32], eps: f32) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= eps)
}

/// Minimal reader for the parts of Workbench `.txo` files the tests compare against.
mod txo {
    #[derive(Debug, Default)]
    pub struct Mesh {
        pub faces: Vec<Vec<f32>>,
        pub face_verts: Vec<Vec<f32>>,
        pub verts: Vec<Vec<f32>>,
        pub tex_coords: Vec<Vec<f32>>,
        pub normals: Vec<Vec<f32>>,
        pub vertex_colors: Vec<Vec<f32>>,
        /// Name and rows `vertex dx dy dz` of every `$morph`.
        pub morphs: Vec<(String, Vec<Vec<f32>>)>,
    }

    #[derive(Debug, Default)]
    pub struct Lod {
        pub factor: f32,
        pub meshes: Vec<Mesh>,
    }

    /// One face corner: position, normal and the uv of every uv set.
    pub struct Corner {
        pub face: usize,
        pub position: Vec<f32>,
        pub normal: Vec<f32>,
        pub uvs: Vec<Vec<f32>>,
        pub color: Option<Vec<f32>>,
    }

    pub fn parse(text: &str) -> Vec<Lod> {
        let mut lods: Vec<Lod> = Vec::new();
        let mut stack: Vec<String> = Vec::new();
        for line in text.lines().map(str::trim) {
            if let Some(header) = line.strip_suffix('{') {
                let header = header.trim().to_owned();
                if header.starts_with("$lod ") && stack.len() == 1 {
                    lods.push(Lod::default());
                }
                if header.starts_with("$mesh ") {
                    lods.last_mut().unwrap().meshes.push(Mesh::default());
                }
                if let Some(name) = header.strip_prefix("$morph ") {
                    let mesh = lods.last_mut().unwrap().meshes.last_mut().unwrap();
                    mesh.morphs
                        .push((name.trim_matches('"').to_owned(), Vec::new()));
                }
                stack.push(header);
            } else if line == "}" {
                stack.pop();
            } else if let Some(factor) = line.strip_prefix("#lodFactor ") {
                lods.last_mut().unwrap().factor = factor.parse().unwrap();
            } else if !line.is_empty() && !line.starts_with(['#', '$', '"']) {
                let Some(mesh) = lods.last_mut().and_then(|l| l.meshes.last_mut()) else {
                    continue;
                };
                let row = line
                    .split_whitespace()
                    .map(|v| v.parse().unwrap())
                    .collect();
                match stack.last().map(String::as_str) {
                    Some("$faces") => mesh.faces.push(row),
                    Some("$faceVerts") => mesh.face_verts.push(row),
                    Some("$verts") => mesh.verts.push(row),
                    Some("$texCoords") => mesh.tex_coords.push(row),
                    Some("$normals") => mesh.normals.push(row),
                    Some("$vertexColors") => mesh.vertex_colors.push(row),
                    Some(h) if h.starts_with("$morph ") => {
                        mesh.morphs.last_mut().unwrap().1.push(row);
                    }
                    _ => {}
                }
            }
        }
        lods
    }

    impl Mesh {
        pub fn corners(&self, face_offset: usize) -> Vec<Corner> {
            let mut corners = Vec::new();
            for (face, row) in self.faces.iter().enumerate() {
                let count = row[1] as usize;
                for &corner in &row[2..2 + count] {
                    let fv = &self.face_verts[corner as usize];
                    let has_colors = !self.vertex_colors.is_empty();
                    let uv_end = if has_colors { fv.len() - 1 } else { fv.len() };
                    corners.push(Corner {
                        face: face_offset + face,
                        position: self.verts[fv[0] as usize][..3].to_vec(),
                        normal: self.normals[fv[1] as usize].clone(),
                        uvs: fv[2..uv_end]
                            .iter()
                            .map(|&uv| self.tex_coords[uv as usize].clone())
                            .collect(),
                        color: has_colors
                            .then(|| self.vertex_colors[fv[fv.len() - 1] as usize].clone()),
                    });
                }
            }
            corners
        }

        pub fn triangle_count(&self) -> usize {
            self.faces.iter().map(|f| f[1] as usize - 2).sum()
        }
    }

    pub fn read(name: &str) -> Vec<Lod> {
        parse(&std::fs::read_to_string(format!("{}{name}.txo", super::PROBE_PATH_PREFIX)).unwrap())
    }
}

/// Checks every triangle of every section against the face corners of the Workbench TXO export.
fn assert_matches_txo(name: &str) {
    let xob = read_probe(name);
    let txo = txo::read(name);

    assert_eq!(xob.lods.len(), txo.len(), "{name}: lod count");
    for (lod_index, ((lod, header), txo_lod)) in
        xob.lods.iter().zip(&xob.head.lods).zip(&txo).enumerate()
    {
        assert!(
            (header.lod_factor - txo_lod.factor).abs() < 1e-6,
            "{name}: lod factor"
        );

        let mut face_offset = 0;
        let corners: Vec<_> = txo_lod
            .meshes
            .iter()
            .flat_map(|m| {
                let c = m.corners(face_offset);
                face_offset += m.faces.len();
                c
            })
            .collect();

        let triangles: usize = header
            .sections
            .iter()
            .map(|s| s.triangle_count as usize)
            .sum();
        let txo_triangles: usize = txo_lod.meshes.iter().map(txo::Mesh::triangle_count).sum();
        assert_eq!(
            triangles, txo_triangles,
            "{name} lod {lod_index}: triangle count"
        );

        for (mesh, section) in lod.sections.iter().zip(&header.sections) {
            assert_eq!(mesh.uvs.len(), section.uv_sets.len());
            for triangle in mesh.indices.chunks(3) {
                let faces: Vec<Vec<usize>> = triangle
                    .iter()
                    .map(|&vertex| matching_faces(mesh, usize::from(vertex), &corners))
                    .collect();
                assert!(
                    faces[0]
                        .iter()
                        .any(|f| faces[1].contains(f) && faces[2].contains(f)),
                    "{name} lod {lod_index}: triangle {triangle:?} does not lie on a TXO face"
                );
            }
        }
    }
}

fn matching_faces(mesh: &XobMesh, vertex: usize, corners: &[txo::Corner]) -> Vec<usize> {
    corners
        .iter()
        .filter(|c| {
            close(&mesh.positions[vertex], &c.position, POSITION_EPS)
                && close(&mesh.normals[vertex], &c.normal, NORMAL_EPS)
                && c.uvs
                    .iter()
                    .zip(&mesh.uvs)
                    .all(|(uv, set)| close(&set[vertex], uv, UV_EPS))
                && c.color.as_ref().is_none_or(|color| {
                    let rgba: Vec<f32> = mesh.colors[vertex]
                        .iter()
                        .map(|&v| f32::from(v) / 255.0)
                        .collect();
                    close(&rgba, color, 1.0 / 255.0)
                })
        })
        .map(|c| c.face)
        .collect()
}

/// Structural invariants that must hold for every section of every XOB.
fn assert_consistent(xob: &Xob) {
    for (lod, header) in xob.lods.iter().zip(&xob.head.lods) {
        assert_eq!(lod.sections.len(), header.sections.len());
        for (mesh, section) in lod.sections.iter().zip(&header.sections) {
            let vertices = usize::from(section.vertex_count);
            let tangent_vertices = usize::from(section.tangent_vertex_count);
            assert_eq!(mesh.indices.len(), section.triangle_count as usize * 3);
            assert_eq!(mesh.tangent_indices.len(), mesh.indices.len());
            assert_eq!(mesh.positions.len(), vertices);
            assert_eq!(mesh.normals.len(), vertices);
            assert!(mesh.indices.iter().all(|&i| usize::from(i) < vertices));
            assert!(
                mesh.tangent_indices
                    .iter()
                    .all(|&i| usize::from(i) < tangent_vertices)
            );
            assert!(
                mesh.tangent_vertex_map
                    .iter()
                    .all(|&i| (i as usize) < vertices)
            );

            // Morphs: one block per table entry, known morphs, sorted vertices that move.
            assert_eq!(mesh.morphs.len(), section.morphs.len());
            for (morph, entry) in mesh.morphs.iter().zip(&section.morphs) {
                assert_eq!(morph.morph, entry.morph);
                assert!((morph.morph as usize) < xob.head.morphs.len());
                assert_eq!(morph.vertices.len(), entry.vertex_count as usize);
                assert!(morph.vertices.windows(2).all(|w| w[0].vertex < w[1].vertex));
                assert!(
                    morph
                        .vertices
                        .iter()
                        .all(|v| (v.vertex as usize) < vertices)
                );
                assert!(
                    morph
                        .vertices
                        .iter()
                        .all(|v| v.offset.iter().any(|c| *c != 0.0))
                );
            }

            let size: Vec<f32> = (0..3)
                .map(|a| section.bbox_max[a] - section.bbox_min[a])
                .collect();
            let eps = 1e-3 + 1e-4 * size.iter().fold(0.0_f32, |m, v| m.max(v.abs()));
            for p in &mesh.positions {
                for ((v, min), max) in p.iter().zip(section.bbox_min).zip(section.bbox_max) {
                    assert!(*v >= min - eps && *v <= max + eps);
                }
            }
            for n in &mesh.normals {
                let length = n.iter().map(|c| c * c).sum::<f32>();
                assert!(length == 0.0 || (length - 1.0).abs() < 0.02, "normal {n:?}");
            }
            assert_eq!(
                oriented_triangles(mesh.indices.iter().map(|&i| u32::from(i))),
                oriented_triangles(
                    mesh.tangent_indices
                        .iter()
                        .map(|&i| mesh.tangent_vertex_map[usize::from(i)])
                ),
                "tangent triangles differ from vertex triangles"
            );
            if section.is_skinned() {
                assert_eq!(
                    mesh.skin_weights.len(),
                    usize::from(section.skin_weight_count)
                );
                assert!(
                    mesh.skin_weight_indices
                        .iter()
                        .all(|&i| (i as usize) < mesh.skin_weights.len())
                );
                for weight in &mesh.skin_weights {
                    assert!((weight.weights.iter().sum::<f32>() - 1.0).abs() < 1e-3);
                    assert!(
                        weight
                            .bones
                            .iter()
                            .all(|&b| b == 0 || usize::from(b) < section.bone_palette.len())
                    );
                }
            }
        }
    }

    if let Some(grid) = &xob.occupancy {
        let cells: usize = grid.dimensions.iter().map(|&d| usize::from(d)).product();
        assert_eq!(grid.cells.len(), cells.div_ceil(8));
    }

    let string = |index: usize| xob.head.strings[index].as_str();
    for material in &xob.head.materials {
        assert_eq!(string(usize::from(material.name_index)), material.name);
        assert_eq!(string(usize::from(material.path_index)), material.path);
    }
    for bone in &xob.head.bones {
        assert_eq!(string(bone.name_index as usize), bone.name);
    }

    for collider in &xob.colliders {
        assert_eq!(string(usize::from(collider.name_index)), collider.name);
        assert_eq!(string(usize::from(collider.layer_index)), collider.layer);
        assert_eq!(
            string(usize::from(collider.material_index)),
            collider.material
        );
        if let XobColliderShape::TriMesh(mesh) = &collider.shape {
            for range in &mesh.material_ranges {
                assert_eq!(string(usize::from(range.material_index)), range.material);
            }
        }
        assert_eq!(collider.unknown_52, 0, "{}", collider.name);
        assert!(
            collider
                .bone
                .is_none_or(|b| usize::from(b) < xob.head.bones.len())
        );
        for axis in &collider.rotation {
            assert!((axis.iter().map(|c| c * c).sum::<f32>() - 1.0).abs() < 1e-3);
        }
        match &collider.shape {
            XobColliderShape::Convex(hull) => {
                let (vertices, faces) = (hull.vertices.len(), hull.faces.len());
                assert_eq!(hull.face_edges.len(), hull.edges.len() * 2);
                assert!(hull.edges.iter().all(|e| {
                    e.vertices.iter().all(|&v| usize::from(v) < vertices)
                        && e.faces.iter().all(|&f| usize::from(f) < faces)
                }));
                let mut next = 0;
                for face in &hull.faces {
                    assert_eq!(face.first_edge, next);
                    next += face.edge_count;
                    for &edge in &hull.face_edges[usize::from(face.first_edge)..usize::from(next)] {
                        assert!(usize::from(edge) < hull.edges.len());
                    }
                }
                assert_eq!(usize::from(next), hull.face_edges.len());
            }
            XobColliderShape::TriMesh(mesh) => {
                assert!(
                    mesh.triangles
                        .iter()
                        .flatten()
                        .all(|&v| usize::from(v) < mesh.vertices.len())
                );
                let last: Vec<_> = mesh
                    .material_ranges
                    .iter()
                    .map(|r| r.last_triangle)
                    .collect();
                assert!(last.is_sorted());
                if let Some(&end) = last.last() {
                    assert_eq!(usize::from(end) + 1, mesh.triangles.len());
                }
            }
            _ => {}
        }
    }
}

/// Sorted triangles, each rotated to start at its smallest index so the winding is kept.
fn oriented_triangles(indices: impl Iterator<Item = u32>) -> Vec<[u32; 3]> {
    let indices: Vec<u32> = indices.collect();
    let mut triangles: Vec<[u32; 3]> = indices
        .chunks(3)
        .map(|t| {
            let start = (0..3).min_by_key(|&i| t[i]).unwrap();
            [t[start], t[(start + 1) % 3], t[(start + 2) % 3]]
        })
        .collect();
    triangles.sort_unstable();
    triangles
}

#[test]
fn xob_occupancy_test() {
    // flat models have no volume grid
    assert!(read_probe("quad").occupancy.is_none());

    // closed 1 x 3 x 2 box: the 4 x 4 x 4 grid marks the shell, the 2 x 2 x 2 interior is empty
    let grid = read_probe("box").occupancy.unwrap();
    assert_eq!(grid.dimensions, [4, 4, 4]);
    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                let interior = [x, y, z].iter().all(|c| (1..3).contains(c));
                assert_eq!(grid.is_occupied(x, y, z), !interior, "cell {x} {y} {z}");
            }
        }
    }

    // every cell the authored triangle passes through is marked
    let xob = read_probe("tri");
    let grid = xob.occupancy.as_ref().unwrap();
    let (min, max) = (xob.head.bbox_min, xob.head.bbox_max);
    let cell = grid.cell_size(min, max);
    let [a, b, c]: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 3.0, 2.0]];
    let steps = 50;
    for i in 0..=steps {
        for j in 0..=steps - i {
            let (u, v) = (i as f32 / steps as f32, j as f32 / steps as f32);
            let index = [0, 1, 2].map(|k| {
                let p = (c[k] - a[k]).mul_add(v, (b[k] - a[k]).mul_add(u, a[k]));
                (((p - min[k]) / cell[k]) as usize).min(usize::from(grid.dimensions[k]) - 1)
            });
            assert!(grid.is_occupied(index[0], index[1], index[2]));
        }
    }

    // 10 x 10 m grid with 1 m cells, the flat axis is padded to the minimum of 4 cells
    assert_eq!(
        read_probe("biggrid").occupancy.unwrap().dimensions,
        [10, 4, 10]
    );
}

#[test]
fn xob_colliders_test() {
    let xob = read_probe("colliders");
    assert_consistent(&xob);
    let collider =
        |name: &str| -> &XobCollider { xob.colliders.iter().find(|c| c.name == name).unwrap() };
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let assert_rotation = |c: &XobCollider, rows: [[f32; 3]; 3]| {
        for (axis, expected) in c.rotation.iter().zip(rows) {
            assert!(close(axis, &expected, 1e-4), "{}: {:?}", c.name, c.rotation);
        }
    };
    assert!(xob.colliders.iter().all(|c| c.bone.is_none()));

    // 1 x 2 x 3 box at Blender (2, 0, 0), rotated 30 degrees about Blender z (engine y)
    let box_ = collider("UBX_box");
    assert_eq!(
        box_.shape,
        XobColliderShape::Box {
            half_extents: [0.5, 1.5, 1.0]
        }
    );
    assert!(close(&box_.position, &[2.0, 0.0, 0.0], POSITION_EPS));
    let (sin, cos) = 30_f32.to_radians().sin_cos();
    assert_rotation(box_, [[cos, 0.0, sin], [0.0, 1.0, 0.0], [-sin, 0.0, cos]]);
    assert!((box_.margin - 0.25).abs() < 1e-6);
    assert_eq!(box_.layer, "");
    assert!(box_.material.ends_with("material/metal.gamemat"));

    // radius 0.5 at Blender (0, 3, 0), layer preset and margin set in the meta file
    let sphere = collider("USP_sphere");
    assert_eq!(sphere.shape, XobColliderShape::Sphere { radius: 0.5 });
    assert!(close(&sphere.position, &[0.0, 0.0, 3.0], POSITION_EPS));
    assert_eq!(sphere.layer, "Prop");
    assert!((sphere.margin - 0.04).abs() < 1e-6);

    // radius 0.3, length 1.2 along Blender z, which is the local y axis
    let cylinder = collider("UCL_cylinder");
    let XobColliderShape::Cylinder {
        radius,
        half_length,
    } = cylinder.shape
    else {
        panic!("{:?}", cylinder.shape);
    };
    assert!(close(&[radius, half_length], &[0.3, 0.6], 1e-6));
    assert!(close(&cylinder.position, &[-2.0, 0.0, 0.0], POSITION_EPS));
    assert_rotation(cylinder, identity);

    // radius 0.25 with a 1.0 long straight part, laid along Blender x
    let capsule = collider("UCS_capsule");
    let XobColliderShape::Capsule {
        radius,
        half_length,
    } = capsule.shape
    else {
        panic!("{:?}", capsule.shape);
    };
    assert!(close(&[radius, half_length], &[0.25, 0.5], 1e-6));
    assert!(close(&capsule.position, &[0.0, 0.0, -3.0], POSITION_EPS));
    assert!(close(
        &capsule.rotation[1].map(f32::abs),
        &[1.0, 0.0, 0.0],
        1e-4
    ));

    // square pyramid, base at Blender z = 2, apex at z = 3; vertices are relative to the centroid
    let pyramid = collider("UCX_pyramid");
    let XobColliderShape::Convex(hull) = &pyramid.shape else {
        panic!("{:?}", pyramid.shape);
    };
    assert_rotation(pyramid, identity);
    assert_eq!(
        (hull.vertices.len(), hull.edges.len(), hull.faces.len()),
        (5, 8, 5)
    );
    let mut edge_counts: Vec<_> = hull.faces.iter().map(|f| f.edge_count).collect();
    edge_counts.sort_unstable();
    assert_eq!(edge_counts, [3, 3, 3, 3, 4]);
    let heights: Vec<f32> = hull
        .vertices
        .iter()
        .map(|v| v[1] + pyramid.position[1])
        .collect();
    assert_eq!(
        heights
            .iter()
            .filter(|&&y| (y - 2.0).abs() < POSITION_EPS)
            .count(),
        4
    );
    assert_eq!(
        heights
            .iter()
            .filter(|&&y| (y - 3.0).abs() < POSITION_EPS)
            .count(),
        1
    );

    // both UTM_ objects are merged into one mesh: the quad (2 triangles, metal) and the
    // strip with ColA on one face and ColB on two (2 + 4 triangles)
    let tri_mesh = collider("UTM_quad");
    let XobColliderShape::TriMesh(mesh) = &tri_mesh.shape else {
        panic!("{:?}", tri_mesh.shape);
    };
    assert_eq!(mesh.triangles.len(), 8);
    let ranges: Vec<_> = mesh
        .material_ranges
        .iter()
        .map(|r| (r.material.rsplit('}').next().unwrap(), r.last_triangle))
        .collect();
    assert_eq!(
        ranges,
        [
            ("material/metal.gamemat", 1),
            ("ColA.gamemat", 3),
            ("ColB.gamemat", 7)
        ]
    );
    for triangle in &mesh.triangles[..2] {
        assert!(
            triangle
                .iter()
                .all(|&v| (mesh.vertices[usize::from(v)][1] + 2.0).abs() < POSITION_EPS)
        );
    }
}

#[test]
fn xob_unpack_vector_test() {
    assert_eq!(unpack_vector(0x7FEF_FC00), [0.0, 0.0, -1.0]);
    assert_eq!(unpack_vector(0x7FEF_FFFE), [0.0, 0.0, 1.0]);
    assert_eq!(unpack_vector(0x7FFF_F9FF), [0.0, 1.0, 0.0]);
    assert_eq!(unpack_vector(0x7FE0_01FF), [0.0, -1.0, 0.0]);
    assert_eq!(unpack_vector(0xFFCF_FDFF), [1.0, 0.0, 0.0]);
    assert_eq!(unpack_vector(0x000F_FDFF), [-1.0, 0.0, 0.0]);
}

#[test]
fn xob_txo_oracle_test() {
    for name in [
        "tri", "quad", "colors", "uv2", "box", "box2mat", "lods", "skinned",
    ] {
        assert_matches_txo(name);
        assert_consistent(&read_probe(name));
    }
}

#[test]
fn xob_triangle_test() {
    let xob = read_probe("tri");
    assert_eq!(xob.head.materials.len(), 1);
    assert_eq!(xob.head.materials[0].name, "TriMat");
    assert!(
        xob.head.materials[0]
            .path
            .ends_with("Assets/Probes/Data/TriMat.emat")
    );
    assert_eq!(xob.head.lods.len(), 1);

    let mesh = &xob.lods[0].sections[0];
    assert_eq!(mesh.indices.len(), 3);

    // authored in Blender as (0,0,0), (1,0,0), (0,2,3) with uvs (0,0), (1,0), (0,1)
    let authored = [
        ([0.0, 0.0, 0.0], [0.0, 1.0]),
        ([1.0, 0.0, 0.0], [1.0, 1.0]),
        ([0.0, 3.0, 2.0], [0.0, 0.0]),
    ];
    for (position, uv) in authored {
        let vertex = mesh
            .positions
            .iter()
            .position(|p| close(p, &position, POSITION_EPS))
            .unwrap_or_else(|| panic!("vertex {position:?} missing"));
        assert!(close(&mesh.uvs[0][vertex], &uv, UV_EPS));
    }

    let normal = [0.0, 0.5547, -0.8321];
    assert!(mesh.normals.iter().all(|n| close(n, &normal, NORMAL_EPS)));
    assert!(close(&xob.head.bbox_min, &[0.0, 0.0, 0.0], POSITION_EPS));
    assert!(close(&xob.head.bbox_max, &[1.0, 3.0, 2.0], POSITION_EPS));
}

#[test]
fn xob_uv_bounds_test() {
    let xob = read_probe("quad");
    let uv_set = xob.head.lods[0].sections[0].uv_sets[0];
    // authored uvs span u 0.25..0.75 and v 0.25..0.5, v is flipped on import
    assert!(close(
        &[uv_set.u_min, uv_set.u_max, uv_set.v_min, uv_set.v_max],
        &[0.25, 0.75, 0.5, 0.75],
        UV_EPS
    ));

    let mesh = &xob.lods[0].sections[0];
    for uv in &mesh.uvs[0] {
        assert!([0.25, 0.75].iter().any(|u| (uv[0] - u).abs() < UV_EPS));
        assert!([0.5, 0.75].iter().any(|v| (uv[1] - v).abs() < UV_EPS));
    }
}

#[test]
fn xob_vertex_color_test() {
    let xob = read_probe("colors");
    let section = &xob.head.lods[0].sections[0];
    assert!(section.has_colors());

    let mesh = &xob.lods[0].sections[0];
    // authored per corner in Blender, positions are (x, z, y) of the Blender quad
    let authored = [
        ([-1.0, -1.0, 0.0], [255, 0, 0, 255]),
        ([1.0, -1.0, 0.0], [0, 255, 0, 255]),
        ([1.0, 1.0, 0.0], [0, 0, 255, 255]),
        ([-1.0, 1.0, 0.0], [255, 255, 255, 128]),
    ];
    for (position, color) in authored {
        let vertex = mesh
            .positions
            .iter()
            .position(|p| close(p, &position, POSITION_EPS))
            .unwrap();
        assert_eq!(mesh.colors[vertex], color);
    }
}

#[test]
fn xob_second_uv_set_test() {
    let xob = read_probe("uv2");
    let section = &xob.head.lods[0].sections[0];
    assert_eq!(section.uv_sets.len(), 2);

    let mesh = &xob.lods[0].sections[0];
    assert_eq!(mesh.uvs.len(), 2);
    assert_eq!(mesh.tangent_frames.len(), 2);
    // second set authored as (0.5, 0.5)..(0.75, 0.75)
    for uv in &mesh.uvs[1] {
        assert!((0.5 - UV_EPS..=0.75 + UV_EPS).contains(&uv[0]));
        assert!((0.25 - UV_EPS..=0.5 + UV_EPS).contains(&uv[1]));
    }
}

#[test]
fn xob_materials_test() {
    let xob = read_probe("box2mat");
    let names: Vec<_> = xob.head.materials.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["MatA", "MatB"]);

    let sections = &xob.head.lods[0].sections;
    assert_eq!(sections.len(), 2);
    let mut materials: Vec<_> = sections.iter().map(|s| s.material_index).collect();
    materials.sort_unstable();
    assert_eq!(materials, [0, 1]);
    // three quads per material
    assert!(sections.iter().all(|s| s.triangle_count == 6));
}

#[test]
fn xob_box_test() {
    let xob = read_probe("box");
    // 1 x 2 x 3 box in Blender, y and z swap on import
    assert!(close(&xob.head.bbox_min, &[-0.5, -1.5, -1.0], POSITION_EPS));
    assert!(close(&xob.head.bbox_max, &[0.5, 1.5, 1.0], POSITION_EPS));
    assert!((xob.head.radius - (0.25_f32 + 2.25 + 1.0).sqrt()).abs() < 1e-4);

    let mesh = &xob.lods[0].sections[0];
    assert_eq!(mesh.positions.len(), 24);
    for (position, normal) in mesh.positions.iter().zip(&mesh.normals) {
        // flat shaded: the normal points along the axis on which the vertex lies on the face
        let axis = normal.iter().position(|c| c.abs() > 0.99).unwrap();
        assert!((position[axis] - xob.head.bbox_max[axis] * normal[axis]).abs() < POSITION_EPS);
    }
}

#[test]
fn xob_lods_test() {
    let xob = read_probe("lods");
    let lods = &xob.head.lods;
    assert_eq!(lods.len(), 3);
    let factors: Vec<_> = lods.iter().map(|l| l.lod_factor).collect();
    assert!(close(&factors, &[0.5, 0.15, 0.02], 1e-6));
    // icospheres with subdivision 3, 2, 1
    let triangles: Vec<_> = lods.iter().map(|l| l.sections[0].triangle_count).collect();
    assert_eq!(triangles, [320, 80, 20]);

    for lod in &xob.lods {
        for position in &lod.sections[0].positions {
            let radius = position.iter().map(|c| c * c).sum::<f32>().sqrt();
            assert!((radius - 1.0).abs() < 1e-3);
        }
    }
}

#[test]
fn xob_skinned_test() {
    let xob = read_probe("skinned");
    let bones: Vec<_> = xob.head.bones.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(bones, ["Scene_Root", "bone_root", "bone_tip"]);
    // Scene_Root -> bone_root -> bone_tip
    assert_eq!(xob.head.bones[0].first_child, Some(1));
    assert_eq!(xob.head.bones[1].first_child, Some(2));
    assert_eq!(xob.head.bones[2].first_child, None);
    assert!(xob.head.bones.iter().all(|b| b.next_sibling.is_none()));

    let section = &xob.head.lods[0].sections[0];
    assert!(section.is_skinned());
    let mesh = &xob.lods[0].sections[0];

    // bottom ring is weighted to bone_root, top ring to bone_tip
    for (position, &weight_index) in mesh.positions.iter().zip(&mesh.skin_weight_indices) {
        let weight = mesh.skin_weights[weight_index as usize];
        let bone = section.bone_palette[usize::from(weight.bones[0])];
        let expected = if position[1] < 1.0 {
            "bone_root"
        } else {
            "bone_tip"
        };
        assert_eq!(xob.head.bones[usize::from(bone)].name, expected);
        assert!((weight.weights[0] - 1.0).abs() < 1e-6);
    }
}

#[test]
fn xob_large_mesh_test() {
    let xob = read_probe("biggrid");
    assert_consistent(&xob);

    // 257 x 257 vertices exceed the u16 index range, so the grid is split into sections
    let sections = &xob.head.lods[0].sections;
    assert_eq!(sections.len(), 2);
    let triangles: u32 = sections.iter().map(|s| s.triangle_count).sum();
    assert_eq!(triangles, 2 * 256 * 256);
    assert!(
        sections
            .iter()
            .any(|s| s.triangle_count > u32::from(u16::MAX))
    );

    for mesh in &xob.lods[0].sections {
        assert!(
            mesh.normals
                .iter()
                .all(|n| close(n, &[0.0, 1.0, 0.0], NORMAL_EPS))
        );
        assert!(
            mesh.positions
                .iter()
                .all(|p| p[0].abs() <= 5.0 + POSITION_EPS && p[2].abs() <= 5.0 + POSITION_EPS)
        );
    }
}

/// Morph targets (`make_morph_probes.py`, imported with `ExportMorphs 1`): every vertex at a
/// position the TXO morph moves carries its offset, no other vertex does.
#[test]
fn xob_morphs_test() {
    // Probe, morph names and the `(morph, vertex count)` table of each section.
    type Probe = (
        &'static str,
        &'static [&'static str],
        &'static [&'static [(u32, u32)]],
    );
    let probes: [Probe; 3] = [
        (
            "morphs",
            &["up", "skew", "scale"],
            &[&[(0, 1), (1, 3), (2, 8)]],
        ),
        (
            "morphs_smooth",
            &["bulge", "squash"],
            &[&[(1, 16), (0, 5)], &[(1, 16)]],
        ),
        ("morphs_skinned", &["wide"], &[&[(0, 24)]]),
    ];
    for (name, morph_names, tables) in probes {
        let xob = read_probe(name);
        let names: Vec<_> = xob.head.morphs.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, morph_names, "{name}");

        let txo = txo::read(name);
        let txo_mesh = &txo[0].meshes[0];
        let header = &xob.head.lods[0];
        assert_eq!(header.sections.len(), tables.len(), "{name}");
        for ((section, mesh), table) in header
            .sections
            .iter()
            .zip(&xob.lods[0].sections)
            .zip(tables.iter())
        {
            let pairs: Vec<_> = section
                .morphs
                .iter()
                .map(|m| (m.morph, m.vertex_count))
                .collect();
            assert_eq!(pairs, *table, "{name}");
            assert_eq!(
                mesh.morphs.iter().map(|m| m.morph).collect::<Vec<_>>(),
                table.iter().map(|&(m, _)| m).collect::<Vec<_>>()
            );

            for morph in &mesh.morphs {
                let morph_name = &xob.head.morphs[morph.morph as usize].name;
                let (_, rows) = txo_mesh
                    .morphs
                    .iter()
                    .find(|(n, _)| n == morph_name)
                    .unwrap();
                // Offset of the TXO vertex at a position, if the morph moves it.
                let offset_at = |position: &[f32; 3]| {
                    rows.iter()
                        .find(|row| {
                            close(
                                &txo_mesh.verts[row[0] as usize][..3],
                                position,
                                POSITION_EPS,
                            )
                        })
                        .map(|row| &row[1..4])
                };
                assert!(morph.vertices.windows(2).all(|w| w[0].vertex < w[1].vertex));
                for (vertex, position) in mesh.positions.iter().enumerate() {
                    let stored = morph.vertices.iter().find(|v| v.vertex as usize == vertex);
                    match (offset_at(position), stored) {
                        (Some(expected), Some(stored)) => {
                            assert!(
                                close(&stored.offset, expected, POSITION_EPS),
                                "{name} {morph_name} {vertex}: {stored:?} {expected:?}"
                            );
                            // What Workbench writes for Blender's shape keys.
                            let doubled = mesh.normals[vertex].map(|c| (-2.0 * c).clamp(-1.0, 1.0));
                            assert!(close(&stored.normal(), &doubled, 2.0 * NORMAL_EPS));
                        }
                        (None, None) => {}
                        other => panic!("{name} {morph_name} {vertex} {position:?}: {other:?}"),
                    }
                }
            }
        }
    }
}

/// Parses every `.xob` below `AFF_XOB_SCAN_DIR`, e.g. assets extracted from the game paks.
#[test]
fn xob_scan_dir_test() {
    let Ok(dir) = std::env::var("AFF_XOB_SCAN_DIR") else {
        return;
    };

    fn visit(path: &Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(path).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, files);
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("xob"))
            {
                files.push(path);
            }
        }
    }

    let mut files = Vec::new();
    visit(Path::new(&dir), &mut files);
    assert!(!files.is_empty(), "no xob files in {dir}");

    let failures: Vec<_> = files
        .iter()
        .filter_map(|path| {
            let result = std::panic::catch_unwind(|| {
                let xob = Xob::from(&mut BufReader::new(File::open(path).unwrap())).unwrap();
                assert_consistent(&xob);
            });
            result.is_err().then(|| path.display().to_string())
        })
        .collect();
    println!(
        "parsed {} xob files, {} failures",
        files.len(),
        failures.len()
    );
    assert!(failures.is_empty(), "failed: {failures:#?}");
}
