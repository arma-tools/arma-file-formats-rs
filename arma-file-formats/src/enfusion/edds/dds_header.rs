use crate::enfusion::edds::{
    D3D10_Resource_Dimension, DdsCaps2Flags, DdsCapsFlags, DdsHeaderFlags, DdsPixelFormatEnum,
    DdsPixelformatFlags, DxgiFormat, FourCCEnum,
};
use binrw::BinRead;
use enumflags2::BitFlags;

const DDS_HEADER_SIZE: u32 = 124;
const DDS_HEADER_RESERVED_COUNT: u32 = 11;

#[derive(BinRead, Debug, Clone)]
#[br(magic = b"DDS ")]
#[br(little)]
#[br(assert(size == DDS_HEADER_SIZE))]
pub struct DdsHeader {
    pub size: u32,

    #[allow(dead_code)]
    flags_value: u32,

    #[br(calc = BitFlags::<DdsHeaderFlags>::from_bits(flags_value).unwrap_or_default())]
    pub flags: BitFlags<DdsHeaderFlags>,

    pub height: u32,
    pub width: u32,
    pub pitch: u32,
    pub depth: u32,
    pub mipmap_count: u32,

    #[br(count = DDS_HEADER_RESERVED_COUNT)]
    pub reserverd: Vec<u32>,

    pub pixel_format: DdsPixelFormat,

    #[allow(dead_code)]
    caps_value: u32,
    #[br(calc = BitFlags::<DdsCapsFlags>::from_bits(caps_value).unwrap_or_default())]
    pub caps: BitFlags<DdsCapsFlags>,

    #[allow(dead_code)]
    caps2_value: u32,
    #[br(calc = BitFlags::<DdsCaps2Flags>::from_bits(caps2_value).unwrap_or_default())]
    pub caps2: BitFlags<DdsCaps2Flags>,

    _caps3: u32,
    _caps4: u32,
    _reserved2: u32,

    #[br(if(pixel_format.four_cc == FourCCEnum::DX10))]
    pub dx10_header: Option<DdsHeaderDX10>,
}

impl DdsHeader {
    pub fn get_pixel_format(&self) -> DdsPixelFormatEnum {
        match (
            self.pixel_format.rgb_bit_count,
            self.pixel_format.r_bit_mask,
            self.pixel_format.g_bit_mask,
            self.pixel_format.b_bit_mask,
            self.pixel_format.a_bit_mask,
        ) {
            (16, 0x7C00, 0x3E0, 0x1F, 0x8000) => DdsPixelFormatEnum::D3DFMT_A1R5G5B5,
            (32, 0x3FF, 0xFFC00, 0x3FF00000, 0xC0000000) => DdsPixelFormatEnum::D3DFMT_A2B10G10R10,
            (32, 0x3FF00000, 0xFFC00, 0x3FF, 0xC0000000) => DdsPixelFormatEnum::D3DFMT_A2R10G10B10,
            (8, 0xF, 0x0, 0x0, 0xF0) => DdsPixelFormatEnum::D3DFMT_A4L4,
            (16, 0xF00, 0xF0, 0xF, 0xF000) => DdsPixelFormatEnum::D3DFMT_A4R4G4B4,
            (8, 0x0, 0x0, 0x0, 0xFF) => DdsPixelFormatEnum::D3DFMT_A8,
            (32, 0xFF, 0xFF00, 0xFF0000, 0xFF000000) => DdsPixelFormatEnum::D3DFMT_A8B8G8R8,
            (16, 0xFF, 0x0, 0x0, 0xFF00) => DdsPixelFormatEnum::D3DFMT_A8L8,
            (16, 0xE0, 0x1C, 0x3, 0xFF00) => DdsPixelFormatEnum::D3DFMT_A8R3G3B2,
            (32, 0xFF0000, 0xFF00, 0xFF, 0xFF000000) => DdsPixelFormatEnum::D3DFMT_A8R8G8B8,
            (32, 0xFFFF, 0xFFFF0000, 0x0, 0x0) => DdsPixelFormatEnum::D3DFMT_G16R16,
            (16, 0xFFFF, 0x0, 0x0, 0x0) => DdsPixelFormatEnum::D3DFMT_L16,
            (8, 0xFF, 0x0, 0x0, 0x0) => DdsPixelFormatEnum::D3DFMT_L8,
            (16, 0xF800, 0x7E0, 0x1F, 0x0) => DdsPixelFormatEnum::D3FMT_R5G6B5,
            (24, 0xFF0000, 0xFF00, 0xFF, 0x0) => DdsPixelFormatEnum::D3DFMT_R8G8B8,
            (16, 0x7C00, 0x3E0, 0x1F, 0x0) => DdsPixelFormatEnum::D3DFMT_X1R5G5B5,
            (16, 0xF00, 0xF0, 0xF, 0x0) => DdsPixelFormatEnum::D3DFMT_X4R4G4B4,
            (32, 0xFF, 0xFF00, 0xFF0000, 0x0) => DdsPixelFormatEnum::D3DFMT_X8B8G8R8,
            (32, 0xFF0000, 0xFF00, 0xFF, 0x0) => DdsPixelFormatEnum::D3DFMT_X8R8G8B8,
            (_, _, _, _, _) => DdsPixelFormatEnum::Unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, BinRead)]
pub struct DdsHeaderDX10 {
    pub dxgi_format: DxgiFormat,
    pub resource_dimension: D3D10_Resource_Dimension,
    pub misc_flag: u32,
    pub array_size: u32,
    pub misc_flags2: u32,
}

#[derive(BinRead, Debug, Clone)]
pub struct DdsPixelFormat {
    pub size: u32,

    #[allow(dead_code)]
    flag_value: u32,
    #[br(calc = BitFlags::<DdsPixelformatFlags>::from_bits(flag_value).unwrap_or_default())]
    pub flags: BitFlags<DdsPixelformatFlags>,

    pub four_cc: FourCCEnum,
    pub rgb_bit_count: u32,
    pub r_bit_mask: u32,
    pub g_bit_mask: u32,
    pub b_bit_mask: u32,
    pub a_bit_mask: u32,
}
