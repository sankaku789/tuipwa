#!/usr/bin/env python3
"""Protocol-level app_id validation on headless Weston, NOT KDE taskbar validation."""
import json
import os
from pathlib import Path
import re
import signal
import subprocess as sp
import tempfile
import time

BIN = Path(os.environ['TUIPWA_NATIVE']).resolve()
HOST = 'org.tuipwa.NativeHost'
BASE = '/org/tuipwa/NativeHost'

def wait(predicate, description):
    end = time.monotonic() + 15
    while time.monotonic() < end:
        if predicate(): return
        time.sleep(.05)
    raise AssertionError(description)

def alive(pid):
    try: return Path(f'/proc/{pid}/stat').read_text().split(') ', 1)[1][0] != 'Z'
    except FileNotFoundError: return False

def cli(*args):
    p = sp.run([str(BIN), *args], text=True, capture_output=True, timeout=40)
    assert p.returncode == 0, (p.stdout, p.stderr)
    return p.stdout

def close(index):
    p = sp.run(['gdbus', 'call', '--session', '--dest', HOST, '--object-path', f'{BASE}/window/{index}', '--method', 'org.gtk.Actions.Activate', 'close', '[]', '{}'], text=True, capture_output=True)
    assert p.returncode == 0, (p.stdout, p.stderr)

with tempfile.TemporaryDirectory(prefix='tuipwa-wayland-') as td:
    root = Path(td)
    runtime = root/'runtime'; runtime.mkdir(mode=0o700)
    os.environ.update(XDG_RUNTIME_DIR=str(runtime), XDG_CONFIG_HOME=str(root/'config'), XDG_DATA_HOME=str(root/'data'), GDK_BACKEND='wayland', WAYLAND_DISPLAY='tuipwa-test', LIBGL_ALWAYS_SOFTWARE='1')
    env = dict(os.environ); env.pop('WAYLAND_DEBUG', None)
    weston = sp.Popen(['weston', '--backend=headless', '--renderer=pixman', '--no-config', '--socket=tuipwa-test', '--idle-time=0', f'--log={root}/weston.log'], env=env, stdout=sp.DEVNULL, stderr=sp.DEVNULL)
    acks = []
    try:
        wait(lambda: (runtime/'tuipwa-test').exists(), 'Weston did not start')
        os.environ['WAYLAND_DEBUG'] = 'client'
        for id in ['nano', 'btop', 'vim']:
            path = root/f'{id}.json'
            path.write_text(json.dumps(dict(schemaVersion=1, id=id, name=id, title=f'PoC-{id}', mode='native', command=id, workingDirectory=str(root))))
            cli('add', '--manifest', str(path))
            acks.append(json.loads(cli('run', id)))
        assert len({a['host_pid'] for a in acks}) == 1, acks
        assert len({a['tty'] for a in acks}) == 3, acks
        assert all(alive(a['child_pid']) for a in acks), acks
        assert all(Path(f"/proc/{a['child_pid']}/stat").read_text().split(') ', 1)[1][0] not in 'TtZ' for a in acks), 'TUI stopped before foreground handoff'
        log = runtime/'tuipwa-native/host.log'
        def last_ids():
            result = {}
            for obj, value in re.findall(r'xdg_toplevel[\@#](\d+)\.set_app_id\("([^"\n]+)"\)', log.read_text(errors='replace')):
                result[obj] = value
            return result
        expected = {'tuipwa-nano', 'tuipwa-btop', 'tuipwa-vim'}
        wait(lambda: expected.issubset(set(last_ids().values())), 'per-window app_ids not transmitted')
        time.sleep(.5)
        identities = last_ids()
        assert expected.issubset(set(identities.values())), identities
        text = log.read_text(errors='replace')
        assert all(f'.set_title("PoC-{id}")' in text for id in ['nano', 'btop', 'vim']), 'titles missing'
        close(1)
        wait(lambda: not alive(acks[0]['child_pid']), 'Wayland close did not terminate first child')
        assert alive(acks[1]['host_pid']) and alive(acks[1]['child_pid'])
        close(2); close(3)
        wait(lambda: not alive(acks[0]['host_pid']), 'Wayland last-close did not exit host')
        # A command that exits immediately closes its Wayland surface; as the
        # only window it also quits the shared host.
        path = root/'true.json'
        path.write_text(json.dumps(dict(schemaVersion=1, id='true', name='true', title='PoC-true', mode='native', command='/usr/bin/true', workingDirectory=str(root))))
        cli('add', '--manifest', str(path))
        quick = json.loads(cli('run', 'true'))
        wait(lambda: not alive(quick['child_pid']), 'immediate command did not exit')
        wait(lambda: not alive(quick['session_pid']), 'session helper survived immediate command')
        wait(lambda: not alive(quick['host_pid']), 'immediate exit did not close the last Wayland window')
        print(json.dumps({'result':'PASS', 'compositor':'Weston headless (not KDE)', 'shared_host_pid':acks[0]['host_pid'], 'ptys':[a['tty'] for a in acks], 'last_app_id_by_toplevel':identities, 'checks':['nano/btop/vim','distinct wire-level app_id/title','Wayland close','last-window host exit','immediate exit closes window/host'], 'kde_taskbar':'UNVERIFIED'}, ensure_ascii=False))
    except BaseException:
        hostlog = runtime/'tuipwa-native/host.log'
        if hostlog.exists():
            for line in hostlog.read_text(errors='replace').splitlines():
                if 'app_id' in line or 'set_title' in line or 'error' in line.lower() or 'critical' in line.lower(): print(line, flush=True)
        raise
    finally:
        for a in acks:
            if alive(a['host_pid']):
                try: os.kill(a['host_pid'], signal.SIGTERM)
                except ProcessLookupError: pass
        weston.terminate()
        try: weston.wait(timeout=5)
        except sp.TimeoutExpired: weston.kill(); weston.wait()
