#![no_std]

#[derive(Debug)]
pub enum InternalError {
    RepeatInitialization,
    DevProbeError,
    DevReadError,
    DevWriteError,
    DevBadState,
    DevNetAgain,
    ElfParseError,
    NotEnoughMem,
    InvalidCpuId,
    InvalidVirtAddr,
    DuplicateTaskId,
    InvalidExecutorId,
    DuplicateExecutorId,
    InvalidInspectorId,
    DuplicateInspectorId,
    InvalidInspectorStatus,
    InvalidRuntimeStatus,
    InvalidRuntimeSchedTable,
    InvalidParam,
    DuplicateRuntimeSchedTable,
    InvalidTimedEventStatus,
}

pub type Result<T> = core::result::Result<T, InternalError>;
