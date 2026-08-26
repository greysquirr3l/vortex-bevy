// Compile-only Unity API surface for local bridge analysis. This file is never shipped to a Unity project.
#nullable enable

using System;
using System.IO;

namespace UnityEditor
{
    public static class AssetDatabase
    {
        public static string GUIDToAssetPath(string assetId) => assetId;

        public static Type? GetMainAssetTypeAtPath(string path) => null;
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

namespace UnityEngine
{
    public static class Application
    {
        public static string dataPath => Directory.GetCurrentDirectory();
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
}
