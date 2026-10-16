"""Small advisory file lock for Windows, Linux and macOS."""
import os
import time

class FileLock:
    def __init__(self, path, blocking=True):
        self.path, self.blocking, self.file = path, blocking, None

    def acquire(self):
        self.file = open(self.path, 'a+b')
        if os.name == 'nt':
            import msvcrt
            if os.fstat(self.file.fileno()).st_size == 0:
                self.file.write(b'\0'); self.file.flush()
            while True:
                self.file.seek(0)
                try:
                    msvcrt.locking(self.file.fileno(), msvcrt.LK_NBLCK, 1)
                    break
                except OSError as error:
                    if not self.blocking:
                        self.file.close(); self.file = None
                        raise BlockingIOError('Another STRATOS helper is running') from error
                    time.sleep(.1)
        else:
            import fcntl
            try:
                fcntl.flock(self.file, fcntl.LOCK_EX | (0 if self.blocking else fcntl.LOCK_NB))
            except OSError:
                self.file.close(); self.file = None
                raise
        return self

    def release(self):
        if self.file is None: return
        if os.name == 'nt':
            import msvcrt
            self.file.seek(0); msvcrt.locking(self.file.fileno(), msvcrt.LK_UNLCK, 1)
        else:
            import fcntl
            fcntl.flock(self.file, fcntl.LOCK_UN)
        self.file.close(); self.file = None

    def __enter__(self): return self.acquire()
    def __exit__(self, *args): self.release()
