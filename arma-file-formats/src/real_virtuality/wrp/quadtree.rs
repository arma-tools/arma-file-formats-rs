use std::io::{Cursor, Seek};

use binrw::{BinRead, BinResult, Endian};

use derivative::Derivative;

#[derive(Debug, Default, PartialEq, Eq, Clone, BinRead)]
#[br(import(element_size: u32))]
pub struct QuadTree {
    #[br(map = |x: u8| x != 0)]
    flag: bool,

    #[br(args(flag, element_size))]
    root: QuadTreeData,
}

#[derive(Debug, PartialEq, Eq, Clone, BinRead, Derivative)]
#[derivative(Default)]
#[br(import(flag: bool, element_size: u32))]
pub enum QuadTreeData {
    #[br(pre_assert(flag))]
    Node(#[br(args(element_size))] QuadTreeNode),
    #[br(pre_assert(!flag))]
    #[derivative(Default)]
    Leaf(#[br(args(element_size))] QuadTreeLeaf),
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct QuadTreeNode {
    sub_trees: Vec<QuadTreeData>,
}

impl BinRead for QuadTreeNode {
    type Args<'a> = (u32,);

    fn read_options<R: std::io::Read + Seek>(
        reader: &mut R,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let mut bit_mask = u16::read_options(reader, endian, ())?;
        assert!(args.0 > 0);
        let mut sub_trees = Vec::with_capacity(16);
        for _ in 0..16 {
            if (bit_mask & 1) == 1 {
                sub_trees.push(QuadTreeData::Node(Self::read_options(
                    reader, endian, args,
                )?));
            } else {
                sub_trees.push(QuadTreeData::Leaf(QuadTreeLeaf::read_options(
                    reader, endian, args,
                )?));
            }
            bit_mask >>= 1;
        }

        Ok(Self { sub_trees })
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct QuadTreeLeaf {
    element_size: u32,
    data: Vec<u8>,
}

impl BinRead for QuadTreeLeaf {
    type Args<'a> = (u32,);

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        _: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let mut data = vec![0_u8; 4];
        reader.read_exact(&mut data)?;

        Ok(Self {
            element_size: args.0,
            data,
        })
    }
}

impl QuadTreeLeaf {
    pub fn get<'a, T: BinRead<Args<'a> = ()>>(&self, x: u32, y: u32) -> BinResult<Option<T>> {
        let offset = u64::from(match self.element_size {
            1 => 0,
            2 => x * 2,
            4 => (y << 1) + x,
            _ => todo!(),
        });

        let mut data_reader = Cursor::new(&self.data);

        if offset > data_reader.stream_position()? {
            return Ok(None);
        }

        data_reader.set_position(offset);

        let val = T::read_options(&mut data_reader, Endian::Little, ())?;
        Ok(Some(val))
    }
}
