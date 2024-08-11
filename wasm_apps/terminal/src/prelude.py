import sys

def __rustpython_host_console(buf):
    raise NotImplementedError

class __RustPythonHostConsole:
    def write(self, buf):
        __rustpython_host_console(buf)
    def flush(self):
        pass

sys.stdout = __RustPythonHostConsole()
sys.stderr = __RustPythonHostConsole()
