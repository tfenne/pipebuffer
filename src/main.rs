use std::env;
use std::io;
use std::io::{Read,Write};
use std::cmp;
use std::sync::{Arc, Mutex, Condvar};
use std::thread;

////////////////////////////////////////////////////////////////////////////////
/// A wrapper around a vector of a fixed size to implement a ring-buffer
////////////////////////////////////////////////////////////////////////////////
struct RingBuffer {
    capacity          : usize,
    buffer            : Vec<u8>,
    write_pos         : usize,
    available_to_write: usize,
    read_pos          : usize,
    available_to_read : usize,
    closed            : bool
}

impl RingBuffer {
    // Constructs a new ring buffer of the requested size
    fn new (size: usize) -> RingBuffer {
        RingBuffer {
            capacity           : size,
            buffer             : vec![0; size], // only touch via slices
            write_pos          : 0,
            available_to_write : size,
            read_pos           : 0,
            available_to_read  : 0,
            closed             : false
        }
    }
    
    // Attempts to put items from the slice into the buffer; returns the number actually put
    fn put(&mut self, input: &[u8]) -> usize {
        if self.closed { panic!("Cannot write to closed buffer."); }
        if self.available_to_write == 0 { return 0; }
        
        let distance_to_end = self.capacity - self.write_pos;
        let available       = cmp::min(distance_to_end, self.available_to_write);
        let length          = cmp::min(available, input.len());
        let target_slice = &mut self.buffer[self.write_pos..self.write_pos+length];
        let source_slice = &input[0..length];
        target_slice.clone_from_slice(source_slice);
        self.available_to_write -= length;
        self.available_to_read  += length;
        self.write_pos           = (self.write_pos + length) % self.capacity;        
        length
    }
    
    // Attempts to retrieve bytes from the buffer into the output slice
    fn get(&mut self, output: &mut [u8]) -> usize {
        let distance_to_end = self.capacity - self.read_pos;
        let available       = cmp::min(distance_to_end, self.available_to_read);
        let length          = cmp::min(available, output.len());
        let source_slice = & self.buffer[self.read_pos..self.read_pos+length];
        let target_slice = &mut output[0..length];
        target_slice.clone_from_slice(source_slice);
        self.available_to_read  -= length;
        self.available_to_write += length;
        self.read_pos = (self.read_pos + length) % self.capacity;
        length
    }
    
    fn close(&mut self) -> () {
        self.closed = true;
    }
    
    fn is_closed(&self) -> bool {
        self.closed
    }
}

#[test]
fn test_basic_read_write() {    
    let mut buffer = RingBuffer::new(100);
    let xs: [u8; 10] = [0,1,2,3,4,5,6,7,8,9];
    let mut ys: [u8; 10] = [0; 10];
    for iteration in 0..100 {
        buffer.put(&xs);
        buffer.get(&mut ys);
        for i in 0..10 {
            assert!(xs[i] == ys[i]);
        }
    }
}

#[test]
fn test_write_on_full_buffer() {    
    let mut buffer = RingBuffer::new(10);
    let xs: [u8; 10] = [0,1,2,3,4,5,6,7,8,9];
    buffer.put(&xs);
    let n = buffer.put(&xs);
    assert!(n == 0);
}

#[test]
fn test_read_on_empty_buffer() {
    let mut buffer = RingBuffer::new(10);
    let mut xs: [u8; 10] = [7; 10];
    let n = buffer.get(&mut xs);
    assert!(n == 0);
    for i in 0..10 { assert!(xs[i] == 7); }
}


////////////////////////////////////////////////////////////////////////////////
// Main program that uses a pair of threads to move data from Stdin to Stdout
// with a RungBuffer in the middle.
////////////////////////////////////////////////////////////////////////////////
fn main() {
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
                while buffer.available_to_read == 0 && !buffer.is_closed() {
                    buffer = cond.wait(buffer).unwrap();
                }
                
                let n = buffer.get(&mut bytes);
                if n > 0 {
                    let mut start = 0;
                    while start < n { start += output.write(&bytes[start..n]).unwrap(); }
                    output.flush().unwrap();
                    cond.notify_one();
                }
                else if buffer.available_to_read == 0 && buffer.is_closed() {
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
                if buffer.available_to_write == 0 {
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