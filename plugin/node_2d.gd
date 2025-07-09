extends Node2D


# Called when the node enters the scene tree for the first time.
func _ready():
	var ap = get_node("%AnimationPlayer")
	ap.current_animation = "dumb"



# Called every frame. 'delta' is the elapsed time since the previous frame.
func _process(delta):
	pass
