use Error;

pub fn split(pointer_str: &str) -> Result<Vec<&str>, Error> {
    if !(pointer_str.is_empty() || pointer_str.starts_with("/")) {
        return Err(Error::DoesNotExist)
    }
    Ok(pointer_str.split("/").skip(1).collect())
}

pub fn split_xml_children(pointer_str: &str) -> Result<Vec<usize>, Error> {
    if !pointer_str.starts_with("/") { return Err(Error::InvalidPointer) }
    pointer_str.split("/").skip(1).map(|s| usize::from_str(s)).collect()
}

pub fn split_xml_nodes(pointer_str: &str) -> Result<(Vec<usize>, &str), Error> {
    if !pointer_str.starts_with("/") { return Err(Error::InvalidPointer) }
    let mut pointer = pointer_str.split("/").skip(1).collect();
    let key = pointer.pop().ok_or(Error::InvalidPointer)?;
    let pointer = pointer.into_iter().map(|s| usize::from_str(s)).collect()?;
    Ok((pointer, key))
}
