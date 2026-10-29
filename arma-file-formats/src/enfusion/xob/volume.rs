use binrw::binread;

/// Contents of the `VOLM` chunk: a voxel grid over [`super::XobHead`]'s bounding box that marks
/// every cell touched by the surface of the model. Closed meshes leave their interior empty.
#[binread]
#[derive(Debug, Clone, PartialEq, Eq)]
#[br(little)]
pub struct XobOccupancyGrid {
    pub unknown_0: u16,
    /// Cell count along x, y and z.
    pub dimensions: [u16; 3],
    #[br(temp)]
    size: u32,
    /// One bit per cell, least significant bit first, see [`Self::is_occupied`].
    #[br(count = size)]
    pub cells: Vec<u8>,
}

impl XobOccupancyGrid {
    #[must_use]
    pub fn is_occupied(&self, x: usize, y: usize, z: usize) -> bool {
        let [_, ny, nz] = self.dimensions.map(usize::from);
        let index = (x * ny + y) * nz + z;
        self.cells
            .get(index / 8)
            .is_some_and(|byte| byte >> (index % 8) & 1 != 0)
    }

    /// Size of one cell, the grid evenly divides the bounding box.
    #[must_use]
    pub fn cell_size(&self, bbox_min: [f32; 3], bbox_max: [f32; 3]) -> [f32; 3] {
        [0, 1, 2].map(|axis| (bbox_max[axis] - bbox_min[axis]) / f32::from(self.dimensions[axis]))
    }
}
