[![License](http://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Language](http://img.shields.io/badge/language-rust-brightgreen.svg)](http://www.rust-lang.org/)


# pipebuffer

A simple command line program for buffering `stdin`/`stdout` between piped processes when the operating system's pipe buffer is insufficient.  Modern linux limits the buffer between pipes in a pipeline to `64kb`, which can cause bottlenecks when working with processes with "lumpy" IO profiles.

`pipebuffer` is particularly useful when dealing with large volumes of data and a mix of processes that work in "chunks" of data and more stream-oriented processes.

## Usage

To use, you just replace:
```bash
foo | bar
```
with
```bash
foo | pipebuffer | bar
```
or 
```bash
foo | pipebuffer --size=512m | bar
```

And, of course, you can use many `pipebuffer`s together:
```bash
foo | pipebuffer --size=128m | bar | pipebuffer --size=64m | splat | pipebuffer --size=1g | whee
```

## License

`pipebuffer` is open source software released under the [MIT License](LICENSE).

## Building

`pipebuffer` is written in [Rust](https://www.rust-lang.org/) and currently requires Rust 1.7 (beta) or greater.  You'll need Rust installed.  You can [download here](https://www.rust-lang.org/downloads.html), or run the first command below:

```bash
// Optionally install Rust: if 1.7 is stable, which it should be after ~3rd March 2016, omit '--channel=beta'
curl -sSf https://static.rust-lang.org/rustup.sh | sh -s -- --channel=beta

// Clone the repo
git clone https://github.com/tfenne/pipebuffer.git

// Build and run the tests
pushd pipebuffer && (cargo test; cargo build --release); popd

// Produces executable at ./pipebuffer/target/release/pipebuffer
```
