// Authorized Unity Editor export bridge for Vortex-Bevy.
// Install this file in an editable authorized Unity project under Assets/Editor/.

#nullable enable

using System;
using System.Collections.Generic;
using System.IO;
using System.Text.RegularExpressions;
using UnityEditor;
using UnityEngine;

namespace Vortex.UnityBridge
{
    /// <summary>
    /// Emits a bounded report and copies only explicit source assets to a confined staging directory.
    /// </summary>
    public static class VortexUnityBridge
    {
        private const string ProtocolVersion = "vortex.editor-bridge/v1";
        private const string GuidListVariable = "VORTEX_UNITY_GUIDS";
        private const string ReportVariable = "VORTEX_UNITY_REPORT";
        private const string StagingRootVariable = "VORTEX_UNITY_STAGING_ROOT";
        private static readonly Regex GuidPattern = new(
            "^[a-fA-F0-9]{32}$",
            RegexOptions.CultureInvariant
        );
        private static readonly HashSet<string> DirectCopyExtensions = new(
            StringComparer.OrdinalIgnoreCase
        )
        {
            ".fbx",
            ".gltf",
            ".glb",
            ".jpg",
            ".jpeg",
            ".png",
            ".tga",
            ".exr",
            ".wav",
        };

        /// <summary>
        /// Runs the bridge interactively from the Unity Editor menu.
        /// </summary>
        [MenuItem("Vortex/Run Authorized Export Bridge")]
        public static void RunFromMenu()
        {
            Run(false);
        }

        /// <summary>
        /// Runs the bridge from a Unity batch-mode <c>-executeMethod</c> invocation.
        /// </summary>
        public static void RunFromCommandLine()
        {
            Run(true);
        }

        private static void Run(bool exitWhenComplete)
        {
            try
            {
                BridgeRequest request = BridgeRequest.FromEnvironment();
                BridgeReport report = BuildReport(request);
                WriteReport(request.ReportPath, request.StagingRoot, report);
                Debug.Log($"Vortex Unity bridge wrote {report.records.Count} record(s).");
                if (exitWhenComplete)
                {
                    EditorApplication.Exit(0);
                }
            }
            catch (InvalidOperationException exception)
            {
                ReportFailure(exitWhenComplete, exception);
            }
            catch (IOException exception)
            {
                ReportFailure(exitWhenComplete, exception);
            }
            catch (UnauthorizedAccessException exception)
            {
                ReportFailure(exitWhenComplete, exception);
            }
            catch (ArgumentException exception)
            {
                ReportFailure(exitWhenComplete, exception);
            }
        }

        private static void ReportFailure(bool exitWhenComplete, Exception exception)
        {
            Debug.LogError($"Vortex Unity bridge rejected input: {exception.Message}");
            if (exitWhenComplete)
            {
                EditorApplication.Exit(1);
            }
        }

        private static BridgeReport BuildReport(BridgeRequest request)
        {
            List<BridgeRecord> records = new();
            foreach (string guid in request.AssetGuids)
            {
                records.Add(ExportAsset(request, guid));
            }

            records.Sort(
                (left, right) =>
                    string.CompareOrdinal(left.source_identity.id, right.source_identity.id)
            );
            return new BridgeReport
            {
                engine = "unity",
                protocol_version = ProtocolVersion,
                records = records,
            };
        }

        private static BridgeRecord ExportAsset(BridgeRequest request, string guid)
        {
            string assetPath = AssetDatabase.GUIDToAssetPath(guid);
            if (
                string.IsNullOrWhiteSpace(assetPath)
                || !assetPath.StartsWith("Assets/", StringComparison.Ordinal)
            )
            {
                return BridgeRecord.Rejected(
                    guid,
                    "Asset GUID did not resolve to an Assets/ path."
                );
            }

            Type? assetType = AssetDatabase.GetMainAssetTypeAtPath(assetPath);
            string assetKind = assetType?.Name ?? "Unknown";
            string extension = Path.GetExtension(assetPath);
            if (!DirectCopyExtensions.Contains(extension))
            {
                return BridgeRecord.RequiresAdapter(
                    guid,
                    assetKind,
                    $"No approved direct-source staging policy for '{extension}'."
                );
            }

            string projectRoot =
                Directory.GetParent(Application.dataPath)?.FullName
                ?? throw new InvalidOperationException("Unity project root could not be resolved.");
            string sourcePath = ConfinedPath(projectRoot, assetPath);
            string outputRelativePath = Path.Combine("exports", $"{guid}{extension}");
            string outputPath = ConfinedPath(request.StagingRoot, outputRelativePath);

            try
            {
                Directory.CreateDirectory(Path.GetDirectoryName(outputPath) ?? request.StagingRoot);
                File.Copy(sourcePath, outputPath, true);
            }
            catch (IOException exception)
            {
                return BridgeRecord.Rejected(guid, assetKind, exception.Message);
            }
            catch (UnauthorizedAccessException exception)
            {
                return BridgeRecord.Rejected(guid, assetKind, exception.Message);
            }

            return new BridgeRecord
            {
                asset_kind = assetKind,
                classification = "converted",
                errors = new List<string>(),
                exporter = "unity.direct-source-stage",
                outputs = new List<string> { ToPortablePath(outputRelativePath) },
                source_identity = new SourceIdentity { engine = "unity", id = guid },
                warnings = new List<string>(),
            };
        }

