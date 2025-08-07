#!/bin/bash

set -e

# Add all necessary repositories as git submodules
git submodule add https://github.com/OP-TEE/build.git third_party/build
git submodule add https://github.com/OP-TEE/optee_os.git third_party/optee_os
git submodule add https://github.com/OP-TEE/optee_client.git third_party/optee_client
git submodule add https://github.com/linaro-swg/optee_examples.git third_party/optee_examples
git submodule add https://github.com/OP-TEE/toolchains.git third_party/toolchains
git submodule add https://github.com/linaro-sw-projects/linux.git third_party/linux

# Initialize and update the submodules
git submodule update --init --recursive

# Build the toolchains
cd third_party/build
make -j$(sysctl -n hw.ncpu) toolchains
cd ../..

# Build the QEMU environment
cd third_party/build
make -j$(sysctl -n hw.ncpu) -f qemu_v8.mk all
cd ../..