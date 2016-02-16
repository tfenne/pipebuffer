# pipebuffer
A very rough first attempt at a large threaded buffer for buffering between piped programs.

`pipebuffer` exists for two reasons:

1. Because I needed a command line utility to buffer output from one program and feed it to anothe program, and the linux pipe limit of `64k` was far too low.
2. I decided that writing it was a good exercise through which to begin learning Rust.

It is not terribly usable in it's current state, but should improve rapidly.
