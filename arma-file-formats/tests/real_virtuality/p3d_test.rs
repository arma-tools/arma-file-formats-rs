use std::fs;

use arma_file_formats::real_virtuality::p3d::ODOL;
use serial_test::serial;

const INPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/p3d_in/";
#[allow(dead_code)]
const OUTPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/p3d_out/";

#[test]
#[serial]
fn aa_p3d() {
    let odol = ODOL::from_path(format!("{}APC_Tracked_01_aa_F.p3d", INPUT_PATH_PREFIX)).unwrap();
    println!("{:#?}", odol.use_defaults);
    println!("{:#?}", odol.face_defaults);
    println!("{:#?}", odol.resolutions);
    // println!("{:#?}", odol.lods[0].sections);
    // println!("{:#?}", odol.lods[0].named_selection);
    println!("{:#?}", odol.lods[0].named_properties);
    println!("{:#?}", odol.lods[0].frames);
    println!("{:#?}", odol.lods[0].icon_color);
    println!("{:#?}", odol.lods[0].selected_color);
    println!("{:#?}", odol.lods[0].special);
    println!("{:#?}", odol.lods[0].vertex_bone_ref_is_simple);
    println!("{:#?}", odol.lods[0].size_of_rest_data);
    println!("{:#?}", odol.lods[0].default_uv_set.max_v);

    println!("{:#?}", odol.lods[0].uv_sets.last().unwrap().max_v);
    println!("{:#?}", odol.lods[0].vertices.last());
    println!("Normals Count: {:#?}", odol.lods[0].normals.len());
    println!("Normals: {:#?}", odol.lods[0].normals.last());
    println!("stcoords count: {:#?}", odol.lods[0].st_coords.len());
    println!("stcoords: {:#?}", odol.lods[0].st_coords.last());
    println!(
        "vertex boneref count: {:#?}",
        odol.lods[0].vertex_bone_ref.len()
    );
    println!("vertexbonre: {:#?}", odol.lods[0].vertex_bone_ref.last());
    println!(
        "neighbour boneref count: {:#?}",
        odol.lods[0].neighbour_bone_ref.len()
    );
    //println!("neighbour: {:#?}", odol.lods[0].);
    println!("neighbour: {:#?}", odol.lods[0].neighbour_bone_ref.last());
}

#[test]
fn lzss_debug() {
    let odol = ODOL::from_path(format!("{}fbbear.p3d", INPUT_PATH_PREFIX)).unwrap();
    for lod in odol.lods {
        dbg!(lod.named_selection.clone());
    }
}

#[test]
fn single_debug() {
    let odol = ODOL::from_path(format!("{}sfp_wamako_hut01.p3d", INPUT_PATH_PREFIX)).unwrap();
    for lod in odol.lods {
        dbg!(lod.named_selection.clone());
    }
}

#[test]
fn p3d_version_75() {
    let odol = ODOL::from_path(format!("{}sloup_vn.p3d", INPUT_PATH_PREFIX)).unwrap();
    assert_eq!(odol.version, 75);
    for lod in odol.lods {
        dbg!(lod.named_selection.clone());
    }
}

#[test]
fn test_all() {
    let test_all = fs::read_dir(INPUT_PATH_PREFIX).unwrap();

    test_all.for_each(|e| {
        if let Ok(entry) = e {
            let p = entry.path();
            if p.extension().unwrap_or_default() == "p3d" {
                println!("Testing:  {}", p.display());
                let _o = ODOL::from_path(p).unwrap();
            }
        }
    });
}

fn test_models() -> impl Iterator<Item = (String, ODOL)> {
    let mut paths: Vec<_> = fs::read_dir(INPUT_PATH_PREFIX)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "p3d"))
        .collect();
    paths.sort();
    paths.into_iter().map(|path| {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let odol = ODOL::from_path(&path).unwrap_or_else(|err| panic!("{name}: {err}"));
        (name, odol)
    })
}

#[test]
fn section_faces() {
    for (name, odol) in test_models() {
        for (index, lod) in odol.lods.iter().enumerate() {
            let ranges = lod.section_faces();
            assert_eq!(ranges.len(), lod.sections.len());
            let mut end = 0;
            for range in &ranges {
                let range = range
                    .clone()
                    .unwrap_or_else(|| panic!("{name} LOD {index}"));
                assert_eq!(range.start, end, "{name} LOD {index}: sections leave a gap");
                end = range.end;
            }
            if !ranges.is_empty() {
                assert_eq!(
                    end,
                    lod.faces.len(),
                    "{name} LOD {index}: faces after the sections"
                );
            }
        }
    }
}

#[test]
fn uvs() {
    let mut discretized = false;
    let mut floats = false;
    for (name, odol) in test_models() {
        for lod in odol.lods.iter().filter(|lod| !lod.faces.is_empty()) {
            for set in std::iter::once(&lod.default_uv_set).chain(&lod.uv_sets) {
                let uvs = set.uvs();
                assert_eq!(uvs.len(), lod.vertices.len(), "{name}");
                assert!(uvs.iter().flatten().all(|v| v.is_finite()), "{name}");
                if set.is_discretized {
                    discretized = true;
                    let (min_u, max_u) = (set.min_u.unwrap(), set.max_u.unwrap());
                    let (min_v, max_v) = (set.min_v.unwrap(), set.max_v.unwrap());
                    // `i16::MIN` lies one step below the range.
                    let slack_u = (max_u - min_u) / 65534.0 + 1e-6;
                    let slack_v = (max_v - min_v) / 65534.0 + 1e-6;
                    assert!(
                        uvs.iter().all(|&[u, v]| u >= min_u - slack_u
                            && u <= max_u + slack_u
                            && v >= min_v - slack_v
                            && v <= max_v + slack_v),
                        "{name}: UVs outside of their range"
                    );
                } else {
                    floats = true;
                }
            }
        }
    }
    assert!(discretized && floats);
}

/// Sections of collimator sights have 11 floats after the stages, which used to be read as a
/// 12 float matrix and broke the next section.
#[test]
fn section_unk_floats() {
    let odol = ODOL::from_path(format!("{}acco_aco_f.p3d", INPUT_PATH_PREFIX)).unwrap();
    assert_eq!(odol.version, 73);
    let sections = odol.lods.iter().flat_map(|lod| &lod.sections);
    let with_floats: Vec<_> = sections.filter(|s| s.has_unk_floats).collect();
    assert!(!with_floats.is_empty());
    for section in with_floats {
        let floats = section.unk_floats.unwrap();
        assert!(floats.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
    }
    for lod in &odol.lods {
        assert!(lod.section_faces().iter().all(Option::is_some));
    }
}
