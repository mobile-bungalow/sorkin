# Sorkin

The default AVI encoding from the godot Movie Writer mode requires roughly 20 megabytes a second for a default window size game. This format still requires lossy conversion to mp4, and is unwieldy in size. This addon allows you to record in a smaller, liberally licensed video format - webm with vp9 and opus.

## Installation

Install via the [Godot Asset Library](https://godotengine.org/asset-library/asset/4455) directly from the Godot editor, or download `sorkin-addon.zip` from the [latest release](https://github.com/paulmay/sorkin/releases) and extract the `sorkin/` folder into your project's `addons/` directory. Then enable the plugin in **Project > Project Settings > Plugins**.

> **Note for teams:** The native binaries are large (100 MB+ on Windows) and should not be
> committed to git. Each team member must install the plugin individually. A .gitignore reflecting this constraint is included with the plugin.

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

### Testing

The test_plugin project can be used to test changes made to the movie writer, simply run the project with movie maker mode enabled and check the output test.webm in the test_plugin project is encoded as expected.
