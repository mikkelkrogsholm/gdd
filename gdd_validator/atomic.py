"""Publish completed bundles atomically without replacing any destination."""

import ctypes
import errno
import os
from pathlib import Path
import sys

__all__ = ["rename_exclusive"]


def rename_exclusive(source: Path, destination: Path) -> None:
    """Use the OS no-replace primitive; never fall back to replacing rename.

    macOS: renamex_np(RENAME_EXCL). Linux: renameat2(RENAME_NOREPLACE).
    Windows os.rename already refuses an existing destination.
    """
    old, new = os.fsencode(source), os.fsencode(destination)
    if b"\x00" in old or b"\x00" in new:
        raise ValueError("Embedded NUL in rename path.")
    if sys.platform == "win32":
        os.rename(source, destination)
        return
    library = ctypes.CDLL(None, use_errno=True)
    if sys.platform == "darwin" and hasattr(library, "renamex_np"):
        function = library.renamex_np
        function.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint]
        function.restype = ctypes.c_int
        result = function(old, new, 0x4)  # RENAME_EXCL, from the platform stdio.h.
    elif sys.platform.startswith("linux") and hasattr(library, "renameat2"):
        function = library.renameat2
        function.argtypes = [
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_uint,
        ]
        function.restype = ctypes.c_int
        result = function(-100, old, -100, new, 1)  # AT_FDCWD, RENAME_NOREPLACE.
    else:
        raise OSError(
            errno.ENOTSUP,
            "Atomic exclusive rename is unavailable on this platform.",
            str(destination),
        )
    if result:
        number = ctypes.get_errno()
        raise OSError(number, os.strerror(number), str(destination))
