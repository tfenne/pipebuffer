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

//! A module that provides a generic ring buffer.

use std::cmp;
use std::clone::Clone;

/// Implementation of a non-blocking, fixed size ring-buffer.
/// Allocates enough space on the heap to store `size` items.  Provides
/// non-blocking methods to `put` into the buffer from a slice and `get`
/// from the buffer into a slice.
///
/// When full (as reported by `is_full()`) the buffer will accept calls to
/// `put`, but will copy nothing into the target slice and will report that
/// zero items were put.  Similarly, when `empty`, calls to `get` will return
/// immediately and report that zero items were retrieved.
///
/// After calls to `close()`, further attempts to put into the buffer will
/// cause panics, but `gets()` continue to be allowed in order to let the 
/// buffer be drained.
pub struct RingBuffer<T: Clone> {
    capacity          : usize,
    buffer            : Vec<T>,
    write_pos         : usize,
    available_to_write: usize,
    read_pos          : usize,
    available_to_read : usize,
    closed            : bool
}

impl<T: Clone> RingBuffer<T> {
    /// Constructs a new RingBuffer with capacity `size`.
    pub fn new (size: usize) -> RingBuffer<T> {
        let mut buf = RingBuffer {
            capacity           : size,
            buffer             : Vec::with_capacity(size),
            write_pos          : 0,
            available_to_write : size,
            read_pos           : 0,
            available_to_read  : 0,
            closed             : false
        };
        
        // Push 'size' up to capacity so we can use slices without having to fill first.
        unsafe { buf.buffer.set_len(size); }
        buf
    }
    
    /// Attempts to `put` items from the slice into the buffer. The only guarantees
    /// made by this method are:
    ///
    /// 1. That if the buffer is not full, one or more items will be put
    /// 2. That the number of items put will be reported correctly by the return
    /// 
    /// Specifically, `put` does not guarantee that all items from the `input` slice
    /// will be put, even in the case where there _is_ capacity for all the `input` items.
    /// For this reason, `put` should generally be called in a loop until all items have
    /// been put.
    ///
    /// # Return
    /// The number of items, `>= 0`, that were put into the buffer.

    /// # Panics
    /// Will panic if invoked on a closed buffer.
    pub fn put(&mut self, input: &[T]) -> usize {
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
    
    /// Attempts to `get` items from the buffer and put them into the slice.
    /// The only guarantees made by this method are:
    ///
    /// 1. That if the buffer is not empty, one or more items will be fetched
    /// 2. That the number of items fetched will be reported correctly by the return
    /// 
    /// Specifically, `get` does not guarantee that the `output` slice will be filled,
    /// even in the case where there _is_ enough in the buffer to do so.
    /// 
    /// # Return
    /// The number of items, `>= 0`, that were fetched from the buffer.
    pub fn get(&mut self, output: &mut [T]) -> usize {
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
    
    /// Returns true if the buffer is currently empty, implying calls to `get()` will 
    /// yield zero items.
    pub fn is_empty(&self) -> bool { self.available_to_read == 0 }

    /// Returns true if the buffer is currently full, implying calls to `put()` will 
    /// consume zero items.
    pub fn is_full(&self) -> bool { self.available_to_write == 0 }
    
    /// Closes the buffer such that future calls to `put()` will panic.
    pub fn close(&mut self) -> () { self.closed = true; }
    
    /// Returns true if the buffer is closed, and false otherwise.
    pub fn is_closed(&self) -> bool { self.closed }
}

#[test]
fn test_basic_read_write() {    
    let mut buffer : RingBuffer<u8> = RingBuffer::new(100);
    let xs: [u8; 10] = [0,1,2,3,4,5,6,7,8,9];
    let mut ys: [u8; 10] = [0; 10];
    for _ in 0..100 {
        buffer.put(&xs);
        buffer.get(&mut ys);
        for i in 0..10 {
            assert!(xs[i] == ys[i]);
        }
    }
}

#[test]
fn test_write_on_full_buffer() {    
    let mut buffer : RingBuffer<u8> = RingBuffer::new(10);
    let xs: [u8; 10] = [0,1,2,3,4,5,6,7,8,9];
    buffer.put(&xs);
    let n = buffer.put(&xs);
    assert!(n == 0);
}

#[test]
fn test_read_on_empty_buffer() {
    let mut buffer : RingBuffer<u8> = RingBuffer::new(10);
    let mut xs: [u8; 10] = [7; 10];
    let n = buffer.get(&mut xs);
    assert!(n == 0);
    for i in 0..10 { assert!(xs[i] == 7); }
}