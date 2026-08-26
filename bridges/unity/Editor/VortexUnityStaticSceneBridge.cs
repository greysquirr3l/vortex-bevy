// Install this file in an editable authorized Unity project under Assets/Editor/.

#nullable enable

using System;
using System.Collections.Generic;
using System.IO;
using System.Text.RegularExpressions;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;

namespace Vortex.UnityBridge
{
    /// <summary>
    /// Creates one temporary, deliberately small URP Lit scene and extracts bounded static facts.
    /// </summary>
    public static class VortexUnityStaticSceneBridge
    {
        private const string ProtocolVersion = "vortex.unity-static-scene/v1";
        private const string ReportVariable = "VORTEX_UNITY_STATIC_SCENE_REPORT";
        private const string StagingRootVariable = "VORTEX_UNITY_STATIC_SCENE_STAGING_ROOT";
        private const string TemporaryAssetDirectory = "Assets/VortexBridgeTemporary";
        private const string TemporaryScenePath = TemporaryAssetDirectory + "/StaticScene.unity";
        private const string TemporaryTexturePath = TemporaryAssetDirectory + "/BaseColor.png";
        private const string TemporaryMaterialPath = TemporaryAssetDirectory + "/BaseColor.mat";
        private const string StagedTexturePath = "textures/vortex-static-base-color.png";
        private static readonly Regex GuidPattern = new(
            "^[a-f0-9]{32}$",
            RegexOptions.CultureInvariant
        );
        private static bool temporaryAssetsCreated;

        /// <summary>
        /// Runs the temporary static-scene validation from the Unity Editor menu.
        /// </summary>
        [MenuItem("Vortex/Run Static Scene Canonical Validation")]
        public static void RunFromMenu()
        {
            Run(false);
        }

        /// <summary>
        /// Runs the temporary static-scene validation from a batch-mode execute-method invocation.
        /// </summary>
        public static void RunFromCommandLine()
        {
            Run(true);
        }

        private static void Run(bool exitWhenComplete)
        {
            int exitCode = 0;
            try
            {
                StaticSceneRequest request = StaticSceneRequest.FromEnvironment();
                StaticSceneReport report = BuildReport(request);
                WriteReport(request.ReportPath, request.StagingRoot, report);
                Debug.Log("Vortex Unity static-scene bridge wrote one bounded report.");
            }
            catch (InvalidOperationException exception)
            {
                exitCode = 1;
                Debug.LogError(
                    $"Vortex Unity static-scene bridge rejected input: {exception.Message}"
                );
            }
            catch (IOException exception)
            {
                exitCode = 1;
                Debug.LogError(
                    $"Vortex Unity static-scene bridge I/O failure: {exception.Message}"
                );
            }
            catch (UnauthorizedAccessException exception)
            {
                exitCode = 1;
                Debug.LogError(
                    $"Vortex Unity static-scene bridge access failure: {exception.Message}"
                );
            }
            catch (ArgumentException exception)
            {
                exitCode = 1;
                Debug.LogError(
                    $"Vortex Unity static-scene bridge argument failure: {exception.Message}"
                );
            }
            finally
            {
                CleanupTemporaryAssets();
                if (exitWhenComplete)
                {
                    EditorApplication.Exit(exitCode);
                }
            }
        }

