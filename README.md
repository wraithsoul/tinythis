# tinythis!

tinythis! is a lightweight `ffmpeg` wrapper for Windows. It ships three stable presets:

- `quality`: best quality, slower processing
- `balanced`: good quality, moderate processing
- `speed`: lower quality, faster processing

## Usage

### TUI

Run with no arguments:

```powershell
tinythis
```

- if `ffmpeg.exe` is next to `tinythis.exe`, tinythis will use it automatically (`local mode`)
- otherwise, if bundled ffmpeg assets are installed, tinythis will use those
- otherwise, tinythis will prompt to download ffmpeg assets (required to compress)

Status banner:

- `local mode`: using `ffmpeg.exe` next to `tinythis.exe`
- `ffmpeg missing`: no ffmpeg available (place `ffmpeg.exe` next to `tinythis.exe` or run `tinythis setup`)

Separately, tinythis may prompt to add itself to your user PATH for quick use. If you decline, it remembers that preference (you can enable it later with `tinythis setup path`).

Keys:

- `s`: add files (open picker)
- `u`: update (when available)
- up/down: select a file
- backspace: remove selected file
- left/right: change mode
- enter: compress
- esc: back
- `q`: quit

Supported extensions: `.mp4`, `.mov`, `.avi`, `.webm`, `.ogv`, `.asx`, `.mpeg`, `.m4v`, `.wmv`, `.mpg`.

### CLI

Compress one or more files:

```powershell
tinythis input1.mp4
tinythis balanced input1.mp4 input2.mp4  # or: quality, speed
```

Note: CLI compression will not prompt to download ffmpeg. If ffmpeg isn't available (near `tinythis.exe` or installed assets), run `tinythis setup`.

Download/install `ffmpeg` assets:

```powershell
tinythis setup
tinythis setup --force
```

`setup` can also add `tinythis` to your user PATH (it may prompt).
Use `--yes` to skip the prompt and add it to PATH (when missing):

```powershell
tinythis setup --yes
```

If you previously declined, you can install PATH later with:

```powershell
tinythis setup path
```

Update tinythis from GitHub Releases:

```powershell
tinythis update
tinythis update --yes
```

Remove assets and PATH entry:

```powershell
tinythis uninstall
```

## Outputs

Outputs are written next to the input file as:

`<stem>.tinythis.<preset>.mp4` (and `.2`, `.3`, ... if needed)
