use std::{fs::File, io::BufReader};

use rvff::wrp::{Oprw, QuadTree, QuadTreeInner};
use serial_test::serial;

const INPUT_PATH_PREFIX: &str = "./tests/test-data/wrp_in/";

#[test]
fn test_defaults() {
    Oprw::default();

    QuadTree::<u32>::default();
    QuadTreeInner::<u32>::default();
}

#[test]
#[serial]
fn stratis_wrp() {
    let file = File::open(format!("{}Stratis.wrp", INPUT_PATH_PREFIX)).unwrap();
    let mut reader = BufReader::new(file);

    let wrp = Oprw::from_stream(&mut reader).unwrap();

    assert_eq!(wrp.peak_count, 1408);
    assert_eq!(wrp.rvmat_count, 479);

    assert_eq!(
        wrp.rvmats.last().unwrap().texture,
        "a3\\map_stratis\\data\\layers\\p_007-007_l02_l04_n.rvmat"
    );
    assert_eq!(
        wrp.models.last().unwrap(),
        "a3\\signs_f\\signspecial\\signspec_kaminofiringrange_f.p3d"
    );
    assert_eq!(
        wrp.classes.last().unwrap().model_path,
        "a3\\structures_f\\mil\\cargo\\cargo_house_v1_f.p3d"
    );
    assert_eq!(wrp.max_object_id, 163952);
    assert_eq!(wrp.size_of_roadnets, 306262);

    assert_eq!(wrp.road_nets.len(), 182);
    assert_eq!(
        wrp.road_nets
            .last()
            .unwrap()
            .road_parts
            .first()
            .unwrap()
            .p3d_model,
        "\\structures_f\\bridges\\bridge_01_f.p3d"
    );

    assert!(wrp.objects.iter().any(|x| x.object_id == 156204));

    assert_eq!(wrp.app_id.unwrap_or_default(), 107410);
}
