use std::{
    cmp,
    collections::HashMap,
    io::{BufRead, Seek, SeekFrom, Write},
};

use lzokay_rust_native::compress::Dict;

#[cfg(feature = "parallel")]
use rayon::prelude::{IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator};

use crate::{
    core::{read::ReadExtTrait, types::PaaType, write::WriteExtTrait},
    errors::PaaError,
};

use super::{Mipmap, Tagg};

#[derive(Debug, Clone)]
pub struct Paa {
    pub magic_number: PaaType,
    pub mipmaps: Vec<Mipmap>,
    taggs: HashMap<String, Tagg>,
    palette: Vec<u8>,
}

impl Paa {
    pub fn max_mipmap_count(width: usize, height: usize) -> usize {
        (width.max(height) as f64).log2().floor() as usize
    }

    pub fn dim_at_level(dim: usize, i: u32) -> usize {
        (dim / 2_usize.pow(i)).max(1)
    }

    pub fn new() -> Self {
        Paa {
            magic_number: PaaType::UNKNOWN,
            mipmaps: Vec::new(),
            taggs: HashMap::new(),
            palette: Vec::new(),
        }
    }

    pub fn from_image(width: u16, height: u16, data: Vec<u8>) -> Paa {
        let mut paa = Paa::new();

        let mut mm = Mipmap::new();
        mm.width = width;
        mm.height = height;
        mm.data = data;

        paa.mipmaps.push(mm);

        paa
    }

    pub fn from_reader<R>(reader: &mut R, indicies_to_load: Option<&[u32]>) -> Result<Paa, PaaError>
    where
        R: BufRead + Seek,
    {
        let mut paa = Paa::new();

        reader.rewind()?;
        paa.magic_number = PaaType::try_from(reader.read_u16()?).unwrap_or(PaaType::UNKNOWN);

        while reader.peek_string_lossy(4)?.starts_with("GGAT") {
            let mut tagg = Tagg::new();
            tagg.read(reader)?;
            paa.taggs.insert(tagg.signature.clone(), tagg);
        }

        let palette_length = reader.read_u16()? as usize;
        if palette_length > 0 {
            paa.palette = reader.read_bytes(palette_length)?;
        }

        let mut index_counter: u32 = 0;
        while reader.peek_u16()? != 0 {
            let mut mipmap = Mipmap::new();

            mipmap.read_header(reader)?;

            // TODO: refactor
            if indicies_to_load.is_none()
                || indicies_to_load.unwrap().is_empty()
                || indicies_to_load.unwrap().contains(&index_counter)
            {
                mipmap.read(reader, &paa.magic_number)?;
            }

            paa.mipmaps.push(mipmap);
            index_counter += 1;
        }

        Ok(paa)
    }

