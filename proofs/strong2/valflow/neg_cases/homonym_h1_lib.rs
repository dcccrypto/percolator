pub enum r#Result<T, E> {
    Err(E),
    Ok(T),
}
