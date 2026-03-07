@tool
extends EditorPlugin

const ASSET_LIBRARY_URL = "https://godotengine.org/asset-library/asset/4455"
const BIN_PATH = "res://addons/sorkin/bin/"

func _enter_tree():
	_check_binaries()
	print("Sorkin Video Encoder plugin activated")

func _exit_tree():
	print("Sorkin Video Encoder plugin deactivated")

func _check_binaries() -> void:
	var bin_dir := ProjectSettings.globalize_path(BIN_PATH)
	if not DirAccess.dir_exists_absolute(bin_dir):
		push_error(
			"Sorkin: native binaries not found at '%s'.\n" % BIN_PATH +
			"Download the addon from: %s\n" % ASSET_LIBRARY_URL
		)
		return

	var platform := OS.get_name()
	var binary: String
	match platform:
		"Linux":
			binary = bin_dir + "/libsorkin.linux.x86_64.so"
		"Windows":
			binary = bin_dir + "/sorkin.windows.x86_64.dll"
		"macOS":
			binary = bin_dir + "/libsorkin.framework"
		_:
			return

	var exists := FileAccess.file_exists(binary) or DirAccess.dir_exists_absolute(binary)
	if not exists:
		push_error(
			"Sorkin: native binary not found for platform '%s' at '%s'.\n" % [platform, binary] +
			"Download the pre-built addon from: %s\n" % ASSET_LIBRARY_URL +
			"Or build from source with: just bundle"
		)
