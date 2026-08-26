# Unity Static-Scene Extraction API Sources

This note records the external API constraints used for the authorized temporary static-scene extraction lane. It does not describe a general Unity scene exporter.

Unity 6000.5 documents `EditorSceneManager` as the Edit-mode scene-management API. It provides `NewScene` for creation and `SaveScene` for saving a scene asset under an `Assets/...` project-relative path. The temporary validation scene will therefore be created in an authorized matching Editor context, assigned a visible source identity, reported, and removed after report emission rather than relying on runtime-only APIs.[1]

Unity documents `GameObject.CreatePrimitive` as creating a primitive GameObject with the required mesh components. The approved test scene uses this narrow path to obtain a static mesh whose vertices, normals, UVs, and triangle indices can be read deterministically by the Editor bridge.[2]

Unity documents `EditorSceneManager.SaveScene` as accepting a project-relative destination and returning `false` when saving fails. The bridge must treat a failed save as a typed batch failure and may not write outside the project’s authorized `Assets/` tree.[3]

## References

[1]: https://docs.unity3d.com/6000.5/Documentation/ScriptReference/SceneManagement.EditorSceneManager.html "Unity 6000.5 EditorSceneManager"
[2]: https://docs.unity3d.com/6000.0/Documentation/ScriptReference/GameObject.CreatePrimitive.html "Unity GameObject.CreatePrimitive"
[3]: https://docs.unity3d.com/ScriptReference/SceneManagement.EditorSceneManager.SaveScene.html "Unity EditorSceneManager.SaveScene"