        private static StaticSceneReport BuildReport(StaticSceneRequest request)
        {
            CreateTemporaryAssetDirectory();
            try
            {
                Scene scene = EditorSceneManager.NewScene(
                    NewSceneSetup.EmptyScene,
                    NewSceneMode.Single
                );
                GameObject meshObject = CreateStaticMeshObject();
                Material material = CreateUrpLitMaterial();
                MeshRenderer meshRenderer = meshObject.GetComponent<MeshRenderer>();
                meshRenderer.sharedMaterial = material;
                Camera camera = CreateCamera();
                Light light = CreateDirectionalLight();
                if (!EditorSceneManager.SaveScene(scene, TemporaryScenePath))
                {
                    throw new InvalidOperationException(
                        "Unity did not save the temporary static scene."
                    );
                }

                AssetDatabase.SaveAssets();
                AssetDatabase.Refresh();
                string sourceGuid = AssetDatabase.AssetPathToGUID(TemporaryScenePath);
                if (!GuidPattern.IsMatch(sourceGuid))
                {
                    throw new InvalidOperationException(
                        "Unity did not provide a valid temporary scene GUID."
                    );
                }

                WriteStagedTexture(request.StagingRoot);
                MeshFilter meshFilter = meshObject.GetComponent<MeshFilter>();
                Mesh? mesh = meshFilter.sharedMesh;
                return new StaticSceneReport
                {
                    engine = "unity",
                    protocol_version = ProtocolVersion,
                    editor_version = Application.unityVersion,
                    urp_package_version = ResolveUrpPackageVersion(),
                    scene = new StaticScene
                    {
                        source_guid = sourceGuid,
                        source_path = TemporaryScenePath,
                        object_id = GlobalObjectId.GetGlobalObjectIdSlow(meshObject).targetObjectId,
                        mesh = StaticMesh.FromMesh(mesh),
                        material = StaticMaterial.FromMaterial(material),
                        camera = StaticCamera.FromCamera(camera),
                        light = StaticLight.FromLight(light),
                    },
                    unsupported_components = new List<UnsupportedComponent>
                    {
                        new()
                        {
                            component = nameof(BoxCollider),
                            classification = "requires_adapter",
                            reason =
                                "BoxCollider physics shape is outside this render-scene conversion subset.",
                        },
                    },
                };
            }
            finally
            {
                CleanupTemporaryAssets();
            }
        }

        private static GameObject CreateStaticMeshObject()
        {
            GameObject meshObject = GameObject.CreatePrimitive(PrimitiveType.Cube);
            meshObject.name = "VortexStaticMesh";
            meshObject.transform.position = Vector3.zero;
            meshObject.transform.rotation = Quaternion.identity;
            meshObject.transform.localScale = Vector3.one;
            return meshObject;
        }

        private static Material CreateUrpLitMaterial()
        {
            Shader? shader = Shader.Find("Universal Render Pipeline/Lit");
            if (shader is null)
            {
                throw new InvalidOperationException(
                    "The authorized project does not expose Universal Render Pipeline/Lit."
                );
            }

            Texture2D texture = new(2, 2, TextureFormat.RGBA32, false, false);
            texture.SetPixels(
                new[]
                {
                    new Color(1.0F, 0.2F, 0.1F, 1.0F),
                    new Color(0.1F, 0.8F, 0.3F, 1.0F),
                    new Color(0.2F, 0.4F, 1.0F, 1.0F),
                    new Color(1.0F, 0.9F, 0.2F, 1.0F),
                }
            );
            texture.Apply(false, false);
            File.WriteAllBytes(ConfinedProjectPath(TemporaryTexturePath), texture.EncodeToPNG());
            AssetDatabase.ImportAsset(
                TemporaryTexturePath,
                ImportAssetOptions.ForceSynchronousImport
            );
            Texture2D? importedTexture = AssetDatabase.LoadAssetAtPath<Texture2D>(
                TemporaryTexturePath
            );
            if (importedTexture is null)
            {
                throw new InvalidOperationException(
                    "Unity did not import the temporary base-color PNG."
                );
            }

            Material material = new(shader);
            material.SetColor("_BaseColor", new Color(0.8F, 0.4F, 0.2F, 1.0F));
            material.SetFloat("_Metallic", 0.35F);
            material.SetFloat("_Smoothness", 0.6F);
            material.SetTexture("_BaseMap", importedTexture);
            AssetDatabase.CreateAsset(material, TemporaryMaterialPath);
            return material;
        }

        private static Camera CreateCamera()
        {
            GameObject cameraObject = new("VortexStaticSceneCamera");
            cameraObject.transform.position = new Vector3(0.0F, 1.0F, -5.0F);
            cameraObject.transform.rotation = Quaternion.Euler(10.0F, 0.0F, 0.0F);
            Camera camera = cameraObject.AddComponent<Camera>();
            camera.fieldOfView = 60.0F;
            return camera;
        }

