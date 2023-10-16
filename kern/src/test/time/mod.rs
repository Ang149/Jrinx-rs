pub(super) mod status {
    use core::time::Duration;

    use spin::Mutex;

    use crate::{
        arch, cpudata,
        test::test_define,
        time::{TimedEvent, TimedEventStatus},
    };

    test_define!("time::status" => test);
    fn test() {
        static DATA: Mutex<Option<TimedEventStatus>> = Mutex::new(None);

        TimedEvent::new(
            arch::cpu::time() + Duration::from_secs(1),
            || {
                *DATA.lock() = Some(TimedEventStatus::Timeout);
            },
            || {
                *DATA.lock() = Some(TimedEventStatus::Cancelled);
            },
        );

        arch::wait_for_interrupt();

        assert_eq!(*DATA.lock(), Some(TimedEventStatus::Timeout));

        let tracker = TimedEvent::new(
            arch::cpu::time() + Duration::from_secs(1),
            || {
                *DATA.lock() = Some(TimedEventStatus::Timeout);
            },
            || {
                *DATA.lock() = Some(TimedEventStatus::Cancelled);
            },
        );
        tracker.cancel().unwrap();

        assert_eq!(*DATA.lock(), Some(TimedEventStatus::Cancelled));
        assert!(
            cpudata::with_cpu_timed_event_queue(|queue| queue.peek_outdated())
                .unwrap()
                .is_none()
        );
    }
}

pub(super) mod queue {
    use core::time::Duration;

    use alloc::vec::Vec;
    use spin::Mutex;

    use crate::{arch, test::test_define, time::TimedEvent};

    test_define!("time::queue" => test);
    fn test() {
        const EVENT_MAX: usize = 3;
        static ORDER: Mutex<Vec<usize>> = Mutex::new(Vec::new());

        fn order_push(order: usize) {
            ORDER.lock().push(order);
        }

        for i in (1..=EVENT_MAX).rev() {
            let this_order = i;
            TimedEvent::new(
                arch::cpu::time() + Duration::from_secs(i as u64),
                move || {
                    order_push(this_order);
                },
                move || {
                    order_push(this_order);
                },
            );
        }

        for i in 1..=EVENT_MAX {
            arch::wait_for_interrupt();
            assert_eq!(ORDER.lock().len(), i);
        }

        let order = ORDER.lock();
        for i in 0..EVENT_MAX {
            assert_eq!(order[i], i + 1);
        }
    }
}