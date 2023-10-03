use core::panic::PanicInfo;

use crate::{arch, error::HaltReason};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        error!(
            "panicked at {}:{} {}",
            location.file(),
            location.line(),
            info.message().unwrap()
        );
    } else {
        error!("panicked: {}", info.message().unwrap());
    }
    arch::halt(HaltReason::SysFailure);
}
