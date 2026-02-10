# tinythis!

tinythis! is a lightweight `ffmpeg` wrapper for windows. it ships three stable presets:

- `quality`: best quality, slower processing
- `balanced`: good quality, moderate processing
- `speed`: lower quality, faster processing

## setup (ffmpeg)

tinythis needs `ffmpeg.exe` to compress.

sources:

- `local mode`: `ffmpeg.exe` next to `tinythis.exe`
- `bundled`: downloaded assets installed by `tinythis setup`

install assets:

```powershell
tinythis setup
tinythis setup --force
```

## usage (tui)

run with no arguments:

```powershell
tinythis
```

Status banner:

- `local mode`: using `ffmpeg.exe` next to `tinythis.exe`
- `ffmpeg missing`: no ffmpeg available (place `ffmpeg.exe` next to `tinythis.exe` or run `tinythis setup`)

Keys:

- `s`: add files (open picker)
- `u`: update (when available)
- up/down: select a file
- backspace: remove selected file
- left/right: change mode
- `g`: toggle gpu (use gpu)
- enter: compress
- esc: back
- `q`: quit

supported extensions: `.mp4`, `.mov`, `.avi`, `.webm`, `.ogv`, `.asx`, `.mpeg`, `.m4v`, `.wmv`, `.mpg`.

## usage (cli)

compress one or more files:

```powershell
tinythis input1.mp4
tinythis balanced input1.mp4 input2.mp4  # or: quality, speed
```

## path (optional)

`setup` can also add `tinythis` to your user path (it may prompt).
use `--yes` to skip the prompt and add it to path (when missing):

```powershell
tinythis setup --yes
```

if you previously declined, you can install path later with:

```powershell
tinythis setup path
```

## update

update tinythis from github releases:

```powershell
tinythis update
tinythis update --yes
```

remove assets and path entry:

```powershell
tinythis uninstall
```

## outputs

Outputs are written next to the input file as:

`<stem>.tinythis.<preset>.mp4` (and `.2`, `.3`, ... if needed)

## benchmarks

rough numbers from our runs on a **100 MB** source (higher vmaf is better):

| preset    | cpu vmaf    | gpu vmaf    | cpu size (MB) | gpu size (MB) |
|----------|------------:|------------:|--------------:|--------------:|
| quality  | 98.363088   | 97.386657   | 46.603        | 46.317        |
| balanced | 94.836812   | 94.226505   | 26.645        | 29.151        |
| speed    | 81.750592   | 84.372124   | 13.799        | 15.257        |

notes:

- test: `vmaf_fps: 60/1`
- sizes are final output file sizes for each preset
