use std::{env, path::Path};

fn main() {
    if Path::new("tests/real_virtuality/test-data")
        .read_dir()
        .is_ok_and(|mut rd| rd.next().is_some())
    {
        return;
    }

    dotenvy::dotenv().expect("dotenvy failed");

    println!("Extracting test data...");

    sevenz_rust::decompress_file_with_password(
        "tests/test-data/rv-test-data.7z",
        "tests/real_virtuality/",
        env::var("AFF_TEST_DATA_PW")
            .expect("AFF_TEST_DATA_PW not set")
            .to_string()
            .as_str()
            .into(),
    )
    .expect("Test data decompression failed");

    println!("Done!");
}
