use std::{fs::File, io::BufReader};

use arma_file_formats::real_virtuality::wrp::{MapData, MapInfo, OPRW};
use serial_test::serial;

const INPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/wrp_in/";

#[test]
fn test_defaults() {
    OPRW::default();
}

#[test]
fn gm_test_summer() {
    // gm_weferlingen_summer
    let mut file = File::open(format!("{}gm_weferlingen_summer.wrp", INPUT_PATH_PREFIX)).unwrap();

    let wrp = OPRW::from_read(&mut file).unwrap();

    let rivers: Vec<&MapInfo> = wrp
        .map_infos
        .iter()
        .filter(|x| matches!(&x.data, MapData::MapTypeRiver { .. }))
        .collect();

    dbg!(&rivers);
    dbg!(rivers.len());
}

#[test]
#[serial]
fn tempelan_wrp() {
    // Tembelan.wrp
    let mut file = File::open(format!("{}Tembelan.wrp", INPUT_PATH_PREFIX)).unwrap();

    let _ = OPRW::from_read(&mut file).unwrap();
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
fn ivachev_wrp() {
    let file = File::open(format!("{}Ivachev.wrp", INPUT_PATH_PREFIX)).unwrap();
    let mut reader = BufReader::new(file);

    let wrp = OPRW::from_read(&mut reader).unwrap();

    dbg!(&wrp.app_id);
}

#[test]
#[serial]
fn fjaderholmarna_test() {
    let mut file = File::open(format!("{}fjaderholmarna.wrp", INPUT_PATH_PREFIX)).unwrap();
    let _ = OPRW::from_read(&mut file).unwrap();
}

#[test]
#[serial]
fn al_rayak() {
    let mut file = File::open(format!("{}pja310.wrp", INPUT_PATH_PREFIX)).unwrap();

    let wrp = OPRW::from_read(&mut file).unwrap();

    dbg!(wrp.road_net.len());
    dbg!(&wrp.road_net[123]);
}
