use anyhow::Result;

pub fn align_of(len: usize, align_to: usize) -> usize {
    (len + align_to - 1) & !(align_to - 1)
}

pub fn deserialize<'a, T>(buf: &'a [u8]) -> Result<T>
where
    T: serde::Deserialize<'a>,
{
    bincode::deserialize(buf).map_err(|e| e.into())
}
