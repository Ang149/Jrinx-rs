#![no_std]

#[derive(Debug)]
pub enum InternalError {
    RepeatInitialization,
    DevProbeError,
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
    InvalidInterruptRequestNum,
}

pub type Result<T> = core::result::Result<T, InternalError>;
