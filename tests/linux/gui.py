#!/usr/bin/env python3
"""Run in an isolated Xvfb + D-Bus session. Never touches the real registry."""
import concurrent.futures
import ctypes as C
import json
import os
from pathlib import Path
import signal
import subprocess as sp
import tempfile
import time

BIN = Path(os.environ['TUIPWA_NATIVE']).resolve()
X = C.CDLL('libX11.so.6')
XT = C.CDLL('libXtst.so.6')
X.XOpenDisplay.restype = C.c_void_p
X.XDefaultRootWindow.argtypes = [C.c_void_p]; X.XDefaultRootWindow.restype = C.c_ulong
X.XQueryTree.argtypes = [C.c_void_p, C.c_ulong, C.POINTER(C.c_ulong), C.POINTER(C.c_ulong), C.POINTER(C.POINTER(C.c_ulong)), C.POINTER(C.c_uint)]
X.XFetchName.argtypes = [C.c_void_p, C.c_ulong, C.POINTER(C.c_char_p)]
X.XInternAtom.argtypes = [C.c_void_p, C.c_char_p, C.c_int]; X.XInternAtom.restype = C.c_ulong
X.XSetInputFocus.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_ulong]
X.XStringToKeysym.argtypes = [C.c_char_p]; X.XStringToKeysym.restype = C.c_ulong
X.XKeysymToKeycode.argtypes = [C.c_void_p, C.c_ulong]; X.XKeysymToKeycode.restype = C.c_uint
X.XFree.argtypes = [C.c_void_p]
X.XSendEvent.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_long, C.c_void_p]
X.XFlush.argtypes = [C.c_void_p]
XT.XTestFakeKeyEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
XT.XTestFakeKeyEvent.restype = C.c_int
display = X.XOpenDisplay(None)
assert display, 'X display unavailable'
root = X.XDefaultRootWindow(display)
class Data(C.Union):
    _fields_ = [('b', C.c_char * 20), ('s', C.c_short * 10), ('l', C.c_long * 5)]
class Client(C.Structure):
    _fields_ = [('type', C.c_int), ('serial', C.c_ulong), ('send_event', C.c_int), ('display', C.c_void_p), ('window', C.c_ulong), ('message_type', C.c_ulong), ('format', C.c_int), ('data', Data)]
class Event(C.Union):
    _fields_ = [('client', Client), ('pad', C.c_long * 24)]
def windows():
    rr, parent, children, n = C.c_ulong(), C.c_ulong(), C.POINTER(C.c_ulong)(), C.c_uint()
    X.XQueryTree(display, root, C.byref(rr), C.byref(parent), C.byref(children), C.byref(n))
    result = {}
    for i in range(n.value):
        name = C.c_char_p()
        if X.XFetchName(display, children[i], C.byref(name)) and name.value:
            result[name.value.decode()] = children[i]
            X.XFree(name)
    if children: X.XFree(children)
    return result

def close(title):
    win = windows()[title]
    event = Event()
    event.client = Client(33, 0, 1, display, win, X.XInternAtom(display, b'WM_PROTOCOLS', 0), 32, Data())
    event.client.data.l[0] = X.XInternAtom(display, b'WM_DELETE_WINDOW', 0)
    X.XSendEvent(display, win, 0, 0, C.byref(event)); X.XFlush(display)

def key(title, name, mods=()):
    """Sends one key press through XTEST to the titled window."""
    win = windows()[title]
    X.XSetInputFocus(display, win, 2, 0)
    X.XFlush(display)
    modcodes = [X.XKeysymToKeycode(display, X.XStringToKeysym(m.encode())) for m in mods]
    code = X.XKeysymToKeycode(display, X.XStringToKeysym(name.encode()))
    assert code, name
    for c in modcodes: XT.XTestFakeKeyEvent(display, c, 1, 0)
    XT.XTestFakeKeyEvent(display, code, 1, 0)
    XT.XTestFakeKeyEvent(display, code, 0, 0)
    for c in reversed(modcodes): XT.XTestFakeKeyEvent(display, c, 0, 0)
    X.XFlush(display)

def wait(predicate, message):
    end = time.monotonic() + 15
    while time.monotonic() < end:
        if predicate(): return
        time.sleep(.05)
    raise AssertionError(message)

