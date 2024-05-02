pub(crate) mod binrw_utils;
mod lzss;
pub mod read;
pub mod types;
pub mod write;

pub use self::lzss::check_for_magic_and_decompress_lzss;
pub use self::lzss::check_for_magic_and_decompress_lzss_file;
pub(crate) use self::lzss::decompress_lzss;
pub use self::lzss::decompress_lzss_unk_size;
