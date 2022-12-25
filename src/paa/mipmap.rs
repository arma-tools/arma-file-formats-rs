use std::io::{self, Cursor, SeekFrom};
use std::io::{BufRead, Seek, Write};

use crate::core::decompress_lzss;
use crate::core::read::ReadExtTrait;
use crate::core::write::WriteExtTrait;
use anyhow::Result;
use lzokay_rust_native::compress::{compress_with_dict, Dict};
use lzokay_rust_native::decompress::decompress_stream;
//use lzokay::compress::{self, Dict};
use squish::{Format, Params, COLOUR_WEIGHTS_UNIFORM};

use crate::core::types::PaaType;
use crate::errors::PaaError;

#[derive(Debug, Clone)]
pub struct Mipmap {
    pub width: u16,
    pub height: u16,
    pub data_size: i64,
    pub data: Vec<u8>,
    pub(crate) is_lzo_compressed: bool,
    pub(crate) data_pos: Option<u64>,
}

impl Mipmap {
    pub fn new() -> Self {
        Mipmap {
            width: 0,
            height: 0,
            data_size: 0,
            data: Vec::new(),
            is_lzo_compressed: false,
            data_pos: None,
        }
    }

    pub(crate) fn read_header<R>(&mut self, reader: &mut R) -> Result<(), PaaError>
    where
        R: BufRead + Seek,
    {
        self.width = reader.read_u16()?;

        if self.width & 0x8000 != 0 {
            self.width &= 0x7FFF;
            self.is_lzo_compressed = true;
        }

        self.height = reader.read_u16()?;
        self.data_size = reader.read_u24()? as i64;

        self.data_pos = Some(reader.seek(SeekFrom::Current(0))?);

        reader.seek(SeekFrom::Current(self.data_size))?;

        Ok(())
    }

    pub fn read<T>(&mut self, reader: &mut T, paa_type: &PaaType) -> Result<(), PaaError>
    where
        T: BufRead + Seek,
    {
        if self.data_pos.is_none() {
            self.read_header(reader)?;
        }

        if let Some(data_pos) = self.data_pos {
            reader.seek(SeekFrom::Start(data_pos))?;
            self.data = reader.read_bytes(self.data_size as usize)?;
            self.decompress_data(paa_type)?;
            Ok(())
        } else {
            Err(PaaError::InvalidState)
        }
    }

    fn decompress_data(&mut self, paa_type: &PaaType) -> Result<(), PaaError> {
        let expected_size = self.width as usize * self.height as usize;

        if self.is_lzo_compressed {}

        match paa_type {
            PaaType::UNKNOWN => todo!(),
            PaaType::DXT1 => {
                if self.is_lzo_compressed {
                    //let mut decompressed_lzo = vec![0u8; expected_size / 2];

                    //let _ = decompress(&self.data, &mut decompressed_lzo)?;
                    // lzokay::decompress::decompress(&self.data, &mut decompressed_lzo)
                    //     .unwrap_or_default();

                    // self.data = decompressed_lzo;
                    self.data = decompress_stream(&mut Cursor::new(self.data.clone()), None)?;
                }

                let format = Format::Bc1;
                let mut decompressed = vec![0u8; 4 * expected_size];

                format.decompress(
                    &self.data,
                    self.width as usize,
                    self.height as usize,
                    &mut decompressed,
                );

                self.data = decompressed;

                // image::save_buffer(
                //     "out.png",
                //     &self.data,
                //     self.width as u32,
                //     self.height as u32,
                //     image::ColorType::Rgba8,
                // )
                // .unwrap();
            }
            PaaType::DXT2 => todo!(),
            PaaType::DXT3 => todo!(),
            PaaType::DXT4 => todo!(),
            PaaType::DXT5 => {
                let format = Format::Bc3;
                let mut decompressed = vec![0u8; 4 * expected_size];

                format.decompress(
                    &self.data,
                    self.width as usize,
                    self.height as usize,
                    &mut decompressed,
                );

                self.data = decompressed;

                // image::save_buffer(
                //     "out.png",
                //     &self.data,
                //     self.width as u32,
                //     self.height as u32,
                //     image::ColorType::Rgba8,
                // )
                // .unwrap();
            }
            PaaType::RGBA4444 => todo!(),
            PaaType::RGBA5551 => todo!(),
            PaaType::RGBA8888 => todo!(),
            PaaType::GRAYwAlpha => {
                let mut cursor_data = Cursor::new(&self.data);

                let (_, decompressed_data) = decompress_lzss(
                    &mut cursor_data,
                    (self.width * self.height * 2) as usize,
                    true,
                )?;

                let mut rgba_buf = Vec::with_capacity((self.width * self.height * 4) as usize);

                for i in (0..decompressed_data.len()).step_by(2) {
                    let gray = decompressed_data[i];
                    let alpha = decompressed_data[i + 1];
                    rgba_buf.push(gray);
                    rgba_buf.push(gray);
                    rgba_buf.push(gray);
                    rgba_buf.push(alpha);
                }

                self.data = rgba_buf;
            }
        }

        Ok(())
    }

