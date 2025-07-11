# Sorkin

The default AVI encoding from the godot Movie Writer mode requires roughly 20 megabytes a second for a default window size game. This format still requires lossy conversion to mp4, and is unwieldy in size. This addon allows you to record in a smaller, liberally licensed video format - webm with vp9 and opus.

## Installation

Extract the `sorkin_addon` folder to your Godot project's `addons/` directory.

TODO: release to the godot asset library.

## Usage

Simply change the `Move Writer` output path to somethign with the `.webm` extension, when you run the editor in movie maker mode
your movie will be written with the VP9 codec and Opus Audio.

### Basic Recording Control

The plugin can also pause recording programatically.

```gdscript
# Get the Sorkin singleton
var sorkin = Sorkin.get_singleton()

# Pause/resume recording
sorkin.toggle_pause()

```

### Dependencies

The plugin requires FFmpeg to be installed on the system:

- **Linux**: Install through package manager (`sudo apt-get install ffmpeg libavcodec-dev libavformat-dev libavutil-dev libswscale-dev libavfilter-dev`)
- **macOS**: Install through Homebrew (`brew install ffmpeg`)
- **Windows**: Install through Chocolatey (`choco install ffmpeg`) or download from [FFmpeg.org](https://ffmpeg.org/download.html)