        private static void WriteReport(string reportPath, string stagingRoot, BridgeReport report)
        {
            string confinedReportPath = ConfinedPath(stagingRoot, reportPath);
            string? reportDirectory = Path.GetDirectoryName(confinedReportPath);
            if (reportDirectory is null)
            {
                throw new InvalidOperationException("Report directory could not be resolved.");
            }

            Directory.CreateDirectory(reportDirectory);
            string stableJson = JsonUtility.ToJson(report, true) + Environment.NewLine;
            File.WriteAllText(confinedReportPath, stableJson);
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

        private static string ToPortablePath(string path)
        {
            return path.Replace(Path.DirectorySeparatorChar, '/');
        }

        [Serializable]
        private sealed class SourceIdentity
        {
            public string engine = string.Empty;
            public string id = string.Empty;
        }

        [Serializable]
        private sealed class BridgeRecord
        {
            public string asset_kind = string.Empty;
            public string classification = string.Empty;
            public List<string> errors = new();
            public string exporter = string.Empty;
            public List<string> outputs = new();
            public SourceIdentity source_identity = new();
            public List<string> warnings = new();

            public static BridgeRecord Rejected(string guid, string error)
            {
                return Rejected(guid, "Unknown", error);
            }

            public static BridgeRecord Rejected(string guid, string assetKind, string error)
            {
                return new BridgeRecord
                {
                    asset_kind = assetKind,
                    classification = "rejected",
                    errors = new List<string> { error },
                    exporter = "none",
                    outputs = new List<string>(),
                    source_identity = new SourceIdentity { engine = "unity", id = guid },
                    warnings = new List<string>(),
                };
            }

            public static BridgeRecord RequiresAdapter(
                string guid,
                string assetKind,
                string warning
            )
            {
                return new BridgeRecord
                {
                    asset_kind = assetKind,
                    classification = "requires_adapter",
                    errors = new List<string>(),
                    exporter = "none",
                    outputs = new List<string>(),
                    source_identity = new SourceIdentity { engine = "unity", id = guid },
                    warnings = new List<string> { warning },
                };
            }
        }

        [Serializable]
        private sealed class BridgeReport
        {
            public string engine = string.Empty;
            public string protocol_version = string.Empty;
            public List<BridgeRecord> records = new();
        }

        private sealed class BridgeRequest
        {
            public IReadOnlyList<string> AssetGuids { get; private init; } = Array.Empty<string>();
            public string ReportPath { get; private init; } = string.Empty;
            public string StagingRoot { get; private init; } = string.Empty;

            public static BridgeRequest FromEnvironment()
            {
                string rawGuids = RequiredEnvironment(GuidListVariable);
                string stagingRoot = Path.GetFullPath(RequiredEnvironment(StagingRootVariable));
                string reportPath = Path.GetFullPath(RequiredEnvironment(ReportVariable));
                List<string> assetGuids = ParseGuids(rawGuids);
                if (!IsConfined(reportPath, stagingRoot))
                {
                    throw new InvalidOperationException(
                        "Report path must be confined to staging root."
                    );
                }

                return new BridgeRequest
                {
                    AssetGuids = assetGuids,
                    ReportPath = reportPath,
                    StagingRoot = stagingRoot,
                };
            }

            private static List<string> ParseGuids(string rawGuids)
            {
                List<string> parsed = new();
                foreach (string entry in rawGuids.Split(','))
                {
                    string normalized = entry.Trim().ToUpperInvariant();
                    if (!GuidPattern.IsMatch(normalized))
                    {
                        throw new InvalidOperationException(
                            "Asset selections must be 32-character Unity GUIDs."
                        );
                    }

                    if (!parsed.Contains(normalized))
                    {
                        parsed.Add(normalized);
                    }
                }

                parsed.Sort(StringComparer.Ordinal);
                if (parsed.Count == 0)
                {
                    throw new InvalidOperationException("At least one Unity GUID is required.");
                }

                return parsed;
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
                catch (IOException)
                {
                    return false;
                }
                catch (UnauthorizedAccessException)
                {
                    return false;
                }
                catch (NotSupportedException)
                {
                    return false;
                }
            }
        }
    }
}
