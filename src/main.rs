// The MIT License (MIT)
//
// Copyright (c) 2016 Tim Fennell
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! 
//! Command line program that can be sandwiched between pipes to effectively increase
//! the size of the pipe buffer.  Since linux pipes are generally limited to `64k` it
//! is sometimes useful to provide significantly more buffering between programs in a
//! pipe in order to smooth out any "lumpiness" in the flow of data.
//! 

mod ringbuffer;

#[macro_use] extern crate clap;
extern crate regex;

use std::io;
use std::io::{Read,Write};
use std::sync::{Arc, Mutex, Condvar};
use std::thread;
use clap::{Arg, App};
use ringbuffer::RingBuffer;
use regex::Regex;

/// Main function that coordinates argument parsing and then delegates to the
/// `run()` function to do the actual work.
pub fn main() {
    let matches =
        App::new("pipebuffer")
            .version(crate_version!())
            .about("A tool to rapidly buffer and re-emit data in unix pipelines.")
            .arg(Arg::with_name("size")
                     .short("s").long("size")
                     .help("The size, in bytes or with k[b]/m[b]/g[b] suffix.")
                     .default_value("256m"))
            .get_matches();

    let buffer_size = match parse_memory(matches.value_of("size").unwrap()) {
        Some(size) => size,
        None       => {
            println!("{}", matches.usage());
            println!("Error: Argument {} is not a valid size.", matches.value_of("size").unwrap());
            std::process::exit(1)
        }
    };

    run(buffer_size);
}

/// Parses memory unit values from strings. Specifically accepts any value
/// that is an integer number followed optionally by `k/kb/m/mb/g/gb/p/pb` in
/// either upper or lower case. If the value can be parsed returns a 
/// `Some(bytes)`, otherwise returns a None.
fn parse_memory(s: &str) -> Option<usize> {
    match Regex::new("^([0-9]+)([kmgp])b?").unwrap().captures(&s.to_lowercase()) {
        None => None,
        Some(groups) => {
            let num : usize = groups.at(1).unwrap().parse().unwrap();
            let exp = match groups.at(2) {
                Some("k") => 1,
                Some("m") => 2,
                Some("g") => 3,
                Some("p") => 4,
                _         => panic!("Unreachable")
            };
            Some(num * (1024 as usize).pow(exp))
        }
    }
}

/// Funtion that uses a pair of threads to move data from Stdin to Stdout
/// with a RungBuffer in the middle.
fn run(buffer_size: usize) {
    // The shared ring buffer and the thread handles
    let ring = Arc::new(Mutex::new(RingBuffer::new(buffer_size)));
    let cond = Arc::new(Condvar::new());

    // Setup the writer thread
    let writer_handle = {
        let ring = ring.clone();
        let cond = cond.clone();
        thread::spawn(move || {
            let mut bytes: [u8; 32000] = [0; 32000];
            let mut output = io::stdout();
            loop {
                // writeln!(&mut io::stderr(), "In stdout writing loop.").unwrap();
                let mut buffer = ring.lock().unwrap();
                while buffer.is_empty()  && !buffer.is_closed() {
                    buffer = cond.wait(buffer).unwrap();
                }
                
                let n = buffer.get(&mut bytes);
                if n > 0 {
                    let mut start = 0;
                    while start < n { start += output.write(&bytes[start..n]).unwrap(); }
                    output.flush().unwrap();
                    cond.notify_one();
                }
                else if buffer.is_empty() && buffer.is_closed() {
                    break;
                }
            }
        })
    };

    // Setup this thread as the reader thread
    let mut bytes: [u8; 32000] = [0; 32000];
    let mut input = io::stdin();
    loop {
        // writeln!(&mut io::stderr(), "In stdin reading loop.").unwrap();
        let mut buffer = ring.lock().unwrap();
        let n = input.read(&mut bytes).unwrap();
        
        if n == 0 { // input stream is closed
            // writeln!(&mut io::stderr(), "Stdin is closed.").unwrap();
            buffer.close();
            cond.notify_one();
            break; 
        }
        else {
            let mut start = 0;
            while start < n {
                if buffer.is_full() {
                    buffer = cond.wait(buffer).unwrap();
                }
                start += buffer.put(&bytes[start..n]);
                cond.notify_one();
             }
             // writeln!(&mut io::stderr(), "Put {} bytes into the buffer and unparking the writer.", n).unwrap();
        }
    }
    
    writeln!(&mut io::stderr(), "Attempting to join on the writer.").unwrap();
    writer_handle.join().unwrap();
}
