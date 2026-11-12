/// The application that is frontmost right now (the likely copy source).
pub trait Frontmost {
    /// Its user-facing name, or `None` if unknown on this platform.
    fn app_name(&self) -> Option<String>;
}

#[cfg(test)]
pub mod fakes {
    use super::*;

    pub struct FakeFrontmost(pub Option<String>);

    impl Frontmost for FakeFrontmost {
        fn app_name(&self) -> Option<String> {
            self.0.clone()
        }
    }
}