        private static Light CreateDirectionalLight()
        {
            GameObject lightObject = new("VortexStaticSceneDirectionalLight");
            lightObject.transform.position = new Vector3(0.0F, 3.0F, -1.0F);
            lightObject.transform.rotation = Quaternion.Euler(45.0F, -30.0F, 0.0F);
            Light light = lightObject.AddComponent<Light>();
            light.type = LightType.Directional;
            light.intensity = 1.0F;
            return light;
        }

        private static void WriteStagedTexture(string stagingRoot)
        {
            string projectTexturePath = ConfinedProjectPath(TemporaryTexturePath);
            string stagedTexturePath = ConfinedPath(stagingRoot, StagedTexturePath);
            string? stagedDirectory = Path.GetDirectoryName(stagedTexturePath);
            if (stagedDirectory is null)
            {
                throw new InvalidOperationException(
                    "Static-scene staged texture directory could not be resolved."
                );
            }

            Directory.CreateDirectory(stagedDirectory);
            File.Copy(projectTexturePath, stagedTexturePath, true);
        }

        private static string ResolveUrpPackageVersion()
        {
            string projectRoot = ProjectRoot();
            string manifestPath = ConfinedPath(projectRoot, "Packages/manifest.json");
            string manifest = File.ReadAllText(manifestPath);
            const string packageName = "com.unity.render-pipelines.universal";
            int nameIndex = manifest.IndexOf(packageName, StringComparison.Ordinal);
            if (nameIndex < 0)
            {
                throw new InvalidOperationException(
                    "The authorized project does not declare the URP package."
                );
            }

            int colonIndex = manifest.IndexOf(':', nameIndex);
            int openingQuoteIndex = manifest.IndexOf('"', colonIndex + 1);
            int closingQuoteIndex = manifest.IndexOf('"', openingQuoteIndex + 1);
            if (
                colonIndex < 0
                || openingQuoteIndex < 0
                || closingQuoteIndex <= openingQuoteIndex + 1
            )
            {
                throw new InvalidOperationException(
                    "The authorized project has an unreadable URP package version."
                );
            }

            return manifest.Substring(
                openingQuoteIndex + 1,
                closingQuoteIndex - openingQuoteIndex - 1
            );
        }

        private static void WriteReport(
            string reportPath,
            string stagingRoot,
            StaticSceneReport report
        )
        {
            string confinedReportPath = ConfinedPath(stagingRoot, reportPath);
            string? reportDirectory = Path.GetDirectoryName(confinedReportPath);
            if (reportDirectory is null)
            {
                throw new InvalidOperationException(
                    "Static-scene report directory could not be resolved."
                );
            }

            Directory.CreateDirectory(reportDirectory);
            File.WriteAllText(
                confinedReportPath,
                JsonUtility.ToJson(report, true) + Environment.NewLine
            );
        }

        private static void CreateTemporaryAssetDirectory()
        {
            if (AssetDatabase.IsValidFolder(TemporaryAssetDirectory))
            {
                throw new InvalidOperationException(
                    "The temporary VortexBridgeTemporary folder already exists; refusing to alter it."
                );
            }

            AssetDatabase.CreateFolder("Assets", "VortexBridgeTemporary");
            temporaryAssetsCreated = true;
        }

        private static void CleanupTemporaryAssets()
        {
            if (!temporaryAssetsCreated)
            {
                return;
            }

            if (AssetDatabase.IsValidFolder(TemporaryAssetDirectory))
            {
                AssetDatabase.DeleteAsset(TemporaryAssetDirectory);
                AssetDatabase.Refresh();
            }

            temporaryAssetsCreated = false;
        }

        private static string ProjectRoot()
        {
            return Directory.GetParent(Application.dataPath)?.FullName
                ?? throw new InvalidOperationException("Unity project root could not be resolved.");
        }

        private static string ConfinedProjectPath(string projectRelativePath)
        {
            return ConfinedPath(ProjectRoot(), projectRelativePath);
        }

