use std::{env, path::Path};

fn main() {
    if Path::new("arma-file-formats/tests/real_virtuality/test-data")
        .read_dir()
        .is_ok_and(|mut rd| rd.next().is_some())
    {
        return;
    }

    if Path::new("arma-file-formats/tests/enfusion/test-data")
        .read_dir()
        .is_ok_and(|mut rd| rd.next().is_some())
    {
        return;
    }

    if dotenvy::dotenv().is_err() {
        println!(".env not found...");
    }

    for i in 1..4 {
        let file = format!("arma-file-formats/tests/test-data/rv-test-data-part-{i}.7z");

        println!("Extracting test data: {file}");
        sevenz_rust::decompress_file_with_password(
            file,
            "arma-file-formats/tests/real_virtuality/",
            env::var("AFF_TEST_DATA_PW")
                .expect("AFF_TEST_DATA_PW not set")
                .to_string()
                .as_str()
                .into(),
        )
        .expect("Test data decompression failed");
    }

    for i in 1..2 {
        let file = format!("arma-file-formats/tests/test-data/e-test-data-part-{i}.7z");

        println!("Extracting test data: {file}");
        sevenz_rust::decompress_file_with_password(
            file,
            "arma-file-formats/tests/enfusion/",
            env::var("AFF_TEST_DATA_PW")
                .expect("AFF_TEST_DATA_PW not set")
                .to_string()
                .as_str()
                .into(),
        )
        .expect("Test data decompression failed");
    }

    println!("Done!");
}
