use std::fs;

use rvff::p3d::ODOL;
use serial_test::serial;

const INPUT_PATH_PREFIX: &str = "./tests/test-data/p3d_in/";
#[allow(dead_code)]
const OUTPUT_PATH_PREFIX: &str = "./tests/test-data/p3d_out/";

#[test]
#[serial]
fn aa_p3d() {
    let odol = ODOL::from_path(format!(
        "{}test_all/APC_Tracked_01_aa_F.p3d",
        INPUT_PATH_PREFIX
    ))
    .unwrap();
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
fn test_all() {
    let test_all_dir = fs::read_dir(format!("{}test_all", INPUT_PATH_PREFIX)).unwrap();

    test_all_dir.for_each(|e| {
        if let Ok(entry) = e {
            let p = entry.path();
            if p.extension().unwrap_or_default() == "p3d" {
                println!("Testing:  {}", p.display());
                let _o = ODOL::from_path(p).unwrap();
            }
        }
    });
}
