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
using MediaBrowser.Model.Entities;
using MediaBrowser.Model.Tasks;
using Microsoft.Extensions.Logging;

namespace Jellyfin.Plugin.Tan.ScheduledTasks;

/// <summary>
/// Batch-normalizes library audio/video through a local tan-server instance
/// and makes the result playable in Jellyfin itself.
///
/// For an audio-only item it extracts audio with Jellyfin's own ffmpeg,
/// sends it to tan-server's <c>/normalize</c> endpoint, and writes the
/// result as a WAV. For a video item it does the same, then muxes the
/// TAN-processed audio back with the ORIGINAL video stream (copied, not
/// re-encoded) into a new container - same picture, normalized sound.
/// Either way the original file is never touched or replaced; an existing
/// output is left alone on later runs (delete it to force a redo).
///
/// <see cref="TanMediaSourceProvider"/> is what makes the result show up as
/// a selectable "Play Version" in Jellyfin's own player - this task and that
/// provider agree on where the output lives via <see cref="TanOutput"/>.
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

    public string Description => "Runs library audio/video through TAN (via a local tan-server) and makes the result playable as an alternate version.";

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
        var hasVideo = item.GetMediaStreams().Any(s => s.Type == MediaStreamType.Video);
        var outputPath = TanOutput.PathFor(sourcePath, hasVideo, config.OutputFolder);
        if (File.Exists(outputPath))
        {
            _logger.LogDebug("TAN: already normalized, skipping {Path}", sourcePath);
            return;
        }

        Directory.CreateDirectory(Path.GetDirectoryName(outputPath) ?? ".");

        var extractedWav = Path.Combine(Path.GetTempPath(), $"tan-extract-{Guid.NewGuid():N}.wav");
        var normalizedWav = Path.Combine(Path.GetTempPath(), $"tan-normalized-{Guid.NewGuid():N}.wav");
        try
        {
            await ExtractWavAsync(sourcePath, extractedWav, cancellationToken).ConfigureAwait(false);
            await PostToTanServerAsync(extractedWav, normalizedWav, config, cancellationToken).ConfigureAwait(false);

            var tmpOut = outputPath + ".tmp";
            if (hasVideo)
            {
                // Copy the original video stream verbatim, replace only the
                // audio - same picture, TAN-processed sound.
                await MuxAsync(sourcePath, normalizedWav, tmpOut, cancellationToken).ConfigureAwait(false);
            }
            else
            {
                File.Copy(normalizedWav, tmpOut, overwrite: true);
            }

            File.Move(tmpOut, outputPath, overwrite: true);
            _logger.LogInformation("TAN: normalized {Source} -> {Output}", sourcePath, outputPath);
        }
        finally
        {
            File.Delete(extractedWav);
            File.Delete(normalizedWav);
        }
    }

    /// <summary>
    /// Decodes just the audio of <paramref name="sourcePath"/> to a 48kHz
    /// stereo PCM WAV via Jellyfin's own bundled ffmpeg - works whether the
    /// source is a plain audio file or the audio track of a video.
    /// </summary>
    private async Task ExtractWavAsync(string sourcePath, string wavPath, CancellationToken cancellationToken)
    {
        await RunFfmpegAsync(
            new[] { "-y", "-i", sourcePath, "-vn", "-acodec", "pcm_s16le", "-ar", "48000", wavPath },
            cancellationToken).ConfigureAwait(false);
    }

    private async Task PostToTanServerAsync(string wavPath, string outWavPath, Configuration.PluginConfiguration config, CancellationToken cancellationToken)
    {
        using var wavStream = File.OpenRead(wavPath);
        using var content = new StreamContent(wavStream);
        content.Headers.ContentType = new MediaTypeHeaderValue("audio/wav");

        var url = $"{config.ServerUrl.TrimEnd('/')}/normalize?profile={Uri.EscapeDataString(config.Profile)}";
        using var response = await HttpClient.PostAsync(url, content, cancellationToken).ConfigureAwait(false);
        response.EnsureSuccessStatusCode();

        await using var outStream = File.Create(outWavPath);
        await response.Content.CopyToAsync(outStream, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Muxes the original video (copied, not re-encoded) with the
    /// TAN-processed audio (encoded to AAC - much smaller than embedding raw
    /// PCM in the final file) into a new Matroska container.
    /// </summary>
    private async Task MuxAsync(string originalVideoPath, string normalizedWavPath, string outputPath, CancellationToken cancellationToken)
    {
        await RunFfmpegAsync(
            new[]
            {
                "-y", "-i", originalVideoPath, "-i", normalizedWavPath,
                "-map", "0:v:0", "-map", "1:a:0",
                "-c:v", "copy", "-c:a", "aac", "-b:a", "192k",
                outputPath,
            },
            cancellationToken).ConfigureAwait(false);
    }

    private async Task RunFfmpegAsync(IReadOnlyList<string> args, CancellationToken cancellationToken)
    {
        var psi = new ProcessStartInfo
        {
            FileName = _mediaEncoder.EncoderPath,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
        };
        foreach (var arg in args)
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
