use Error;
use std::str::FromStr;
use std::num::ParseIntError;

pub fn split_xml_children(pointer_str: &str) -> Result<Vec<usize>, Error> {
    if !pointer_str.starts_with("/") { return Err(Error::InvalidPointer) }
    pointer_str.split("/").skip(1).map(|s| Ok(usize::from_str(s)?)).collect()
}

pub fn split_xml_nodes(pointer_str: &str) -> Result<(Vec<usize>, &str), Error> {
    if !pointer_str.starts_with("/") { return Err(Error::InvalidPointer) }
    let mut pointer = pointer_str.split("/").skip(1).collect::<Vec<_>>();
    let key = pointer.pop().ok_or(Error::InvalidPointer)?;
    let pointer = pointer.into_iter().map(usize::from_str).collect::<Result<Vec<usize>, ParseIntError>>()?;
    Ok((pointer, key))
}
