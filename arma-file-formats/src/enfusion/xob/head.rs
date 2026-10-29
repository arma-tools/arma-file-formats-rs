use binrw::binread;

/// Section format flag: positions carry a skin weight index and a weight table follows.
pub const XOB_FORMAT_SKINNED: u32 = 0x1000_0000;
/// Section format flag: an RGBA8 vertex color stream is present.
pub const XOB_FORMAT_COLORS: u32 = 0x2000_0000;

/// Contents of the `HEAD` chunk: model wide data and the descriptors of every LOD.
#[binread]
#[derive(Debug, Clone)]
#[br(little)]
pub struct XobHead {
    pub unknown_0: u32,
    pub bbox_min: [f32; 3],
    pub bbox_max: [f32; 3],
    pub unknown_vec: [f32; 3],
    /// Radius of the bounding sphere around the model origin.
    pub radius: f32,

    #[br(temp)]
    material_count: u16,
    #[br(temp)]
    bone_count: u16,
    #[br(temp)]
    lod_count: u8,
    #[br(temp)]
    point_count: u8,
    pub unknown_32: u16,
    #[br(temp)]
    volume_count: u32,
    #[br(temp)]
    string_table_size: u32,

    /// Every string of the model: material names and paths, bone names, mesh and collider names.
    #[br(count = string_table_size, map = |bytes: Vec<u8>| split_strings(&bytes))]
    pub strings: Vec<String>,

    #[br(args { count: usize::from(material_count), inner: (&strings,) })]
    pub materials: Vec<XobMaterial>,

    #[br(args { count: usize::from(bone_count), inner: (&strings,) })]
    pub bones: Vec<XobBone>,

    #[br(count = point_count)]
    pub points: Vec<[f32; 3]>,

    #[br(count = lod_count)]
    pub lods: Vec<XobLodHeader>,

    /// Box volumes (`BOXVOL_*`) and portals (`PRT_*`) of buildings.
    #[br(count = volume_count)]
    pub volumes: Vec<XobVolume>,
}

#[binread]
#[derive(Debug, Clone, PartialEq, Eq)]
#[br(little, import(strings: &[String]))]
pub struct XobMaterial {
    /// Index into [`XobHead::strings`].
    pub name_index: u16,
    #[br(calc = string_at(strings, u32::from(name_index)))]
    pub name: String,
    /// Index into [`XobHead::strings`].
    pub path_index: u16,
    /// Resource path of the `.emat`, prefixed with its GUID, e.g. `{3C10D749A4E84B38}Common/Materials/Default.emat`.
    #[br(calc = string_at(strings, u32::from(path_index)))]
    pub path: String,
}

#[binread]
#[derive(Debug, Clone, PartialEq)]
#[br(little, import(strings: &[String]))]
pub struct XobBone {
    /// Index into [`XobHead::strings`].
    pub name_index: u32,
    #[br(calc = string_at(strings, name_index))]
    pub name: String,
    pub translation: [f32; 3],
    /// Quaternion as `[x, y, z, w]`.
    pub rotation: [f32; 4],
    /// Index of the next bone with the same parent.
    #[br(map = bone_link)]
    pub next_sibling: Option<u16>,
    #[br(map = bone_link)]
    pub first_child: Option<u16>,
}

/// Descriptor of one LOD, its mesh data is a LZ4 chain stored in the `LODS` chunk.
#[binread]
#[derive(Debug, Clone)]
#[br(little, magic = b"LZO4")]
pub struct XobLodHeader {
    #[br(temp)]
    section_count: u32,
    pub unknown_8: u32,
    pub lod_factor: f32,
    #[br(temp)]
    bone_bounds_count: u32,
    /// Absolute file offset of the compressed data.
    pub data_offset: u32,
    pub compressed_size: u32,
    pub size: u32,
    #[br(count = section_count)]
    pub sections: Vec<XobSectionHeader>,
    #[br(count = bone_bounds_count)]
    pub bone_bounds: Vec<XobBoneBounds>,
}

