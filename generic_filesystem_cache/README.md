# Generic Filesystem Cache

A small rust library for caching slow-to-compute data based on hard drive contents. Given a set of starting paths and a 'processing function' supplied by you, this library will recursively scan the filesystem from those starting paths and apply the processing function to each file.

The cache will save cached data to disk at a path given by you whenever a set number of changes has occurred inside the cache.

When directed by you, the cache will update itself if the 'modification time' of any cached file is changed.


## Features
* Supports Parallel loading (through rayon)
* Will cache any serializable type
 

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.