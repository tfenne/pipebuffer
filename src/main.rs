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

use std::env;
use std::io;
use std::io::{Read,Write};
use std::sync::{Arc, Mutex, Condvar};
use std::thread;
use ringbuffer::RingBuffer;


/// Main program that uses a pair of threads to move data from Stdin to Stdout
/// with a RungBuffer in the middle.
pub fn main() {
    // Grab the arguments array
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 { usage_and_exit("No buffer_size supplied.") }
    

    let buffer_size : usize = match args[1].parse() {
        Ok(n)  => n,
        Err(_) => usage_and_exit("Non-numeric buffer_size.")
    };
    
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

// Prints the usage and then exits with error code 1
fn usage_and_exit(msg: &str) -> ! {
    let name = env::args().next().expect("no args[0]!");
    println!("Usage: {} buffer_size", name);
    println!("Error: {}", msg);
    std::process::exit(1)    
}