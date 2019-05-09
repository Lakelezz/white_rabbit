[![ci-badge][]][ci] [![docs-badge][]][docs] [![rust badge]][rust link] [![crates.io version]][crates.io link]

## *“I'm late! I'm late! For a very important date!”*
*by “The White Rabbit”* 『Alice's Adventures in Wonderland』

# About

`white_rabbit` schedules your tasks and can repeat them!

One funny use case are chat bots, e.g. a *remind me*-command that might be
repeated after a user-specified time.

We are using chrono's `DateTime<Utc>`, enabling you to serialise and thus backup currently
running tasks, in case you want to shutdown/restart your application,
constructing a new scheduler is doable. However, please make sure your internal
clock is synced.

Everyone is welcome to contribute, check out
the [`CONTRIBUTING.md`](CONTRIBUTING.md) for further guidance.

# Example
Let's have a look at a code-example:

```rust
use white_rabbit::{DateResult, Duration, Scheduler};

fn main() {
    let mut scheduler = Scheduler::new(4);

    scheduler.add_task_duration(Duration::seconds(5), |_| {
        println!("I'm here!");

        DateResult::Done
    });
}
```

# Precision

The scheduler sleeps when unneeded. Due to preemption or different
implementations across operating systems, it may occur that the scheduler will
sleep longer than intended.

Be aware, an incorrect system clock may lead to tasks being run earlier or
later.

# Installation
Add this to your `Cargo.toml`:

```toml
[dependencies]
white_rabbit = "0.1.0"
```

[ci]: https://dev.azure.com/lakeware/white_rabbit/_build?definitionId=7
[ci-badge]: https://img.shields.io/azure-devops/build/lakeware/dfa18c4f-23ad-4c36-810f-144481ab4c93/7/master.svg?style=flat-square

[docs-badge]: https://img.shields.io/badge/docs-online-5023dd.svg?style=flat-square&colorB=32b6b7
[docs]: https://docs.rs/white_rabbit

[rust badge]: https://img.shields.io/badge/rust-1.34.1+-93450a.svg?style=flat-square&colorB=ff9a0d
[rust link]: https://blog.rust-lang.org/2019/04/25/Rust-1.34.1.html

[crates.io link]: https://crates.io/crates/white_rabbit
[crates.io version]: https://img.shields.io/crates/v/white_rabbit.svg?style=flat-square&colorB=dfccc7
