use std::sync::Arc;

use crate::error::Result;

pub trait Clipboard {
    fn get_contents(&self) -> Result<String>;
    fn set_contents(&self, contents: String) -> Result<()>;
}

pub fn create() -> Result<Arc<dyn Clipboard>> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "system-clipboard")] {
            system::create()
        } else {
            local::create()
        }
    }
}

#[cfg(feature = "system-clipboard")]
mod system {
    use crossclip::Clipboard;
    use parking_lot::RwLock;
    use std::sync::Arc;

    use crate::error::Result;

    pub(crate) fn create() -> Result<Arc<dyn super::Clipboard>> {
        Ok(SystemClipboard::new().map(std::sync::Arc::new)?)
    }

    struct SystemClipboard {
        context: RwLock<crossclip::SystemClipboard>,
    }

    impl SystemClipboard {
        fn new() -> Result<Self> {
            let context = crossclip::SystemClipboard::new()?;
            let context = RwLock::new(context);
            Ok(Self { context })
        }
    }

    impl super::Clipboard for SystemClipboard {
        fn get_contents(&self) -> Result<String> {
            Ok(self.context.write().get_string_contents()?)
        }

        fn set_contents(&self, contents: String) -> Result<()> {
            self.context.write().set_string_contents(contents)?;
            Ok(())
        }
    }
}

#[cfg(not(feature = "system-clipboard"))]
mod local {
    use parking_lot::RwLock;
    use std::sync::Arc;

    use super::Clipboard;
    use crate::error::Result;

    pub(crate) fn create() -> Result<Arc<dyn Clipboard>> {
        Ok(Arc::new(LocalClipboard::new()))
    }

    struct LocalClipboard {
        contents: RwLock<String>,
    }

    impl LocalClipboard {
        fn new() -> Self {
            Self {
                contents: RwLock::new(String::new()),
            }
        }
    }

    impl Clipboard for LocalClipboard {
        fn get_contents(&self) -> Result<String> {
            Ok(self.contents.read().clone())
        }

        fn set_contents(&self, contents: String) -> Result<()> {
            *self.contents.write() = contents;
            Ok(())
        }
    }
}
