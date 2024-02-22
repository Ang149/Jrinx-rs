use jrinx_hal::{hal, Hal, Interrupt};
use spin::RwLock;

use crate::{GenericContext, TrapReason};

static SOFT_INT_COUNTER: RwLock<u64> = RwLock::new(0);

pub fn handle(ctx: &mut impl GenericContext) {
    let TrapReason::SoftwareInterrupt = ctx.trap_reason() else {
        panic!("not a software interrupt");
    };

    *SOFT_INT_COUNTER.write() += 1;

    hal!().interrupt().clr_soft();
}
pub fn software_interrupt_handler() {
    let mut counter = SOFT_INT_COUNTER.write();
    *counter += 1;
    hal!().interrupt().clr_soft();
}
pub fn count() -> u64 {
    *SOFT_INT_COUNTER.read()
}
