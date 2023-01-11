use rvff::p3d::ODOL;
use serial_test::serial;

const INPUT_PATH_PREFIX: &str = "./tests/test-data/p3d_in/";
#[allow(dead_code)]
const OUTPUT_PATH_PREFIX: &str = "./tests/test-data/p3d_out/";

#[test]
#[serial]
fn aa_p3d() {
    let odol = ODOL::from_path(format!("{}APC_Tracked_01_aa_F.p3d", INPUT_PATH_PREFIX)).unwrap();
    println!("{:#?}", odol);
}
