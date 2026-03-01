#!/usr/bin/env python3
"""
Fix TA ELF for OP-TEE compatibility on STM32MP1.

OP-TEE's ELF loader only supports: PT_LOAD, PT_DYNAMIC, PT_GNU_STACK, PT_ARM_EXIDX.
This script removes unsupported program headers (NOTE, TLS) and fixes alignment
to match the 4096-byte page size used by OP-TEE.

It also patches GNU_STACK flags from RW to RWE to match standard OP-TEE TAs.
"""
import struct
import sys
import shutil

def patch_elf(input_path, output_path):
    with open(input_path, 'rb') as f:
        data = bytearray(f.read())

    # Parse ELF header (32-bit LE)
    assert data[0:4] == b'\x7fELF', "Not an ELF file"
    assert data[4] == 1, "Not 32-bit ELF"
    assert data[5] == 1, "Not little-endian"

    e_phoff = struct.unpack_from('<I', data, 28)[0]
    e_phentsize = struct.unpack_from('<H', data, 42)[0]
    e_phnum = struct.unpack_from('<H', data, 44)[0]

    PT_NULL = 0
    PT_LOAD = 1
    PT_DYNAMIC = 2
    PT_NOTE = 4
    PT_TLS = 7
    PT_GNU_STACK = 0x6474e551
    PT_ARM_EXIDX = 0x70000001

    # Allowed segment types for OP-TEE
    allowed = {PT_LOAD, PT_DYNAMIC, PT_GNU_STACK, PT_ARM_EXIDX}

    kept = []
    for i in range(e_phnum):
        off = e_phoff + i * e_phentsize
        p_type = struct.unpack_from('<I', data, off)[0]

        if p_type in allowed:
            # Fix GNU_STACK: set flags to RWE (7) instead of RW (6)
            if p_type == PT_GNU_STACK:
                p_flags_off = off + 24
                struct.pack_into('<I', data, p_flags_off, 7)  # PF_R|PF_W|PF_X
                # Also fix alignment to 0x10
                struct.pack_into('<I', data, off + 28, 0x10)

            # Fix EXIDX: if section was removed, zero it out but keep the header
            if p_type == PT_ARM_EXIDX:
                p_filesz = struct.unpack_from('<I', data, off + 16)[0]
                if p_filesz == 0:
                    # Empty EXIDX, convert to NULL
                    struct.pack_into('<I', data, off, PT_NULL)
                    continue

            kept.append(i)
        else:
            print(f"  Removing segment {i}: type=0x{p_type:08x}")

    # Repack: move kept headers to the front, null out the rest
    new_phdrs = []
    for i in kept:
        off = e_phoff + i * e_phentsize
        new_phdrs.append(data[off:off + e_phentsize])

    # Write kept headers
    for idx, phdr in enumerate(new_phdrs):
        off = e_phoff + idx * e_phentsize
        data[off:off + e_phentsize] = phdr

    # Null out remaining headers
    for idx in range(len(new_phdrs), e_phnum):
        off = e_phoff + idx * e_phentsize
        data[off:off + e_phentsize] = b'\x00' * e_phentsize

    # Update e_phnum
    struct.pack_into('<H', data, 44, len(new_phdrs))

    print(f"  Kept {len(new_phdrs)} program headers (was {e_phnum})")

    with open(output_path, 'wb') as f:
        f.write(data)

if __name__ == '__main__':
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <input_elf> <output_elf>")
        sys.exit(1)
    patch_elf(sys.argv[1], sys.argv[2])
    print("Done!")
