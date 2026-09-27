/* tan_ladspa - a stereo LADSPA plugin wrapping TAN's streaming normalizer
 * (tan-ffi / tan.lib). Lets ffmpeg apply TAN inline in a single pass:
 *
 *   ffmpeg -i SRC -map 0:v -c:v copy -map 0:a \
 *     -af "aresample=48000,aformat=channel_layouts=stereo,ladspa=file=tan_ladspa:plugin=tan" \
 *     -c:a aac -f mpegts -
 *
 * A single 2-in/2-out instance processes both channels together, so TAN's
 * cross-channel loudness metering stays intact (a per-channel plugin would
 * shift the stereo image). Profile is fixed to "movie" (tan-ffi exposes
 * movie/music only); a control port can select it later.
 */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include "ladspa.h"

#define TRACE(...) do { if (getenv("TAN_LADSPA_TRACE")) { fprintf(stderr, "[tan_ladspa] " __VA_ARGS__); fflush(stderr); } } while (0)

/* tan-ffi C ABI (see tan-ffi/tan_ffi.h) - linked from tan.lib. */
typedef struct TanNormalizer TanNormalizer;
extern TanNormalizer *tan_normalizer_new(unsigned int sample_rate, unsigned int channels, unsigned int profile_id);
extern void tan_normalizer_process(TanNormalizer *handle, float *interleaved, size_t len);
extern void tan_normalizer_free(TanNormalizer *handle);

#define TAN_CHANNELS 2u
#define TAN_PROFILE_MOVIE 0u

typedef struct {
    unsigned long sample_rate;
    TanNormalizer *norm;
    LADSPA_Data *port[5]; /* inL, inR, outL, outR, profile(control) */
    float *scratch;       /* interleaved work buffer */
    unsigned long scratch_cap; /* in floats */
} TanInstance;

static LADSPA_Handle tan_instantiate(const LADSPA_Descriptor *desc, unsigned long sample_rate)
{
    (void)desc;
    TanInstance *ti = (TanInstance *)calloc(1, sizeof(TanInstance));
    if (ti) {
        ti->sample_rate = sample_rate;
    }
    return (LADSPA_Handle)ti;
}

static void tan_connect_port(LADSPA_Handle instance, unsigned long port, LADSPA_Data *data)
{
    TanInstance *ti = (TanInstance *)instance;
    if (port < 5) {
        ti->port[port] = data;
    }
}

static void tan_activate(LADSPA_Handle instance)
{
    TanInstance *ti = (TanInstance *)instance;
    if (ti->norm) {
        tan_normalizer_free(ti->norm);
    }
    /* Profile from the control port (0=movie default; 1=music, 2=universal,
     * 3=speech, 4=night, 5=game - matches tan-ffi profile_from_id). */
    unsigned int prof = TAN_PROFILE_MOVIE;
    if (ti->port[4]) {
        float v = *ti->port[4];
        if (v < 0.0f) v = 0.0f;
        if (v > 5.0f) v = 5.0f;
        prof = (unsigned int)(v + 0.5f);
    }
    ti->norm = tan_normalizer_new((unsigned int)ti->sample_rate, TAN_CHANNELS, prof);
    TRACE("activate: sr=%lu profile=%u norm=%p\n", ti->sample_rate, prof, (void *)ti->norm);
}

static void tan_run(LADSPA_Handle instance, unsigned long n)
{
    TanInstance *ti = (TanInstance *)instance;
    const LADSPA_Data *inL = ti->port[0];
    const LADSPA_Data *inR = ti->port[1];
    LADSPA_Data *outL = ti->port[2];
    LADSPA_Data *outR = ti->port[3];
    TRACE("run: n=%lu norm=%p inL=%p inR=%p outL=%p outR=%p\n", n, (void *)ti->norm, (void *)inL, (void *)inR, (void *)outL, (void *)outR);
    if (!ti->norm || !inL || !inR || !outL || !outR) {
        return;
    }

    unsigned long need = n * TAN_CHANNELS;
    if (ti->scratch_cap < need) {
        float *g = (float *)realloc(ti->scratch, need * sizeof(float));
        if (!g) {
            return;
        }
        ti->scratch = g;
        ti->scratch_cap = need;
    }

    /* Read both channels first (handles in==out aliasing), TAN in place,
     * then scatter back out. */
    for (unsigned long i = 0; i < n; i++) {
        ti->scratch[2 * i] = inL[i];
        ti->scratch[2 * i + 1] = inR[i];
    }
    tan_normalizer_process(ti->norm, ti->scratch, (size_t)need);
    for (unsigned long i = 0; i < n; i++) {
        outL[i] = ti->scratch[2 * i];
        outR[i] = ti->scratch[2 * i + 1];
    }
}

static void tan_deactivate(LADSPA_Handle instance)
{
    TanInstance *ti = (TanInstance *)instance;
    if (ti->norm) {
        tan_normalizer_free(ti->norm);
        ti->norm = NULL;
    }
}

static void tan_cleanup(LADSPA_Handle instance)
{
    TanInstance *ti = (TanInstance *)instance;
    if (ti->norm) {
        tan_normalizer_free(ti->norm);
    }
    free(ti->scratch);
    free(ti);
}

static const LADSPA_PortDescriptor g_ports[5] = {
    LADSPA_PORT_INPUT | LADSPA_PORT_AUDIO,
    LADSPA_PORT_INPUT | LADSPA_PORT_AUDIO,
    LADSPA_PORT_OUTPUT | LADSPA_PORT_AUDIO,
    LADSPA_PORT_OUTPUT | LADSPA_PORT_AUDIO,
    LADSPA_PORT_INPUT | LADSPA_PORT_CONTROL,
};
static const char *const g_port_names[5] = {"Input L", "Input R", "Output L", "Output R", "Profile"};
static const LADSPA_PortRangeHint g_hints[5] = {{0, 0, 0}, {0, 0, 0}, {0, 0, 0}, {0, 0, 0}, {0, 0, 5}};

static LADSPA_Descriptor g_desc = {
    0x54414E01, /* "TAN" + 1 - private-use unique id */
    "tan",
    0,
    "TAN True Audio Normalizer (stereo)",
    "Brandon Knieriem",
    "MIT",
    5,
    g_ports,
    g_port_names,
    g_hints,
    NULL,           /* ImplementationData */
    tan_instantiate,
    tan_connect_port,
    tan_activate,
    tan_run,
    NULL,           /* run_adding */
    NULL,           /* set_run_adding_gain */
    tan_deactivate,
    tan_cleanup,
};

__declspec(dllexport) const LADSPA_Descriptor *ladspa_descriptor(unsigned long index)
{
    return index == 0 ? &g_desc : NULL;
}
