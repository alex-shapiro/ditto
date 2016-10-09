extern crate rand;

mod array;
mod attributed_string;
mod object;
mod op;
mod sequence;
mod value;

pub use array::Array;
pub use attributed_string::AttributedString;
pub use object::Object;
pub use value::Value;

pub type Site = u32;
pub type Counter = u32;
pub type Index = usize;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