    pub fn write<W>(&mut self, writer: &mut W, paa_type: Option<PaaType>) -> Result<(), PaaError>
    where
        W: Write + Seek,
    {
        self.mipmaps.sort_by(|a, b| b.width.cmp(&a.width));

        if self.mipmaps.is_empty() {
            return Err(PaaError::NoMipmapError);
        }

        for i in 1..self.mipmaps.len() {
            let mipmap = &self.mipmaps[i];
            if (mipmap.width as usize * mipmap.height as usize * 4) != mipmap.data.len() {
                return Err(PaaError::InvalidMipmapError(i));
            }
        }

        let initial_width = self.mipmaps[0].width as usize;
        let initial_height = self.mipmaps[0].height as usize;

        let level_count =
            (f32::log2(cmp::min(initial_width, initial_height) as f32) - 1.0) as usize;
        self.mipmaps.resize(level_count, Mipmap::default());

        let mut prev_data = self.mipmaps[0].data.clone();

        let mut mipmaps: Vec<Mipmap> = (1..level_count)
            .into_iter()
            .map(|level| -> Mipmap {
                let desired_width = initial_width >> level;
                let desired_height = initial_height >> level;

                let previous_width = initial_width >> (level - 1);

                let mut data = vec![0u8; desired_width * desired_height * 4];

                #[cfg(feature = "parallel")]
                let iter = data.par_iter_mut();
                #[cfg(not(feature = "parallel"))]
                let iter = data.iter_mut();

                iter.enumerate().for_each(|(i, d)| {
                    let x = (i / 4) % desired_width;
                    let y = i / (desired_width * 4);
                    let i = i % 4;

                    let y0 = y << 1;
                    let y1 = cmp::min(y0 + 1, cmp::max(1, initial_height >> (level - 1)) - 1);

                    let x0 = x << 1;
                    let x1 = cmp::min(x0 + 1, cmp::max(1, initial_width >> (level - 1)) - 1);

                    *d = (0.25
                        * (prev_data[x0 * 4 + y0 * 4 * previous_width + i] as usize
                            + prev_data[x1 * 4 + y0 * 4 * previous_width + i] as usize
                            + prev_data[x0 * 4 + y1 * 4 * previous_width + i] as usize
                            + prev_data[x1 * 4 + y1 * 4 * previous_width + i] as usize)
                            as f64) as u8;
                });

                let mut mipmap = Mipmap::new();
                mipmap.width = desired_width as u16;
                mipmap.height = desired_height as u16;
                mipmap.data_size = data.len() as i64;
                mipmap.data = data.clone();

                prev_data = data;

                mipmap
            })
            .collect();

        self.mipmaps.truncate(1);
        self.mipmaps.append(&mut mipmaps);

        // Calc average color for "AVGCTAGG"
        let (mut avg_r, mut avg_g, mut avg_b, mut avg_a): (usize, usize, usize, usize) =
            (0, 0, 0, 0);

        let mipmap = self.mipmaps.first().unwrap();

        for i in 0..mipmap.data.len() / 4 {
            avg_r += *mipmap.data.get(i * 4).unwrap() as usize;
            avg_g += *mipmap.data.get(i * 4 + 1).unwrap() as usize;
            avg_b += *mipmap.data.get(i * 4 + 2).unwrap() as usize;
            avg_a += *mipmap.data.get(i * 4 + 3).unwrap() as usize;
        }

        let pixel_count = mipmap.width as usize * mipmap.height as usize;
        avg_r /= pixel_count;
        avg_g /= pixel_count;
        avg_b /= pixel_count;
        avg_a /= pixel_count;

        let mut taggs: HashMap<String, Tagg> = HashMap::new();
        taggs.insert(
            "GGATCGVA".to_string(),
            Tagg {
                signature: "GGATCGVA".to_string(),
                data: vec![avg_r as u8, avg_g as u8, avg_b as u8, avg_a as u8],
            },
        );

        // "MAXCTAGG"
        if !taggs.contains_key("GGATCXAM") {
            taggs.insert(
                "GGATCXAM".to_string(),
                Tagg {
                    signature: "GGATCXAM".to_string(),
                    data: vec![0xff; 4],
                },
            );
        }

        // "FLAGTAGG"
        taggs.remove("GGATGALF");
        if avg_a != 0xff {
            taggs.insert(
                "GGATGALF".to_string(),
                Tagg {
                    signature: "GGATGALF".to_string(),
                    data: vec![0x01, 0xff, 0xff, 0xff],
                },
            );
        }

        let magic_number = match paa_type {
            Some(pt) => pt,
            None => {
                if avg_a == 0xFF {
                    PaaType::DXT1
                } else {
                    PaaType::DXT5
                }
            }
        };

        self.write_internal(writer, magic_number, &mut taggs, Vec::new())?;

        Ok(())
    }

    fn write_internal<W>(
        &mut self,
        writer: &mut W,
        magic_number: PaaType,
        taggs: &mut HashMap<String, Tagg>,
        palette: Vec<u8>,
    ) -> Result<(), PaaError>
    where
        W: Write + Seek,
    {
        taggs.remove("GGATSFFO");

        writer.write_u16(magic_number.into())?;

        // Taggs
        for tagg in taggs.values() {
            tagg.write(writer)?;
        }

        // Offsets
        writer.write_string("GGATSFFO")?;
        writer.write_u32(16 * 4)?; // Always 16 mipmaps
        let offset_offset = writer.stream_position()?;
        writer.write_bytes(&[0u8; 16 * 4])?;

        // Palette
        writer.write_u16(palette.len().try_into().unwrap_or_default())?;
        if !palette.is_empty() {
            writer.write_bytes(&palette)?;
        }

        // Mipmaps
        let mut mipmap_offsets = Vec::<u32>::with_capacity(16);
        let mut dict = Dict::new();
        for mipmap in &mut self.mipmaps {
            mipmap_offsets.push(writer.stream_position()? as u32);
            mipmap.write(writer, &magic_number, &mut dict)?;
        }

        writer.write_bytes(&[0u8; 6])?;

        writer.seek(SeekFrom::Start(offset_offset))?;
        for mipmap_offset in mipmap_offsets {
            writer.write_u32(mipmap_offset)?;
        }

        Ok(())
    }
}

impl Default for Paa {
    fn default() -> Self {
        Self::new()
    }
}
