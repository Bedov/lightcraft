#!/usr/bin/env python3
"""Build the Bedov fork as a separate macOS app; library paths stay in the local bundle."""
import argparse
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--library', required=True, type=Path)
    parser.add_argument('--config', required=True, type=Path)
    parser.add_argument('--control-port', type=int, default=18761)
    parser.add_argument('--output', type=Path, default=ROOT.parent / 'LightCraft-Fork-Local')
    parser.add_argument('--skip-build', action='store_true', help='Package existing release binaries')
    args = parser.parse_args()
    if not 1024 <= args.control_port <= 65535:
        parser.error('--control-port must be between 1024 and 65535')
    library, config, output = (p.expanduser().resolve() for p in (args.library, args.config, args.output))
    if not (library / 'catalog.snap').is_file() or not config.is_dir():
        parser.error('Choose an existing LightCraft library and settings directory')
    output.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    env.setdefault('CARGO_TARGET_DIR', str(output / 'target'))
    env.setdefault('CARGO_BUILD_JOBS', '4')
    if not args.skip_build:
        subprocess.run(['cargo', 'build', '--locked', '--release', '-p', 'lightcraft', '-p', 'lightcraft-cli'], cwd=ROOT, env=env, check=True)
    target = Path(env['CARGO_TARGET_DIR']).resolve() / 'release'
    app = output / 'LightCraft Bedov.app'
    if app.exists():
        parser.error(f'{app} already exists; move it aside or choose another --output directory')
    macos = app / 'Contents/MacOS'
    resources = app / 'Contents/Resources'
    licenses = resources / 'Licenses'
    macos.mkdir(parents=True)
    licenses.mkdir(parents=True)
    shutil.copy2(target / 'lightcraft', macos / 'LightCraft')
    shutil.copy2(target / 'lightcraft-cli', output / 'lightcraft-cli-bedov')
    shutil.copy2(ROOT / 'assets/app-icon/lightcraft.icns', resources / 'LightCraft.icns')
    for filename in ['LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE']:
        shutil.copy2(ROOT / filename, licenses / filename)
    shutil.copy2(ROOT / 'packaging/licenses/OxideAV-JPEGXS-MIT.txt', licenses / 'OxideAV-JPEGXS-MIT.txt')
    shutil.copy2(ROOT / 'assets/fonts/OFL-Inter.txt', licenses / 'OFL-Inter.txt')
    sha = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    dirty = bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True).strip())
    meta = json.loads(subprocess.check_output(['cargo', 'metadata', '--no-deps', '--format-version', '1', '--locked'], cwd=ROOT, env=env, text=True))
    version = next(p['version'] for p in meta['packages'] if p['name'] == 'lightcraft')
    info = plistlib.loads((ROOT / 'packaging/macos/Info.plist.in').read_bytes())
    info.update(CFBundleDisplayName='LightCraft Bedov', CFBundleName='LightCraft Bedov',
                CFBundleIdentifier='ai.storyteller.lightcraft-bedov', CFBundleShortVersionString=version.split('-')[0],
                CFBundleVersion=version.split('-')[0], LightCraftVersion=version + '-bedov.1',
                LightCraftBuildCommit=sha + (' + working changes' if dirty else ''),
                LSEnvironment={'LIGHTCRAFT_LIBRARY': str(library), 'LIGHTCRAFT_CONFIG_DIR': str(config),
                               'LIGHTCRAFT_CONTROL_PORT': str(args.control_port)})
    (app / 'Contents/Info.plist').write_bytes(plistlib.dumps(info))
    (app / 'Contents/PkgInfo').write_bytes(b'APPL????')
    for path in [macos / 'LightCraft', app, output / 'lightcraft-cli-bedov']:
        subprocess.run(['codesign', '--force', '--sign', '-', '--timestamp=none', '--options', 'runtime',
                        '--entitlements', str(ROOT / 'packaging/macos/entitlements.plist'), str(path)], check=True)
    subprocess.run(['codesign', '--verify', '--strict', '--deep', str(app)], check=True)
    (output / 'build-info.json').write_text(json.dumps({'app': str(app), 'commit': sha, 'dirty': dirty,
        'version': info['LightCraftVersion'], 'library': str(library), 'config': str(config)}, indent=2))
    print(app)


if __name__ == '__main__':
    try:
        main()
    except (OSError, subprocess.CalledProcessError, ValueError, StopIteration) as error:
        print(f'Build failed: {error}', file=sys.stderr)
        sys.exit(1)
