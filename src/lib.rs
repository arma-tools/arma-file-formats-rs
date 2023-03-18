//pub mod binary;
//pub mod common;
pub mod core;

pub mod p3d;
pub mod paa;
pub mod pbo;
pub mod rap;
pub mod sign;
pub mod wrp;

pub mod errors;

#[cfg(test)]
mod tests {

    // #[test]
    // #[serial]
    // fn kit_test() {
    //     let img = image::open("Test_Input.png").unwrap();

    //     let mut paa = PaaEncoder::from_image_data(
    //         img.dimensions().0 as u16,
    //         img.dimensions().1 as u16,
    //         img.as_rgba8().unwrap(),
    //     );

    //     let mut out = File::create("grad_dlc_low_messenger_black_medic_co.paa").unwrap();
    //     paa.write(&mut out, None).unwrap();
    // }

    // #[test]
    // #[serial]
    // fn kit_test() {
    //     let img = image::open("Test_Input.png").unwrap();

    //     let mut paa = PaaEncoder::from_image_data(
    //         img.dimensions().0 as u16,
    //         img.dimensions().1 as u16,
    //         img.as_rgba8().unwrap(),
    //     );

    //     let mut out = File::create("grad_dlc_low_messenger_black_medic_co.paa").unwrap();
    //     paa.write(&mut out, None).unwrap();
    // }

    // #[test]
    // #[serial]
    // fn it_works() {
    //     //let mut paa = Paa::new(BufReader::new(File::open("test.paa").unwrap()));
    //     let mut paa = PaaDecoder::new(BufReader::new(File::open("Bundle_Test.paa").unwrap()));
    //     paa.read_headers().unwrap();
    //     let mipmap = paa.get_mipmap(0).unwrap().unwrap();
    //     image::save_buffer_with_format(
    //         "Bundle_Test_write_out.png",
    //         &mipmap.data,
    //         mipmap.width.into(),
    //         mipmap.height.into(),
    //         image::ColorType::Rgba8,
    //         image::ImageFormat::Png,
    //     )
    //     .unwrap();
    //     //let mut paa = Paa::from_stream().unwrap();

    //     // let mut out = File::create("test_out2.paa").unwrap();
    //     // paa.generate_mipmaps_and_taggs().unwrap();
    //     // paa.write(&mut out).unwrap();
    // }

    // #[test]
    // #[serial]
    // fn png_to_paa() {
    //     let img = image::open("Bundle_Test.png").unwrap();

    //     let mut paa = PaaEncoder::from_image_data(
    //         img.dimensions().0 as u16,
    //         img.dimensions().1 as u16,
    //         img.as_rgba8().unwrap(),
    //     );

    //     let mut out = File::create("Bundle_convert.paa").unwrap();
    //     paa.write(&mut out, None).unwrap();
    // }

    // #[test]
    // #[serial]
    // fn png_to_paa_dxt1() {
    //     let img = image::open("DXT1_LZO_Test.png").unwrap();

    //     let mut paa = PaaEncoder::from_image_data(
    //         img.dimensions().0 as u16,
    //         img.dimensions().1 as u16,
    //         img.as_rgba8().unwrap(),
    //     );

    //     let mut out = File::create("DXT1_LZO_Test_convert.paa").unwrap();
    //     paa.write(&mut out, None).unwrap();
    // }

    // #[test]
    // #[serial]
    // fn read_pbo() {
    //     let mut pbo =
    //         PboArchive::from_stream(BufReader::new(File::open("grad_meh_main.pbo").unwrap()));
    //     pbo.read_header().unwrap();
    //     let entry = pbo
    //         .get_entry("functions\\fn_export.sqf".to_string())
    //         .unwrap()
    //         .unwrap();
    //     fs::write("out.pbo.txt", entry.data).unwrap();
    // }

    // #[test]
    // #[serial]
    // fn read_wrp_hdeku() {
    //     let data = fs::read("Stratis.wrp").unwrap();
    //     let (_rest, val) = OprwDeku::from_bytes((&data, 0)).unwrap();
    //     let mut s1 = val;
    //     //println!("{:?}", s1);
    //     println!("{:?}", s1.peak_count);
    //     println!("{:?}", s1.peaks);
    //     println!("{:?}", s1.rvmat_count);
    //     //let s = &s1.rvmats[0..s1.rvmats.len() - 1];
    //     println!("{:?}", s1.rvmats.last().unwrap().texture);
    //     println!("{:?}", s1.models.last().unwrap());
    //     println!("{:?}", s1.classes.last().unwrap());
    //     println!("{:?}", s1.max_object_id);
    //     println!("{:?}", s1.size_of_roadnets);
    //     s1.road_nets.retain(|x| x.road_parts_count > 0);
    //     println!("{:?}", s1.road_nets.len());
    //     println!("{:?}", s1.road_nets.last().unwrap());
    //     println!(
    //         "{:?}",
    //         s1.objects.iter().find(|x| x.object_id == 156204).unwrap()
    //     );
    //     if s1.app_id.unwrap_or_default() == 107410 {
    //         println!("A");
    //     }
    // }

    // #[test]
    // #[serial]
    // fn read_pbo_deku() {
    //     let data = fs::read("grad_meh_main.pbo").unwrap();

    //     let (s, s2) = PboArchiveDeku::from_bytes((&data, 0)).unwrap();
    //     dbg!(&s2);
    //     dbg!(s);
    // }

    // #[test]
    // #[serial]
    // fn read_sig() {
    //     let data = fs::read("grad_adminMessages.pbo.grad_2.22.0-4a0b4a4c.bisign").unwrap();

    //     let (s, s2) = Signature::from_bytes((&data, 0)).unwrap();
    //     dbg!(&s2);
    //     dbg!(s);
    // }

    // #[test]
    // #[serial]
    // fn read_pub_key() {
    //     let data = fs::read("grad_2.22.0.bikey").unwrap();

    //     let (s, s2) = PublicKey::from_bytes((&data, 0)).unwrap();
    //     dbg!(&s2);
    //     dbg!(s);
    // }

    // #[test]
    // #[serial]
    // fn read_pbo_2() {
    //     let pub_key_data = fs::read("grad_2.22.0.bikey").unwrap();
    //     let (_, pub_key) = PublicKey::from_bytes((&pub_key_data, 0)).unwrap();

    //     let sig_data = fs::read("grad_adminMessages.pbo.grad_2.22.0-4a0b4a4c.bisign").unwrap();
    //     let (_, sig) = Signature::from_bytes((&sig_data, 0)).unwrap();

    //     let data = fs::read("grad_adminMessages.pbo").unwrap();
    //     let pbo = Pbo::from(Cursor::new(data));
    //     if pbo.verify(&pub_key, &sig).is_ok() {
    //         println!("🦀 🦀 🦀");
    //     }
    // }
}
