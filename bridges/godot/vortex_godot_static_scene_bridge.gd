# Invoke only for an authorized editable Godot 4.7 project:
# godot --headless --path /approved/project --script /approved/bridge/vortex_godot_static_scene_bridge.gd -- \
#   --staging-root /approved/staging --report /approved/staging/vortex-godot-static-scene-report.json
#
# This script creates no res:// resources. It constructs a small scene only in memory,
# emits a GLB and PNG only beneath the caller-provided staging root, then frees the scene.

extends SceneTree

const PROTOCOL_VERSION := "vortex.editor-bridge/v1"
const GENERATED_SCENE_ID := "godot://temporary/VortexStaticScene"
const GENERATED_TEXTURE_ID := "godot://temporary/VortexStaticScene/BaseColor"
const GENERATED_PARTICLE_ID := "godot://temporary/VortexStaticScene/UnsupportedParticles"
const GLB_OUTPUT := "exports/VortexStaticScene.glb"
const PNG_OUTPUT := "textures/vortex-static-base-color.png"


func _init() -> void:
	var exit_code := _run_validation()
	quit(exit_code)


func _run_validation() -> int:
	var request_result: Variant = _parse_request(OS.get_cmdline_user_args())
	if request_result is String:
		push_error("Vortex Godot static-scene bridge rejected input: %s" % request_result)
		return 1

	var request: Dictionary = request_result
	var root := _build_static_scene()
	var export_error := _export_scene(root, request)
	root.free()
	if not export_error.is_empty():
		push_error("Vortex Godot static-scene bridge failed: %s" % export_error)
		return 1

	var report := _report()
	var report_error := _write_report(String(request.report), String(request.staging_root), report)
	if not report_error.is_empty():
		push_error("Vortex Godot static-scene bridge failed to write report: %s" % report_error)
		return 1
	print("Vortex Godot static-scene bridge wrote %d record(s)." % report.records.size())
	return 0


func _parse_request(arguments: PackedStringArray) -> Variant:
	var values := _argument_values(arguments)
	if not values.has("--report") or not values.has("--staging-root"):
		return "Expected --report and --staging-root arguments."
	if values.size() != 2:
		return "Only --report and --staging-root arguments are accepted."

	var staging_root := _normalized_absolute_path(String(values["--staging-root"]))
	var report := _normalized_absolute_path(String(values["--report"]))
	if staging_root.is_empty() or report.is_empty():
		return "Staging root and report path cannot be empty."
	if not _is_confined(report, staging_root):
		return "Report path must be confined to staging root."
	return {"report": report, "staging_root": staging_root}


func _argument_values(arguments: PackedStringArray) -> Dictionary:
	var values := {}
	var index := 0
	while index < arguments.size():
		var argument := arguments[index]
		if not argument.begins_with("--") or index + 1 >= arguments.size():
			return {}
		values[argument] = arguments[index + 1]
		index += 2
	return values


func _build_static_scene() -> Node3D:
	var root := Node3D.new()
	root.name = "VortexStaticScene"

	var mesh_instance := MeshInstance3D.new()
	mesh_instance.name = "StaticCube"
	var mesh := BoxMesh.new()
	mesh.size = Vector3(1.0, 1.0, 1.0)
	mesh_instance.mesh = mesh
	mesh_instance.material_override = _base_color_material()
	root.add_child(mesh_instance)

	var camera := Camera3D.new()
	camera.name = "Camera"
	camera.look_at_from_position(Vector3(0.0, 1.25, 4.0), Vector3(0.0, 0.0, 0.0))
	root.add_child(camera)

	var light := DirectionalLight3D.new()
	light.name = "DirectionalLight"
	light.rotation_degrees = Vector3(-50.0, -35.0, 0.0)
	light.light_energy = 1.0
	root.add_child(light)

	var unsupported_particles := GPUParticles3D.new()
	unsupported_particles.name = "UnsupportedParticles"
	unsupported_particles.emitting = false
	root.add_child(unsupported_particles)
	return root


func _base_color_material() -> StandardMaterial3D:
	var image := Image.create(2, 2, false, Image.FORMAT_RGBA8)
	image.set_pixel(0, 0, Color(0.12, 0.45, 0.90, 1.0))
	image.set_pixel(1, 0, Color(0.90, 0.32, 0.12, 1.0))
	image.set_pixel(0, 1, Color(0.18, 0.80, 0.32, 1.0))
	image.set_pixel(1, 1, Color(0.95, 0.85, 0.18, 1.0))

	var material := StandardMaterial3D.new()
	material.albedo_color = Color.WHITE
	material.albedo_texture = ImageTexture.create_from_image(image)
	material.metallic = 0.15
	material.roughness = 0.62
	return material


