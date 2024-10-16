# Tiansuan Cubesat Space Protocol
# Todo
There are some known problems need to address:
* The bineary size is too large. 
* The crate should separate into at least 3 parts (`tcsp-adaptor`,`tcsp-application`,`tcsp-protocol`)
* The naming of two `frame` structs is ambiguous. Rename it.
* The `size` field in meta of `adaptor::Frame` should not be edited by user. Instead, providing an interface for user to update and read length of comming package.
* Lacking of real hardware tests and benchmark.
* Lacking of documents of protocol and a method to generate document for others to read.
* The client side is not yet implemented.
