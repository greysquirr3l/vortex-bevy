// Compile-only Unity API surface for local bridge analysis. This file is never shipped to a Unity project.
#nullable enable

using System;
using System.Diagnostics.CodeAnalysis;
using System.IO;
using UnityEngine.SceneManagement;

namespace UnityEditor
{
    public enum ImportAssetOptions
    {
        ForceSynchronousImport,
    }

    public sealed class GlobalObjectId
    {
        public ulong targetObjectId { get; set; }

        public static GlobalObjectId GetGlobalObjectIdSlow(UnityEngine.GameObject gameObject) =>
            new();
    }

    public static class AssetDatabase
    {
        public static string GUIDToAssetPath(string assetId) => assetId;

        public static Type? GetMainAssetTypeAtPath(string path) => null;

        public static bool IsValidFolder(string path) => false;

        public static string CreateFolder(string parentFolder, string newFolderName) =>
            string.Empty;

        public static bool DeleteAsset(string path) => false;

        public static void Refresh() { }

        public static void SaveAssets() { }

        public static string AssetPathToGUID(string path) => string.Empty;

        public static void ImportAsset(string path, ImportAssetOptions options) { }

        public static T? LoadAssetAtPath<T>(string path)
            where T : UnityEngine.UnityObject => null;

        public static void CreateAsset(UnityEngine.UnityObject asset, string path) { }
    }

    public static class EditorApplication
    {
        public static void Exit(int returnValue) { }
    }

    [AttributeUsage(AttributeTargets.Method)]
    public sealed class MenuItemAttribute : Attribute
    {
        public MenuItemAttribute(string itemName)
        {
            ItemName = itemName;
        }

        public string ItemName { get; }
    }
}

namespace UnityEditor.SceneManagement
{
    public enum NewSceneSetup
    {
        EmptyScene,
    }

    public enum NewSceneMode
    {
        [SuppressMessage(
            "Naming",
            "CA1720:Identifier contains type name",
            Justification = "This compile-only stub matches Unity's NewSceneMode.Single member."
        )]
        Single,
    }

    public static class EditorSceneManager
    {
        public static Scene NewScene(NewSceneSetup setup, NewSceneMode mode) => new();

        public static bool SaveScene(Scene scene, string destinationScenePath) => true;
    }
}

namespace UnityEngine.SceneManagement
{
    public sealed class Scene;
}

namespace UnityEngine
{
    public class UnityObject;

    public static class Application
    {
        public static string dataPath => Directory.GetCurrentDirectory();

        public static string unityVersion => "6000.5.10f1";
    }

    public static class Debug
    {
        public static void Log(string message) { }

        public static void LogError(string message) { }
    }

    public static class JsonUtility
    {
        public static string ToJson(object value, bool prettyPrint) => string.Empty;
    }

    public enum PrimitiveType
    {
        Cube,
    }

    public enum TextureFormat
    {
        RGBA32,
    }

    public enum LightType
    {
        Directional,
    }

    public sealed class GameObject : UnityObject
    {
        private readonly Transform transformValue = new();
        private int componentAccessCount;

        public GameObject() { }

        public GameObject(string name)
        {
            this.name = name;
        }

        public string name { get; set; } = string.Empty;

        public Transform transform => transformValue;

        public static GameObject CreatePrimitive(PrimitiveType type) => new();

        public T GetComponent<T>()
            where T : new()
        {
            componentAccessCount++;
            return new T();
        }

        public T AddComponent<T>()
            where T : new()
        {
            componentAccessCount++;
            return new T();
        }
    }

    public sealed class Transform
    {
        public Vector3 position { get; set; } = new();

        public Quaternion rotation { get; set; } = new();

        public Vector3 localScale { get; set; } = new();
    }

    public sealed class MeshFilter : UnityObject
    {
        private readonly Mesh sharedMeshValue = new();

        public Mesh sharedMesh => sharedMeshValue;
    }

    public sealed class MeshRenderer : UnityObject
    {
        public Material? sharedMaterial { get; set; }
    }

    public sealed class Mesh : UnityObject
    {
        private readonly Vector3[] positions = Array.Empty<Vector3>();
        private readonly Vector3[] normalValues = Array.Empty<Vector3>();
        private readonly Vector2[] uvValues = Array.Empty<Vector2>();
        private readonly int[] triangleValues = Array.Empty<int>();

        [SuppressMessage(
            "Performance",
            "CA1819:Properties should not return arrays",
            Justification = "This compile-only stub matches Unity's Mesh array properties."
        )]
        public Vector3[] vertices => positions;

