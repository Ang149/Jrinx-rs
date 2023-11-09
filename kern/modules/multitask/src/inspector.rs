use core::{fmt::Display, pin::Pin};

use alloc::{boxed::Box, collections::BTreeMap};
use jrinx_addr::VirtAddr;
use jrinx_error::{InternalError, Result};
use jrinx_hal::{Hal, Interrupt};
use jrinx_serial_id_macro::SerialId;
use jrinx_util::fastpq::FastPriorityQueueWithLock;

use crate::{
    arch,
    executor::{Executor, ExecutorId, ExecutorPriority, ExecutorStatus},
    runtime::{Runtime, RuntimeStatus},
};

type ExecutorQueue = FastPriorityQueueWithLock<ExecutorPriority, ExecutorId>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, SerialId)]
pub struct InspectorId(u64);

impl Display for InspectorId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorMode {
    Bootstrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorStatus {
    Idle,
    Running(ExecutorId),
    Finished,
}

pub struct Inspector {
    id: InspectorId,
    mode: InspectorMode,
    status: InspectorStatus,
    executor_registry: BTreeMap<ExecutorId, Pin<Box<Executor>>>,
    executor_queue: ExecutorQueue,
}

impl Inspector {
    pub fn new(mode: InspectorMode, root_executor: Pin<Box<Executor>>) -> Self {
        let mut inspector = Self {
            id: InspectorId::new(),
            mode,
            status: InspectorStatus::Idle,
            executor_registry: BTreeMap::new(),
            executor_queue: ExecutorQueue::new(),
        };

        inspector.register_executor(root_executor).unwrap();

        inspector
    }

    pub fn id(&self) -> InspectorId {
        self.id
    }

    pub fn mode(&self) -> InspectorMode {
        self.mode
    }

    pub fn status(&self) -> InspectorStatus {
        self.status
    }

    pub fn is_empty(&self) -> bool {
        self.executor_registry.is_empty()
    }

    pub fn register_executor(&mut self, executor: Pin<Box<Executor>>) -> Result<()> {
        let id = executor.id();
        let priority = executor.priority();
        self.executor_registry
            .try_insert(id, executor)
            .map_err(|_| InternalError::DuplicateExecutorId)?;
        self.executor_queue.enqueue(priority, id);
        Ok(())
    }

    pub fn unregister_executor(&mut self, executor_id: ExecutorId) -> Result<()> {
        self.executor_registry
            .remove(&executor_id)
            .ok_or(InternalError::InvalidExecutorId)?;
        Ok(())
    }

    pub fn mark_finished(&mut self) {
        self.status = InspectorStatus::Finished;
    }

    pub fn with_current<F, R>(f: F) -> Result<R>
    where
        F: FnOnce(&mut Inspector) -> R,
    {
        Runtime::with_current(|rt| {
            let f = |is: &mut _| f(is);
            let RuntimeStatus::Running(inspector_id) = rt.status() else {
                return Err(InternalError::InvalidRuntimeStatus);
            };
            rt.with_inspector(inspector_id, f)
        })?
    }

    pub(crate) fn with_executor<F, R>(&mut self, id: ExecutorId, f: F) -> Result<R>
    where
        F: FnOnce(&mut Pin<Box<Executor>>) -> R,
    {
        let executor = self
            .executor_registry
            .get_mut(&id)
            .ok_or(InternalError::InvalidExecutorId)?;
        Ok(f(executor))
    }

    pub(crate) fn pop_executor(&mut self) -> Option<ExecutorId> {
        self.executor_queue.dequeue().map(|(_, id)| id)
    }

    pub(crate) fn push_executor(&mut self, id: ExecutorId) -> Result<()> {
        let Some(executor) = self.executor_registry.get(&id) else {
            return Err(InternalError::InvalidExecutorId);
        };
        self.executor_queue.enqueue(executor.priority(), id);
        Ok(())
    }

    pub(crate) fn set_executor(&mut self, id: Option<ExecutorId>) {
        if let Some(id) = id {
            match self.status {
                InspectorStatus::Running(ref mut executor_id) => {
                    *executor_id = id;
                }
                _ => {
                    self.status = InspectorStatus::Running(id);
                }
            }
        } else {
            self.status = InspectorStatus::Idle;
        }
    }

    pub(crate) fn run(runtime_switch_ctx: VirtAddr) {
        loop {
            if Runtime::with_current(|rt| rt.get_inspector_switch_pending()).unwrap()
                || Inspector::with_current(|is| is.status() == InspectorStatus::Finished).unwrap()
            {
                break;
            }

            let Some(executor_id) = Inspector::with_current(|is| is.pop_executor()).unwrap() else {
                hal!().interrupt().wait();
                continue;
            };
            trace!("switch into executor {:?}", executor_id);

            Inspector::with_current(|is| {
                is.set_executor(Some(executor_id));
            })
            .unwrap();

            let executor_switch_ctx = Executor::with_current(|ex| ex.switch_context()).unwrap();

            unsafe {
                arch::switch(
                    runtime_switch_ctx.as_usize(),
                    executor_switch_ctx.as_usize(),
                );
            }

            Inspector::with_current(|is| is.set_executor(None)).unwrap();

            trace!("switch from executor {:?}", executor_id);

            if Inspector::with_current(|is| {
                is.with_executor(executor_id, |ex| ex.status() == ExecutorStatus::Finished)
                    .unwrap()
            })
            .unwrap()
            {
                Inspector::with_current(|is| {
                    is.unregister_executor(executor_id).unwrap();
                })
                .unwrap();
            } else {
                Inspector::with_current(|is| {
                    is.push_executor(executor_id).unwrap();
                })
                .unwrap();
            }

            if Inspector::with_current(|is| is.is_empty() && is.mode() == InspectorMode::Bootstrap)
                .unwrap()
            {
                Inspector::with_current(|is| is.mark_finished()).unwrap();
            }
        }
    }
}