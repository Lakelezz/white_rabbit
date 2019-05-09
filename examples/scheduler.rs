//! This example shows you how to use the `Scheduler`.

// `white_rabbit` re-exports `chrono`'s Datetime, Utc, and Duration.
use white_rabbit::{Context, DateResult, Duration, Scheduler, Utc};

// This function's signature matches what the `Scheduler` wants,
// allowing us to provide this to `add_task`.
// `Context` carries the `DateTime<Utc>`, in case you want to print it.
fn do_something(_context: &mut Context) -> DateResult {
    println!("I'm doing something!");

    DateResult::Done
}

fn main() {
    // Create a scheduler with two threads, threads are used for dispatching
    // ready tasks.
    let mut scheduler = Scheduler::new(2);

    // Add task that will be dispatched in 5000ms.
    scheduler.add_task_duration(Duration::milliseconds(5000), |_| {
        println!("Task is ready!");

        DateResult::Done
    });

    // We can calculate the time too.
    let time = Utc::now() + Duration::milliseconds(5000);

    // Does the same as above but takes a `DateTime<Utc>`.
    scheduler.add_task_datetime(time, |_| {
        println!("Task is ready!");

        DateResult::Done
    });

    let mut counter = 0;
    scheduler.add_task_datetime(time, move |_| {
        println!("I got repeated {} times!", counter);
        counter += 1;

        DateResult::Repeat(Utc::now() + Duration::milliseconds(500))
    });

    // One could also just provide a function.
    scheduler.add_task_datetime(time, do_something);
}
