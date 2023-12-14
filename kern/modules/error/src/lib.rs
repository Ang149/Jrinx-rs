#![no_std]

#[derive(Debug)]
pub enum InternalError {
    RepeatInitialization,
    DevProbeError,
    DevReadError,
    DevWriteError,
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
    InvalidTimedEventStatus,
}

pub type Result<T> = core::result::Result<T, InternalError>;