        private static string ConfinedPath(string root, string candidate)
        {
            string normalizedRoot = Path.GetFullPath(root);
            string normalizedCandidate = Path.GetFullPath(
                Path.IsPathRooted(candidate) ? candidate : Path.Combine(normalizedRoot, candidate)
            );
            string rootWithSeparator = normalizedRoot.EndsWith(
                Path.DirectorySeparatorChar.ToString(),
                StringComparison.Ordinal
            )
                ? normalizedRoot
                : normalizedRoot + Path.DirectorySeparatorChar;
            if (
                !normalizedCandidate.StartsWith(
                    rootWithSeparator,
                    StringComparison.OrdinalIgnoreCase
                )
                && !string.Equals(
                    normalizedCandidate,
                    normalizedRoot,
                    StringComparison.OrdinalIgnoreCase
                )
            )
            {
                throw new InvalidOperationException("Path escapes its configured root.");
            }

            return normalizedCandidate;
        }

        [Serializable]
        private sealed class StaticSceneRequest
        {
            public string ReportPath { get; private set; } = string.Empty;
            public string StagingRoot { get; private set; } = string.Empty;

            public static StaticSceneRequest FromEnvironment()
            {
                string stagingRoot = Path.GetFullPath(RequiredEnvironment(StagingRootVariable));
                string reportPath = Path.GetFullPath(RequiredEnvironment(ReportVariable));
                if (!IsConfined(reportPath, stagingRoot))
                {
                    throw new InvalidOperationException(
                        "Static-scene report path must be confined to the staging root."
                    );
                }

                return new StaticSceneRequest
                {
                    ReportPath = reportPath,
                    StagingRoot = stagingRoot,
                };
            }

            private static string RequiredEnvironment(string name)
            {
                string? value = Environment.GetEnvironmentVariable(name);
                if (string.IsNullOrWhiteSpace(value))
                {
                    throw new InvalidOperationException($"Missing environment variable: {name}");
                }

                return value;
            }

            private static bool IsConfined(string candidate, string root)
            {
                try
                {
                    string normalizedRoot = Path.GetFullPath(root);
                    string normalizedCandidate = Path.GetFullPath(candidate);
                    string rootWithSeparator = normalizedRoot.EndsWith(
                        Path.DirectorySeparatorChar.ToString(),
                        StringComparison.Ordinal
                    )
                        ? normalizedRoot
                        : normalizedRoot + Path.DirectorySeparatorChar;
                    return normalizedCandidate.StartsWith(
                        rootWithSeparator,
                        StringComparison.OrdinalIgnoreCase
                    );
                }
                catch (ArgumentException)
                {
                    return false;
                }
            }
        }

        [Serializable]
        private sealed class StaticSceneReport
        {
            public string engine = string.Empty;
            public string protocol_version = string.Empty;
            public string editor_version = string.Empty;
            public string urp_package_version = string.Empty;
            public StaticScene scene = new();
            public List<UnsupportedComponent> unsupported_components = new();
        }

        [Serializable]
        private sealed class StaticScene
        {
            public string source_guid = string.Empty;
            public string source_path = string.Empty;
            public ulong object_id;
            public StaticMesh mesh = new();
            public StaticMaterial material = new();
            public StaticCamera camera = new();
            public StaticLight light = new();
        }

        [Serializable]
        private sealed class StaticMesh
        {
            public List<Vector3Value> positions = new();
            public List<Vector3Value> normals = new();
            public List<Vector2Value> uvs = new();
            public List<int> indices = new();

            public static StaticMesh FromMesh(Mesh mesh)
            {
                if (mesh is null)
                {
                    throw new InvalidOperationException("Temporary static mesh is unavailable.");
                }

                Vector3[] positions = mesh.vertices;
                Vector3[] normals = mesh.normals;
                Vector2[] uvs = mesh.uv;
                int[] indices = mesh.triangles;
                if (
                    positions.Length < 3
                    || positions.Length != normals.Length
                    || positions.Length != uvs.Length
                    || indices.Length < 3
                    || indices.Length % 3 != 0
                )
                {
                    throw new InvalidOperationException(
                        "Temporary static mesh is not an aligned triangle mesh."
                    );
                }

                StaticMesh result = new();
                foreach (Vector3 position in positions)
                {
                    result.positions.Add(Vector3Value.FromVector3(position));
                }
                foreach (Vector3 normal in normals)
                {
                    result.normals.Add(Vector3Value.FromVector3(normal));
                }
                foreach (Vector2 uv in uvs)
                {
                    result.uvs.Add(Vector2Value.FromVector2(uv));
                }
                foreach (int index in indices)
                {
                    if (index < 0)
                    {
                        throw new InvalidOperationException(
                            "Temporary static mesh contains a negative index."
                        );
                    }

                    result.indices.Add(index);
                }

                return result;
            }
        }

