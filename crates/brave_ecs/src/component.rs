use std::any::Any;

/// Трейт для всех компонентов. Реализуй на любой структуре.
pub trait Component: Any + 'static {}
