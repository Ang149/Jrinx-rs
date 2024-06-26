pub(super) mod status {
    use core::time::Duration;

    use jrinx_hal::{Cpu, Hal, Interrupt};
    use jrinx_testdef::testdef;
    use jrinx_timed_event::{TimedEvent, TimedEventHandler, TimedEventStatus};
    use jrinx_trap::timer_int;
    use spin::Mutex;

    #[testdef]
    fn test() {
        static DATA: Mutex<Option<TimedEventStatus>> = Mutex::new(None);

        TimedEvent::create(
            hal!().cpu().get_time() + Duration::from_secs(1),
            TimedEventHandler::new(
                || {
                    *DATA.lock() = Some(TimedEventStatus::Timeout);
                },
                || {
                    *DATA.lock() = Some(TimedEventStatus::Cancelled);
                },
            ),
        );

        let timer_int_count = timer_int::count();

        while timer_int::count() == timer_int_count {
            hal!().interrupt().wait();
        }

        assert_eq!(*DATA.lock(), Some(TimedEventStatus::Timeout));

        let tracker = TimedEvent::create(
            hal!().cpu().get_time() + Duration::from_secs(1),
            TimedEventHandler::new(
                || {
                    *DATA.lock() = Some(TimedEventStatus::Timeout);
                },
                || {
                    *DATA.lock() = Some(TimedEventStatus::Cancelled);
                },
            ),
        );
        tracker.cancel().unwrap();

        assert_eq!(*DATA.lock(), Some(TimedEventStatus::Cancelled));
        assert!(jrinx_timed_event::with_current(|tq| tq.peek_outdated()).is_none());
    }
}

pub(super) mod queue {
    use core::time::Duration;

    use alloc::vec::Vec;
    use jrinx_hal::{Cpu, Hal, Interrupt};
    use jrinx_testdef::testdef;
    use jrinx_timed_event::{TimedEvent, TimedEventHandler};
    use jrinx_trap::timer_int;
    use spin::Mutex;

    #[testdef]
    fn test() {
        const EVENT_MAX: usize = 3;
        static ORDER: Mutex<Vec<usize>> = Mutex::new(Vec::new());

        fn order_push(order: usize) {
            ORDER.lock().push(order);
        }

        for i in (1..=EVENT_MAX).rev() {
            let this_order = i;
            TimedEvent::create(
                hal!().cpu().get_time() + Duration::from_secs(i as u64),
                TimedEventHandler::new(
                    move || {
                        order_push(this_order);
                    },
                    || panic!("this timed-event should not be cancelled"),
                ),
            );
        }

        for i in 1..=EVENT_MAX {
            let timer_int_count = timer_int::count();
            while timer_int::count() == timer_int_count {
                hal!().interrupt().wait();
            }
            assert_eq!(ORDER.lock().len(), i);
        }

        let order = ORDER.lock();
        for i in 0..EVENT_MAX {
            assert_eq!(order[i], i + 1);
        }
    }
}
