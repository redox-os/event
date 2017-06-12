extern crate syscall;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::io::Result as IOResult;
use std::io::Error as IOError;
use std::os::unix::io::RawFd;
use std::convert::From;
use std::result::Result;

pub struct EventQueue<R, E = IOError>
{
    /// The file to read events from
    file: File,
    /// A map of registered file descriptors to their handler callbacks
    callbacks: BTreeMap<RawFd, Box<FnMut(usize) -> Result<Option<R>, E>>>,
}

impl<R, E> EventQueue<R, E>
    where E: From<IOError>
{
    /// Create a new event queue
    pub fn new() -> IOResult<EventQueue<R, E>> {
        Ok(EventQueue {
               file: File::open("event:")?,
               callbacks: BTreeMap::new(),
           })
    }

    /// Add a file to the event queue, calling a callback when an event occurs
    ///
    /// The callback is given a mutable reference to the file and the event data
    /// (typically the length of data available for read)
    ///
    /// The callback returns Ok(None) if it wishes to continue the event loop,
    /// or Ok(Some(R)) to break the event loop and return the value.
    /// Err can be used to allow the callback to return an error, and break the
    /// event loop
    pub fn add<F: FnMut(usize) -> Result<Option<R>, E> + 'static>(&mut self,
                                                                  fd: RawFd,
                                                                  callback: F)
                                                                  -> IOResult<()> {
        syscall::fevent(fd as usize, syscall::EVENT_READ)
            .map_err(|x| IOError::from_raw_os_error(x.errno))?;

        self.callbacks.insert(fd, Box::new(callback));

        Ok(())
    }

    /// Remove a file from the event queue, returning its callback if found
    pub fn remove(&mut self,
                  fd: RawFd)
                  -> IOResult<Option<Box<FnMut(usize) -> Result<Option<R>, E>>>> {
        if let Some(callback) = self.callbacks.remove(&fd) {
            syscall::fevent(fd as usize, 0)
                .map_err(|x| IOError::from_raw_os_error(x.errno))?;

            Ok(Some(callback))
        } else {
            Ok(None)
        }
    }

    /// Send an event to a descriptor callback
    pub fn trigger(&mut self, fd: RawFd, count: usize) -> Result<Option<R>, E> {
        if let Some(callback) = self.callbacks.get_mut(&fd) {
            callback(count)
        } else {
            Ok(None)
        }
    }

    /// Send an event to all descriptor callbacks, useful for cleaning out buffers after init
    pub fn trigger_all(&mut self, count: usize) -> Result<Vec<R>, E> {
        let mut rets = Vec::new();
        for (_fd, callback) in self.callbacks.iter_mut() {
            if let Some(ret) = callback(count)? {
                rets.push(ret);
            }
        }
        Ok(rets)
    }

    /// Process the event queue until a callback returns Some(R)
    pub fn run(&mut self) -> Result<R, E> {
        loop {
            let mut event = syscall::Event::default();
            if self.file.read(&mut event).map_err(E::from)? > 0 {
                if let Some(ret) = self.trigger(event.id as RawFd, event.data)? {
                    return Ok(ret);
                }
            }
        }
    }
}
