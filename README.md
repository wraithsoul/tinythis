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

On first launch, tinythis will auto-run `setup` (download `ffmpeg/ffprobe` assets and add `tinythis` to your user PATH).

Keys:

- `s` / `ы`: add files (open picker)
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
tinythis --mode balanced input1.mp4 input2.mov
tinythis --mode quality  input.mp4
tinythis --mode speed    input.mp4
```

Download/install `ffmpeg` assets:

```powershell
tinythis setup
tinythis setup --force
```

`setup` also installs `tinythis` to your user PATH.

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