    pub fn write<W>(
        &mut self,
        writer: &mut W,
        paa_type: &PaaType,
        dict: &mut Dict,
    ) -> Result<(), PaaError>
    where
        W: Write + Seek,
    {
        let mut out_data: Vec<u8>;
        match paa_type {
            PaaType::UNKNOWN => todo!(),
            PaaType::DXT1 => {
                let format = Format::Bc1;

                let comp_size = format.compressed_size(self.width.into(), self.height.into());
                out_data = vec![0u8; comp_size];

                format.compress(
                    &self.data,
                    self.width.into(),
                    self.height.into(),
                    Params {
                        algorithm: squish::Algorithm::IterativeClusterFit,
                        weigh_colour_by_alpha: true,
                        weights: COLOUR_WEIGHTS_UNIFORM,
                    }, // ToDo
                    &mut out_data,
                );

                if let Some(compressed_data) = self.compress_lzo(dict, &out_data)? {
                    out_data = compressed_data;
                }
            }
            PaaType::DXT2 => todo!(),
            PaaType::DXT3 => todo!(),
            PaaType::DXT4 => todo!(),
            PaaType::DXT5 => {
                let format = Format::Bc3;

                let comp_size = format.compressed_size(self.width.into(), self.height.into());
                out_data = vec![0u8; comp_size];

                format.compress(
                    &self.data,
                    self.width.into(),
                    self.height.into(),
                    Params::default(), // ToDo
                    &mut out_data,
                );

                if let Some(compressed_data) = self.compress_lzo(dict, &out_data)? {
                    out_data = compressed_data;
                }
            }
            PaaType::RGBA4444 => todo!(),
            PaaType::RGBA5551 => todo!(),
            PaaType::RGBA8888 => todo!(),
            PaaType::GRAYwAlpha => todo!(),
        }

        self.data_size = self.data.len() as i64;

        // Write
        self.writer_internal(writer, out_data)?;

        Ok(())
    }

    fn writer_internal<W>(&mut self, writer: &mut W, out_data: Vec<u8>) -> io::Result<()>
    where
        W: Write + Seek,
    {
        if self.is_lzo_compressed {
            writer.write_u16(self.width | 0x8000)?;
        } else {
            writer.write_u16(self.width)?;
        }
        writer.write_u16(self.height)?;
        writer.write_u24(out_data.len() as u32)?;
        writer.write_bytes(&out_data)?;
        Ok(())
    }

    fn compress_lzo(&mut self, dict: &mut Dict, data: &[u8]) -> Result<Option<Vec<u8>>, PaaError> {
        if self.width > 128 {
            self.is_lzo_compressed = true;
            //compress::compress_worst_size(self.data.len());
            //lzo_rs::compress_worst_size(self.data.len());

            Ok(Some(compress_with_dict(data, dict)?))

            // match compress_with_dict(data, dict) {
            //     Ok(data) => Ok(Some(data)),
            //     Err(err) => Err(PaaEncodingError::PaaLzoCompressionErr(err)),
            // }

            // Some(compress_with_dict(data, dict)?);

            // return match compress_with_dict(data, dict) {
            //     //return match compress::compress_with_dict(data, dict) {
            //     Ok(it) => Ok(Some(it)),
            //     Err(_) => Err(anyhow!("LZO Compression failed!")),
            // };
        } else {
            self.is_lzo_compressed = false;
            Ok(None)
        }
    }
}

impl Default for Mipmap {
    fn default() -> Self {
        Self::new()
    }
}
