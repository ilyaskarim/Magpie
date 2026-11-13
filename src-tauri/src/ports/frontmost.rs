use crate::domain::TargetApp;

/// The application that is frontmost right now (the likely copy source, and
/// the paste-back target).
pub trait Frontmost {
    /// Its user-facing name, or `None` if unknown on this platform.
    fn app_name(&self) -> Option<String>;

    /// Its process id and name, or `None` if unknown or if Magpie itself is
    /// frontmost.
    fn app(&self) -> Option<TargetApp>;
}

#[cfg(test)]
pub mod fakes {
    use super::*;

    pub struct FakeFrontmost(pub Option<String>);

    impl Frontmost for FakeFrontmost {
        fn app_name(&self) -> Option<String> {
            self.0.clone()
        }

        fn app(&self) -> Option<TargetApp> {
            self.0.clone().map(|name| TargetApp {
                pid: 42,
                bundle_id: Some("com.example.fake".into()),
                name: Some(name),
            })
        }
    }
}