/// Descriptor of a part of a LOD that uses a single material.
#[binread]
#[derive(Debug, Clone)]
#[br(little)]
pub struct XobSectionHeader {
    pub format: u32,
    pub bbox_min: [f32; 3],
    pub bbox_max: [f32; 3],
    pub unknown_vec4: [f32; 4],
    #[br(temp)]
    triangle_count_low: u16,
    pub vertex_count: u16,
    pub tangent_vertex_count: u16,
    pub skin_weight_count: u16,
    pub unknown_52: u16,
    pub unknown_54: u16,
    pub material_index: u16,
    #[br(temp)]
    uv_set_count: u8,
    #[br(temp)]
    bone_count: u8,
    pub max_bone_influences: u16,
    #[br(temp)]
    triangle_count_high: u16,
    #[br(calc = u32::from(triangle_count_low) | (u32::from(triangle_count_high) << 16))]
    pub triangle_count: u32,
    #[br(count = uv_set_count)]
    pub uv_sets: Vec<XobUvSet>,
    /// Maps the bone indices of the skin weights to indices into [`XobHead::bones`].
    #[br(count = bone_count)]
    pub bone_palette: Vec<u8>,
}

impl XobSectionHeader {
    #[must_use]
    pub const fn is_skinned(&self) -> bool {
        self.format & XOB_FORMAT_SKINNED != 0
    }

    #[must_use]
    pub const fn has_colors(&self) -> bool {
        self.format & XOB_FORMAT_COLORS != 0
    }

    /// Size of the section inside the decompressed LOD data.
    #[must_use]
    pub fn data_size(&self) -> usize {
        let vertices = usize::from(self.vertex_count);
        let tangent_vertices = usize::from(self.tangent_vertex_count);
        let uv_sets = self.uv_sets.len();
        let position_size = if self.is_skinned() { 16 } else { 12 };
        let color_size = if self.has_colors() { 4 } else { 0 };
        let skin_weights = if self.is_skinned() {
            usize::from(self.skin_weight_count) * 16
        } else {
            0
        };

        self.triangle_count as usize * 3 * 2 * 2
            + vertices * (position_size + 4 + color_size + 4 * uv_sets)
            + tangent_vertices * (4 + 8 * tangent_frame_count(uv_sets))
            + skin_weights
    }
}

/// Every tangent vertex has a frame per UV set, but at least one.
#[must_use]
pub fn tangent_frame_count(uv_sets: usize) -> usize {
    uv_sets.max(1)
}

/// Texture coordinates are stored normalized to these bounds.
#[derive(binrw::BinRead, Debug, Clone, Copy, PartialEq)]
#[br(little)]
pub struct XobUvSet {
    pub u_min: f32,
    pub u_max: f32,
    pub v_min: f32,
    pub v_max: f32,
    pub unknown_scale: f32,
}

#[derive(binrw::BinRead, Debug, Clone, Copy, PartialEq)]
#[br(little)]
pub struct XobBoneBounds {
    pub bone: u32,
    pub bbox_min: [f32; 3],
    pub bbox_max: [f32; 3],
}

#[binread]
#[derive(Debug, Clone, PartialEq)]
#[br(little)]
pub struct XobVolume {
    pub bbox_min: [f32; 3],
    pub bbox_max: [f32; 3],
    /// Position followed by a `[x, y, z, w]` quaternion.
    pub transform: [f32; 7],
    /// `0` for box volumes, `1` for portals.
    pub kind: u8,
    #[br(temp)]
    polygon_size: u8,
    pub unknown_54: u16,
    #[br(count = polygon_size)]
    pub polygon: Vec<[f32; 3]>,
}

fn split_strings(bytes: &[u8]) -> Vec<String> {
    let bytes = bytes.strip_suffix(&[0]).unwrap_or(bytes);
    if bytes.is_empty() {
        return Vec::new();
    }
    bytes
        .split(|b| *b == 0)
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

fn string_at(strings: &[String], index: u32) -> String {
    strings.get(index as usize).cloned().unwrap_or_default()
}

const fn bone_link(index: u16) -> Option<u16> {
    if index == u16::MAX { None } else { Some(index) }
}
