"""Secure atomic JSON storage for disposable signing identity state."""
from __future__ import annotations

import contextlib
import fcntl
import json
import os
import secrets
import stat
from pathlib import Path
from typing import Any, Callable


class StateFileError(RuntimeError):
    pass


def canonical_state_path(path: str | Path) -> Path:
    value = Path(os.path.abspath(os.path.expanduser(os.fspath(path))))
    cursor = Path(value.anchor)
    for part in value.parts[1:]:
        cursor /= part
        try:
            info = cursor.lstat()
        except FileNotFoundError:
            continue
        if stat.S_ISLNK(info.st_mode):
            raise StateFileError("Signing state paths may not contain symlinks")
    return value


class SecureJsonFile:
    def __init__(self, path: str | Path):
        self.path = canonical_state_path(path)
        self.parent = self.path.parent
        self.lock_name = self.path.name + ".lock"
        self.temp_prefix = "." + self.path.name + ".tmp-"

    def _parent_fd(self) -> int:
        flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
        current_fd = os.open(self.parent.anchor, flags)
        try:
            for part in self.parent.parts[1:]:
                next_fd = os.open(part, flags, dir_fd=current_fd)
                os.close(current_fd)
                current_fd = next_fd
            info = os.fstat(current_fd)
            if info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
                raise StateFileError("Signing state parent must be Owner-owned with mode 0700")
            return current_fd
        except StateFileError:
            os.close(current_fd)
            raise
        except OSError as exc:
            os.close(current_fd)
            raise StateFileError("Signing state parent is missing or contains a symlink") from exc

    @contextlib.contextmanager
    def locked(self):
        directory_fd = self._parent_fd()
        lock_fd = None
        try:
            lock_fd = os.open(self.lock_name, os.O_CREAT | os.O_RDWR | getattr(os, "O_NOFOLLOW", 0),
                              0o600, dir_fd=directory_fd)
            info = os.fstat(lock_fd)
            if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_nlink != 1 or \
                    stat.S_IMODE(info.st_mode) != 0o600:
                raise StateFileError("Signing state lock is unsafe")
            fcntl.flock(lock_fd, fcntl.LOCK_EX)
            if any(name.startswith(self.temp_prefix) for name in os.listdir(directory_fd)):
                raise StateFileError("Stale signing state staging file; explicit recovery is required")
            yield directory_fd
        except OSError as exc:
            raise StateFileError("Cannot safely lock signing state") from exc
        finally:
            if lock_fd is not None:
                with contextlib.suppress(OSError):
                    fcntl.flock(lock_fd, fcntl.LOCK_UN)
                os.close(lock_fd)
            os.close(directory_fd)

    def _load(self, directory_fd: int) -> Any:
        try:
            fd = os.open(self.path.name, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0), dir_fd=directory_fd)
        except OSError as exc:
            raise StateFileError("Signing state is missing or cannot be opened safely") from exc
        try:
            info = os.fstat(fd)
            if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_nlink != 1 or \
                    stat.S_IMODE(info.st_mode) != 0o600:
                raise StateFileError("Signing state file must be Owner-owned, regular and mode 0600")
            with os.fdopen(fd, "r", encoding="utf-8") as stream:
                fd = -1
                return json.load(stream)
        except (OSError, UnicodeError, json.JSONDecodeError) as exc:
            raise StateFileError("Signing state JSON is corrupt") from exc
        finally:
            if fd >= 0:
                os.close(fd)

    def read(self) -> Any:
        with self.locked() as directory_fd:
            return self._load(directory_fd)

    def create(self, value: Any) -> None:
        with self.locked() as directory_fd:
            if self.path.name in os.listdir(directory_fd):
                raise StateFileError("Signing state already exists; refusing replacement")
            self._write(directory_fd, value, create=True)

    def update(self, transform: Callable[[Any], Any]) -> Any:
        with self.locked() as directory_fd:
            value = transform(self._load(directory_fd))
            self._write(directory_fd, value, create=False)
            return value

    def _write(self, directory_fd: int, value: Any, *, create: bool) -> None:
        temporary = self.temp_prefix + secrets.token_hex(16)
        fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
                     0o600, dir_fd=directory_fd)
        published = False
        try:
            data = json.dumps(value, sort_keys=True, indent=2).encode("utf-8") + b"\n"
            view = memoryview(data)
            while view:
                size = os.write(fd, view)
                if size <= 0:
                    raise StateFileError("Signing state write made no progress")
                view = view[size:]
            os.fsync(fd)
            os.close(fd)
            fd = -1
            if create:
                os.link(temporary, self.path.name, src_dir_fd=directory_fd, dst_dir_fd=directory_fd,
                        follow_symlinks=False)
                published = True
                os.unlink(temporary, dir_fd=directory_fd)
            else:
                current = os.stat(self.path.name, dir_fd=directory_fd, follow_symlinks=False)
                if not stat.S_ISREG(current.st_mode) or current.st_uid != os.getuid() or current.st_nlink != 1 or \
                        stat.S_IMODE(current.st_mode) != 0o600:
                    raise StateFileError("Signing state was replaced or has unsafe permissions")
                os.replace(temporary, self.path.name, src_dir_fd=directory_fd, dst_dir_fd=directory_fd)
                published = True
            os.fsync(directory_fd)
        except FileExistsError as exc:
            raise StateFileError("Signing state already exists; refusing replacement") from exc
        except OSError as exc:
            if published:
                raise StateFileError("Signing state publish durability is uncertain; stop and reconcile") from exc
            raise StateFileError("Signing state update failed before atomic publication") from exc
        finally:
            if fd >= 0:
                os.close(fd)
            if not published:
                with contextlib.suppress(FileNotFoundError):
                    os.unlink(temporary, dir_fd=directory_fd)
