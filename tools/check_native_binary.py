#!/usr/bin/env python3
"""Fail closed unless a generated executable has the requested native ISA."""

from __future__ import annotations

import pathlib
import struct
import sys


MACHINES = {
    "x86_64-unknown-linux-gnu": "x86_64",
    "aarch64-unknown-linux-gnu": "aarch64",
    "x86_64-pc-windows-msvc": "x86_64",
    "aarch64-pc-windows-msvc": "aarch64",
    "x86_64-apple-darwin": "x86_64",
    "aarch64-apple-darwin": "aarch64",
}


def identify(data: bytes) -> str:
    if data.startswith(b"\x7fELF") and len(data) >= 20:
        endian = "<" if data[5] == 1 else ">" if data[5] == 2 else None
        if endian is None:
            raise ValueError("invalid ELF byte order")
        machine = struct.unpack_from(endian + "H", data, 18)[0]
        return {62: "x86_64", 183: "aarch64"}.get(machine, f"ELF machine {machine}")

    if data.startswith(b"MZ") and len(data) >= 0x40:
        pe_offset = struct.unpack_from("<I", data, 0x3C)[0]
        if data[pe_offset : pe_offset + 4] != b"PE\0\0":
            raise ValueError("invalid PE signature")
        machine = struct.unpack_from("<H", data, pe_offset + 4)[0]
        return {0x8664: "x86_64", 0xAA64: "aarch64"}.get(machine, f"PE machine {machine:#x}")

    if len(data) >= 8:
        magic = data[:4]
        endian = {b"\xfe\xed\xfa\xce": ">", b"\xce\xfa\xed\xfe": "<",
                  b"\xfe\xed\xfa\xcf": ">", b"\xcf\xfa\xed\xfe": "<"}.get(magic)
        if endian:
            cpu = struct.unpack_from(endian + "I", data, 4)[0]
            return {0x01000007: "x86_64", 0x0100000C: "aarch64"}.get(cpu, f"Mach-O CPU {cpu:#x}")

    raise ValueError("unrecognized executable format")


def main() -> int:
    if len(sys.argv) != 3 or sys.argv[2] not in MACHINES:
        print("usage: check_native_binary.py EXECUTABLE TARGET_TRIPLE", file=sys.stderr)
        return 2
    path = pathlib.Path(sys.argv[1])
    try:
        actual = identify(path.read_bytes())
    except (OSError, ValueError) as error:
        print(f"binary architecture check failed: {error}", file=sys.stderr)
        return 1
    expected = MACHINES[sys.argv[2]]
    print(f"{path}: {actual}; expected {expected} ({sys.argv[2]})")
    return 0 if actual == expected else 1


if __name__ == "__main__":
    raise SystemExit(main())
