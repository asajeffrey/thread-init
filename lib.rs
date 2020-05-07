/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#![feature(thread_spawn_unchecked)]

//! Add a `spawn_init` function to thread building that allows
//! initialization of the thread to use borrowed data.
//!
//! For example:
//! ```rust
//! let ref hello = String::from("hello");
//! let world = String::from("world");
//! let thread = thread_init::spawn(move || {
//!     // We have access to the borrowed `hello`
//!     let hi = hello.clone();
//!     move || {
//!         // Now we can use the owned data
//!         assert_eq!(hi, "hello");
//!         assert_eq!(world, "world");
//!         world
//!     }
//! });
//! // At this point, the thread is initialized, so we get the borrows back again
//! assert_eq!(hello, "hello");
//! // We need to use `thread.join()` to get back any owned data
//! assert_eq!(thread.join().unwrap(), "world");
//! ```

use std::io;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;

/// A trait for spawning with borrowed initialization.
pub trait SpawnInit {
    fn spawn_init<F, G, T>(self, f: F) -> io::Result<JoinHandle<T>>
    where
        F: Send + FnOnce() -> G,
        G: 'static + FnOnce() -> T,
        T: 'static + Send;
}

impl SpawnInit for thread::Builder {
    fn spawn_init<F, G, T>(self, f: F) -> io::Result<JoinHandle<T>>
    where
        F: Send + FnOnce() -> G,
        G: 'static + FnOnce() -> T,
        T: 'static + Send,
    {
        let (sender, receiver) = mpsc::channel();
        let thread = unsafe {
            self.spawn_unchecked(|| {
                let g = {
                    let _guard = Guard(sender);
                    f()
                };
                g()
            })
        };
        let _ = receiver.recv();
        thread
    }
}

/// A helper function that tries to create a new thread with borrowed initialization.
pub fn try_spawn<F, G, T>(f: F) -> io::Result<JoinHandle<T>>
where
    F: Send + FnOnce() -> G,
    G: 'static + FnOnce() -> T,
    T: 'static + Send,
{
    thread::Builder::new().spawn_init(f)
}

/// A helper function that creates a new thread with borrowed initialization.
pub fn spawn<F, G, T>(f: F) -> JoinHandle<T>
where
    F: Send + FnOnce() -> G,
    G: 'static + FnOnce() -> T,
    T: 'static + Send,
{
    try_spawn(f).expect("Spawning failed")
}

// A guard that will send on the sender when it is dropped
struct Guard(Sender<()>);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn spawn_test() {
        let ref hello = String::from("hello");
        let world = String::from("world");
        let thread = crate::spawn(move || {
            let hi = hello.clone();
            move || {
                assert_eq!(hi, "hello");
                assert_eq!(world, "world");
                world
            }
        });
        assert_eq!(hello, "hello");
        assert_eq!(thread.join().unwrap(), "world");
    }
}
