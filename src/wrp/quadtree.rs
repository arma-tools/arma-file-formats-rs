use deku::{
    bitvec::{BitSlice, BitVec, Msb0},
    DekuContainerWrite, DekuError, DekuRead, DekuUpdate, DekuWrite,
};

#[derive(PartialEq, Debug, DekuRead, DekuWrite)]
pub struct QuadTree<T: for<'a> DekuRead<'a>> {
    flag: u8,

    #[deku(cond = "*flag == 0")]
    flag_data: Option<u8>,

    #[deku(cond = "*flag != 0")]
    inner_data: QuadTreeInner<T>,
}

impl<T> QuadTree<T>
where
    T: for<'a> deku::DekuRead<'a>,
{
    pub fn new() -> Self {
        QuadTree {
            flag: 0,
            flag_data: None,
            inner_data: QuadTreeInner::<T>::new(),
        }
    }
}

impl<T> Default for QuadTree<T>
where
    T: for<'a> deku::DekuRead<'a>,
{
    fn default() -> Self {
        Self::new()
    }
}

#[derive(PartialEq, Debug, DekuRead, DekuWrite)]
pub struct QuadTreeInner<T: for<'a> DekuRead<'a>> {
    bit_mask: u16,
    #[deku(
        reader = "QuadTreeInner::<T>::read_data(*bit_mask, deku::rest)",
        writer = "QuadTreeInner::<T>::write_data(deku::output, &self.data)"
    )]
    data: Vec<QuadTreeData<T>>,
}

#[allow(clippy::type_complexity)]
impl<T> QuadTreeInner<T>
where
    T: for<'a> deku::DekuRead<'a>,
{
    pub fn new() -> Self {
        QuadTreeInner {
            bit_mask: 0,
            data: vec![],
        }
    }

    fn read_data(
        bit_mask: u16,
        rest: &BitSlice<u8, Msb0>,
    ) -> Result<(&BitSlice<u8, Msb0>, Vec<QuadTreeData<T>>), DekuError> {
        let mut bit_mask = bit_mask;
        let mut rest = rest;
        let mut data: Vec<QuadTreeData<T>> = Vec::new();

        for _ in 0..16 {
            if (bit_mask & 1) == 1 {
                let (rest_ret, value) = QuadTreeInner::<T>::read(rest, ())?;
                data.push(QuadTreeData::Tree(value));
                rest = rest_ret;
            } else {
                let (rest_ret, value) = T::read(rest, ())?;
                data.push(QuadTreeData::Value(value));
                rest = rest_ret;
            }
            bit_mask >>= 1;
        }
        Ok((rest, data))
    }

    fn write_data(
        _output: &mut BitVec<u8, Msb0>,
        _flags: &[QuadTreeData<T>],
    ) -> Result<(), DekuError> {
        // let value: u32 = flags.bits();
        // value.write(output, ())
        Ok(())
    }
}

impl<T> Default for QuadTreeInner<T>
where
    T: for<'a> deku::DekuRead<'a>,
{
    fn default() -> Self {
        QuadTreeInner::new()
    }
}

#[derive(PartialEq, Debug)]
enum QuadTreeData<T: for<'a> DekuRead<'a>> {
    Tree(QuadTreeInner<T>),
    Value(T),
}
