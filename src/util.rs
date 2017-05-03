use std::cmp::Ord;

/// Removes elements from a vec.
pub fn remove_elements<T: Ord>(vec: &mut Vec<T>, removed: &[T]) {
    for element in removed {
        if let Ok(index) = vec.binary_search_by(|e| e.cmp(element)) {
            vec.remove(index);
        }
    }
}
