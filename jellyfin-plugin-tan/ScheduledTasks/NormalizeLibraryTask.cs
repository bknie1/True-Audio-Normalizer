using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Threading;
using System.Threading.Tasks;
using Jellyfin.Data.Enums;
using MediaBrowser.Controller.Entities;
using MediaBrowser.Controller.Library;
using MediaBrowser.Controller.MediaEncoding;
using MediaBrowser.Model.Tasks;
using Microsoft.Extensions.Logging;

namespace Jellyfin.Plugin.Tan.ScheduledTasks;

/// <summary>
/// Batch-normalizes library audio/video through a local tan-server instance.
/// For each item it extracts audio with Jellyfin's own ffmpeg, POSTs the WAV
/// to tan-server's <c>/normalize</c> endpoint, and writes the result as a
/// "&lt;name&gt; [TAN].wav" file - never touching or replacing the original,
/// so this is safe to run repeatedly (an existing output is left alone;
/// delete it to force a redo).
///
/// Scope, honestly stated: this proves and runs the actual DSP pipeline
/// end to end. It does not yet register the output as a selectable
/// alternate audio track in Jellyfin's player - that needs deeper library/
/// metadata integration and is the natural next step once this pipeline
/// itself is confirmed working against a real server.
/// </summary>
public class NormalizeLibraryTask : IScheduledTask
{
    private static readonly HttpClient HttpClient = new();

    private readonly ILibraryManager _libraryManager;
    private readonly IMediaEncoder _mediaEncoder;
    private readonly ILogger<NormalizeLibraryTask> _logger;

    public NormalizeLibraryTask(ILibraryManager libraryManager, IMediaEncoder mediaEncoder, ILogger<NormalizeLibraryTask> logger)
    {
        _libraryManager = libraryManager;
        _mediaEncoder = mediaEncoder;
        _logger = logger;
    }

    public string Name => "Normalize Audio with TAN";

    public string Key => "TanNormalizeLibrary";

    public string Description => "Runs library audio/video through TAN (via a local tan-server) and writes normalized copies alongside the originals.";

    public string Category => "TAN";

    // Manual-run only by default - this touches every item in the library on
    // a run, which isn't something to schedule silently without opting in.
    public IEnumerable<TaskTriggerInfo> GetDefaultTriggers() => Array.Empty<TaskTriggerInfo>();

    public async Task ExecuteAsync(IProgress<double> progress, CancellationToken cancellationToken)
    {
        var config = Plugin.Instance?.Configuration ?? new Configuration.PluginConfiguration();

        var items = _libraryManager.GetItemList(new InternalItemsQuery
        {
            IncludeItemTypes = new[]
            {
                BaseItemKind.Movie,
                BaseItemKind.Episode,
                BaseItemKind.Audio,
                BaseItemKind.MusicVideo,
            },
            Recursive = true,
            IsFolder = false,
        });

        var total = items.Count;
        var done = 0;
        _logger.LogInformation("TAN: normalizing {Count} library items via {ServerUrl}", total, config.ServerUrl);

        foreach (var item in items)
        {
            cancellationToken.ThrowIfCancellationRequested();
            if (!string.IsNullOrEmpty(item.Path) && File.Exists(item.Path))
            {
                try
                {
                    await NormalizeOneAsync(item, config, cancellationToken).ConfigureAwait(false);
                }
                catch (Exception ex)
                {
                    _logger.LogWarning(ex, "TAN: failed to normalize {Path}", item.Path);
                }
            }

            done++;
            progress.Report(100.0 * done / Math.Max(1, total));
        }
    }

    private async Task NormalizeOneAsync(BaseItem item, Configuration.PluginConfiguration config, CancellationToken cancellationToken)
    {
        var sourcePath = item.Path;
        var outputDir = string.IsNullOrWhiteSpace(config.OutputFolder)
            ? Path.GetDirectoryName(sourcePath) ?? "."
            : config.OutputFolder;
        Directory.CreateDirectory(outputDir);
        var outputPath = Path.Combine(outputDir, $"{Path.GetFileNameWithoutExtension(sourcePath)} [TAN].wav");
        if (File.Exists(outputPath))
        {
            _logger.LogDebug("TAN: already normalized, skipping {Path}", sourcePath);
            return;
        }

        var wavPath = Path.Combine(Path.GetTempPath(), $"tan-extract-{Guid.NewGuid():N}.wav");
        try
        {
            await ExtractWavAsync(sourcePath, wavPath, cancellationToken).ConfigureAwait(false);

            using var wavStream = File.OpenRead(wavPath);
            using var content = new StreamContent(wavStream);
            content.Headers.ContentType = new MediaTypeHeaderValue("audio/wav");

            var url = $"{config.ServerUrl.TrimEnd('/')}/normalize?profile={Uri.EscapeDataString(config.Profile)}";
            using var response = await HttpClient.PostAsync(url, content, cancellationToken).ConfigureAwait(false);
            response.EnsureSuccessStatusCode();

            var tmpOut = outputPath + ".tmp";
            await using (var outStream = File.Create(tmpOut))
            {
                await response.Content.CopyToAsync(outStream, cancellationToken).ConfigureAwait(false);
            }

            File.Move(tmpOut, outputPath, overwrite: true);
            _logger.LogInformation("TAN: normalized {Source} -> {Output}", sourcePath, outputPath);
        }
        finally
        {
            File.Delete(wavPath);
        }
    }

    /// <summary>
    /// Decodes just the audio of <paramref name="sourcePath"/> to a 48kHz
    /// stereo PCM WAV via Jellyfin's own bundled ffmpeg - works whether the
    /// source is a plain audio file or the audio track of a video.
    /// </summary>
    private async Task ExtractWavAsync(string sourcePath, string wavPath, CancellationToken cancellationToken)
    {
        var psi = new ProcessStartInfo
        {
            FileName = _mediaEncoder.EncoderPath,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
        };
        foreach (var arg in new[] { "-y", "-i", sourcePath, "-vn", "-acodec", "pcm_s16le", "-ar", "48000", wavPath })
        {
            psi.ArgumentList.Add(arg);
        }

        using var proc = Process.Start(psi) ?? throw new InvalidOperationException("couldn't start ffmpeg");
        await proc.WaitForExitAsync(cancellationToken).ConfigureAwait(false);
        if (proc.ExitCode != 0)
        {
            var stderr = await proc.StandardError.ReadToEndAsync(cancellationToken).ConfigureAwait(false);
            throw new InvalidOperationException($"ffmpeg exited {proc.ExitCode}: {stderr}");
        }
    }
}
