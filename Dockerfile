FROM alpine:3.22

# Install dependencies. We need a C++ toolchain to compile the highs crate.
RUN apk add --no-cache cargo rust-src rustfmt cmake binutils make g++ clang19-libclang git mdbook
