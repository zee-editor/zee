use backtrace::Backtrace;
use once_cell::sync::Lazy;
use std::{
    cell::RefCell,
    fmt::{Debug, Formatter},
    panic::{PanicInfo, UnwindSafe},
};

pub fn print_panic_after_unwind<F: FnOnce() -> R + UnwindSafe, R>(function: F) -> R {
    std::panic::set_hook(Box::new(save_panic_backtrace_hook));

    match std::panic::catch_unwind(function) {
        Err(err) => {
            eprint!("Internal zee error -- ");
            eprintln!("this is a bug, please submit an issue at https://github.com/zee-editor/zee");
            PANIC_BACKTRACE.with(|cell| {
                if let Some(description) = cell.borrow().as_ref() {
                    eprintln!("{:?}", description)
                }
            });
            std::panic::resume_unwind(err);
        }
        Ok(result) => result,
    }
}

fn save_panic_backtrace_hook(info: &PanicInfo) {
    // The current implementation always returns `Some`.
    let location = info.location().unwrap();

    let message = match info.payload().downcast_ref::<&'static str>() {
        Some(s) => *s,
        None => match info.payload().downcast_ref::<String>() {
            Some(s) => &s[..],
            None => "Box<dyn Any>",
        },
    };
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("<unnamed>");

    let current_backtrace = Backtrace::new();
    PANIC_BACKTRACE.with(|panic_backtrace| {
        *panic_backtrace.borrow_mut() = Some(PanicDescription {
            name: name.to_string(),
            location: location.to_string(),
            message: message.to_string(),
            backtrace: current_backtrace,
        })
    });
}

struct PanicDescription {
    name: String,
    location: String,
    message: String,
    backtrace: Backtrace,
}

impl Debug for PanicDescription {
    fn fmt(&self, fmt: &mut Formatter) -> std::fmt::Result {
        writeln!(
            fmt,
            "Thread '{}' panicked at '{}', {}",
            self.name, self.message, self.location
        )?;
        writeln!(fmt, "{:?}", self.backtrace,)
    }
}

thread_local! {
    static PANIC_BACKTRACE: Lazy<RefCell<Option<PanicDescription>>> =
        Lazy::new(|| RefCell::new(None));
}
