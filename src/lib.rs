//! *“I'm late! I'm late! For a very important date!”*
//! *by “The White Rabbit”* 『Alice's Adventures in Wonderland』
//!
//! `white_rabbit` schedules your tasks and can repeat them!
//!
//! One funny use case are chat bot commands: Imagine a *remind me*-command,
//! the command gets executed and you simply create a one-time job to be
//! scheduled for whatever time the user desires.
//!
//! We are using chrono's `DateTime<Utc>`, enabling you to serialise and thus
//! backup currently running tasks,
//! in case you want to shutdown/restart your application,
//! constructing a new scheduler is doable.
//! However, please make sure your internal clock is synced.
#![deny(rust_2018_idioms)]
use chrono::Duration as ChronoDuration;
use parking_lot::{Condvar, Mutex, RwLock};
use std::{cmp::Ordering, collections::BinaryHeap, sync::Arc, time::Duration as StdDuration};
use threadpool::ThreadPool;

pub use chrono::{DateTime, Duration, Utc};

/// Compare if an `enum`-variant matches another variant.
macro_rules! cmp_variant {
    ($expression:expr, $($variant:tt)+) => {
        match $expression {
            $($variant)+ => true,
            _ => false
        }
    }
}

/// When a task is due, this will be passed to the task.
/// Currently, there is not much use to this. However, this might be extended
/// in the future.
pub struct Context {
    time: DateTime<Utc>,
}

/// Every task will return this `enum`.
pub enum DateResult {
    /// The task is considered finished and can be fully removed.
    Done,
    /// The task will be scheduled for a new date on passed `DateTime<Utc>`.
    Repeat(DateTime<Utc>),
}

/// Every job gets a planned `Date` with the scheduler.
pub struct Date {
    pub context: Context,
    pub job: Box<dyn FnMut(&mut Context) -> DateResult + Send + Sync + 'static>,
}

impl Eq for Date {}

/// Invert comparisions to create a min-heap.
impl Ord for Date {
    fn cmp(&self, other: &Date) -> Ordering {
        match self.context.time.cmp(&other.context.time) {
            Ordering::Less => Ordering::Greater,
            Ordering::Greater => Ordering::Less,
            Ordering::Equal => Ordering::Equal,
        }
    }
}

/// Invert comparisions to create a min-heap.
impl PartialOrd for Date {
    fn partial_cmp(&self, other: &Date) -> Option<Ordering> {
        Some(match self.context.time.cmp(&other.context.time) {
            Ordering::Less => Ordering::Greater,
            Ordering::Greater => Ordering::Less,
            Ordering::Equal => Ordering::Equal,
        })
    }
}

impl PartialEq for Date {
    fn eq(&self, other: &Date) -> bool {
        self.context.time == other.context.time
    }
}

/// The [`Scheduler`]'s worker thread switches through different states
/// while running, each state changes the behaviour.
///
/// [`Scheduler`]: struct.Scheduler.html
enum SchedulerState {
    /// No dates being awaited, sleep until one gets added.
    PauseEmpty,
    /// Pause until next date is due.
    PauseTime(StdDuration),
    /// If the next date is already waiting to be executed,
    /// the thread continues running without sleeping.
    Run,
    /// Exits the thread.
    Exit,
}

impl SchedulerState {
    fn is_running(&self) -> bool {
        cmp_variant!(*self, SchedulerState::Run)
    }

    fn is_paused(&self) -> bool {
        cmp_variant!(*self, SchedulerState::PauseEmpty)
            || cmp_variant!(*self, SchedulerState::PauseTime(_))
    }

    fn new_pause_time(duration: ChronoDuration) -> Self {
        SchedulerState::PauseTime(
            duration
                .to_std()
                .unwrap_or_else(|_| StdDuration::from_millis(0)),
        )
    }
}

/// This scheduler exists on two levels: The handle, granting you the
/// ability of adding new tasks, and the executor, dating and executing these
/// tasks when specified time is met.
///
/// **Info**: This scheduler may not be precise due to anomalies such as
/// preemption or platform differences.
pub struct Scheduler {
    /// The mean of communication with the running scheduler.
    condvar: Arc<(Mutex<SchedulerState>, Condvar)>,
    /// Every job has its date listed inside this.
    dates: Arc<RwLock<BinaryHeap<Date>>>,
}

impl Scheduler {
    /// Add a task to be executed when `time` is reached.
    pub fn add_task_datetime<T>(&mut self, time: DateTime<Utc>, to_execute: T)
    where
        T: FnMut(&mut Context) -> DateResult + Send + Sync + 'static,
    {
        let &(ref state_lock, ref notifier) = &*self.condvar;

        let task = Date {
            context: Context { time },
            job: Box::new(to_execute),
        };

        let mut locked_heap = self.dates.write();

        if locked_heap.is_empty() {
            let mut scheduler_state = state_lock.lock();
            let left = task.context.time.signed_duration_since(Utc::now());

            if !scheduler_state.is_running() {
                *scheduler_state = SchedulerState::new_pause_time(left);
                notifier.notify_one();
            }
        } else {
            let mut scheduler_state = state_lock.lock();

            if let SchedulerState::PauseTime(_) = *scheduler_state {
                let peeked = locked_heap.peek().expect("Expected heap to be filled.");

                if task.context.time < peeked.context.time {
                    let left = task.context.time.signed_duration_since(Utc::now());

                    if !scheduler_state.is_running() {
                        *scheduler_state = SchedulerState::PauseTime(
                            left.to_std()
                                .unwrap_or_else(|_| StdDuration::from_millis(0)),
                        );
                        notifier.notify_one();
                    }
                }
            }
        }

        locked_heap.push(task);
    }

