FROM ubuntu:25.04

# Update list of packages and update base packages
RUN apt update && apt upgrade -y

# Install dependencies. We need a C++ toolchain to compile the highs crate.
RUN apt install -y rustup cmake build-essential libclang-20-dev git mdbook just curl

# Install uv (required for some scripts + pre-commit)
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
