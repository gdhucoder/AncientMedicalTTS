from pathlib import Path

from PyInstaller.utils.hooks import collect_submodules


worker_root = Path(SPECPATH)

# main.py imports these modules inside request handlers. Keep them explicit so
# the one-file build does not depend on PyInstaller's static import discovery.
hiddenimports = [
    *collect_submodules("pronunciation"),
    *collect_submodules("tts"),
    *collect_submodules("tencentcloud.common"),
    *collect_submodules("tencentcloud.tts.v20190823"),
]

analysis = Analysis(
    [str(worker_root / "main.py")],
    pathex=[str(worker_root)],
    binaries=[],
    datas=[(str(worker_root / "dictionaries"), "dictionaries")],
    hiddenimports=hiddenimports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=["pytest", "_pytest"],
    noarchive=False,
)
pyz = PYZ(analysis.pure)
executable = EXE(
    pyz,
    analysis.scripts,
    analysis.binaries,
    analysis.datas,
    [],
    name="ancient-tts-worker",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=True,
    disable_windowed_traceback=False,
)