func _export_scene(root: Node3D, request: Dictionary) -> String:
	var staging_root := String(request.staging_root)
	var glb_path := _joined_confined_path(staging_root, GLB_OUTPUT)
	var png_path := _joined_confined_path(staging_root, PNG_OUTPUT)
	if glb_path.is_empty() or png_path.is_empty():
		return "Derived output escaped staging root."
	for output_path in [glb_path, png_path]:
		var directory_error := DirAccess.make_dir_recursive_absolute(output_path.get_base_dir())
		if directory_error != OK:
			return "Could not create output directory: %s" % error_string(directory_error)

	var source_image := _base_color_image()
	var png_error := source_image.save_png(png_path)
	if png_error != OK:
		return "Could not write generated base-color PNG: %s" % error_string(png_error)

	var document := GLTFDocument.new()
	document.image_format = "PNG"
	document.root_node_mode = GLTFDocument.ROOT_NODE_MODE_KEEP_ROOT
	var state := GLTFState.new()
	var append_error := document.append_from_scene(root, state)
	if append_error != OK:
		return "Could not convert the generated scene to GLTF state: %s" % error_string(append_error)
	var write_error := document.write_to_filesystem(state, glb_path)
	if write_error != OK:
		return "Could not write generated GLB: %s" % error_string(write_error)
	if not FileAccess.file_exists(glb_path) or not FileAccess.file_exists(png_path):
		return "Expected generated output was not written."
	return ""


func _base_color_image() -> Image:
	var image := Image.create(2, 2, false, Image.FORMAT_RGBA8)
	image.set_pixel(0, 0, Color(0.12, 0.45, 0.90, 1.0))
	image.set_pixel(1, 0, Color(0.90, 0.32, 0.12, 1.0))
	image.set_pixel(0, 1, Color(0.18, 0.80, 0.32, 1.0))
	image.set_pixel(1, 1, Color(0.95, 0.85, 0.18, 1.0))
	return image


func _joined_confined_path(staging_root: String, output_relative: String) -> String:
	var candidate := _normalized_absolute_path(staging_root.path_join(output_relative))
	if not _is_confined(candidate, staging_root):
		return ""
	return candidate


func _report() -> Dictionary:
	var records: Array[Dictionary] = [
		_record(
			GENERATED_SCENE_ID,
			"Node3D",
			"converted",
			"godot.gltf-document",
			[GLB_OUTPUT],
			[],
			[]
		),
		_record(
			GENERATED_TEXTURE_ID,
			"ImageTexture",
			"converted",
			"godot.generated-png",
			[PNG_OUTPUT],
			[],
			[]
		),
		_record(
			GENERATED_PARTICLE_ID,
			"GPUParticles3D",
			"requires_adapter",
			"none",
			[],
			[
				"GPU particle process materials are outside the controlled static-scene export subset."
			],
			[]
		),
	]
	records.sort_custom(
		func(left: Dictionary, right: Dictionary) -> bool: return (
			String(left.source_identity.id) < String(right.source_identity.id)
		)
	)
	return {"engine": "godot", "protocol_version": PROTOCOL_VERSION, "records": records}


func _record(
	source_id: String,
	asset_kind: String,
	classification: String,
	exporter: String,
	outputs: Array,
	warnings: Array,
	errors: Array,
) -> Dictionary:
	return {
		"asset_kind": asset_kind,
		"classification": classification,
		"errors": errors,
		"exporter": exporter,
		"outputs": outputs,
		"source_identity": {"engine": "godot", "id": source_id},
		"warnings": warnings,
	}


func _write_report(report_path: String, staging_root: String, report: Dictionary) -> String:
	if not _is_confined(report_path, staging_root):
		return "Report path escaped staging root."
	var directory_error := DirAccess.make_dir_recursive_absolute(report_path.get_base_dir())
	if directory_error != OK:
		return "Could not create report directory: %s" % error_string(directory_error)
	var output := FileAccess.open(report_path, FileAccess.WRITE)
	if output == null:
		return "Could not open report output: %s" % error_string(FileAccess.get_open_error())
	output.store_string(JSON.stringify(report, "  ", true) + "\n")
	output.close()
	return ""


func _normalized_absolute_path(path: String) -> String:
	return path.simplify_path().replace("\\", "/")


func _is_confined(candidate: String, root: String) -> bool:
	var normalized_root := _normalized_absolute_path(root).trim_suffix("/")
	var normalized_candidate := _normalized_absolute_path(candidate)
	return (
		normalized_candidate == normalized_root
		or normalized_candidate.begins_with(normalized_root + "/")
	)
