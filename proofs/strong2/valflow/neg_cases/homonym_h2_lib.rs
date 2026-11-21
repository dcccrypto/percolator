pub mod core {
    pub mod option {
        pub enum r#Option<T> {
            Some(T),
            None,
        }
    }
}