        [Serializable]
        private sealed class StaticMaterial
        {
            public string shader = string.Empty;
            public ColorValue base_color = new();
            public float metallic;
            public float smoothness;
            public string base_color_texture = StagedTexturePath;

            public static StaticMaterial FromMaterial(Material material)
            {
                if (
                    material.shader is null
                    || material.shader.name != "Universal Render Pipeline/Lit"
                )
                {
                    throw new InvalidOperationException(
                        "Temporary material is not Universal Render Pipeline/Lit."
                    );
                }

                return new StaticMaterial
                {
                    shader = material.shader.name,
                    base_color = ColorValue.FromColor(material.GetColor("_BaseColor")),
                    metallic = material.GetFloat("_Metallic"),
                    smoothness = material.GetFloat("_Smoothness"),
                    base_color_texture = StagedTexturePath,
                };
            }
        }

        [Serializable]
        private sealed class StaticCamera
        {
            public Vector3Value position = new();
            public QuaternionValue rotation = new();
            public Vector3Value scale = new();
            public float vertical_fov_radians;

            public static StaticCamera FromCamera(Camera camera)
            {
                return new StaticCamera
                {
                    position = Vector3Value.FromVector3(camera.transform.position),
                    rotation = QuaternionValue.FromQuaternion(camera.transform.rotation),
                    scale = Vector3Value.FromVector3(camera.transform.localScale),
                    vertical_fov_radians = camera.fieldOfView * Mathf.Deg2Rad,
                };
            }
        }

        [Serializable]
        private sealed class StaticLight
        {
            public Vector3Value position = new();
            public QuaternionValue rotation = new();
            public Vector3Value scale = new();
            public string kind = string.Empty;
            public float intensity;

            public static StaticLight FromLight(Light light)
            {
                if (light.type != LightType.Directional)
                {
                    throw new InvalidOperationException("Temporary light is not directional.");
                }

                return new StaticLight
                {
                    position = Vector3Value.FromVector3(light.transform.position),
                    rotation = QuaternionValue.FromQuaternion(light.transform.rotation),
                    scale = Vector3Value.FromVector3(light.transform.localScale),
                    kind = "directional",
                    intensity = light.intensity,
                };
            }
        }

        [Serializable]
        private sealed class UnsupportedComponent
        {
            public string component = string.Empty;
            public string classification = string.Empty;
            public string reason = string.Empty;
        }

        [Serializable]
        private sealed class Vector2Value
        {
            public float x;
            public float y;

            public static Vector2Value FromVector2(Vector2 value)
            {
                return new Vector2Value { x = value.x, y = value.y };
            }
        }

        [Serializable]
        private sealed class Vector3Value
        {
            public float x;
            public float y;
            public float z;

            public static Vector3Value FromVector3(Vector3 value)
            {
                return new Vector3Value
                {
                    x = value.x,
                    y = value.y,
                    z = value.z,
                };
            }
        }

        [Serializable]
        private sealed class QuaternionValue
        {
            public float x;
            public float y;
            public float z;
            public float w;

            public static QuaternionValue FromQuaternion(Quaternion value)
            {
                return new QuaternionValue
                {
                    x = value.x,
                    y = value.y,
                    z = value.z,
                    w = value.w,
                };
            }
        }

        [Serializable]
        private sealed class ColorValue
        {
            public float r;
            public float g;
            public float b;
            public float a;

            public static ColorValue FromColor(Color value)
            {
                return new ColorValue
                {
                    r = value.r,
                    g = value.g,
                    b = value.b,
                    a = value.a,
                };
            }
        }
    }
}
