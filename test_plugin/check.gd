extends SceneTree
func _init():
	if Engine.has_singleton("Sorkin"):
		print("OK: Sorkin singleton registered")
		quit(0)
	else:
		printerr("FAIL: Sorkin singleton not found")
		quit(1)
