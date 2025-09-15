FROM alpine:3.22

# Install dependencies. We need a C++ toolchain to compile the highs crate.
RUN apk add --no-cache rustup cmake binutils make g++ clang20-libclang git mdbook just uv

# Install rustup
RUN yes ""|rustup-init --default-toolchain none