def alive(pid):
    try:
        # A zombie is no longer a running host/child.
        return Path(f'/proc/{pid}/stat').read_text().split(') ', 1)[1][0] != 'Z'
    except FileNotFoundError: return False

def cli(*args, ok=True):
    p = sp.run([str(BIN), *args], text=True, capture_output=True, timeout=40)
    if ok: assert p.returncode == 0, (p.args, p.stdout, p.stderr)
    return p

def shown(title):
    return title in windows()

with tempfile.TemporaryDirectory(prefix='tuipwa-gui-') as td:
    p = Path(td)
    os.environ['XDG_CONFIG_HOME'] = str(p / 'config')
    os.environ['XDG_DATA_HOME'] = str(p / 'data')
    runtime = p / 'runtime'; runtime.mkdir(mode=0o700)
    os.environ['XDG_RUNTIME_DIR'] = str(runtime)
    # Explicit Python argv probe, no shell interpolation. Also records child cwd.
    argv = ['', 'a b', '"quoted"', '$(touch NEVER)', '日本語']
    code = 'import os,sys,json,time;json.dump([sys.argv[2:],os.getcwd()],open(sys.argv[1],"w"));sys.stdout.write("\\x1b]0;attempted-override\\x07");sys.stdout.flush();time.sleep(600)'
    def register(id, command, args):
        m = dict(schemaVersion=1, id=id, name=id, title='PoC-' + id, mode='native', command=command, arguments=args, workingDirectory=str(p))
        path = p / (id + '.json'); path.write_text(json.dumps(m))
        cli('validate', str(path)); cli('add', '--manifest', str(path))
    register('a', '/usr/bin/python3', ['-c', code, str(p/'argv.json'), *argv])
    register('b', '/usr/bin/nano', [])
    with concurrent.futures.ThreadPoolExecutor(2) as pool:
        a, b = list(pool.map(lambda id: json.loads(cli('run', id).stdout), ['a', 'b']))
    assert a['host_pid'] == b['host_pid'], (a,b)
    assert a['tty'] != b['tty'] and a['tty'].startswith('/dev/pts/'), (a,b)
    assert Path(f"/proc/{b['child_pid']}/stat").read_text().split(') ', 1)[1][0] not in 'TtZ', 'nano stopped before foreground handoff'
    wait(lambda: (p/'argv.json').exists(), 'argv probe not executed')
    assert json.loads((p/'argv.json').read_text()) == [argv, str(p)]
    wait(lambda: all(t in windows() for t in ['PoC-a', 'PoC-b']), 'separate titled windows missing')
    # `tuipwa run` is a short-lived launcher that already returned above. Its
    # exit must not close the windows it requested.
    time.sleep(.5)
    assert shown('PoC-a') and shown('PoC-b'), 'launcher exit closed a window'

    # A signal exit closes only its own window.
    os.kill(a['child_pid'], signal.SIGTERM)
    wait(lambda: not alive(a['child_pid']), 'signaled child did not exit')
    wait(lambda: not shown('PoC-a'), 'signaled window stayed open')
    wait(lambda: not alive(a['session_pid']), 'session helper stayed after signal')
    assert alive(b['host_pid']) and alive(b['child_pid']) and shown('PoC-b'), 'other session stopped'

    # nano exits normally with Ctrl-X; it is the last window, so the host quits.
    key('PoC-b', 'x', mods=('Control_L',))
    wait(lambda: not alive(b['child_pid']), 'nano did not exit')
    wait(lambda: not shown('PoC-b'), 'nano window stayed open')
    wait(lambda: not alive(b['session_pid']), 'session helper survived nano')
    wait(lambda: not alive(b['host_pid']), 'last-window host did not exit')

    # btop quits with `q`.
    register('btop', '/usr/bin/btop', [])
    bt = json.loads(cli('run', 'btop').stdout)
    wait(lambda: shown('PoC-btop'), 'btop window missing')
    time.sleep(.5)
    key('PoC-btop', 'q')
    wait(lambda: not alive(bt['child_pid']), 'btop did not exit')
    wait(lambda: not shown('PoC-btop'), 'btop window stayed open')
    wait(lambda: not alive(bt['session_pid']), 'session helper survived btop')
    wait(lambda: not alive(bt['host_pid']), 'btop last-window host did not exit')

    # vim quits with `:q`.
    register('vim', '/usr/bin/vim', [])
    v = json.loads(cli('run', 'vim').stdout)
    wait(lambda: shown('PoC-vim'), 'vim window missing')
    time.sleep(.5)
    key('PoC-vim', 'semicolon', mods=('Shift_L',))
    key('PoC-vim', 'q')
    key('PoC-vim', 'Return')
    wait(lambda: not alive(v['child_pid']), 'vim did not exit')
    wait(lambda: not shown('PoC-vim'), 'vim window stayed open')
    wait(lambda: not alive(v['session_pid']), 'session helper survived vim')
    wait(lambda: not alive(v['host_pid']), 'vim last-window host did not exit')

    # Commands that exit immediately, successfully or not, race with the window
    # creation; the window must still end up closed and the host gone.
    for id, command, status in [('true', '/usr/bin/true', 'success'), ('false', '/usr/bin/false', 'non-zero')]:
        register(id, command, [])
        quick = json.loads(cli('run', id).stdout)
        wait(lambda: not alive(quick['child_pid']), f'{command} did not exit')
        wait(lambda: not shown('PoC-' + id), f'{status} exit left the window open')
        wait(lambda: not alive(quick['session_pid']), f'session helper survived {command}')
        wait(lambda: not alive(quick['host_pid']), f'{status} exit left the host running')

    # Host crash recovery.
    d = json.loads(cli('run', 'b').stdout)
    wait(lambda: shown('PoC-b'), 'recovery window missing')
    os.kill(d['host_pid'], signal.SIGKILL)
    wait(lambda: not alive(d['session_pid']) and not alive(d['child_pid']), 'crash left children')
    e = json.loads(cli('run', 'b').stdout)
    assert e['host_pid'] != d['host_pid']
    wait(lambda: shown('PoC-b'), 'restarted window missing')
    close('PoC-b')
    wait(lambda: not shown('PoC-b'), 'restarted window survived close')
    wait(lambda: not alive(e['host_pid']), 'restarted host survived last close')
    wait(lambda: not alive(e['session_pid']) and not alive(e['child_pid']), 'close left processes')

    # A command that cannot be started keeps its window so the reason is
    # readable. This is the only case that still holds the window.
    register('missing', '/nonexistent/tuipwa-command', [])
    f = cli('run', 'missing', ok=False)
    assert f.returncode == 1 and json.loads(f.stdout)['error']
    wait(lambda: shown('PoC-missing'), 'spawn failure window missing')
    time.sleep(.3)
    assert shown('PoC-missing'), 'spawn failure window closed before it could be read'
    close('PoC-missing')
    wait(lambda: not alive(json.loads(f.stdout)['host_pid']), 'failed command host survived close')

    assert not list((runtime/'tuipwa-native').glob('*.sock')), 'request sockets leaked'
    runtime.chmod(0o755)
    assert cli('run', 'b', ok=False).returncode == 1, 'insecure runtime accepted'
    runtime.chmod(0o700)
    # Simulate launcher death before sending any first request: no empty daemon.
    host = os.environ.get('TUIPWA_GHOSTTY', str(BIN.parent/'ghostty-host'))
    with (p/'startup.log').open('w') as log:
        unused = sp.Popen([host, '--config-default-files=false', '--class=org.tuipwa.NativeHost', '--gtk-single-instance=true', '--initial-window=false', '--quit-after-last-window-closed=true'], stdout=sp.DEVNULL, stderr=log)
        assert unused.wait(timeout=40) == 0, 'unused startup host did not exit cleanly'
    print(json.dumps({'result':'PASS', 'shared_host_pid':a['host_pid'], 'independent_ptys':[a['tty'],b['tty']], 'checks':['concurrent startup','launcher exit keeps windows','argv/cwd','nano foreground','signal exit closes only its window','nano Ctrl-X auto close','btop q auto close','vim :q auto close','immediate true/false auto close','last-window host exit','crash cleanup/restart','spawn failure window kept','runtime permissions','unused startup watchdog']}, ensure_ascii=False))