        [SuppressMessage(
            "Performance",
            "CA1819:Properties should not return arrays",
            Justification = "This compile-only stub matches Unity's Mesh array properties."
        )]
        public Vector3[] normals => normalValues;

        [SuppressMessage(
            "Performance",
            "CA1819:Properties should not return arrays",
            Justification = "This compile-only stub matches Unity's Mesh array properties."
        )]
        public Vector2[] uv => uvValues;

        [SuppressMessage(
            "Performance",
            "CA1819:Properties should not return arrays",
            Justification = "This compile-only stub matches Unity's Mesh array properties."
        )]
        public int[] triangles => triangleValues;
    }

    public sealed class BoxCollider : UnityObject;

    public sealed class Texture2D : UnityObject
    {
        private readonly byte[] pngBytes = Array.Empty<byte>();
        private int operationCount;

        public Texture2D(int width, int height, TextureFormat format, bool mipChain, bool linear)
        { }

        public void SetPixels(Color[] colors)
        {
            ArgumentNullException.ThrowIfNull(colors);
            operationCount += colors.Length;
        }

        public void Apply(bool updateMipmaps, bool makeNoLongerReadable)
        {
            operationCount++;
        }

        public byte[] EncodeToPNG()
        {
            operationCount++;
            return pngBytes;
        }
    }

    public sealed class Material : UnityObject
    {
        private readonly Shader shaderValue;
        private int propertyAccessCount;

        public Material(Shader shader)
        {
            shaderValue = shader;
        }

        public Shader shader => shaderValue;

        public void SetColor(string propertyName, Color color)
        {
            ArgumentNullException.ThrowIfNull(propertyName);
            ArgumentNullException.ThrowIfNull(color);
            propertyAccessCount += propertyName.Length;
        }

        public void SetFloat(string propertyName, float value)
        {
            ArgumentNullException.ThrowIfNull(propertyName);
            propertyAccessCount += propertyName.Length;
        }

        public void SetTexture(string propertyName, Texture2D texture)
        {
            ArgumentNullException.ThrowIfNull(propertyName);
            ArgumentNullException.ThrowIfNull(texture);
            propertyAccessCount += propertyName.Length;
        }

        public Color GetColor(string propertyName)
        {
            ArgumentNullException.ThrowIfNull(propertyName);
            propertyAccessCount += propertyName.Length;
            return new Color(0.0F, 0.0F, 0.0F, 0.0F);
        }

        public float GetFloat(string propertyName)
        {
            ArgumentNullException.ThrowIfNull(propertyName);
            propertyAccessCount += propertyName.Length;
            return 0.0F;
        }
    }

    public sealed class Shader : UnityObject
    {
        public string name { get; } = string.Empty;

        public static Shader? Find(string name) => new();
    }

    public sealed class Camera : UnityObject
    {
        private readonly Transform transformValue = new();

        public Transform transform => transformValue;

        public float fieldOfView { get; set; }
    }

    public sealed class Light : UnityObject
    {
        private readonly Transform transformValue = new();

        public Transform transform => transformValue;

        public LightType type { get; set; }

        public float intensity { get; set; }
    }

    public sealed class Vector2
    {
        public float x { get; set; }
        public float y { get; set; }

        public Vector2() { }

        public Vector2(float x, float y)
        {
            this.x = x;
            this.y = y;
        }
    }

    public sealed class Vector3
    {
        public float x { get; set; }
        public float y { get; set; }
        public float z { get; set; }

        public Vector3() { }

        public Vector3(float x, float y, float z)
        {
            this.x = x;
            this.y = y;
            this.z = z;
        }

        public static Vector3 zero => new();

        public static Vector3 one => new(1.0F, 1.0F, 1.0F);
    }

    public sealed class Quaternion
    {
        public float x { get; set; }
        public float y { get; set; }
        public float z { get; set; }
        public float w { get; set; }

        public static Quaternion identity => new() { w = 1.0F };

        public static Quaternion Euler(float x, float y, float z) => identity;
    }

    public sealed class Color
    {
        public Color(float r, float g, float b, float a)
        {
            this.r = r;
            this.g = g;
            this.b = b;
            this.a = a;
        }

        public float r { get; }
        public float g { get; }
        public float b { get; }
        public float a { get; }
    }

    public static class Mathf
    {
        public const float Deg2Rad = 0.0174532924F;
    }
}
