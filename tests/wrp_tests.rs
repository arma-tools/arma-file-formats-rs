use std::{fs::File, io::BufReader};

use rvff::wrp::OPRW;
use serial_test::serial;

const INPUT_PATH_PREFIX: &str = "./tests/test-data/wrp_in/";

#[test]
fn test_defaults() {
    OPRW::default();

    // QuadTree::<u32>::default();
    // QuadTreeInner::<u32>::default();
}

#[test]
#[serial]
fn tempelan_wrp() {
    //Tembelan.wrp
    let mut file = File::open(format!("{}Tembelan.wrp", INPUT_PATH_PREFIX)).unwrap();

    let wrp = OPRW::from_read(&mut file).unwrap();
}

#[test]
#[serial]
fn stratis_wrp() {
    let mut file = File::open(format!("{}Stratis.wrp", INPUT_PATH_PREFIX)).unwrap();

    let wrp = OPRW::from_read(&mut file).unwrap();
    dbg!(&wrp.map_infos.len());
    dbg!(&wrp.mountains[700]);
    dbg!(&wrp.mountains.len());
    dbg!(&wrp.classed_models.as_ref().unwrap().last());

    assert_eq!(wrp.mountains.len(), 1408);
    assert_eq!(wrp.texures.len(), 479);

    assert_eq!(
        wrp.texures.last().unwrap().texture_filename.to_string(),
        "a3\\map_stratis\\data\\layers\\p_007-007_l02_l04_n.rvmat"
    );
    assert_eq!(
        wrp.models.last().unwrap().to_string(),
        "a3\\signs_f\\signspecial\\signspec_kaminofiringrange_f.p3d"
    );
    assert_eq!(
        wrp.classed_models
            .unwrap()
            .last()
            .unwrap()
            .model_path
            .to_string(),
        "a3\\structures_f\\mil\\cargo\\cargo_house_v1_f.p3d"
    );
    assert_eq!(wrp.max_object_id, 163952);

    assert_eq!(wrp.road_net.len(), 182);
    assert_eq!(
        wrp.road_net
            .last()
            .unwrap()
            .road_parts
            .first()
            .unwrap()
            .p3d_path
            .as_ref()
            .unwrap()
            .to_string(),
        "a3\\structures_f\\bridges\\bridge_01_f.p3d"
    );

    assert!(wrp.objects.iter().any(|x| x.object_id == 156204));

    assert_eq!(wrp.app_id.unwrap_or_default(), 107410);

    assert_eq!(wrp.map_infos.len(), 80420);
}

#[test]
#[serial]
fn ivf_wrp() {
    let file = File::open(format!("{}Ivachev.wrp", INPUT_PATH_PREFIX)).unwrap();
    let mut reader = BufReader::new(file);

    let wrp = OPRW::from_read(&mut reader).unwrap();

    dbg!(&wrp.app_id);
}

#[test]
#[serial]
fn fjae_test() {
    let mut file = File::open(format!("{}fjaderholmarna.wrp", INPUT_PATH_PREFIX)).unwrap();
    let oprw = OPRW::from_read(&mut file).unwrap();
}
