pub mod cleaner;
pub mod filesystem;
pub mod matcher;
pub mod walker;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
