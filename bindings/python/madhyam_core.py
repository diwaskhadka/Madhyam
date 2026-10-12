"""Thin ctypes wrapper around the madhyam_core C ABI.

    core = MadhyamCore("path/to/libmadhyam_core_ffi.so")
    src = core.call("add_m3u_source", name="Home")["id"]
    core.import_bytes("m3u", open("list.m3u", "rb").read(), source=src)
    print(core.call("search", query="news", limit=5))
"""

import ctypes
import json
import sys


class IptvError(Exception):
    def __init__(self, code, message):
        super().__init__(f"{code}: {message}")
        self.code = code


def _default_lib():
    if sys.platform == "win32":
        return "madhyam_core_ffi.dll"
    if sys.platform == "darwin":
        return "libmadhyam_core_ffi.dylib"
    return "libmadhyam_core_ffi.so"


class MadhyamCore:
    def __init__(self, lib_path=None):
        lib = ctypes.CDLL(lib_path or _default_lib())
        lib.iptv_core_new.restype = ctypes.c_void_p
        lib.iptv_core_free.argtypes = [ctypes.c_void_p]
        # Return c_void_p (not c_char_p) so we keep the pointer to free it.
        lib.iptv_call.restype = ctypes.c_void_p
        lib.iptv_call.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p]
        lib.iptv_import.restype = ctypes.c_void_p
        lib.iptv_import.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_size_t, ctypes.c_char_p]
        lib.iptv_string_free.argtypes = [ctypes.c_void_p]
        lib.iptv_version.restype = ctypes.c_char_p
        self._lib = lib
        self._core = lib.iptv_core_new()

    @property
    def version(self):
        return self._lib.iptv_version().decode()

    def _unwrap(self, ptr):
        try:
            text = ctypes.string_at(ptr).decode("utf-8")
        finally:
            self._lib.iptv_string_free(ptr)
        env = json.loads(text)
        if "error" in env:
            raise IptvError(env["error"]["code"], env["error"]["message"])
        return env["ok"]

    def call(self, method, **args):
        return self._unwrap(self._lib.iptv_call(self._core, method.encode(), json.dumps(args).encode()))

    def import_bytes(self, method, data, **args):
        return self._unwrap(
            self._lib.iptv_import(self._core, method.encode(), data, len(data), json.dumps(args).encode())
        )

    def close(self):
        if self._core:
            self._lib.iptv_core_free(self._core)
            self._core = None

    def __del__(self):
        self.close()