    pub fn add_task_duration<T>(&mut self, how_long: ChronoDuration, to_execute: T)
    where
        T: FnMut(&mut Context) -> DateResult + Send + Sync + 'static,
    {
        let time = Utc::now() + how_long;
        self.add_task_datetime(time, to_execute);
    }
}

fn set_state_lock(state_lock: &Mutex<SchedulerState>, to_set: SchedulerState) {
    let mut state = state_lock.lock();
    *state = to_set;
}

/// This function pushes a `date` onto `data_pooled` and notifies the
/// dispatching-thread in case they are sleeping.
#[inline]
fn push_and_notfiy(
    dispatcher_pair: &Arc<(Mutex<SchedulerState>, Condvar)>,
    data_pooled: &Arc<RwLock<BinaryHeap<Date>>>,
    when: &DateTime<Utc>,
    date: Date,
) {
    let &(ref state_lock, ref notifier) = &**dispatcher_pair;

    let mut state = state_lock.lock();

    let mut heap_lock = data_pooled.write();

    if let Some(peek) = heap_lock.peek() {
        if peek.context.time < *when {
            let left = peek.context.time.signed_duration_since(Utc::now());

            *state = SchedulerState::new_pause_time(left);
            heap_lock.push(date);
            notifier.notify_one();
        } else {
            let left = when.signed_duration_since(Utc::now());

            *state = SchedulerState::new_pause_time(left);
            heap_lock.push(date);
            notifier.notify_one();
        }
    } else {
        let left = when.signed_duration_since(Utc::now());
        heap_lock.push(date);

        *state = SchedulerState::new_pause_time(left);
        notifier.notify_one();
    }

}

#[must_use]
enum Break {
    Yes,
    No,
}

#[inline]
fn process_states(state_lock: &Mutex<SchedulerState>, notifier: &Condvar) -> Break {
    let mut scheduler_state = state_lock.lock();

    while let SchedulerState::PauseEmpty = *scheduler_state {
        notifier.wait(&mut scheduler_state);
    }

    while let SchedulerState::PauseTime(duration) = *scheduler_state {
        if notifier
            .wait_for(&mut scheduler_state, duration)
            .timed_out()
        {
            break;
        }
    }

    if let SchedulerState::Exit = *scheduler_state {
        return Break::Yes;
    }

    Break::No
}

fn dispatch_date(
    threadpool: &ThreadPool,
    dates: &Arc<RwLock<BinaryHeap<Date>>>,
    pair_scheduler: &Arc<(Mutex<SchedulerState>, Condvar)>,
) {
    let mut date = {
        let mut dates = dates.write();

        dates.pop().expect("Should not run on empty heap.")
    };

    let date_dispatcher = dates.clone();
    let dispatcher_pair = pair_scheduler.clone();

    threadpool.execute(move || {
        if let DateResult::Repeat(when) = (date.job)(&mut date.context) {
            date.context.time = when;

            push_and_notfiy(&dispatcher_pair, &date_dispatcher, &when, date);
        }
    });
}

fn check_peeking_date(dates: &Arc<RwLock<BinaryHeap<Date>>>, state_lock: &Mutex<SchedulerState>) {
    if let Some(next) = dates.read().peek() {
        let now = Utc::now();

        if next.context.time > now {
            let left = next.context.time.signed_duration_since(now);

            set_state_lock(&state_lock, SchedulerState::new_pause_time(left));
        } else {
            set_state_lock(&state_lock, SchedulerState::Run);
        }
    } else {
        set_state_lock(&state_lock, SchedulerState::PauseEmpty);
    }
}

impl Scheduler {
    /// Creates a new [`Scheduler`] which will use `thread_count` number of
    /// threads when tasks are being dispatched/dated.
    ///
    /// [`Scheduler`]: struct.Scheduler.html
    pub fn new(thread_count: usize) -> Self {
        let pair = Arc::new((Mutex::new(SchedulerState::PauseEmpty), Condvar::new()));
        let pair_scheduler = pair.clone();
        let dates: Arc<RwLock<BinaryHeap<Date>>> = Arc::new(RwLock::new(BinaryHeap::new()));

        let dates_scheduler = Arc::clone(&dates);
        std::thread::spawn(move || {
            let &(ref state_lock, ref notifier) = &*pair_scheduler;
            let threadpool = ThreadPool::new(thread_count);

            loop {
                if let Break::Yes = process_states(&state_lock, &notifier) {
                    break;
                }

                dispatch_date(&threadpool, &dates_scheduler, &pair_scheduler);

                check_peeking_date(&dates_scheduler, &state_lock);
            }
        });

        Scheduler {
            condvar: pair,
            dates,
        }
    }
}

/// Once the scheduler is dropped, we also need to join and finish the thread.
impl<'a> Drop for Scheduler {
    fn drop(&mut self) {
        let &(ref state_lock, ref notifier) = &*self.condvar;

        let mut state = state_lock.lock();
        *state = SchedulerState::Exit;
        notifier.notify_one();
    }
}
