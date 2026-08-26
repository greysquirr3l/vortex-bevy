# Authorized Godot headless export bridge for Vortex-Bevy.
# Invoke only for an authorized editable Godot project:
# godot --headless --path /approved/project --script /approved/bridge/vortex_godot_bridge.gd -- \
#   --assets res://Models/robot.glb,res://Textures/albedo.png \
#   --staging-root /approved/staging --report /approved/staging/bridge-report.json

extends SceneTree

const PROTOCOL_VERSION := "vortex.editor-bridge/v1"
const SUPPORTED_DIRECT_EXTENSIONS := PackedStringArray(
	[
		"glb",
		"gltf",
		"jpg",
		"jpeg",
		"png",
		"svg",
		"tga",
		"webp",
		"wav",
		"ogg",
	]
)


func _init() -> void:
	var exit_code := _run_bridge()
	quit(exit_code)


func _run_bridge() -> int:
	var request_result := _parse_request(OS.get_cmdline_user_args())
	if request_result is String:
		push_error("Vortex Godot bridge rejected input: %s" % request_result)
		return 1

	var request: Dictionary = request_result
	var records: Array[Dictionary] = []
	for asset_path_variant in request.assets:
		records.append(_export_resource(request, String(asset_path_variant)))
	records.sort_custom(
		func(left: Dictionary, right: Dictionary) -> bool: return (
			String(left.source_identity.id) < String(right.source_identity.id)
		)
	)

	var report := {
		"engine": "godot",
		"protocol_version": PROTOCOL_VERSION,
		"records": records,
	}
	var write_error := _write_report(String(request.report), String(request.staging_root), report)
	if not write_error.is_empty():
		push_error("Vortex Godot bridge failed to write report: %s" % write_error)
		return 1
	print("Vortex Godot bridge wrote %d record(s)." % records.size())
	return 0


func _parse_request(arguments: PackedStringArray) -> Variant:
	var values := _argument_values(arguments)
	if not values.has("--assets") or not values.has("--report") or not values.has("--staging-root"):
		return "Expected --assets, --report, and --staging-root arguments."

	var staging_root := _normalized_absolute_path(String(values["--staging-root"]))
	var report := _normalized_absolute_path(String(values["--report"]))
	if not _is_confined(report, staging_root):
		return "Report path must be confined to staging root."

	var assets: PackedStringArray = []
	for raw_asset in String(values["--assets"]).split(",", false):
		var asset := raw_asset.strip_edges()
		if not asset.begins_with("res://"):
			return "Every selected Godot asset must be a res:// path."
		if not assets.has(asset):
			assets.append(asset)
	assets.sort()
	if assets.is_empty():
		return "At least one Godot asset is required."

	return {
		"assets": assets,
		"report": report,
		"staging_root": staging_root,
	}


func _argument_values(arguments: PackedStringArray) -> Dictionary:
	var values := {}
	var index := 0
	while index + 1 < arguments.size():
		var argument := arguments[index]
		if argument.begins_with("--"):
			values[argument] = arguments[index + 1]
			index += 2
		else:
			index += 1
	return values


func _export_resource(request: Dictionary, asset_path: String) -> Dictionary:
	var resource := ResourceLoader.load(asset_path)
	if resource == null:
		return _record(
			asset_path,
			"Unknown",
			"rejected",
			"none",
			[],
			[],
			["Resource could not be loaded."],
		)

	var extension := asset_path.get_extension().to_lower()
	if not SUPPORTED_DIRECT_EXTENSIONS.has(extension):
		return _record(
			asset_path,
			resource.get_class(),
			"requires_adapter",
			"none",
			[],
			["No approved direct-source staging policy for '.%s'." % extension],
			[]
		)

	var source_path := _normalized_absolute_path(ProjectSettings.globalize_path(asset_path))
	var output_relative := "exports/%s.%s" % [asset_path.sha256_text(), extension]
	var joined_output_path := String(request.staging_root).path_join(output_relative)
	var output_path := _normalized_absolute_path(joined_output_path)
	if not _is_confined(output_path, String(request.staging_root)):
		return _record(
			asset_path,
			resource.get_class(),
			"rejected",
			"none",
			[],
			[],
			["Derived output escaped staging root."],
		)

	var staging_error := _stage_file(source_path, output_path)
	if not staging_error.is_empty():
		return _record(
			asset_path,
			resource.get_class(),
			"rejected",
			"godot.direct-source-stage",
			[],
			[],
			[staging_error],
		)
	return _record(
		asset_path,
		resource.get_class(),
		"converted",
		"godot.direct-source-stage",
		[output_relative],
		[],
		[]
	)


func _stage_file(source_path: String, output_path: String) -> String:
	var directory_error := DirAccess.make_dir_recursive_absolute(output_path.get_base_dir())
	if directory_error != OK:
		return "Could not create confined output directory: %s" % error_string(directory_error)
	var source_bytes := FileAccess.get_file_as_bytes(source_path)
	var source_error := FileAccess.get_open_error()
	if source_error != OK:
		return "Could not read selected asset: %s" % error_string(source_error)
	var output := FileAccess.open(output_path, FileAccess.WRITE)
	if output == null:
		return "Could not open staged output: %s" % error_string(FileAccess.get_open_error())
	output.store_buffer(source_bytes)
	output.close()
	return ""


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


func _record(
	asset_path: String,
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
		"source_identity": {"engine": "godot", "id": asset_path},
		"warnings": warnings,
	}


func _normalized_absolute_path(path: String) -> String:
	return path.simplify_path().replace("\\", "/")


func _is_confined(candidate: String, root: String) -> bool:
	var normalized_root := _normalized_absolute_path(root).trim_suffix("/")
	var normalized_candidate := _normalized_absolute_path(candidate)
	return (
		normalized_candidate == normalized_root
		or normalized_candidate.begins_with(normalized_root + "/")
	)
