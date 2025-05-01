FROM alpine:3.21

# Build C++ code with clang
ENV CXX=clang++

# Install dependencies. We need a C++ toolchain to compile the highs crate.
RUN apk add --no-cache cargo cmake binutils make clang clang19-libclang git mdbook
